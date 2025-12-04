use std::str::FromStr;
use serde::Deserialize;

/// Behaviour when a new trigger arrives while a DAG run is already in progress.
///
/// - `Queue`: remember the trigger and start a new DAG run when the current one
///   finishes (default behaviour).
/// - `Cancel`: conceptually means "drop any previously queued run and only keep
///   the latest trigger". Actual cancellation of the *running* DAG is handled
///   at a higher level; here we only manage queued triggers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TriggerWhileRunningBehaviour {
    Queue,
    Cancel,
}

impl Default for TriggerWhileRunningBehaviour {
    fn default() -> Self {
        TriggerWhileRunningBehaviour::Queue
    }
}

impl FromStr for TriggerWhileRunningBehaviour {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "queue" => Ok(TriggerWhileRunningBehaviour::Queue),
            "cancel" => Ok(TriggerWhileRunningBehaviour::Cancel),
            other => Err(format!(
                "invalid triggered_while_running_behaviour: {other} (expected \"queue\" or \"cancel\")"
            )),
        }
    }
}

/// Mode for storing task hashes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HashStorageMode {
    /// Store hashes in a file (`.watchdag/hashes`).
    File,
    /// Store hashes in memory only (lost on restart).
    Memory,
}

impl Default for HashStorageMode {
    fn default() -> Self {
        HashStorageMode::Memory
    }
}

