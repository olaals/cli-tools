// src/errors.rs

//! Crate-wide error aliases and helpers.
//!
//! At the moment this is just a thin wrapper around `anyhow`, but the module
//! gives you a single place to add more structured error types later.

pub use anyhow::{Error, Result};
