// src/watch/path_utils.rs

//! Utility functions for path handling in the watcher.

use std::path::Path;

/// Convert a path into a string relative to `root`, with forward slashes.
///
/// This is intentionally robust:
/// - First we try a direct `strip_prefix(root)`.
/// - If that fails (e.g. due to symlinks or different absolute prefixes),
///   we canonicalize both paths and try again.
/// - Only if both attempts fail do we give up.
///
/// Returns `None` if the path cannot be reasonably related to `root`.
pub fn relative_str(root: &Path, path: &Path) -> Option<String> {
    // Fast path: event path already starts with our root.
    if let Ok(rel) = path.strip_prefix(root) {
        let s = rel.to_string_lossy().replace('\\', "/");
        return Some(s);
    }

    // More robust path: canonicalize both, then try again. This helps on
    // platforms (notably macOS) where different absolute prefixes may be used
    // for the same underlying directory (e.g. symlinks, /private/var/...).
    if let (Ok(root_canon), Ok(path_canon)) =
        (root.canonicalize(), path.canonicalize())
    {
        if let Ok(rel) = path_canon.strip_prefix(&root_canon) {
            let s = rel.to_string_lossy().replace('\\', "/");
            return Some(s);
        }
    }

    None
}
