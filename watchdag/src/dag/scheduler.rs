// src/dag/scheduler.rs

use std::collections::HashMap;

use tracing::{debug, info, warn};

use crate::config::model::ConfigFile;
use crate::dag::graph::DagGraph;
use crate::engine::{TaskName, TaskOutcome};

/// Per-run state of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunState {
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

/// Static task information derived from config, plus per-run state.
#[derive(Debug, Clone)]
struct TaskInfo {
    name: TaskName,
    cmd: String,
    long_lived: bool,
    rerun: bool,
    progress_on_stdout: Option<String>,
    trigger_on_stdout: Option<String>,
    progress_on_time: Option<String>,
    use_hash: bool,
    /// Direct dependencies for this task (names in `after = [...]`).
    deps: Vec<TaskName>,

    /// Per-run state (None if not participating in the current run).
    run_state: Option<RunState>,

    /// Last run ID in which this task "succeeded" (see note below).
    ///
    /// We update this when:
    /// - the task completes with success, OR
    /// - the task reports progress via `progress_on_*` (for long-lived tasks).
    ///
    /// This allows semantics like:
    /// - If A->B->C and B is triggered and A previously succeeded,
    ///   then B and C can run without re-running A.
    last_successful_run: Option<u64>,

    /// Last run ID in which this task failed.
    last_failed_run: Option<u64>,
}

impl TaskInfo {
    fn from_config(
        name: TaskName,
        cfg: &crate::config::model::TaskConfig,
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
}

impl ScheduledTask {
    fn from_task_info(info: &TaskInfo) -> Self {
        Self {
            name: info.name.clone(),
            cmd: info.cmd.clone(),
            long_lived: info.long_lived,
            rerun: info.rerun,
            progress_on_stdout: info.progress_on_stdout.clone(),
            trigger_on_stdout: info.trigger_on_stdout.clone(),
            progress_on_time: info.progress_on_time.clone(),
            use_hash: info.use_hash,
        }
    }
}

/// Scheduler holds the immutable DAG plus mutable per-run state.
///
/// It is responsible for:
/// - remembering which tasks are part of the current run
/// - deciding when a triggered task is "ready" to run (deps satisfied)
/// - marking tasks as succeeded/failed/progressed
/// - scheduling dependents when appropriate
/// - failing dependents when a task fails
pub struct Scheduler {
    graph: DagGraph,
    tasks: HashMap<TaskName, TaskInfo>,
    default_use_hash: bool,

    /// Monotonically increasing run ID.
    run_counter: u64,
    /// Currently active run ID, or `None` if there is no active run.
    current_run_id: Option<u64>,
}

impl Scheduler {
    /// Construct a scheduler from a validated [`ConfigFile`].
    pub fn from_config(cfg: &ConfigFile) -> Self {
        let graph = DagGraph::from_config(cfg);
        let default_use_hash = cfg.default.use_hash.unwrap_or(false);

        let mut tasks = HashMap::new();

        for (name, tc) in cfg.task.iter() {
            let deps = graph
                .dependencies_of(name)
                .iter()
                .cloned()
                .collect::<Vec<_>>();
            let info = TaskInfo::from_config(name.clone(), tc, deps, default_use_hash);
            tasks.insert(name.clone(), info);
        }

        Self {
            graph,
            tasks,
            default_use_hash,
            run_counter: 0,
            current_run_id: None,
        }
    }

    /// Returns `true` if there is currently no active run.
    pub fn is_idle(&self) -> bool {
        self.current_run_id.is_none()
    }

    /// Start a new run, resetting per-run state but keeping historical success
    /// information (for dependency satisfaction on later runs).
    pub fn start_new_run(&mut self) {
        self.run_counter += 1;
        self.current_run_id = Some(self.run_counter);

        for info in self.tasks.values_mut() {
            info.run_state = None;
        }

        debug!(
            run_id = self.run_counter,
            "scheduler: starting new DAG run"
        );
    }

    /// Handle a trigger for a task name.
    ///
    /// This is called by the runtime whenever we decide that a task should
    /// participate in the current run (e.g. due to file changes or manual
    /// triggering).
    ///
    /// Returns a list of tasks that are now ready to be executed.
    pub fn handle_trigger(&mut self, task: &str) -> Vec<ScheduledTask> {
        if self.current_run_id.is_none() {
            // This shouldn't happen if the runtime calls `start_new_run` first,
            // but be defensive and create a new run if needed.
            warn!(
                "handle_trigger called with no active run; implicitly starting a new run"
            );
            self.start_new_run();
        }

        if let Some(info) = self.tasks.get_mut(task) {
            match info.run_state {
                None => {
                    info.run_state = Some(RunState::Pending);
                    debug!(task = %info.name, "task marked as Pending in this run");
                }
                Some(RunState::Pending)
                | Some(RunState::Running)
                | Some(RunState::DoneSuccess)
                | Some(RunState::DoneFailed) => {
                    // Already part of this run; ignore duplicate trigger.
                    debug!(
                        task = %info.name,
                        "task already participating in current run; ignoring additional trigger"
                    );
                }
            }
        } else {
            warn!(task = %task, "trigger for unknown task; ignoring");
        }

        let ready = self.collect_new_ready_tasks();
        self.maybe_finish_run();
        ready
    }

    /// Handle "progress" from a long-lived task (or any task using
    /// `progress_on_*` to indicate logical completion).
    ///
    /// This marks the task as `DoneSuccess` for the current run and may
    /// schedule dependents whose dependencies are now satisfied.
    pub fn handle_progress(&mut self, task: &str) -> Vec<ScheduledTask> {
        let run_id = match self.current_run_id {
            Some(id) => id,
            None => {
                warn!(
                    task = %task,
                    "handle_progress called with no active run; ignoring"
                );
                return Vec::new();
            }
        };

        if let Some(info) = self.tasks.get_mut(task) {
            debug!(
                task = %info.name,
                "task reported progress; marking DoneSuccess for this run"
            );
            info.run_state = Some(RunState::DoneSuccess);
            info.last_successful_run = Some(run_id);
        } else {
            warn!(task = %task, "progress from unknown task; ignoring");
            return Vec::new();
        }

        let ready = self.collect_new_ready_tasks();
        self.maybe_finish_run();
        ready
    }

    /// Handle completion of a task process with a concrete outcome.
    ///
    /// - On success, we mark it as `DoneSuccess`, update historical success,
    ///   and schedule dependents where possible.
    /// - On failure, we mark it as `DoneFailed` and mark all triggered
    ///   dependents in this run as `DoneFailed` as well.
    pub fn handle_completion(
        &mut self,
        task: &str,
        outcome: TaskOutcome,
    ) -> Vec<ScheduledTask> {
        let run_id = match self.current_run_id {
            Some(id) => id,
            None => {
                warn!(
                    task = %task,
                    "handle_completion called with no active run; ignoring"
                );
                return Vec::new();
            }
        };

        let mut newly_ready = Vec::new();

        match self.tasks.get_mut(task) {
            Some(info) => {
                match outcome {
                    TaskOutcome::Success => {
                        info.run_state = Some(RunState::DoneSuccess);
                        info.last_successful_run = Some(run_id);
                        debug!(task = %info.name, "task completed successfully");
                        newly_ready.extend(self.collect_new_ready_tasks());
                    }
                    TaskOutcome::Failed(code) => {
                        info.run_state = Some(RunState::DoneFailed);
                        info.last_failed_run = Some(run_id);
                        warn!(
                            task = %info.name,
                            exit_code = code,
                            "task failed; failing dependents in this run"
                        );
                        self.mark_dependents_failed(task);
                    }
                }
            }
            None => {
                warn!(task = %task, "completion for unknown task; ignoring");
            }
        }

        self.maybe_finish_run();
        newly_ready
    }

    /// Returns a snapshot of static information for debugging / dry-run output.
    pub fn task_names(&self) -> impl Iterator<Item = &str> {
        self.graph.tasks()
    }

    /// Determine whether all tasks are in a terminal state and clear
    /// `current_run_id` if so.
    fn maybe_finish_run(&mut self) {
        if self.current_run_id.is_none() {
            return;
        }

        let any_active = self.tasks.values().any(|info| {
            matches!(
                info.run_state,
                Some(RunState::Pending) | Some(RunState::Running)
            )
        });

        if !any_active {
            info!(
                run_id = self.current_run_id,
                "scheduler: all tasks terminal; marking run as finished"
            );
            self.current_run_id = None;
        }
    }

    /// Collect tasks that are `Pending` and whose dependencies are satisfied,
    /// mark them as `Running`, and return them as `ScheduledTask`s.
    fn collect_new_ready_tasks(&mut self) -> Vec<ScheduledTask> {
        let mut ready = Vec::new();

        // We need to iterate twice: first to decide, then to mutate, to avoid
        // borrowing conflicts.
        let candidates: Vec<TaskName> = self
            .tasks
            .values()
            .filter_map(|info| {
                if matches!(info.run_state, Some(RunState::Pending))
                    && self.deps_satisfied(info)
                {
                    Some(info.name.clone())
                } else {
                    None
                }
            })
            .collect();

        for name in candidates {
            if let Some(info) = self.tasks.get_mut(&name) {
                debug!(task = %info.name, "dependencies satisfied; marking Running");
                info.run_state = Some(RunState::Running);
                ready.push(ScheduledTask::from_task_info(info));
            }
        }

        ready
    }

    /// Check whether all dependencies of the given task are satisfied for the
    /// *current run*.
    ///
    /// A dependency is considered satisfied if:
    /// - In this run: its `run_state` is `DoneSuccess`, OR
    /// - It is not part of this run (`run_state == None`) **and** it has a
    ///   `last_successful_run` recorded (i.e., it succeeded in a previous run).
    ///
    /// If a dependency is `DoneFailed` in this run, or has never succeeded,
    /// the dependencies are not satisfied.
    fn deps_satisfied(&self, info: &TaskInfo) -> bool {
        for dep_name in &info.deps {
            let dep = match self.tasks.get(dep_name) {
                Some(d) => d,
                None => {
                    // Should not happen since config is validated.
                    warn!(
                        task = %info.name,
                        dep = %dep_name,
                        "dependency missing from tasks map"
                    );
                    return false;
                }
            };

            match dep.run_state {
                Some(RunState::DoneSuccess) => {
                    // Satisfied in this run.
                }
                Some(RunState::DoneFailed) => {
                    // Dependency failed this run; not satisfied.
                    return false;
                }
                Some(RunState::Pending) | Some(RunState::Running) => {
                    // Dependency hasn't finished/progressed yet.
                    return false;
                }
                None => {
                    // Not part of this run; rely on history.
                    if dep.last_successful_run.is_none() {
                        // Has never succeeded; can't treat as satisfied.
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Mark all *triggered* dependents (and their transitively triggered
    /// dependents) of a failed task as `DoneFailed` for this run.
    ///
    /// This enforces the rule: "Failure of a task should fail dependents".
    fn mark_dependents_failed(&mut self, failed_task: &str) {
        let mut stack: Vec<TaskName> = self
            .graph
            .dependents_of(failed_task)
            .iter()
            .cloned()
            .collect();

        while let Some(name) = stack.pop() {
            if let Some(info) = self.tasks.get_mut(&name) {
                match info.run_state {
                    Some(RunState::Pending) | Some(RunState::Running) => {
                        info.run_state = Some(RunState::DoneFailed);
                        debug!(
                            task = %info.name,
                            "marking dependent as DoneFailed due to upstream failure"
                        );
                        // Recurse to its own dependents.
                        stack.extend(
                            self.graph
                                .dependents_of(&name)
                                .iter()
                                .cloned(),
                        );
                    }
                    Some(RunState::DoneSuccess) | Some(RunState::DoneFailed) | None => {
                        // Either already terminal or not participating in this run.
                    }
                }
            }
        }
    }
}
