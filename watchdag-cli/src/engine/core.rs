// src/engine/core.rs

//! Pure core runtime state machine.
//!
//! This module contains a synchronous, deterministic "core runtime" that
//! consumes [`RuntimeEvent`]s and produces:
//! - an updated core state
//! - a list of "commands" describing what the IO shell should do next
//!
//! The async/IO-heavy shell (`engine::runtime::Runtime`) is responsible for:
//! - reading events from channels
//! - sending `ScheduledTask`s to the executor
//! - handling Ctrl+C / shutdown
//!
//! The core is intended to be extensively unit tested without any Tokio,
//! channels, filesystem, or processes.

use crate::dag::Scheduler;
use crate::engine::event_handlers::{
    handle_task_completion, handle_task_progress, handle_task_trigger,
    CoreStep,
};
use crate::engine::queue::TriggerQueue;
use crate::types::TriggerWhileRunningBehaviour;
use crate::engine::{RuntimeEvent, RuntimeOptions};

/// Pure core runtime state.
///
/// This owns:
/// - the DAG scheduler
/// - the trigger queue
/// - runtime options (e.g. `exit_when_idle`)
///
/// It has **no** channels, no Tokio types, and does not perform any IO.
#[derive(Debug)]
pub struct CoreRuntime {
    scheduler: Scheduler,
    queue: TriggerQueue,
    options: RuntimeOptions,
}

impl CoreRuntime {
    pub fn new(
        scheduler: Scheduler,
        behaviour: TriggerWhileRunningBehaviour,
        queue_length: usize,
        options: RuntimeOptions,
    ) -> Self {
        let queue = TriggerQueue::new(behaviour, queue_length);
        Self {
            scheduler,
            queue,
            options,
        }
    }

    /// Expose whether the scheduler is idle (for tests).
    pub fn is_idle(&self) -> bool {
        self.scheduler.is_idle()
    }

    /// Expose queue emptiness (for tests).
    pub fn queue_is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Handle a single runtime event, updating core state and returning the
    /// resulting commands for the IO shell.
    pub fn step(&mut self, event: RuntimeEvent) -> CoreStep {
        match event {
            RuntimeEvent::TaskTriggered { task, reason } => {
                handle_task_trigger(&mut self.scheduler, &mut self.queue, task, reason)
            }
            RuntimeEvent::TaskProgressed { task } => {
                handle_task_progress(&mut self.scheduler, &mut self.queue, task)
            }
            RuntimeEvent::TaskCompleted { task, outcome } => {
                handle_task_completion(
                    &mut self.scheduler,
                    &mut self.queue,
                    &self.options,
                    task,
                    outcome,
                )
            }
            RuntimeEvent::ShutdownRequested => CoreStep {
                commands: Vec::new(),
                keep_running: false,
            },
        }
    }
}
