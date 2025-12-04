use std::collections::HashMap;

use tracing::{debug, info, warn};

use crate::config::model::ConfigFile;
use crate::dag::graph::DagGraph;
use crate::dag::scheduler_step::SchedulerStep;
use crate::dag::state_manager::{ReadOnlyStateManager, StateManager};
use crate::dag::task_info::{RunState, ScheduledTask, TaskInfo, TaskRunState};
use crate::engine::{TaskName, TaskOutcome};

/// Scheduler holds the immutable DAG plus mutable per-run state.
///
/// It is responsible for:
/// - remembering which tasks are part of the current run
/// - deciding when a triggered task is "ready" to run (deps satisfied)
/// - marking tasks as succeeded/failed/progressed
/// - scheduling dependents when appropriate
/// - failing dependents when a task fails
#[derive(Debug)]
pub struct Scheduler {
    graph: DagGraph,
    tasks: HashMap<TaskName, TaskInfo>,
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
            run_counter: 0,
            current_run_id: None,
        }
    }

    /// Returns `true` if there is currently no active run.
    pub fn is_idle(&self) -> bool {
        self.current_run_id.is_none()
    }

    /// Current run ID, if any.
    pub fn current_run_id(&self) -> Option<u64> {
        self.current_run_id
    }

    /// Read-only view of the given task's run state.
    pub fn run_state_of(&self, task: &str) -> Option<TaskRunState> {
        let info = self.tasks.get(task)?;
        Some(info.run_state.into())
    }

    /// Names of tasks that are currently participating in the *active* run.
    ///
    /// If there is no active run (`current_run_id.is_none()`), this returns an
    /// empty vector, even though tasks may still have a terminal `run_state`
    /// from the previous run.
    pub fn tasks_in_current_run(&self) -> Vec<TaskName> {
        if self.current_run_id.is_none() {
            return Vec::new();
        }

        self.tasks
            .values()
            .filter_map(|info| {
                if info.run_state.is_some() {
                    Some(info.name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Whether the dependencies of `task` are satisfied for the *current run*.
    ///
    /// Returns `None` if the task is unknown.
    pub fn deps_satisfied(&self, task: &str) -> Option<bool> {
        let info = self.tasks.get(task)?;
        // Delegate to ReadOnlyStateManager logic.
        let mgr = ReadOnlyStateManager::new(&self.tasks);
        Some(mgr.deps_satisfied_for_info(info))
    }

    /// Start a new run, resetting per-run state but keeping historical success
    /// information (for dependency satisfaction on later runs).
    pub fn start_new_run(&mut self) {
        self.run_counter += 1;
        self.current_run_id = Some(self.run_counter);

        for info in self.tasks.values_mut() {
            info.run_state = None;
        }

        debug!(run_id = self.run_counter, "scheduler: starting new DAG run");
    }

    /// Handle a trigger for a task name (production API).
    pub fn handle_trigger(&mut self, task: &str) -> Vec<ScheduledTask> {
        self.trigger_step_internal(task).newly_scheduled
    }

    /// Handle "progress" from a long-lived task (production API).
    pub fn handle_progress(&mut self, task: &str) -> Vec<ScheduledTask> {
        self.progress_step_internal(task).newly_scheduled
    }

    /// Handle completion of a task process with a concrete outcome (production API).
    pub fn handle_completion(&mut self, task: &str, outcome: TaskOutcome) -> Vec<ScheduledTask> {
        self.completion_step_internal(task, outcome)
            .newly_scheduled
    }

    /// Manual-step variant of `handle_trigger` that returns a rich [`SchedulerStep`].
    pub fn step_trigger(&mut self, task: &str) -> SchedulerStep {
        self.trigger_step_internal(task)
    }

    /// Manual-step variant of `handle_progress` that returns a rich [`SchedulerStep`].
    pub fn step_progress(&mut self, task: &str) -> SchedulerStep {
        self.progress_step_internal(task)
    }

    /// Manual-step variant of `handle_completion` that returns a rich [`SchedulerStep`].
    pub fn step_completion(&mut self, task: &str, outcome: TaskOutcome) -> SchedulerStep {
        self.completion_step_internal(task, outcome)
    }

    /// Returns a snapshot of static information for debugging / dry-run output.
    pub fn task_names(&self) -> impl Iterator<Item = &str> {
        self.graph.tasks()
    }

    /// Determine whether all tasks are in a terminal state and clear
    /// `current_run_id` if so.
    ///
    /// Returns `true` if this call transitioned the scheduler from running
    /// to idle.
    fn maybe_finish_run(&mut self) -> bool {
        if self.current_run_id.is_none() {
            return false;
        }

        let manager = StateManager::new(&self.graph, &mut self.tasks, self.current_run_id);

        if manager.all_tasks_terminal() {
            info!(
                run_id = self.current_run_id,
                "scheduler: all tasks terminal; marking run as finished"
            );
            self.current_run_id = None;
            true
        } else {
            false
        }
    }

    /// Internal implementation of `handle_trigger` / `step_trigger`.
    fn trigger_step_internal(&mut self, task: &str) -> SchedulerStep {
        if self.current_run_id.is_none() {
            warn!(
                task = %task,
                "handle_trigger called with no active run; implicitly starting a new run"
            );
            self.start_new_run();
        }

        if self.tasks.contains_key(task) {
            let mut manager = StateManager::new(&self.graph, &mut self.tasks, self.current_run_id);
            manager.mark_task_and_dependents_pending(task);
        } else {
            warn!(task = %task, "trigger for unknown task; ignoring");
        }

        let mut manager = StateManager::new(&self.graph, &mut self.tasks, self.current_run_id);
        let newly_scheduled = manager.collect_new_ready_tasks();
        let run_just_finished = self.maybe_finish_run();

        SchedulerStep {
            newly_scheduled,
            newly_failed: Vec::new(),
            run_just_finished,
        }
    }

    /// Internal implementation of `handle_progress` / `step_progress`.
    fn progress_step_internal(&mut self, task: &str) -> SchedulerStep {
        let run_id = match self.current_run_id {
            Some(id) => id,
            None => {
                warn!(
                    task = %task,
                    "handle_progress called with no active run; ignoring"
                );
                return SchedulerStep {
                    newly_scheduled: Vec::new(),
                    newly_failed: Vec::new(),
                    run_just_finished: false,
                };
            }
        };

        if let Some(info) = self.tasks.get_mut(task) {
            debug!(
                task = %info.name,
                run_id,
                "task reported progress; marking DoneSuccess for this run"
            );
            info.run_state = Some(RunState::DoneSuccess);
            info.last_successful_run = Some(run_id);
        } else {
            warn!(task = %task, "progress from unknown task; ignoring");
            return SchedulerStep {
                newly_scheduled: Vec::new(),
                newly_failed: Vec::new(),
                run_just_finished: false,
            };
        }

        let mut manager = StateManager::new(&self.graph, &mut self.tasks, self.current_run_id);
        let newly_scheduled = manager.collect_new_ready_tasks();
        let run_just_finished = self.maybe_finish_run();

        SchedulerStep {
            newly_scheduled,
            newly_failed: Vec::new(),
            run_just_finished,
        }
    }

    /// Internal implementation of `handle_completion` / `step_completion`.
    fn completion_step_internal(&mut self, task: &str, outcome: TaskOutcome) -> SchedulerStep {
        let run_id = match self.current_run_id {
            Some(id) => id,
            None => {
                warn!(
                    task = %task,
                    "handle_completion called with no active run; ignoring"
                );
                return SchedulerStep {
                    newly_scheduled: Vec::new(),
                    newly_failed: Vec::new(),
                    run_just_finished: false,
                };
            }
        };

        let mut newly_scheduled = Vec::new();
        let mut newly_failed = Vec::new();

        match self.tasks.get_mut(task) {
            Some(info) => match outcome {
                TaskOutcome::Success => {
                    info.run_state = Some(RunState::DoneSuccess);
                    info.last_successful_run = Some(run_id);
                    debug!(task = %info.name, run_id, "task completed successfully");
                    let mut manager = StateManager::new(&self.graph, &mut self.tasks, self.current_run_id);
                    newly_scheduled.extend(manager.collect_new_ready_tasks());
                }
                TaskOutcome::Failed(code) => {
                    info.run_state = Some(RunState::DoneFailed);
                    info.last_failed_run = Some(run_id);
                    warn!(
                        task = %info.name,
                        run_id,
                        exit_code = code,
                        "task failed; failing dependents in this run"
                    );
                    newly_failed.push(info.name.clone());
                    let mut manager = StateManager::new(&self.graph, &mut self.tasks, self.current_run_id);
                    let mut dep_failures = manager.mark_dependents_failed(task);
                    newly_failed.append(&mut dep_failures);
                }
            },
            None => {
                warn!(task = %task, "completion for unknown task; ignoring");
            }
        }

        let run_just_finished = self.maybe_finish_run();

        SchedulerStep {
            newly_scheduled,
            newly_failed,
            run_just_finished,
        }
    }
}