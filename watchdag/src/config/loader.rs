// src/config/loader.rs

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::config::model::ConfigFile;
use crate::config::validate::validate_config;

/// Load a configuration file from a given path and return the raw `ConfigFile`.
///
/// This only performs TOML deserialization; it does **not** perform semantic
/// validation (DAG correctness, etc.). Use [`load_and_validate`] for that.
pub fn load_from_path(path: impl AsRef<Path>) -> Result<ConfigFile> {
    let path = path.as_ref();
    let contents = fs::read_to_string(path)
        .with_context(|| format!("reading config file at {:?}", path))?;

    let config: ConfigFile = toml::from_str(&contents)
        .with_context(|| format!("parsing TOML config from {:?}", path))?;

    Ok(config)
}

/// Load a configuration file from path and run basic validation.
///
/// This is the recommended entry point for the rest of the application:
///
/// - Reads TOML.
/// - Applies defaults (handled by `serde` + `Default` impls).
/// - Checks for:
///   - unknown `after` references,
///   - DAG cycles,
///   - basic global config sanity.
///
/// Higher-level modules can then transform `ConfigFile` into:
/// - watch patterns (`RawTaskPatternSpec`, `WatchDefaults`)
/// - DAG structures
/// - runtime options, etc.
pub fn load_and_validate(path: impl AsRef<Path>) -> Result<ConfigFile> {
    let config = load_from_path(&path)?;
    validate_config(&config)?;
    Ok(config)
}

/// Helper to resolve a default config path.
///
/// Currently this just returns `Watchdag.toml` in the current working
/// directory, but this function exists so you can later:
///
/// - Respect an env var (e.g. `WATCHDAG_CONFIG`).
/// - Look for multiple default locations.
/// - Support project-local config discovery.
pub fn default_config_path() -> PathBuf {
    PathBuf::from("Watchdag.toml")
}
