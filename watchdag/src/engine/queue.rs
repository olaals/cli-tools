// src/engine/queue.rs

use std::collections::{HashSet, VecDeque};
use std::str::FromStr;

use tracing::{debug, warn};

use super::runtime::TaskName;

/// Behaviour when a new trigger arrives while a DAG run is already in progress.
///
/// - `Queue`: remember the trigger and start a new DAG run when the current one
///   finishes (default behaviour).
/// - `Cancel`: conceptually means "drop any previously queued run and only keep
///   the latest trigger". Actual cancellation of the *running* DAG is handled
///   at a higher level; here we only manage queued triggers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerWhileRunningBehaviour {
    Queue,
    Cancel,
}

impl Default for TriggerWhileRunningBehaviour {
    fn default() -> Self {
        TriggerWhileRunningBehaviour::Queue
    }
}

impl FromStr for TriggerWhileRunningBehaviour {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "queue" => Ok(TriggerWhileRunningBehaviour::Queue),
            "cancel" => Ok(TriggerWhileRunningBehaviour::Cancel),
            other => Err(format!(
                "invalid triggered_while_running_behaviour: {other} (expected \"queue\" or \"cancel\")"
            )),
        }
    }
}

/// Queue of triggers that arrive while a DAG run is already executing.
///
/// Semantics:
/// - Each queued entry represents a *batch* of task names that should be
///   treated as triggers for a future DAG run.
/// - `queue_length` (max_runs) defines how many such batches to keep. The
///   README states the default is 1, meaning "at most one future run is queued".
/// - When the runtime is idle and wants to start a new run, it calls
///   `drain_pending()`, which merges all queued batches into a single set of
///   task names for that run.
///
/// This works well with the rules:
/// - If A->B->C and only B triggers while running, the next run will start from B.
/// - If A and B both trigger while running, the next run will union them and
///   the scheduler can decide that A->B->C should run once.
#[derive(Debug)]
pub struct TriggerQueue {
    behaviour: TriggerWhileRunningBehaviour,
    max_runs: usize,
    /// Each entry is a set of tasks that should be triggered together as one
    /// "batch" when the current DAG run completes.
    runs: VecDeque<HashSet<TaskName>>,
}

impl TriggerQueue {
    /// Create a new queue with the given behaviour and maximum queued runs.
    ///
    /// `max_runs` is clamped to at least 1, as a zero-length queue would make
    /// queuing semantics meaningless.
    pub fn new(behaviour: TriggerWhileRunningBehaviour, max_runs: usize) -> Self {
        let max_runs = max_runs.max(1);
        Self {
            behaviour,
            max_runs,
            runs: VecDeque::new(),
        }
    }

    /// Returns true if there are no queued triggers.
    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }

    /// Returns the configured behaviour.
    pub fn behaviour(&self) -> TriggerWhileRunningBehaviour {
        self.behaviour
    }

    /// Record that a task was triggered while a DAG run is in progress.
    ///
    /// How this is stored depends on the configured behaviour:
    ///
    /// - `Queue`:
    ///   - If there is already at least one queued batch, we merge this task
    ///     into the *last* batch (coalescing multiple triggers into the same
    ///     future run).
    ///   - If there are no batches yet, we create a new one.
    ///   - If the number of batches exceeds `max_runs`, we drop the oldest.
    ///
    /// - `Cancel`:
    ///   - We drop all existing batches and keep only a single batch containing
    ///     this task. Higher-level logic is responsible for actually cancelling
    ///     the currently running DAG; the queue only manages what should run
    ///     *afterwards*.
    pub fn record_trigger(&mut self, task: &str) {
        let name = task.to_string();

        match self.behaviour {
            TriggerWhileRunningBehaviour::Queue => {
                if let Some(last_batch) = self.runs.back_mut() {
                    let inserted = last_batch.insert(name.clone());
                    debug!(
                        task = %name,
                        inserted,
                        "merged trigger into last queued batch (queue mode)",
                    );
                } else {
                    let mut set = HashSet::new();
                    set.insert(name.clone());
                    self.runs.push_back(set);
                    debug!(task = %name, "created first queued batch (queue mode)");
                }

                if self.runs.len() > self.max_runs {
                    warn!(
                        current_batches = self.runs.len(),
                        max_runs = self.max_runs,
                        "exceeded max_runs; dropping oldest queued batches"
                    );
                    while self.runs.len() > self.max_runs {
                        self.runs.pop_front();
                    }
                }
            }
            TriggerWhileRunningBehaviour::Cancel => {
                debug!(
                    task = %name,
                    "resetting queued batches to this task only (cancel mode)"
                );
                self.runs.clear();
                let mut set = HashSet::new();
                set.insert(name.clone());
                self.runs.push_back(set);
            }
        }
    }

    /// Drain all pending queued batches and merge them into a single vector of
    /// task names.
    ///
    /// This is called by the runtime when the DAG becomes idle and we want to
    /// start a new run based on everything that was queued while it was
    /// running.
    pub fn drain_pending(&mut self) -> Vec<TaskName> {
        let mut merged: HashSet<TaskName> = HashSet::new();

        while let Some(batch) = self.runs.pop_front() {
            merged.extend(batch);
        }

        let tasks: Vec<TaskName> = merged.into_iter().collect();
        debug!(drained = tasks.len(), "drained queued triggers into new run");
        tasks
    }
}
