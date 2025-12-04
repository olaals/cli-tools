// src/dag/mod.rs

//! DAG representation and scheduling.
//!
//! - [`graph`] holds a simple directed acyclic graph of tasks.
//! - [`scheduler`] contains the per-run state machine that decides
//!   which tasks are ready to run, and when dependents can be scheduled.
//! - [`task_info`] provides task metadata and scheduled task types.
//! - [`scheduler_step`] defines the result type for scheduler steps.
//! - [`state_manager`] manages per-run state transitions.

pub mod graph;
pub mod scheduler;
pub mod scheduler_step;
pub mod state_manager;
pub mod task_info;

pub use graph::DagGraph;
pub use scheduler::Scheduler;
pub use scheduler_step::SchedulerStep;
pub use task_info::{ScheduledTask, TaskRunState};
