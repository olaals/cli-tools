use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use blake3::Hasher;
use tracing::{debug, info};

use crate::engine::TaskName;

/// Relative path (from the watch root) to the hashes file.
///
/// The effective path on disk is:
///
/// `<root>/.watchdag/hashes`
///
/// where `<root>` is the directory passed to `spawn_watcher`.
pub const HASH_FILE_PATH: &str = ".watchdag/hashes";

fn hash_file_path(root: &Path) -> PathBuf {
    root.join(HASH_FILE_PATH)
}

/// Compute the hash of a single file.
pub fn compute_file_hash(path: &Path) -> Result<String> {
    let mut hasher = Hasher::new();
    let mut file = File::open(path)
        .with_context(|| format!("opening file for hashing: {:?}", path))?;
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

/// Compute a deterministic hash over the contents of the given files.
///
/// The caller is responsible for deciding which files belong to a task (e.g.
/// all files matching the effective watch patterns). Order of `paths` does not
/// matter; we sort them before hashing to keep the hash stable.
pub fn compute_hash_for_paths<I, P>(paths: I) -> Result<String>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let mut hasher = Hasher::new();

    // Collect and sort paths to ensure stable hashing independent of iteration order.
    let mut paths_vec: Vec<PathBuf> = paths
        .into_iter()
        .map(|p| p.as_ref().to_path_buf())
        .collect();
    paths_vec.sort();

    for path in paths_vec {
        if path.is_file() {
            debug!("hashing file {:?}", path);
            let file_hash = compute_file_hash(&path)?;
            hasher.update(file_hash.as_bytes());
        }
    }

    let hash = hasher.finalize().to_hex().to_string();
    debug!(hash = %hash, "computed aggregate hash");
    Ok(hash)
}

/// Compute aggregate hash from a list of file hashes.
///
/// `hashes` must be sorted by the corresponding file path to ensure stability.
pub fn compute_aggregate_hash(hashes: &[String]) -> String {
    let mut hasher = Hasher::new();
    for h in hashes {
        hasher.update(h.as_bytes());
    }
    hasher.finalize().to_hex().to_string()
}

/// Abstract storage for task hashes.
pub trait HashStore: Send + Sync {
    fn load(&self, task: &str) -> Result<Option<String>>;
    fn save(&mut self, task: &str, hash: &str) -> Result<()>;
    /// Remove hashes for tasks that are not in the `active_tasks` list.
    fn prune(&mut self, active_tasks: &[&str]) -> Result<()>;
}

/// Stores hashes in a file (`.watchdag/hashes`).
pub struct FileHashStore {
    root: PathBuf,
}

impl FileHashStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl HashStore for FileHashStore {
    fn load(&self, task: &str) -> Result<Option<String>> {
        let map = load_all_hashes(&self.root)?;
        Ok(map.get(task).cloned())
    }

    fn save(&mut self, task: &str, hash: &str) -> Result<()> {
        let mut map = load_all_hashes(&self.root)?;
        map.insert(task.to_string(), hash.to_string());
        save_all_hashes(&self.root, &map)?;
        info!(task = %task, hash = %hash, "stored task hash (file)");
        Ok(())
    }

    fn prune(&mut self, active_tasks: &[&str]) -> Result<()> {
        let mut map = load_all_hashes(&self.root)?;
        let initial_len = map.len();
        map.retain(|k, _| active_tasks.contains(&k.as_str()));
        
        if map.len() < initial_len {
            save_all_hashes(&self.root, &map)?;
            info!(
                removed = initial_len - map.len(),
                "pruned stale task hashes (file)"
            );
        }
        Ok(())
    }
}

/// Stores hashes in memory only.
pub struct MemoryHashStore {
    map: HashMap<String, String>,
}

impl MemoryHashStore {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
}

impl Default for MemoryHashStore {
    fn default() -> Self {
        Self::new()
    }
}

impl HashStore for MemoryHashStore {
    fn load(&self, task: &str) -> Result<Option<String>> {
        Ok(self.map.get(task).cloned())
    }

    fn save(&mut self, task: &str, hash: &str) -> Result<()> {
        self.map.insert(task.to_string(), hash.to_string());
        info!(task = %task, hash = %hash, "stored task hash (memory)");
        Ok(())
    }

    fn prune(&mut self, active_tasks: &[&str]) -> Result<()> {
        let initial_len = self.map.len();
        self.map.retain(|k, _| active_tasks.contains(&k.as_str()));
        if self.map.len() < initial_len {
            info!(
                removed = initial_len - self.map.len(),
                "pruned stale task hashes (memory)"
            );
        }
        Ok(())
    }
}

/// Load all stored task hashes from `<root>/.watchdag/hashes`.
fn load_all_hashes(root: &Path) -> Result<HashMap<TaskName, String>> {
    let path = hash_file_path(root);

    if !path.exists() {
        return Ok(HashMap::new());
    }

    let file = File::open(&path)
        .with_context(|| format!("opening hash file at {:?}", path))?;
    let reader = BufReader::new(file);

    let mut map = HashMap::new();

    for line_res in reader.lines() {
        let line = line_res?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((name, hash)) = trimmed.split_once(char::is_whitespace) {
            map.insert(name.to_string(), hash.trim().to_string());
        }
    }

    Ok(map)
}

/// Persist all task hashes to `<root>/.watchdag/hashes`.
fn save_all_hashes(root: &Path, map: &HashMap<TaskName, String>) -> Result<()> {
    let path = hash_file_path(root);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!("creating hash directory at {:?}", parent)
        })?;
    }

    let file = File::create(&path)
        .with_context(|| format!("creating hash file at {:?}", path))?;
    let mut writer = BufWriter::new(file);

    for (name, hash) in map.iter() {
        writeln!(writer, "{} {}", name, hash)?;
    }

    writer.flush()?;
    Ok(())
}
