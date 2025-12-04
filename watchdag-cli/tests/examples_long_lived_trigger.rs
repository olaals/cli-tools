// tests/examples_long_lived_trigger.rs
mod common;
use crate::common::init_tracing;

use std::error::Error;
use std::path::PathBuf;

use watchdag::config::load_and_validate;
use watchdag::dag::Scheduler;
use watchdag::engine::TaskOutcome;

type TestResult = Result<(), Box<dyn Error>>;

/// This test is tied to `examples/long-lived-trigger.toml`:
///
/// [task.A]
/// long_lived = true
/// trigger_on_stdout = "hello"
///
/// [task.B]
/// long_lived = true
/// progress_on_time = "3s"
/// after = ["A"]
///
/// It verifies the basic DAG rule:
/// when A is triggered and then completes successfully,
/// B (which has `after = ["A"]`) is scheduled afterwards.
#[test]
fn triggering_a_then_completing_schedules_b_after_a() -> TestResult {
    init_tracing();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cfg = load_and_validate(manifest_dir.join("examples/long-lived-trigger.toml"))?;

    // Sanity-check that the example still looks like we expect.
    let task_a = cfg.task.get("A").expect("task A must exist");
    let task_b = cfg.task.get("B").expect("task B must exist");

    assert!(task_a.long_lived, "A should be long_lived in example");
    assert_eq!(
        task_a.trigger_on_stdout.as_deref(),
        Some("hello"),
        "A.trigger_on_stdout should be 'hello' in example"
    );

    assert!(task_b.long_lived, "B should be long_lived in example");
    assert_eq!(
        task_b.after,
        vec!["A".to_string()],
        "B should depend on A via after = [\"A\"]"
    );

    let mut scheduler = Scheduler::from_config(&cfg);

    // Start a new DAG run.
    scheduler.start_new_run();

    // Trigger A: only A should be ready initially.
    let ready = scheduler.handle_trigger("A");
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].name, "A");

    // Simulate A completing successfully.
    // (In a real run, this "completion" could come from either a
    // TaskProgressed or TaskCompleted event depending on long_lived logic.)
    let ready_after_a = scheduler.handle_completion("A", TaskOutcome::Success);
    assert_eq!(ready_after_a.len(), 1);
    assert_eq!(ready_after_a[0].name, "B");

    // Completing B should leave the scheduler idle.
    let ready_after_b = scheduler.handle_completion("B", TaskOutcome::Success);
    assert!(ready_after_b.is_empty());
    assert!(scheduler.is_idle());

    Ok(())
}
