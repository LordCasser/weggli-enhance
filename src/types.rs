use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::query::QueryTree;
use crate::result::QueryResult;

/// A single YAML rule file, deserialized from a .yaml file.
#[derive(Serialize, Deserialize, Clone)]
pub struct Rules {
    pub issue: String,
    pub description: String,
    pub level: Option<Level>,
    pub rules: Vec<Rule>,
}

/// Severity level for a rule, used in SARIF output.
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Error,
    Warning,
    Note,
}

/// A single rule within a YAML rules file.
#[derive(Serialize, Deserialize, Clone)]
pub struct Rule {
    pub reason: String,
    pub regexes: Vec<String>,
    pub patterns: Vec<String>,
}

/// A compiled work item ready for query execution.
pub struct WorkItem {
    pub qt: QueryTree,
    pub identifiers: Vec<String>,
    pub reason: String,
    pub issue: String,
}

/// Context for a single query result, used in the result pipeline.
#[derive(Debug)]
pub struct ResultsCtx {
    pub query_index: usize,
    pub path: String,
    pub source: Arc<String>,
    pub result: QueryResult,
    pub reason: String,
    pub issue: String,
}

/// Deduplicated output result for SARIF or terminal output.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OutputResults {
    pub query_index: usize,
    pub path: String,
    pub reason: String,
    pub issue: String,
    pub start_line: i64,
    pub start_column: i64,
    pub end_line: i64,
    pub end_column: i64,
}

/// CLI-driven options that control query result filtering.
pub struct Options {
    pub limit: bool,
    pub unique: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_deserialization() {
        let yaml = "error";
        let level: Level = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(level, Level::Error);

        let yaml = "warning";
        let level: Level = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(level, Level::Warning);

        let yaml = "note";
        let level: Level = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(level, Level::Note);
    }

    #[test]
    fn test_rules_deserialization_minimal() {
        let yaml = r#"
issue: "test"
description: "desc"
rules: []
"#;
        let rules: Rules = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(rules.issue, "test");
        assert_eq!(rules.description, "desc");
        assert!(rules.level.is_none());
        assert!(rules.rules.is_empty());
    }

    #[test]
    fn test_output_results_dedup_key() {
        let a = OutputResults {
            query_index: 0,
            path: "/tmp/test.c".into(),
            reason: "r1".into(),
            issue: "i1".into(),
            start_line: 10,
            start_column: 1,
            end_line: 15,
            end_column: 1,
        };
        let b = a.clone();
        assert_eq!(a, b); // Eq derived
    }
}
