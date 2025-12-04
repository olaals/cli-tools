// src/dag/scheduler_step.rs

//! Step-by-step execution result types for the scheduler.

use crate::dag::task_info::ScheduledTask;
use crate::engine::TaskName;

/// Structured result of a single scheduler "step".
///
/// This is useful for tests that want to manually step the DAG and make
/// assertions about what changed.
#[derive(Debug, Clone)]
pub struct SchedulerStep {
    /// Tasks that became ready to run as a result of this step.
    pub newly_scheduled: Vec<ScheduledTask>,
    /// Tasks that were newly marked as failed in this step (including the
    /// task that failed and any dependents).
    pub newly_failed: Vec<TaskName>,
    /// Whether this step caused the current run to finish (i.e. the scheduler
    /// is now idle).
    pub run_just_finished: bool,
}
