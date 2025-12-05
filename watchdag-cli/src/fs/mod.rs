// src/fs/mod.rs

use std::fmt::Debug;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub mod mock;

/// Abstract filesystem interface.
pub trait FileSystem: Send + Sync + Debug {
    fn read_to_string(&self, path: &Path) -> Result<String>;
    fn open_read(&self, path: &Path) -> Result<Box<dyn Read + Send>>;
    fn write(&self, path: &Path, contents: &[u8]) -> Result<()>;
    fn exists(&self, path: &Path) -> bool;
    fn is_file(&self, path: &Path) -> bool;
    fn is_dir(&self, path: &Path) -> bool;
    fn canonicalize(&self, path: &Path) -> Result<PathBuf>;
    
    /// Return a list of entries in a directory.
    /// Returns full paths.
    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>>;
}

/// Implementation that uses `std::fs`.
#[derive(Debug, Clone, Default)]
pub struct RealFileSystem;

impl FileSystem for RealFileSystem {
    fn read_to_string(&self, path: &Path) -> Result<String> {
        fs::read_to_string(path).with_context(|| format!("reading file {:?}", path))
    }

    fn open_read(&self, path: &Path) -> Result<Box<dyn Read + Send>> {
        let file = fs::File::open(path).with_context(|| format!("opening file {:?}", path))?;
        Ok(Box::new(file))
    }

    fn write(&self, path: &Path, contents: &[u8]) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating dir {:?}", parent))?;
        }
        let mut file = fs::File::create(path).with_context(|| format!("creating file {:?}", path))?;
        file.write_all(contents).with_context(|| format!("writing to file {:?}", path))?;
        Ok(())
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf> {
        fs::canonicalize(path).with_context(|| format!("canonicalizing {:?}", path))
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(path).with_context(|| format!("reading dir {:?}", path))? {
            let entry = entry?;
            entries.push(entry.path());
        }
        Ok(entries)
    }
}
