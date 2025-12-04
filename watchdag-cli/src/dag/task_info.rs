// src/dag/task_info.rs

//! Task metadata and per-run state management.

use crate::config::model::TaskConfig;
use crate::engine::TaskName;

/// Per-run state of a task (internal).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    /// Task was triggered for this run but is waiting on dependencies.
    Pending,
    /// Task has been dispatched to the executor and is currently running.
    Running,
    /// Task has logically completed successfully for this run
    /// (either via `progress_on_*` or exit with success).
    DoneSuccess,
    /// Task failed in this run (or was blocked by a failed dependency).
    DoneFailed,
}

/// Public, read-only view of a task's per-run state.
///
/// This is exposed for tests and diagnostics without leaking the internal
/// `RunState` type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskRunState {
    /// The task is not currently participating in this run.
    NotInRun,
    Pending,
    Running,
    DoneSuccess,
    DoneFailed,
}

impl From<Option<RunState>> for TaskRunState {
    fn from(state: Option<RunState>) -> Self {
        match state {
            None => TaskRunState::NotInRun,
            Some(RunState::Pending) => TaskRunState::Pending,
            Some(RunState::Running) => TaskRunState::Running,
            Some(RunState::DoneSuccess) => TaskRunState::DoneSuccess,
            Some(RunState::DoneFailed) => TaskRunState::DoneFailed,
        }
    }
}

/// Static task information derived from config, plus per-run state.
#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub name: TaskName,
    pub cmd: String,
    pub long_lived: bool,
    pub rerun: bool,
    pub progress_on_stdout: Option<String>,
    pub trigger_on_stdout: Option<String>,
    pub progress_on_time: Option<String>,
    pub use_hash: bool,
    /// Direct dependencies for this task (names in `after = [...]`).
    pub deps: Vec<TaskName>,

    /// Per-run state (None if not participating in the current run).
    pub run_state: Option<RunState>,

    /// Last run ID in which this task "succeeded".
    pub last_successful_run: Option<u64>,

    /// Last run ID in which this task failed.
    pub last_failed_run: Option<u64>,
}

impl TaskInfo {
    pub fn from_config(
        name: TaskName,
        cfg: &TaskConfig,
        deps: Vec<TaskName>,
        default_use_hash: bool,
    ) -> Self {
        Self {
            name: name.clone(),
            cmd: cfg.cmd.clone(),
            long_lived: cfg.long_lived,
            rerun: cfg.effective_rerun(),
            progress_on_stdout: cfg.progress_on_stdout.clone(),
            trigger_on_stdout: cfg.trigger_on_stdout.clone(),
            progress_on_time: cfg.progress_on_time.clone(),
            use_hash: cfg.effective_use_hash(default_use_hash),
            deps,
            run_state: None,
            last_successful_run: None,
            last_failed_run: None,
        }
    }
}

/// Description of a task that the scheduler wants the executor to run now.
#[derive(Debug, Clone)]
pub struct ScheduledTask {
    pub name: TaskName,
    pub cmd: String,
    pub long_lived: bool,
    pub rerun: bool,
    pub progress_on_stdout: Option<String>,
    pub trigger_on_stdout: Option<String>,
    pub progress_on_time: Option<String>,
    pub use_hash: bool,
    /// Monotonically increasing DAG run identifier.
    ///
    /// All tasks that belong to the same DAG run share the same `run_id`.
    pub run_id: u64,
}

impl ScheduledTask {
    pub fn from_task_info(info: &TaskInfo, run_id: u64) -> Self {
        Self {
            name: info.name.clone(),
            cmd: info.cmd.clone(),
            long_lived: info.long_lived,
            rerun: info.rerun,
            progress_on_stdout: info.progress_on_stdout.clone(),
            trigger_on_stdout: info.trigger_on_stdout.clone(),
            progress_on_time: info.progress_on_time.clone(),
            use_hash: info.use_hash,
            run_id,
        }
    }
}
