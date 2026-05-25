use std::collections::HashSet;
use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;

use serde_sarif::sarif;
use serde_sarif::sarif::ReportingDescriptor;

use crate::types::{OutputResults, ResultsCtx};

/// Generate SARIF output from query results and write to the specified path.
/// Fixes Bug 9: SARIF results now include end_line, start_column, and end_column.
pub fn generate_sarif(
    results: Vec<ResultsCtx>,
    descriptors: Vec<ReportingDescriptor>,
    sarif_path: PathBuf,
) {
    let mut counter = 0;

    // Build tool component
    let tool_components = sarif::ToolComponentBuilder::default()
        .name("weggli-enhance")
        .version(env!("CARGO_PKG_VERSION"))
        .rules(descriptors)
        .build()
        .expect("failed to build SARIF tool component");

    let tools = sarif::ToolBuilder::default()
        .driver(tool_components)
        .build()
        .expect("failed to build SARIF tool");

    // Convert ResultsCtx to OutputResults with richer location info
    let mut output_results = vec![];
    for r in results {
        let start_offset = r.result.start_offset();
        let source_str = &r.source;
        let start_line = byte_to_line(source_str, start_offset);
        let start_column = byte_to_column(source_str, start_offset);
        let end_line = byte_to_line(source_str, r.result.function_range().end);
        let end_column = byte_to_column(source_str, r.result.function_range().end);

        output_results.push(OutputResults {
            query_index: r.query_index,
            path: r.path,
            reason: r.reason,
            issue: r.issue,
            start_line: start_line as i64,
            start_column: start_column as i64,
            end_line: end_line as i64,
            end_column: end_column as i64,
        });
    }

    // Deduplicate by (path, start_line, issue)
    let mut unique_results = Vec::new();
    let mut seen = HashSet::new();
    for result in output_results {
        let key = (result.path.clone(), result.start_line, result.issue.clone());
        if seen.insert(key) {
            unique_results.push(result);
        }
    }

    let mut sarif_results = vec![];
    for result in unique_results {
        let rule_id = result.issue.clone();
        let rule_index = result.query_index as i64;

        let sarif_rule = sarif::ReportingDescriptorReferenceBuilder::default()
            .id(rule_id.clone())
            .index(rule_index)
            .build()
            .expect("failed to build SARIF rule reference");

        let sarif_message = sarif::MessageBuilder::default()
            .text(result.reason.clone())
            .build()
            .expect("failed to build SARIF message");

        let sarif_artifact_location = sarif::ArtifactLocationBuilder::default()
            .uri(result.path.clone())
            .build()
            .expect("failed to build SARIF artifact location");

        let sarif_region = sarif::RegionBuilder::default()
            .start_line(result.start_line)
            .start_column(result.start_column)
            .end_line(result.end_line)
            .end_column(result.end_column)
            .build()
            .expect("failed to build SARIF region");

        let sarif_physical_location = sarif::PhysicalLocationBuilder::default()
            .artifact_location(sarif_artifact_location)
            .region(sarif_region)
            .build()
            .expect("failed to build SARIF physical location");

        let sarif_location = sarif::LocationBuilder::default()
            .physical_location(sarif_physical_location)
            .build()
            .expect("failed to build SARIF location");

        let sarif_result = sarif::ResultBuilder::default()
            .rule_id(rule_id)
            .rule_index(rule_index)
            .rule(sarif_rule)
            .message(sarif_message)
            .locations(vec![sarif_location])
            .build()
            .expect("failed to build SARIF result");

        sarif_results.push(sarif_result);
        counter += 1;
    }

    let sarif_struct = sarif::SarifBuilder::default()
        .schema("https://json.schemastore.org/sarif-2.1.0")
        .version("2.1.0")
        .runs(vec![
            sarif::RunBuilder::default()
                .tool(tools)
                .results(sarif_results)
                .build()
                .expect("failed to build SARIF run"),
        ])
        .build()
        .expect("failed to build SARIF document");

    println!("{counter} matches");

    let sarif_json = serde_json::to_string(&sarif_struct)
        .expect("failed to serialize SARIF to JSON");

    let mut file = File::create(&sarif_path).unwrap_or_else(|e| {
        eprintln!("Error creating SARIF output file '{}': {}", sarif_path.display(), e);
        std::process::exit(1);
    });
    file.write_all(sarif_json.as_bytes()).unwrap_or_else(|e| {
        eprintln!("Error writing SARIF output: {e}");
        std::process::exit(1);
    });
}

/// Convert a byte offset to a 0-based line number.
fn byte_to_line(source: &str, byte_offset: usize) -> usize {
    source[..byte_offset.min(source.len())].matches('\n').count()
}

/// Convert a byte offset to a 1-based column number on its line.
fn byte_to_column(source: &str, byte_offset: usize) -> usize {
    let clamped = byte_offset.min(source.len());
    if let Some(last_newline) = source[..clamped].rfind('\n') {
        clamped - last_newline
    } else {
        clamped + 1 // 1-based
    }
}
