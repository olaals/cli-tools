// src/dag/mod.rs

//! DAG representation and scheduling.
//!
//! - [`graph`] holds a simple directed acyclic graph of tasks.
//! - [`scheduler`] contains the per-run state machine that decides
//!   which tasks are ready to run, and when dependents can be scheduled.

pub mod graph;
pub mod scheduler;

pub use graph::DagGraph;
pub use scheduler::{ScheduledTask, Scheduler};
