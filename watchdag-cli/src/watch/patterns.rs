// src/watch/patterns.rs

use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::engine::TaskName;
use crate::fs::FileSystem;

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
/// - `use_hash` indicates whether this task should only trigger when its
///   aggregated file contents actually change.
/// - `deps` is the list of direct dependencies (`after = [...]`) so that the
///   watcher can be made DAG-aware.
#[derive(Debug, Clone)]
pub struct RawTaskPatternSpec {
    pub name: TaskName,
    pub watch: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
    pub append_default_watch: bool,
    pub append_default_exclude: bool,
    pub use_hash: bool,
    pub deps: Vec<TaskName>,
}

impl RawTaskPatternSpec {
    pub fn new<N: Into<TaskName>>(
        name: N,
        watch: Option<Vec<String>>,
        exclude: Option<Vec<String>>,
        append_default_watch: bool,
        append_default_exclude: bool,
        use_hash: bool,
    ) -> Self {
        Self {
            name: name.into(),
            watch,
            exclude,
            append_default_watch,
            append_default_exclude,
            use_hash,
            // Callers that use `new` directly can fill `deps` later if they care.
            deps: Vec::new(),
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
    /// Direct dependencies (`after = [...]`) of this task.
    deps: Vec<TaskName>,
    watch_set: GlobSet,
    exclude_set: Option<GlobSet>,
    use_hash: bool,
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

    /// Direct dependencies (`after = [...]`) for this task.
    ///
    /// Used by the watcher to determine ancestor relationships for a given
    /// path and only trigger "root" tasks for that path.
    pub fn deps(&self) -> &[TaskName] {
        &self.deps
    }

    /// Whether this task uses content hashing (`use_hash = true`).
    pub fn use_hash(&self) -> bool {
        self.use_hash
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
            deps: spec.deps.clone(),
            watch_set,
            exclude_set,
            use_hash: spec.use_hash,
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

/// Collect all files under `root` that match this task's watch/exclude patterns.
///
/// This is used by the watcher when computing aggregated hashes for
/// `use_hash = true` tasks.
pub fn collect_matching_files(
    fs: &dyn FileSystem,
    root: &Path,
    profile: &TaskWatchProfile,
) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        for path in fs.read_dir(&dir)? {
            if fs.is_dir(&path) {
                stack.push(path);
            } else if fs.is_file(&path) {
                if let Ok(rel) = path.strip_prefix(root) {
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    if profile.matches(&rel_str) {
                        files.push(path);
                    }
                }
            }
        }
    }

    Ok(files)
}

/// Convenience: build `WatchDefaults` + compiled `TaskWatchProfile`s directly
/// from a loaded `ConfigFile`.
///
/// This centralises the logic that was previously duplicated across the
/// application and tests: it creates the effective `WatchDefaults`, derives
/// `RawTaskPatternSpec` for each task (including wiring `after` -> `deps`),
/// and compiles the `TaskWatchProfile`s.
pub fn build_profiles_from_config(
    cfg: &crate::config::model::ConfigFile,
) -> Result<(WatchDefaults, Vec<TaskWatchProfile>)> {
    let defaults = WatchDefaults {
        watch: cfg.default_section().watch.clone(),
        exclude: cfg.default_section().exclude.clone(),
    };

    let default_use_hash = cfg.default_section().use_hash.unwrap_or(false);

    let specs: Vec<RawTaskPatternSpec> = cfg
        .tasks()
        .iter()
        .map(|(name, t)| RawTaskPatternSpec {
            name: name.clone(),
            watch: t.watch.clone(),
            exclude: t.exclude.clone(),
            append_default_watch: t.append_default_watch,
            append_default_exclude: t.append_default_exclude,
            use_hash: t.effective_use_hash(default_use_hash),
            deps: t.after.clone(),
        })
        .collect();

    let profiles = build_task_watch_profiles(&defaults, &specs)?;
    Ok((defaults, profiles))
}
