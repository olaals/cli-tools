// src/engine/runtime.rs

use std::collections::HashSet;

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::dag::scheduler::{ScheduledTask, Scheduler};
use crate::engine::queue::TriggerQueue;

/// Public type alias for task names throughout the engine.
pub type TaskName = String;

/// Reason why a task was triggered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerReason {
    FileWatch,
    StdoutTrigger,
    Manual,
}

/// Result of a task process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskOutcome {
    Success,
    Failed(i32), // exit code
}

/// Events sent into the runtime from watchers, executors, or external signals.
///
/// The idea is that:
/// - watchers send `TaskTriggered`
/// - long-lived logic sends `TaskProgressed`
/// - executor sends `TaskCompleted`
/// - Ctrl-C handling sends `ShutdownRequested`
#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    TaskTriggered {
        task: TaskName,
        reason: TriggerReason,
    },
    TaskProgressed {
        task: TaskName,
    },
    TaskCompleted {
        task: TaskName,
        outcome: TaskOutcome,
    },
    ShutdownRequested,
}

/// Options that influence how the runtime behaves.
///
/// Higher layers (e.g. CLI/lib) can expand this later if needed.
#[derive(Debug, Clone)]
pub struct RuntimeOptions {
    /// If true, exit as soon as there is nothing left to run and no queued triggers.
    /// In watch mode this should be `false`.
    pub exit_when_idle: bool,
}

impl Default for RuntimeOptions {
    fn default() -> Self {
        Self {
            exit_when_idle: false,
        }
    }
}

/// The main orchestration runtime.
///
/// Responsibilities:
/// - Consume `RuntimeEvent`s from watchers/executor/ctrl-c.
/// - Apply queue semantics (queue vs cancel – currently queue only).
/// - Drive the DAG scheduler.
/// - Send `ScheduledTask`s to the executor when tasks are ready.
pub struct Runtime {
    scheduler: Scheduler,
    queue: TriggerQueue,
    options: RuntimeOptions,

    /// Unified event stream from all producers (watchers, executor, signal handler).
    events_rx: mpsc::Receiver<RuntimeEvent>,

    /// Channel to executor: whenever the scheduler marks a task as ready, we send it here.
    exec_tx: mpsc::Sender<ScheduledTask>,
}

impl Runtime {
    pub fn new(
        scheduler: Scheduler,
        queue: TriggerQueue,
        options: RuntimeOptions,
        events_rx: mpsc::Receiver<RuntimeEvent>,
        exec_tx: mpsc::Sender<ScheduledTask>,
    ) -> Self {
        Self {
            scheduler,
            queue,
            options,
            events_rx,
            exec_tx,
        }
    }

    /// Main event loop.
    ///
    /// This should be called from `lib.rs` after:
    /// - config is loaded & validated
    /// - `Scheduler` is constructed from config
    /// - `TriggerQueue` is built from `[config]` section
    /// - watchers & executor have been spawned and given a clone of the
    ///   `mpsc::Sender<RuntimeEvent>`
    pub async fn run(mut self) -> Result<()> {
        info!("watchdag runtime started");

        while let Some(event) = self.events_rx.recv().await {
            debug!(?event, "runtime received event");

            let keep_running = match event {
                RuntimeEvent::TaskTriggered { task, reason } => {
                    self.handle_task_trigger(task, reason).await?
                }
                RuntimeEvent::TaskProgressed { task } => self.handle_task_progress(task).await?,
                RuntimeEvent::TaskCompleted { task, outcome } => {
                    self.handle_task_completion(task, outcome).await?
                }
                RuntimeEvent::ShutdownRequested => {
                    info!("shutdown requested, stopping runtime");
                    false
                }
            };

            if !keep_running {
                break;
            }
        }

        info!("watchdag runtime exiting");
        Ok(())
    }

    /// Handle a trigger (usually from file watching, but also from stdout/manual).
    async fn handle_task_trigger(
        &mut self,
        task: TaskName,
        reason: TriggerReason,
    ) -> Result<bool> {
        info!(task = %task, ?reason, "task triggered");

        if self.scheduler.is_idle() {
            // We're starting a new DAG run. Combine this trigger with anything that
            // was queued while we were idle (e.g. from a previous run completion).
            let mut triggers: HashSet<TaskName> = self.queue.drain_pending().into_iter().collect();
            triggers.insert(task);

            self.start_new_run(triggers.into_iter().collect()).await?;
        } else {
            // DAG currently running – delegate to queue to record the trigger
            // (queue length / cancel behaviour is implemented inside TriggerQueue).
            self.queue.record_trigger(&task);
            debug!(task = %task, "task trigger recorded in queue");
        }

        Ok(true)
    }

    /// Handle progress notification from a long-lived task.
    ///
    /// This is used when `progress_on_stdout` or `progress_on_time` marks the
    /// task as "logically done" while the process may continue running.
    async fn handle_task_progress(&mut self, task: TaskName) -> Result<bool> {
        info!(task = %task, "task reported progress");

        let newly_ready = self.scheduler.handle_progress(&task);
        self.spawn_ready_tasks(newly_ready).await?;

        self.maybe_start_queued_run().await?;
        Ok(true)
    }

    /// Handle completion of a task process.
    ///
    /// This includes both success and failure; failures should cause dependents
    /// to fail/never run, which is handled inside `Scheduler::handle_completion`.
    async fn handle_task_completion(
        &mut self,
        task: TaskName,
        outcome: TaskOutcome,
    ) -> Result<bool> {
        match outcome {
            TaskOutcome::Success => info!(task = %task, "task completed successfully"),
            TaskOutcome::Failed(code) => {
                warn!(task = %task, exit_code = code, "task failed");
            }
        }

        let newly_ready = self.scheduler.handle_completion(&task, outcome);
        self.spawn_ready_tasks(newly_ready).await?;

        self.maybe_start_queued_run().await?;

        // In `--once` mode, we can exit when the DAG is idle and there are no
        // pending triggers in the queue.
        if self.options.exit_when_idle && self.scheduler.is_idle() && self.queue.is_empty() {
            info!("runtime idle and exit_when_idle=true, stopping");
            return Ok(false);
        }

        Ok(true)
    }

    /// Start a brand-new DAG run from the given set of root triggers.
    ///
    /// This resets the scheduler's per-run state (but NOT the underlying config).
    async fn start_new_run(&mut self, triggers: Vec<TaskName>) -> Result<()> {
        if triggers.is_empty() {
            debug!("start_new_run called with empty trigger set; nothing to do");
            return Ok(());
        }

        info!(triggers = ?triggers, "starting new DAG run");

        self.scheduler.start_new_run();

        for task in triggers {
            let newly_ready = self.scheduler.handle_trigger(&task);
            self.spawn_ready_tasks(newly_ready).await?;
        }

        Ok(())
    }

    /// If the scheduler is idle and there are queued triggers, start a new run.
    async fn maybe_start_queued_run(&mut self) -> Result<()> {
        if !self.scheduler.is_idle() {
            return Ok(());
        }

        let triggers = self.queue.drain_pending();
        if triggers.is_empty() {
            return Ok(());
        }

        self.start_new_run(triggers).await
    }

    /// Send all ready tasks to the executor.
    async fn spawn_ready_tasks(&mut self, tasks: Vec<ScheduledTask>) -> Result<()> {
        for task in tasks {
            debug!(task = %task.name, "dispatching task to executor");
            if let Err(err) = self.exec_tx.send(task).await {
                error!(error = %err, "failed to send task to executor");
                // If the executor channel is closed, there's not much we can do.
                // Bubble up the error so higher layers can decide what to do.
                return Err(err.into());
            }
        }
        Ok(())
    }
}
