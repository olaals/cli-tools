// src/config/model.rs

use std::collections::BTreeMap;

use serde::Deserialize;

/// Top-level configuration as read from a TOML file.
///
/// This is a direct mapping of your examples:
///
/// ```toml
/// [config]
/// triggered_while_running_behaviour = "queue"
/// queue_length = 1
///
/// [default]
/// watch = ["src/**/*.py"]
/// exclude = ["src/**/*tmp.py"]
///
/// [task.A]
/// cmd = "echo A"
/// after = ["B"]
/// ```
///
/// All sections are optional and have reasonable defaults.
#[derive(Debug, Clone, Deserialize)]
pub struct ConfigFile {
    /// Global behaviour config from `[config]`.
    #[serde(default)]
    pub config: ConfigSection,

    /// Defaults for `watch`, `exclude`, `use_hash` from `[default]`.
    #[serde(default)]
    pub default: DefaultSection,

    /// All tasks from `[task.<name>]`.
    ///
    /// Keys are the *task names* (e.g. `"A"`, `"first"`, `"B2"`).
    #[serde(default)]
    pub task: BTreeMap<String, TaskConfig>,
}

/// `[config]` section.
///
/// Currently controls behaviour when triggers arrive while a DAG run is active.
#[derive(Debug, Clone, Deserialize)]
pub struct ConfigSection {
    /// `"queue"` or `"cancel"`.
    ///
    /// - `"queue"` (default): remember triggers and run them after the current
    ///   DAG finishes.
/// - `"cancel"`: drop any queued runs and only keep the latest trigger.
///   (Actual cancellation of the running DAG is handled at a higher level.)
    #[serde(
        default = "default_triggered_while_running_behaviour",
        rename = "triggered_while_running_behaviour"
    )]
    pub triggered_while_running_behaviour: String,

    /// Maximum number of queued "runs" to remember.
    ///
    /// In your current README, the default is effectively 1.
    #[serde(default = "default_queue_length")]
    pub queue_length: usize,
}

fn default_triggered_while_running_behaviour() -> String {
    "queue".to_string()
}

fn default_queue_length() -> usize {
    1
}

impl Default for ConfigSection {
    fn default() -> Self {
        Self {
            triggered_while_running_behaviour: default_triggered_while_running_behaviour(),
            queue_length: default_queue_length(),
        }
    }
}

/// `[default]` section.
///
/// Mirrors examples like:
///
/// ```toml
/// [default]
/// watch = ["src/**/*.py", "scripts/**/*.sh"]
/// exclude = ["src/**/*tmp.py"]
/// use_hash = true
/// ```
#[derive(Debug, Clone, Deserialize, Default)]
pub struct DefaultSection {
    /// Default `watch` patterns applied to tasks that do not override them.
    #[serde(default)]
    pub watch: Vec<String>,

    /// Default `exclude` patterns applied to tasks that do not override them.
    #[serde(default)]
    pub exclude: Vec<String>,

    /// Default `use_hash` behaviour; if `None`, the global default is `false`.
    #[serde(default)]
    pub use_hash: Option<bool>,
}

/// `[task.<name>]` section.
///
/// This is intentionally quite close to your TOML examples so it’s easy to
/// reason about and extend.
#[derive(Debug, Clone, Deserialize)]
pub struct TaskConfig {
    /// The command to execute.
    pub cmd: String,

    /// Optional task-local watch patterns.
    ///
    /// If `None`, the task uses `default.watch`.
    #[serde(default)]
    pub watch: Option<Vec<String>>,

    /// Optional task-local exclude patterns.
    ///
    /// If `None`, the task uses `default.exclude`.
    #[serde(default)]
    pub exclude: Option<Vec<String>>,

    /// If true, `default.watch` is appended to `task.watch`.
    ///
    /// Otherwise, `task.watch` replaces `default.watch`.
    #[serde(default)]
    pub append_default_watch: bool,

    /// If true, `default.exclude` is appended to `task.exclude`.
    ///
    /// Otherwise, `task.exclude` replaces `default.exclude`.
    #[serde(default)]
    pub append_default_exclude: bool,

    /// Dependency list: this task waits for all tasks listed here.
    ///
    /// This is the TOML `after = ["A", "B"]` field.
    #[serde(default)]
    pub after: Vec<String>,

    /// Optional per-task hash behaviour; if `None`, falls back to
    /// `default.use_hash` (or global default false).
    #[serde(default)]
    pub use_hash: Option<bool>,

    /// Whether this is a long-lived command (may run indefinitely).
    ///
    /// This matches `long_lived = true` in your examples.
    #[serde(default)]
    pub long_lived: bool,

    /// Whether to re-run the command on subsequent triggers.
    ///
    /// In your description:
    /// - `false`: do not re-run; attach to the original stdout.
    /// - `true`: kill and restart on each trigger (for long-lived tasks).
    ///
    /// We keep this as `Option<bool>` so that the higher layers can decide
    /// a default (e.g. default to `true` as described in the comments).
    #[serde(default)]
    pub rerun: Option<bool>,

    /// Regex used to mark this task as "progressed" based on stdout.
    ///
    /// This corresponds to `progress_on_stdout = "^hello"` in examples.
    #[serde(default)]
    pub progress_on_stdout: Option<String>,

    /// Regex used to *trigger* additional runs or downstream tasks based on
    /// stdout (from `trigger_on_stdout` in your examples).
    #[serde(default)]
    pub trigger_on_stdout: Option<String>,

    /// Duration string (e.g. `"3s"`) used to mark this task as "progressed"
    /// after a fixed amount of time.
    ///
    /// This corresponds to `progress_on_time = "3s"` in examples.
    #[serde(default)]
    pub progress_on_time: Option<String>,
}

impl TaskConfig {
    /// Convenience: get the effective `rerun` value, with a default of `true`
    /// when unspecified, matching the comments in your examples.
    pub fn effective_rerun(&self) -> bool {
        self.rerun.unwrap_or(true)
    }

    /// Convenience: effective `use_hash` given a default from `[default]`.
    pub fn effective_use_hash(&self, default_use_hash: bool) -> bool {
        self.use_hash.unwrap_or(default_use_hash)
    }
}
