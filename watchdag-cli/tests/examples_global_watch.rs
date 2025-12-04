mod common;
use crate::common::init_tracing;

use std::error::Error;
use std::path::PathBuf;

use watchdag::config::load_and_validate;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn global_watch_and_exclude_apply_to_all_tasks() -> TestResult {
    init_tracing();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config_path = manifest_dir.join("examples/global-watch.toml");

    let cfg = load_and_validate(&config_path)?;
    let (_defaults, profiles) = watchdag::watch::build_profiles_from_config(&cfg)?;

    let src_py = "src/main.py";
    let matching = profiles.iter().filter(|p| p.matches(src_py)).count();
    assert_eq!(matching, cfg.task.len());

    let tmp_script = "scripts/foo.tmp.sh";
    assert!(!profiles.iter().any(|p| p.matches(tmp_script)));

    Ok(())
}
