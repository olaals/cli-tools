use std::collections::BTreeMap;
use std::error::Error;

use watchdag::config::{ConfigFile, ConfigSection, DefaultSection, TaskConfig};
use watchdag::dag::Scheduler;
use watchdag::engine::TaskOutcome;

type TestResult = Result<(), Box<dyn Error>>;

fn chain() -> ConfigFile {
    let mut tasks = BTreeMap::new();

    tasks.insert("A".into(), TaskConfig {
        cmd: "echo A".into(),
        watch: None,
        exclude: None,
        append_default_watch: false,
        append_default_exclude: false,
        after: vec![],
        use_hash: None,
        long_lived: false,
        rerun: None,
        progress_on_stdout: None,
        trigger_on_stdout: None,
        progress_on_time: None,
    });

    tasks.insert("B".into(), TaskConfig {
        cmd: "echo B".into(),
        watch: None,
        exclude: None,
        append_default_watch: false,
        append_default_exclude: false,
        after: vec!["A".into()],
        use_hash: None,
        long_lived: false,
        rerun: None,
        progress_on_stdout: None,
        trigger_on_stdout: None,
        progress_on_time: None,
    });

    tasks.insert("C".into(), TaskConfig {
        cmd: "echo C".into(),
        watch: None,
        exclude: None,
        append_default_watch: false,
        append_default_exclude: false,
        after: vec!["B".into()],
        use_hash: None,
        long_lived: false,
        rerun: None,
        progress_on_stdout: None,
        trigger_on_stdout: None,
        progress_on_time: None,
    });

    ConfigFile {
        config: ConfigSection::default(),
        default: DefaultSection::default(),
        task: tasks,
    }
}

#[test]
fn triggering_b_after_a_success_runs_b_then_c_only() -> TestResult {
    let mut scheduler = Scheduler::from_config(&chain());

    scheduler.start_new_run();
    let r = scheduler.handle_trigger("A");
    assert_eq!(r[0].name, "A");

    let r = scheduler.handle_completion("A", TaskOutcome::Success);
    assert_eq!(r[0].name, "B");

    let r = scheduler.handle_completion("B", TaskOutcome::Success);
    assert_eq!(r[0].name, "C");

    scheduler.handle_completion("C", TaskOutcome::Success);
    assert!(scheduler.is_idle());

    scheduler.start_new_run();

    let ready = scheduler.handle_trigger("B");
    assert_eq!(ready[0].name, "B");

    let ready = scheduler.handle_completion("B", TaskOutcome::Success);
    assert_eq!(ready[0].name, "C");

    Ok(())
}

#[test]
fn triggering_a_and_b_together_runs_full_chain_once() -> TestResult {
    let mut scheduler = Scheduler::from_config(&chain());

    scheduler.start_new_run();

    let rA = scheduler.handle_trigger("A");
    assert_eq!(rA[0].name, "A");

    let rB = scheduler.handle_trigger("B");
    assert!(rB.is_empty());

    let r = scheduler.handle_completion("A", TaskOutcome::Success);
    assert_eq!(r[0].name, "B");

    let r = scheduler.handle_completion("B", TaskOutcome::Success);
    assert_eq!(r[0].name, "C");

    let r = scheduler.handle_completion("C", TaskOutcome::Success);
    assert!(r.is_empty());
    assert!(scheduler.is_idle());

    Ok(())
}
