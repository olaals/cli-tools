use std::error::Error;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

use watchdag::config::{load_and_validate, TaskConfig};
use watchdag::watch::compute_hash_for_paths;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn config_use_hash_defaults_and_overrides() -> TestResult {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cfg = load_and_validate(manifest_dir.join("examples/use-hash.toml"))?;

    let default_use_hash = cfg.default.use_hash.unwrap_or(false);

    let a: &TaskConfig = cfg.task.get("A").unwrap();
    let b: &TaskConfig = cfg.task.get("B").unwrap();

    assert!(a.effective_use_hash(default_use_hash));
    assert!(!b.effective_use_hash(default_use_hash));

    Ok(())
}

#[test]
fn compute_hash_is_order_insensitive_and_tracks_content_changes() -> TestResult {
    let dir = tempdir()?;
    let f1 = dir.path().join("a.txt");
    let f2 = dir.path().join("b.txt");

    fs::write(&f1, "hello")?;
    fs::write(&f2, "world")?;

    let h1 = compute_hash_for_paths([&f1, &f2])?;
    let h2 = compute_hash_for_paths([&f2, &f1])?;
    assert_eq!(h1, h2);

    fs::write(&f1, "HELLO")?;
    let h3 = compute_hash_for_paths([&f1, &f2])?;
    assert_ne!(h1, h3);

    Ok(())
}
