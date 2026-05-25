/*
Copyright 2021 Google LLC

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

     https://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;

use colored::Colorize;
use fancy_regex::Regex;
use rayon::Scope;
use serde_sarif::sarif;
use log::{info, warn};

use weggli_enhance::parse_search_pattern;
use weggli_enhance::rules::{iter_files, process_regexes, rule_path_seek};
use weggli_enhance::pipeline::{execute_queries_worker, parse_files_worker};
use weggli_enhance::types::{Level, Options, ResultsCtx, WorkItem};
use weggli_enhance::output;

mod cli;

fn main() {
    reset_signal_pipe_handler();

    let args = cli::parse_arguments();

    if args.force_color {
        colored::control::set_override(true)
    }

    // Validate that the --include and --exclude regexes are valid.
    let helper_regex = |v: &[String]| -> Vec<Regex> {
        v.iter()
            .map(|s| {
                let r = Regex::new(s);
                match r {
                    Ok(regex) => regex,
                    Err(e) => {
                        eprintln!("Regex error: {e}");
                        std::process::exit(1)
                    }
                }
            })
            .collect()
    };

    let exclude_re = helper_regex(&args.exclude);
    let include_re = helper_regex(&args.include);

    // Collect and filter our input file set ONCE (Fix Bug 3: was inside the per-rule loop)
    let mut files: Vec<PathBuf> = iter_files(&args.code_path, args.extensions.clone())
        .map(|d| d.into_path())
        .collect();

    if !exclude_re.is_empty() || !include_re.is_empty() {
        files.retain(|f| {
            if exclude_re
                .iter()
                .any(|r| r.is_match(&f.to_string_lossy()).unwrap())
            {
                return false;
            }
            if include_re.is_empty() {
                return true;
            }
            include_re
                .iter()
                .any(|r| r.is_match(&f.to_string_lossy()).unwrap())
        });
    }

    info!("parsing {} files", files.len());
    if files.is_empty() {
        eprintln!(
            "{}",
            String::from("No files to parse. Exiting...").red()
        );
        std::process::exit(1)
    }

    let files = Arc::new(files);

    // Process each YAML rules file
    for rules in rule_path_seek(args.rule_path.as_path()) {
        info!("[+] Issue loading: {}", rules.issue.blue());

        let level = match rules.level {
            Some(Level::Error) => "error",
            Some(Level::Warning) => "warning",
            Some(Level::Note) => "note",
            None => "none",
        };

        let descriptors = vec![
            sarif::ReportingDescriptorBuilder::default()
                .name(rules.issue.clone())
                .id(rules.issue.clone())
                .default_configuration(
                    sarif::ReportingConfigurationBuilder::default()
                        .enabled(true)
                        .level(level)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap(),
        ];

        // Compile all patterns from all rules in this YAML file (Fix Bug 3)
        let mut works: Vec<WorkItem> = vec![];

        for rule in &rules.rules {
            let mut variables = HashSet::new();

            let regex_constraints = process_regexes(&rule.regexes).unwrap_or_else(|e| {
                let msg = match e {
                    weggli_enhance::rules::RegexError::InvalidArg(s) => format!(
                        "'{}' is not a valid argument of the form var=regex",
                        s.red()
                    ),
                    weggli_enhance::rules::RegexError::InvalidRegex(s) => {
                        format!("Regex error: {s}")
                    }
                };
                eprintln!("{msg}");
                std::process::exit(1)
            });

            let work_items: Vec<WorkItem> = rule
                .patterns
                .iter()
                .map(|pattern| {
                    match parse_search_pattern(
                        pattern,
                        args.force_query,
                        Some(&regex_constraints),
                    ) {
                        Ok(qt) => {
                            let identifiers = qt.identifiers();
                            variables.extend(qt.variables());
                            WorkItem {
                                qt,
                                identifiers,
                                reason: rule.reason.clone(),
                                issue: rules.issue.clone(),
                            }
                        }
                        Err(qe) => {
                            eprintln!("{}", qe.message);
                            if parse_search_pattern(
                                pattern,
                                args.force_query,
                                Some(&regex_constraints),
                            )
                            .is_ok()
                            {
                                eprintln!(
                                    "{} This query is valid in C++ mode (-X)",
                                    "Note:".bold()
                                );
                            }
                            std::process::exit(1);
                        }
                    }
                })
                .collect();

            works.extend(work_items);

            // Validate that all regex constraints reference valid variables
            for v in regex_constraints.variables() {
                if !variables.contains(v) {
                    eprintln!("'{}' is not a valid query variable", v.red());
                    std::process::exit(1)
                }
            }
        }

        if works.is_empty() {
            warn!("No valid patterns compiled for issue '{}'", rules.issue);
            continue;
        }

        // Run the worker pipeline ONCE for all compiled patterns (Fix Bug 3)
        let mut results: Vec<ResultsCtx> = vec![];
        let options = Options {
            limit: args.limit,
            unique: args.unique,
        };
        let files_for_pipeline = Arc::clone(&files);

        rayon::scope(|s| {
            let (ast_tx, ast_rx) = mpsc::channel();
            let (results_tx, results_rx) = mpsc::channel();

            let w = &works;

            s.spawn(move |_: &Scope<'_>| {
                parse_files_worker(files_for_pipeline, ast_tx, w)
            });
            s.spawn(move |_: &Scope<'_>| {
                execute_queries_worker(ast_rx, results_tx, w, options)
            });

            results.extend(results_rx.iter());
        });

        // Apply cross-query variable consistency filter (Fix Bug 1)
        let filtered_results = filter_chainable_results(results);

        // Output results
        match &args.output_path {
            Some(path) => {
                let mut tmp_path = PathBuf::from(path);
                if tmp_path.is_dir() {
                    if tmp_path.is_absolute() {
                        tmp_path.push("results.sarif");
                    } else {
                        tmp_path = std::env::current_dir()
                            .unwrap()
                            .join(path)
                            .join("results.sarif")
                    }
                } else if !tmp_path.is_absolute() {
                    tmp_path = std::env::current_dir().unwrap().join(path)
                }
                output::sarif::generate_sarif(
                    filtered_results,
                    descriptors,
                    tmp_path,
                );
            }
            None => {
                output::terminal::print_terminal(filtered_results, args.enable_line_numbers);
            }
        }
    }
}

/// Filter results so that only results with compatible variable assignments
/// across different queries are kept. (Fix Bug 1)
///
/// For each result, we check whether there exists at least one result from
/// every other query that has compatible variable assignments. Results that
/// cannot be chained with any result from some other query are discarded.
fn filter_chainable_results(results: Vec<ResultsCtx>) -> Vec<ResultsCtx> {
    if results.is_empty() {
        return results;
    }

    // Group results by query_index
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for (idx, r) in results.iter().enumerate() {
        groups
            .entry(r.query_index)
            .or_default()
            .push(idx);
    }

    let num_groups = groups.len();
    if num_groups <= 1 {
        // Single query: no cross-query filtering needed
        return results;
    }

    // For each result, determine if it chains with at least one result from
    // every other query group.
    let mut keep = vec![false; results.len()];

    for (&query_i, indices_i) in &groups {
        for &idx_a in indices_i {
            let a = &results[idx_a];
            let mut chainable_with_all = true;

            for (&query_j, indices_j) in &groups {
                if query_i == query_j {
                    continue;
                }

                let has_chainable = indices_j.iter().any(|&idx_b| {
                    let b = &results[idx_b];
                    a.result.chainable(&a.source, &b.result, &b.source)
                });

                if !has_chainable {
                    chainable_with_all = false;
                    break;
                }
            }

            keep[idx_a] = chainable_with_all;
        }
    }

    // Collect only kept results
    let mut filtered = Vec::with_capacity(results.len());
    for (idx, r) in results.into_iter().enumerate() {
        if keep[idx] {
            filtered.push(r);
        }
    }
    filtered
}

// Exit on SIGPIPE
// see https://github.com/rust-lang/rust/issues/46016#issuecomment-605624865
fn reset_signal_pipe_handler() {
    #[cfg(target_family = "unix")]
    {
        use nix::sys::signal;

        unsafe {
            let _ = signal::signal(signal::Signal::SIGPIPE, signal::SigHandler::SigDfl)
                .map_err(|e| eprintln!("{e}"));
        }
    }
}
