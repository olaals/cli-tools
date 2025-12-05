use watchdag_test_utils::init_tracing;

use std::error::Error;
use std::path::PathBuf;

use watchdag::config::load_and_validate;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn task_first_appends_default_watch_and_exclude() -> TestResult {
    init_tracing();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = manifest_dir.join("examples/include-default.toml");

    let cfg = load_and_validate(&path)?;
    let (_defaults, profiles) = watchdag::watch::build_profiles_from_config(&cfg)?;

    let first = profiles.iter().find(|p| p.name() == "first").unwrap();
    let second = profiles.iter().find(|p| p.name() == "second").unwrap();

    assert!(first.matches("src/a/file.py"));
    assert!(first.matches("scripts/deploy.sh"));
    assert!(!first.matches("src/a/tests/test_mod.py"));
    assert!(!first.matches("src/foo.tmp.py"));

    assert!(second.matches("scripts/deploy.sh"));
    assert!(second.matches("src/a/tests/test_mod.py"));

    Ok(())
}