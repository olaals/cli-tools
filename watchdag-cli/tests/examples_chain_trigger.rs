// tests/examples_chain_trigger.rs
mod common;
use crate::common::init_tracing;
use crate::common::builders::{ConfigFileBuilder, TaskConfigBuilder};

use std::error::Error;

use watchdag::config::ConfigFile;
use watchdag::dag::{Scheduler, TaskRunState};
use watchdag::engine::TaskOutcome;

type TestResult = Result<(), Box<dyn Error>>;

fn chain() -> ConfigFile {
    ConfigFileBuilder::new()
        .with_task("A", TaskConfigBuilder::new("echo A").build())
        .with_task("B", TaskConfigBuilder::new("echo B").after("A").build())
        .with_task("C", TaskConfigBuilder::new("echo C").after("B").build())
        .build()
}

#[tokio::test]
async fn triggering_b_after_a_success_runs_b_then_c_only() -> TestResult {
    crate::common::with_timeout(async {
        init_tracing();

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
    }).await
}

#[tokio::test]
async fn triggering_a_and_b_together_runs_full_chain_once() -> TestResult {
    crate::common::with_timeout(async {
        init_tracing();

    let mut scheduler = Scheduler::from_config(&chain());

    scheduler.start_new_run();

    let r_a = scheduler.handle_trigger("A");
    assert_eq!(r_a[0].name, "A");

    let r_b = scheduler.handle_trigger("B");
    assert!(r_b.is_empty());

    let r = scheduler.handle_completion("A", TaskOutcome::Success);
    assert_eq!(r[0].name, "B");

    let r = scheduler.handle_completion("B", TaskOutcome::Success);
    assert_eq!(r[0].name, "C");

    let r = scheduler.handle_completion("C", TaskOutcome::Success);
    assert!(r.is_empty());
    assert!(scheduler.is_idle());

    Ok(())
    }).await
}

#[tokio::test]
async fn manual_stepping_exposes_run_state_and_run_completion() -> TestResult {
    crate::common::with_timeout(async {
        init_tracing();

    let mut scheduler = Scheduler::from_config(&chain());

    // Initially idle, no run.
    assert!(scheduler.is_idle());
    assert_eq!(scheduler.current_run_id(), None);

    scheduler.start_new_run();
    let run_id = scheduler.current_run_id().expect("run should be active");
    assert!(run_id > 0);

    // Trigger A using the manual-step API.
    let step = scheduler.step_trigger("A");
    assert_eq!(
        step.newly_scheduled
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>(),
        vec!["A"]
    );
    assert!(!step.run_just_finished);

    // All tasks in the chain should now be part of the run.
    let mut in_run = scheduler.tasks_in_current_run();
    in_run.sort();
    assert_eq!(in_run, vec!["A".to_string(), "B".to_string(), "C".to_string()]);

    // A is running, B/C are not yet ready.
    assert_eq!(
        scheduler.run_state_of("A"),
        Some(TaskRunState::Running)
    );
    assert_eq!(
        scheduler.run_state_of("B"),
        Some(TaskRunState::Pending)
    );
    assert_eq!(
        scheduler.run_state_of("C"),
        Some(TaskRunState::Pending)
    );

    // B depends on A, so deps should not be satisfied before A succeeds.
    assert_eq!(scheduler.deps_satisfied("B"), Some(false));

    // Completing A should schedule B.
    let step = scheduler.step_completion("A", TaskOutcome::Success);
    assert_eq!(
        step.newly_scheduled
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>(),
        vec!["B"]
    );
    assert!(!step.run_just_finished);
    assert!(step.newly_failed.is_empty());

    assert_eq!(
        scheduler.run_state_of("A"),
        Some(TaskRunState::DoneSuccess)
    );
    assert_eq!(
        scheduler.run_state_of("B"),
        Some(TaskRunState::Running)
    );

    // Completing B should schedule C.
    let step = scheduler.step_completion("B", TaskOutcome::Success);
    assert_eq!(
        step.newly_scheduled
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>(),
        vec!["C"]
    );
    assert!(!step.run_just_finished);
    assert!(step.newly_failed.is_empty());

    // Completing C should finish the run.
    let step = scheduler.step_completion("C", TaskOutcome::Success);
    assert!(step.newly_scheduled.is_empty());
    assert!(step.newly_failed.is_empty());
    assert!(step.run_just_finished);
    assert!(scheduler.is_idle());
    assert_eq!(scheduler.current_run_id(), None);
    assert!(scheduler.tasks_in_current_run().is_empty());

    Ok(())
    }).await
}
