use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;

#[test]
fn test_help_flag() {
    let mut cmd = Command::cargo_bin("weggli-enhance").unwrap();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("A YAML file or directory"));
}

#[test]
fn test_missing_arguments() {
    let mut cmd = Command::cargo_bin("weggli-enhance").unwrap();
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("USAGE"));
}

#[test]
fn test_invalid_rule_file() {
    let mut cmd = Command::cargo_bin("weggli-enhance").unwrap();
    cmd.arg("/nonexistent/path/rules.yaml")
        .arg("examples/malloc.c");
    cmd.assert()
        .success(); // warns but doesn't crash — no rules found, no matches
}

#[test]
fn test_basic_search_on_examples() {
    let mut cmd = Command::cargo_bin("weggli-enhance").unwrap();
    cmd.arg("rules/test.yaml")
        .arg("examples/")
        .arg("-e").arg("c");
    // Should find matches or exit cleanly with 0 matches
    let output = cmd.assert().success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    // Either prints match count or exits cleanly
    assert!(stdout.contains("matches") || stdout.is_empty());
}

#[test]
fn test_version_flag() {
    let mut cmd = Command::cargo_bin("weggli-enhance").unwrap();
    cmd.arg("--version");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_sarif_output_flag() {
    use std::env;
    let tmp = env::temp_dir().join("test_weggli_output.sarif");
    let _ = std::fs::remove_file(&tmp);

    let mut cmd = Command::cargo_bin("weggli-enhance").unwrap();
    cmd.arg("rules/test.yaml")
        .arg("examples/")
        .arg("-e").arg("c")
        .arg("-o").arg(tmp.to_str().unwrap());
    cmd.assert().success();

    // Verify SARIF file was created and is valid JSON
    assert!(tmp.exists());
    let content = std::fs::read_to_string(&tmp).unwrap();
    assert!(serde_json::from_str::<serde_json::Value>(&content).is_ok());
    let _ = std::fs::remove_file(&tmp);
}
