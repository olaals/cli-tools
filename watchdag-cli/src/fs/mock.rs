// src/fs/mock.rs

use super::FileSystem;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub enum MockEntry {
    File(Vec<u8>),
    Dir(Vec<String>), // List of child names
}

#[derive(Debug, Clone, Default)]
pub struct MockFileSystem {
    files: Arc<Mutex<HashMap<PathBuf, MockEntry>>>,
}

impl MockFileSystem {
    pub fn new() -> Self {
        let mut files = HashMap::new();
        // Ensure root exists
        files.insert(PathBuf::from("."), MockEntry::Dir(Vec::new()));
        
        Self {
            files: Arc::new(Mutex::new(files)),
        }
    }

    pub fn add_file(&self, path: impl AsRef<Path>, content: impl Into<Vec<u8>>) {
        let path = path.as_ref().to_path_buf();
        let mut files = self.files.lock().unwrap();
        files.insert(path.clone(), MockEntry::File(content.into()));
        
        // Ensure parent directories exist implicitly for simplicity in this mock
        if let Some(parent) = path.parent() {
            let parent = if parent.as_os_str().is_empty() {
                Path::new(".")
            } else {
                parent
            };

            self.ensure_dir_entry(&mut files, parent);
            // Add this file to parent's children
            if let Some(MockEntry::Dir(children)) = files.get_mut(parent) {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if !children.contains(&name.to_string()) {
                        children.push(name.to_string());
                    }
                }
            }
        }
    }

    fn ensure_dir_entry(&self, files: &mut HashMap<PathBuf, MockEntry>, path: &Path) {
        if !files.contains_key(path) {
            files.insert(path.to_path_buf(), MockEntry::Dir(Vec::new()));
            if let Some(parent) = path.parent() {
                let parent = if parent.as_os_str().is_empty() {
                    Path::new(".")
                } else {
                    parent
                };

                if parent != path { // Avoid infinite loop at root
                    self.ensure_dir_entry(files, parent);
                     // Add this dir to parent's children
                    if let Some(MockEntry::Dir(children)) = files.get_mut(parent) {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            if !children.contains(&name.to_string()) {
                                children.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
}

impl FileSystem for MockFileSystem {
    fn read_to_string(&self, path: &Path) -> Result<String> {
        let files = self.files.lock().unwrap();
        match files.get(path) {
            Some(MockEntry::File(content)) => {
                String::from_utf8(content.clone()).map_err(|e| anyhow!("Invalid UTF-8: {}", e))
            }
            Some(MockEntry::Dir(_)) => Err(anyhow!("Is a directory: {:?}", path)),
            None => Err(anyhow!("File not found: {:?}", path)),
        }
    }

    fn open_read(&self, path: &Path) -> Result<Box<dyn Read + Send>> {
        let files = self.files.lock().unwrap();
        match files.get(path) {
            Some(MockEntry::File(content)) => Ok(Box::new(Cursor::new(content.clone()))),
            Some(MockEntry::Dir(_)) => Err(anyhow!("Is a directory: {:?}", path)),
            None => Err(anyhow!("File not found: {:?}", path)),
        }
    }

    fn write(&self, path: &Path, contents: &[u8]) -> Result<()> {
        self.add_file(path, contents);
        Ok(())
    }

    fn exists(&self, path: &Path) -> bool {
        let files = self.files.lock().unwrap();
        files.contains_key(path)
    }

    fn is_file(&self, path: &Path) -> bool {
        let files = self.files.lock().unwrap();
        matches!(files.get(path), Some(MockEntry::File(_)))
    }

    fn is_dir(&self, path: &Path) -> bool {
        let files = self.files.lock().unwrap();
        matches!(files.get(path), Some(MockEntry::Dir(_)))
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf> {
        // In mock, we just return the path as is, assuming absolute paths are used in tests
        Ok(path.to_path_buf())
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let files = self.files.lock().unwrap();
        match files.get(path) {
            Some(MockEntry::Dir(children)) => {
                Ok(children.iter().map(|name| path.join(name)).collect())
            }
            _ => Err(anyhow!("Not a directory or not found: {:?}", path)),
        }
    }
}
