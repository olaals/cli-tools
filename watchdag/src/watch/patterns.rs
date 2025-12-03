// src/watch/patterns.rs

use std::fmt;

use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::engine::TaskName;

/// Default watch configuration from `[default]` in the config.
///
/// This corresponds roughly to:
///
/// ```toml
/// [default]
/// watch = ["src/**/*.py", "scripts/**/*.sh"]
/// exclude = ["src/**/*tmp.py"]
/// ```
#[derive(Debug, Clone, Default)]
pub struct WatchDefaults {
    pub watch: Vec<String>,
    pub exclude: Vec<String>,
}

/// Raw per-task pattern specification coming from the high-level config.
///
/// This is a minimal representation that `config` can map into from your TOML
/// structs. It mirrors the semantics in the examples:
///
/// - `watch` / `exclude` are optional task-local lists.
/// - `append_default_watch` / `append_default_exclude` control whether the
///   task lists are merged with the default lists.
///
/// The final, effective patterns are computed by `build_task_watch_profiles`.
#[derive(Debug, Clone)]
pub struct RawTaskPatternSpec {
    pub name: TaskName,
    pub watch: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
    pub append_default_watch: bool,
    pub append_default_exclude: bool,
}

impl RawTaskPatternSpec {
    pub fn new<N: Into<TaskName>>(
        name: N,
        watch: Option<Vec<String>>,
        exclude: Option<Vec<String>>,
        append_default_watch: bool,
        append_default_exclude: bool,
    ) -> Self {
        Self {
            name: name.into(),
            watch,
            exclude,
            append_default_watch,
            append_default_exclude,
        }
    }
}

/// Compiled watch/exclude glob patterns for a single task.
///
/// The patterns are assumed to be relative to some "project root" directory.
/// The watcher will pass relative paths (e.g. `"src/main.rs"`) into `matches`.
#[derive(Clone)]
pub struct TaskWatchProfile {
    name: TaskName,
    watch_set: GlobSet,
    exclude_set: Option<GlobSet>,
}

impl fmt::Debug for TaskWatchProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TaskWatchProfile")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl TaskWatchProfile {
    /// Name of the task this profile belongs to.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns true if this task should be considered interested in the given
    /// path (relative to project root), e.g. `"src/foo/bar.py"`.
    pub fn matches(&self, rel_path: &str) -> bool {
        if !self.watch_set.is_match(rel_path) {
            return false;
        }
        if let Some(exclude) = &self.exclude_set {
            if exclude.is_match(rel_path) {
                return false;
            }
        }
        true
    }
}

/// Build a compiled watch profile for each task.
///
/// This applies the default + append logic described in the README:
///
/// - If `append_default_watch = true`, effective watch list is
///   `task.watch + default.watch`.
/// - Else, if `task.watch` is Some, use only that.
/// - Else, use `default.watch`.
///
/// Same rules for `exclude`.
pub fn build_task_watch_profiles(
    defaults: &WatchDefaults,
    specs: &[RawTaskPatternSpec],
) -> Result<Vec<TaskWatchProfile>> {
    let mut profiles = Vec::with_capacity(specs.len());

    for spec in specs {
        let watch_patterns = effective_patterns(
            spec.watch.as_ref(),
            &defaults.watch,
            spec.append_default_watch,
        );

        let exclude_patterns = effective_patterns(
            spec.exclude.as_ref(),
            &defaults.exclude,
            spec.append_default_exclude,
        );

        let watch_set =
            build_globset(&watch_patterns).with_context(|| {
                format!("building watch globset for task {}", spec.name)
            })?;

        let exclude_set = if exclude_patterns.is_empty() {
            None
        } else {
            Some(
                build_globset(&exclude_patterns).with_context(|| {
                    format!("building exclude globset for task {}", spec.name)
                })?,
            )
        };

        profiles.push(TaskWatchProfile {
            name: spec.name.clone(),
            watch_set,
            exclude_set,
        });
    }

    Ok(profiles)
}

/// Helper to decide the effective patterns list for a given dimension (watch or exclude).
fn effective_patterns(
    task_list: Option<&Vec<String>>,
    default_list: &Vec<String>,
    append_default: bool,
) -> Vec<String> {
    match (task_list, append_default) {
        (Some(list), true) => {
            let mut combined = list.clone();
            combined.extend(default_list.iter().cloned());
            combined
        }
        (Some(list), false) => list.clone(),
        (None, _) => default_list.clone(),
    }
}

/// Build a GlobSet from simple string patterns.
fn build_globset(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pat in patterns {
        let glob = Glob::new(pat)
            .with_context(|| format!("invalid glob pattern: {pat}"))?;
        builder.add(glob);
    }
    Ok(builder.build()?)
}
