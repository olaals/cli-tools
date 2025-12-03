// src/watch/hash.rs

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use blake3::Hasher;
use tracing::{debug, info};

use crate::engine::TaskName;

/// Path to the hashes file, relative to the current working directory.
///
/// The file format is a simple line-based mapping:
///
/// ```text
/// task_name_1 <whitespace> hex_hash_1
/// task_name_2 <whitespace> hex_hash_2
/// ...
/// ```
///
/// This matches the README description: a text file in `.watchdag/hashes` with
/// key-value pairs for tasks and their aggregated hash.
pub const HASH_FILE_PATH: &str = ".watchdag/hashes";

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
    let mut paths_vec: Vec<PathBuf> = paths.into_iter().map(|p| p.as_ref().to_path_buf()).collect();
    paths_vec.sort();

    for path in paths_vec {
        if path.is_file() {
            debug!("hashing file {:?}", path);
            let mut file = File::open(&path)
                .with_context(|| format!("opening file for hashing: {:?}", path))?;
            let mut buf = [0u8; 8192];
            loop {
                let n = file.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buf[..n]);
            }
        }
    }

    let hash = hasher.finalize().to_hex().to_string();
    debug!(hash = %hash, "computed aggregate hash");
    Ok(hash)
}

/// Load all stored task hashes from `.watchdag/hashes`.
fn load_all_hashes() -> Result<HashMap<TaskName, String>> {
    let path = Path::new(HASH_FILE_PATH);

    if !path.exists() {
        return Ok(HashMap::new());
    }

    let file = File::open(path)
        .with_context(|| format!("opening hash file at {:?}", HASH_FILE_PATH))?;
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

/// Persist all task hashes to `.watchdag/hashes`.
fn save_all_hashes(map: &HashMap<TaskName, String>) -> Result<()> {
    let path = Path::new(HASH_FILE_PATH);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!("creating hash directory at {:?}", parent)
        })?;
    }

    let file = File::create(path)
        .with_context(|| format!("creating hash file at {:?}", HASH_FILE_PATH))?;
    let mut writer = BufWriter::new(file);

    for (name, hash) in map.iter() {
        writeln!(writer, "{} {}", name, hash)?;
    }

    writer.flush()?;
    Ok(())
}

/// Load the previously stored hash for a given task, if present.
pub fn load_task_hash(task: &str) -> Result<Option<String>> {
    let map = load_all_hashes()?;
    Ok(map.get(task).cloned())
}

/// Save the hash for a given task, merging with existing entries.
pub fn save_task_hash(task: &str, hash: &str) -> Result<()> {
    let mut map = load_all_hashes()?;
    map.insert(task.to_string(), hash.to_string());
    save_all_hashes(&map)?;
    info!(task = %task, hash = %hash, "stored task hash");
    Ok(())
}
