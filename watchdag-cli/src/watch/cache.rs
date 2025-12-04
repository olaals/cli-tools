// src/watch/cache.rs

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tracing::debug;

use crate::watch::hash::compute_file_hash;

/// In-memory cache of file hashes.
///
/// This avoids re-reading and re-hashing every file on every event.
/// Instead, we only recompute the hash for the file that actually changed.
#[derive(Debug, Default)]
pub struct FileCache {
    hashes: HashMap<PathBuf, String>,
}

impl FileCache {
    pub fn new() -> Self {
        Self {
            hashes: HashMap::new(),
        }
    }

    /// Get the hash for a file, computing and caching it if necessary.
    pub fn get_or_compute(&mut self, path: &Path) -> Result<String> {
        if let Some(hash) = self.hashes.get(path) {
            return Ok(hash.clone());
        }

        debug!("cache miss: computing hash for {:?}", path);
        let hash = compute_file_hash(path)?;
        self.hashes.insert(path.to_path_buf(), hash.clone());
        Ok(hash)
    }

    /// Invalidate the cached hash for a file (e.g. on change).
    pub fn invalidate(&mut self, path: &Path) {
        if self.hashes.remove(path).is_some() {
            debug!("invalidated cache for {:?}", path);
        }
    }

    /// Force update the hash for a file.
    pub fn update(&mut self, path: &Path) -> Result<String> {
        debug!("updating cache for {:?}", path);
        let hash = compute_file_hash(path)?;
        self.hashes.insert(path.to_path_buf(), hash.clone());
        Ok(hash)
    }
}
