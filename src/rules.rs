use std::collections::HashMap;
use std::io;
use std::fs;
use std::path::{Path, PathBuf};

use fancy_regex::Regex;
use walkdir::WalkDir;

use crate::types::Rules;
use crate::RegexMap;

/// Error type for regex validation in rules.
#[derive(Debug)]
pub enum RegexError {
    InvalidArg(String),
    InvalidRegex(fancy_regex::Error),
}

impl From<fancy_regex::Error> for RegexError {
    fn from(err: fancy_regex::Error) -> RegexError {
        RegexError::InvalidRegex(err)
    }
}

impl std::fmt::Display for RegexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegexError::InvalidArg(s) => write!(f, "'{s}' is not a valid argument of the form var=regex"),
            RegexError::InvalidRegex(e) => write!(f, "Regex error: {e}"),
        }
    }
}

/// Validate all passed regexes and compile them.
/// Returns an error if an invalid regex is supplied, otherwise returns a RegexMap.
pub fn process_regexes(regexes: &[String]) -> Result<RegexMap, RegexError> {
    let mut result = HashMap::new();

    for r in regexes {
        let mut s = r.splitn(2, '=');
        let var = s.next().ok_or_else(|| RegexError::InvalidArg(r.clone()))?;
        let raw_regex = s.next().ok_or_else(|| RegexError::InvalidArg(r.clone()))?;

        let mut normalized_var = if var.starts_with('$') {
            var.to_string()
        } else {
            "$".to_string() + var
        };
        let negative = normalized_var.ends_with('!');

        if negative {
            normalized_var.pop(); // remove !
        }

        let regex = Regex::new(raw_regex)?;
        result.insert(normalized_var, (negative, regex));
    }
    Ok(RegexMap::new(result))
}

/// Recursively search `rule_path` for .yaml files and parse them into Rules.
/// Returns all successfully parsed Rules. Files that fail to read or parse are
/// reported to stderr and skipped.
pub fn rule_path_seek(rule_path: &Path) -> Vec<Rules> {
    let extensions = vec![String::from("yaml")];
    let files: Vec<PathBuf> = iter_files(rule_path, extensions)
        .map(|d| d.into_path())
        .collect();

    if files.is_empty() {
        eprintln!("Warning: No .yaml rule files found in '{}'", rule_path.display());
    }

    let mut rules: Vec<Rules> = vec![];
    for path in files.iter() {
        match read_file(path) {
            Ok(data) => match parse_yaml(&data) {
                Ok(rule) => rules.push(rule),
                Err(e) => eprintln!("Error parsing YAML rule file '{}': {}", path.display(), e),
            },
            Err(e) => eprintln!("Error reading rule file '{}': {}", path.display(), e),
        }
    }
    rules
}

/// Read a file to string. Returns an error instead of silently swallowing it
/// (Fixes Bug 4: was returning empty string and printing unconditionally).
pub fn read_file(path: &Path) -> io::Result<String> {
    fs::read_to_string(path)
}

/// Parse a YAML string into a Rules struct. Returns an error instead of panicking.
pub fn parse_yaml(data: &str) -> Result<Rules, serde_yaml::Error> {
    serde_yaml::from_str(data)
}

/// Recursively iterate through all files under `path` that match an ending listed in `extensions`.
pub fn iter_files(path: &Path, extensions: Vec<String>) -> impl Iterator<Item = walkdir::DirEntry> {
    let is_hidden = |entry: &walkdir::DirEntry| {
        entry
            .file_name()
            .to_str()
            .map(|s| s.starts_with('.'))
            .unwrap_or(false)
    };

    WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_entry(move |e| !is_hidden(e))
        .filter_map(|e| e.ok())
        .filter(move |entry| {
            if entry.file_type().is_dir() {
                return false;
            }

            let path = entry.path();

            match path.extension() {
                None => false,
                Some(ext) => {
                    let s = ext.to_str().unwrap_or_default();
                    extensions.contains(&s.to_string())
                }
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_regexes_valid() {
        let regexes = vec!["$x=foo.*".to_string()];
        let result = process_regexes(&regexes);
        assert!(result.is_ok());
        let map = result.unwrap();
        assert!(map.get("$x").is_some());
    }

    #[test]
    fn test_process_regexes_negative() {
        let regexes = vec!["$y!=bar.*".to_string()];
        let result = process_regexes(&regexes);
        assert!(result.is_ok());
        let map = result.unwrap();
        let (negative, _) = map.get("$y").unwrap();
        assert!(negative);
    }

    #[test]
    fn test_process_regexes_no_dollar_prefix() {
        let regexes = vec!["var=test".to_string()];
        let result = process_regexes(&regexes);
        assert!(result.is_ok());
        let map = result.unwrap();
        assert!(map.get("$var").is_some());
    }

    #[test]
    fn test_process_regexes_invalid_arg() {
        let regexes = vec!["invalid_no_equals".to_string()];
        let result = process_regexes(&regexes);
        assert!(result.is_err());
        match result.unwrap_err() {
            RegexError::InvalidArg(_) => {} // expected
            _ => panic!("expected InvalidArg"),
        }
    }

    #[test]
    fn test_process_regexes_invalid_regex() {
        let regexes = vec!["$x=***invalid[".to_string()];
        let result = process_regexes(&regexes);
        assert!(result.is_err());
        match result.unwrap_err() {
            RegexError::InvalidRegex(_) => {} // expected
            _ => panic!("expected InvalidRegex"),
        }
    }

    #[test]
    fn test_parse_yaml_valid() {
        let yaml = r#"
issue: "test-issue"
description: "test description"
level: error
rules:
  - reason: "test"
    regexes: []
    patterns:
      - "{_ $x;}"
"#;
        let result = parse_yaml(yaml);
        assert!(result.is_ok());
        let rules = result.unwrap();
        assert_eq!(rules.issue, "test-issue");
        assert_eq!(rules.rules.len(), 1);
        assert_eq!(rules.rules[0].patterns[0], "{_ $x;}");
    }

    #[test]
    fn test_parse_yaml_invalid() {
        let result = parse_yaml("{{{invalid yaml");
        assert!(result.is_err());
    }

    #[test]
    fn test_read_file_not_found() {
        let result = read_file(Path::new("/nonexistent/file.yaml"));
        assert!(result.is_err());
    }
}
