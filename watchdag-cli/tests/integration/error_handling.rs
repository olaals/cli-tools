// tests/error_handling.rs

use std::io::Write;
use tempfile::NamedTempFile;
use watchdag::config::load_and_validate;
use watchdag::errors::WatchdagError;

#[test]
fn test_dag_cycle_returns_structured_error() {
    let mut file = NamedTempFile::new().unwrap();
    write!(
        file,
        r#"
[task.A]
cmd = "echo A"
after = ["B"]

[task.B]
cmd = "echo B"
after = ["A"]
"#
    )
    .unwrap();

    let result = load_and_validate(file.path());

    match result {
        Err(WatchdagError::DagCycle(msg)) => {
            assert!(msg.contains("cycle detected"));
            assert!(msg.contains("A") || msg.contains("B"));
        }
        Err(e) => panic!("Expected DagCycle error, got: {:?}", e),
        Ok(_) => panic!("Expected error, got Ok"),
    }
}

#[test]
fn test_unknown_dependency_returns_config_error() {
    let mut file = NamedTempFile::new().unwrap();
    write!(
        file,
        r#"
[task.A]
cmd = "echo A"
after = ["NonExistent"]
"#
    )
    .unwrap();

    let result = load_and_validate(file.path());

    match result {
        Err(WatchdagError::ConfigError(msg)) => {
            assert!(msg.contains("unknown dependency"));
            assert!(msg.contains("NonExistent"));
        }
        Err(e) => panic!("Expected ConfigError, got: {:?}", e),
        Ok(_) => panic!("Expected error, got Ok"),
    }
}
