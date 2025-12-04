// src/dag/state_manager.rs

//! Per-run state management for tasks in the scheduler.

use std::collections::{HashMap, HashSet};

use tracing::{debug, warn};

use crate::dag::task_info::{RunState, ScheduledTask, TaskInfo};
use crate::dag::DagGraph;
use crate::engine::TaskName;

/// Manages per-run state transitions for tasks.
pub struct StateManager<'a> {
    graph: &'a DagGraph,
    tasks: &'a mut HashMap<TaskName, TaskInfo>,
    current_run_id: Option<u64>,
}

impl<'a> StateManager<'a> {
    pub fn new(
        graph: &'a DagGraph,
        tasks: &'a mut HashMap<TaskName, TaskInfo>,
        current_run_id: Option<u64>,
    ) -> Self {
        Self {
            graph,
            tasks,
            current_run_id,
        }
    }

    /// Include a triggered task and all its downstream dependents in this run.
    ///
    /// - Tasks that were not yet part of the run (`run_state == None`) are
    ///   marked `Pending`.
    /// - Tasks already participating in this run keep their current state.
    pub fn mark_task_and_dependents_pending(&mut self, root: &str) {
        let mut stack: Vec<TaskName> = vec![root.to_string()];
        let mut visited: HashSet<TaskName> = HashSet::new();

        while let Some(name) = stack.pop() {
            if !visited.insert(name.clone()) {
                continue;
            }

            if let Some(info) = self.tasks.get_mut(&name) {
                if info.run_state.is_none() {
                    info.run_state = Some(RunState::Pending);
                    debug!(task = %info.name, "marked Pending for this run");
                }

                // Always traverse dependents so that downstream tasks are also
                // included in the run.
                for dep_name in self.graph.dependents_of(&name).iter().cloned() {
                    stack.push(dep_name);
                }
            } else {
                // Should not happen with validated config, but be defensive.
                warn!(task = %name, "node in DAG not present in tasks map");
            }
        }
    }

    /// Determine whether all dependencies of the given task are satisfied for
    /// the *current run*.
    ///
    /// This is the canonical implementation of dependency satisfaction logic.
    /// It checks both the current run state (for tasks participating in this run)
    /// and historical success (for tasks not triggered in this run).
    pub fn deps_satisfied_for_info(&self, info: &TaskInfo) -> bool {
        // Delegate to the read-only implementation to avoid duplication.
        let ro = ReadOnlyStateManager::new(self.tasks);
        ro.deps_satisfied_for_info(info)
    }

    /// Mark all *triggered* dependents (and their transitively triggered
    /// dependents) of a failed task as `DoneFailed` for this run.
    ///
    /// Returns the list of tasks that were newly marked as failed (excluding
    /// the root task; the caller should add that separately if desired).
    pub fn mark_dependents_failed(&mut self, failed_task: &str) -> Vec<TaskName> {
        let mut stack: Vec<TaskName> = self
            .graph
            .dependents_of(failed_task)
            .iter()
            .cloned()
            .collect();

        let mut newly_failed = Vec::new();

        while let Some(name) = stack.pop() {
            if let Some(info) = self.tasks.get_mut(&name) {
                match info.run_state {
                    Some(RunState::Pending) | Some(RunState::Running) => {
                        info.run_state = Some(RunState::DoneFailed);
                        debug!(
                            task = %info.name,
                            "marking dependent as DoneFailed due to upstream failure"
                        );
                        newly_failed.push(info.name.clone());
                        stack.extend(self.graph.dependents_of(&name).iter().cloned());
                    }
                    Some(RunState::DoneSuccess) | Some(RunState::DoneFailed) | None => {
                        // Either already terminal or not participating in this run.
                    }
                }
            }
        }

        newly_failed
    }

    /// Collect tasks that are `Pending` and whose dependencies are satisfied,
    /// mark them as `Running`, and return them as `ScheduledTask`s.
    ///
    /// This is where we log when a command is run or *re-run*.
    pub fn collect_new_ready_tasks(&mut self) -> Vec<ScheduledTask> {
        use tracing::info;

        let mut ready = Vec::new();

        // Decide first, then mutate to avoid borrowing issues.
        let candidates: Vec<TaskName> = self
            .tasks
            .values()
            .filter_map(|info| {
                if matches!(info.run_state, Some(RunState::Pending))
                    && self.deps_satisfied_for_info(info)
                {
                    Some(info.name.clone())
                } else {
                    None
                }
            })
            .collect();

        for name in candidates {
            if let Some(info) = self.tasks.get_mut(&name) {
                let is_rerun = info.last_successful_run.is_some() || info.last_failed_run.is_some();

                if is_rerun {
                    info!(
                        task = %info.name,
                        run_id = self.current_run_id,
                        "scheduling task for re-run in this DAG run"
                    );
                } else {
                    info!(
                        task = %info.name,
                        run_id = self.current_run_id,
                        "scheduling task for first run in this DAG run"
                    );
                }

                debug!(
                    task = %info.name,
                    run_id = self.current_run_id,
                    long_lived = info.long_lived,
                    use_hash = info.use_hash,
                    "dependencies satisfied; marking Running"
                );

                info.run_state = Some(RunState::Running);
                ready.push(ScheduledTask::from_task_info(
                    info,
                    self.current_run_id.unwrap_or(0),
                ));
            }
        }

        ready
    }

    /// Check if all tasks are in a terminal state.
    pub fn all_tasks_terminal(&self) -> bool {
        !self.tasks.values().any(|info| {
            matches!(
                info.run_state,
                Some(RunState::Pending) | Some(RunState::Running)
            )
        })
    }
}

/// A read-only view of the state manager for checking dependency satisfaction.
///
/// This is used when we only have shared access to the tasks map (e.g. in `Scheduler::deps_satisfied`).
pub struct ReadOnlyStateManager<'a> {
    tasks: &'a HashMap<TaskName, TaskInfo>,
}

impl<'a> ReadOnlyStateManager<'a> {
    pub fn new(tasks: &'a HashMap<TaskName, TaskInfo>) -> Self {
        Self { tasks }
    }

    /// Determine whether all dependencies of the given task are satisfied for
    /// the *current run*.
    ///
    /// This logic is identical to `StateManager::deps_satisfied_for_info` but works
    /// with immutable references.
    pub fn deps_satisfied_for_info(&self, info: &TaskInfo) -> bool {
        for dep_name in &info.deps {
            let dep = match self.tasks.get(dep_name) {
                Some(d) => d,
                None => {
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
}
