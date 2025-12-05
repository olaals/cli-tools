#![allow(dead_code)]

use std::collections::BTreeMap;
use watchdag::config::{ConfigFile, RawConfigFile, ConfigSection, DefaultSection, TaskConfig};

/// Builder for `ConfigFile` to simplify test setup.
pub struct ConfigFileBuilder {
    config: RawConfigFile,
}

impl ConfigFileBuilder {
    pub fn new() -> Self {
        Self {
            config: RawConfigFile {
                config: ConfigSection::default(),
                default: DefaultSection::default(),
                task: BTreeMap::new(),
            },
        }
    }

    pub fn with_task(mut self, name: &str, task: TaskConfig) -> Self {
        self.config.task.insert(name.to_string(), task);
        self
    }

    pub fn with_global_watch(mut self, pattern: &str) -> Self {
        self.config.default.watch.push(pattern.to_string());
        self
    }

    pub fn with_global_exclude(mut self, pattern: &str) -> Self {
        self.config.default.exclude.push(pattern.to_string());
        self
    }

    pub fn with_default_use_hash(mut self, val: bool) -> Self {
        self.config.default.use_hash = Some(val);
        self
    }

    pub fn build(self) -> ConfigFile {
        ConfigFile::try_from(self.config).expect("Failed to build valid config from builder")
    }
}

impl Default for ConfigFileBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for `TaskConfig`.
pub struct TaskConfigBuilder {
    task: TaskConfig,
}

impl TaskConfigBuilder {
    pub fn new(cmd: &str) -> Self {
        Self {
            task: TaskConfig {
                cmd: cmd.to_string(),
                watch: None,
                exclude: None,
                append_default_watch: false,
                append_default_exclude: false,
                after: vec![],
                use_hash: None,
                long_lived: false,
                rerun: None,
                progress_on_stdout: None,
                trigger_on_stdout: None,
                progress_on_time: None,
                run_on_own_files_only: false,
            }
        }
    }

    pub fn after(mut self, dep: &str) -> Self {
        self.task.after.push(dep.to_string());
        self
    }

    pub fn watch(mut self, pattern: &str) -> Self {
        let watches = self.task.watch.get_or_insert(vec![]);
        watches.push(pattern.to_string());
        self
    }

    pub fn exclude(mut self, pattern: &str) -> Self {
        let excludes = self.task.exclude.get_or_insert(vec![]);
        excludes.push(pattern.to_string());
        self
    }

    pub fn use_hash(mut self, val: bool) -> Self {
        self.task.use_hash = Some(val);
        self
    }

    pub fn long_lived(mut self, val: bool) -> Self {
        self.task.long_lived = val;
        self
    }

    pub fn rerun(mut self, val: bool) -> Self {
        self.task.rerun = Some(val);
        self
    }

    pub fn progress_on_stdout(mut self, pattern: &str) -> Self {
        self.task.progress_on_stdout = Some(pattern.to_string());
        self
    }

    pub fn trigger_on_stdout(mut self, pattern: &str) -> Self {
        self.task.trigger_on_stdout = Some(pattern.to_string());
        self
    }

    pub fn progress_on_time(mut self, duration: &str) -> Self {
        self.task.progress_on_time = Some(duration.to_string());
        self
    }

    pub fn run_on_own_files_only(mut self, val: bool) -> Self {
        self.task.run_on_own_files_only = val;
        self
    }

    pub fn build(self) -> TaskConfig {
        self.task
    }
}
