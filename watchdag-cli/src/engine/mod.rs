// src/engine/mod.rs

//! Orchestration engine for watchdag.
//!
//! This module ties together:
//! - the DAG scheduler
//! - the trigger queue (what happens when triggers arrive while a run is active)
//! - the main runtime event loop that reacts to:
//!   - file-watch triggers
//!   - long-lived progress events
//!   - task completion events
//!   - shutdown signals
//!
//! The pure core state machine lives in [`core`]; the async/IO shell is
//! implemented in [`runtime`].

/// Canonical task name type used throughout the engine.
pub type TaskName = String;

/// Outcome of a task process for the scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskOutcome {
    Success,
    Failed(i32),
}

/// Why a task was triggered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerReason {
    /// Manual trigger (e.g. initial roots at startup).
    Manual,
    /// Triggered due to a filesystem event.
    FileWatch,
    /// Triggered due to `trigger_on_stdout`.
    StdoutTrigger,
}

/// Runtime options used by both the core and the async shell.
#[derive(Debug, Clone, Copy)]
pub struct RuntimeOptions {
    /// If true, exit the runtime once the DAG is idle and there are no
    /// queued triggers (used for `--once`).
    pub exit_when_idle: bool,
}

/// Events flowing into the runtime from watchers, executors, etc.
#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    /// A task should be (logically) triggered.
    TaskTriggered {
        task: TaskName,
        reason: TriggerReason,
    },
    /// A long-lived task reported logical progress.
    TaskProgressed {
        task: TaskName,
    },
    /// A task process exited with a concrete outcome.
    TaskCompleted {
        task: TaskName,
        outcome: TaskOutcome,
    },
    /// Graceful shutdown requested (e.g. Ctrl-C).
    ShutdownRequested,
}

pub mod core;
pub mod event_handlers;
pub mod queue;
pub mod runtime;

pub use core::CoreRuntime;
pub use event_handlers::{CoreCommand, CoreStep};
pub use queue::TriggerQueue;
pub use crate::types::TriggerWhileRunningBehaviour;
pub use runtime::Runtime;
