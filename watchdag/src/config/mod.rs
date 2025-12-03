// src/config/mod.rs

//! Configuration loading and validation for watchdag.
//!
//! Responsibilities:
//! - Define the TOML-backed data model (`model.rs`).
//! - Load a config file from disk (`loader.rs`).
//! - Validate basic invariants like DAG correctness (`validate.rs`).

pub mod loader;
pub mod model;
pub mod validate;

pub use loader::{load_and_validate, load_from_path};
pub use model::{ConfigFile, ConfigSection, DefaultSection, TaskConfig};
pub use validate::validate_config;
