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

pub mod queue;
pub mod runtime;

pub use queue::{TriggerQueue, TriggerWhileRunningBehaviour};
pub use runtime::{
    Runtime, RuntimeEvent, RuntimeOptions, TaskName, TaskOutcome, TriggerReason,
};
