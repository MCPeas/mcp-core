// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Safe path resolution to prevent path traversal attacks.
//!
//! When serving files from a base directory (e.g. scraped data), user-controlled
//! path segments must be validated to ensure they cannot escape the base directory
//! via `..` components, absolute paths, or symlinks pointing outside.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// Error returned when path resolution fails.
#[derive(Debug)]
pub enum SafePathError {
    /// The base directory does not exist or is inaccessible.
    InvalidBase(io::Error),
    /// The resolved path does not exist or is inaccessible.
    NotFound(io::Error),
    /// The path would escape the base directory (traversal or external symlink).
    PathTraversal,
}

impl fmt::Display for SafePathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBase(e) => write!(f, "invalid base directory: {e}"),
            Self::NotFound(e) => write!(f, "path not found: {e}"),
            Self::PathTraversal => write!(f, "path traversal denied"),
        }
    }
}

impl std::error::Error for SafePathError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidBase(e) | Self::NotFound(e) => Some(e),
            Self::PathTraversal => None,
        }
    }
}

/// Safely resolve a user-provided path within a base directory.
///
/// Returns the canonicalized path on success. The resolved path is guaranteed to
/// be inside `base` — any attempt to escape via `..`, absolute paths, or symlinks
/// pointing outside the base directory will return [`SafePathError::PathTraversal`].
///
/// # Example
///
/// ```rust,no_run
/// use std::path::Path;
/// use mcp_core::config::safe_path::safe_resolve;
///
/// let base = Path::new("/srv/data");
/// let resolved = safe_resolve(base, "esth/2024/topic/inhalt.html.gz").unwrap();
/// assert!(resolved.starts_with("/srv/data"));
/// ```
pub fn safe_resolve(base: &Path, user_path: &str) -> Result<PathBuf, SafePathError> {
    // Reject absolute paths and null bytes early.
    if user_path.starts_with('/') || user_path.starts_with('\\') || user_path.contains('\0') {
        return Err(SafePathError::PathTraversal);
    }

    // Canonicalize the base directory (must exist).
    let canonical_base = base.canonicalize().map_err(SafePathError::InvalidBase)?;

    // Build the candidate and canonicalize it — this resolves `.`, `..`, and
    // symlinks to their real targets on disk.
    let candidate = canonical_base.join(user_path);
    let canonical = candidate.canonicalize().map_err(SafePathError::NotFound)?;

    // The canonical path must still be inside the base directory.
    if !canonical.starts_with(&canonical_base) {
        return Err(SafePathError::PathTraversal);
    }

    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs as unix_fs;

    fn setup_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("data");
        fs::create_dir_all(base.join("sub/nested")).unwrap();
        fs::write(base.join("file.txt"), "hello").unwrap();
        fs::write(base.join("sub/nested/deep.txt"), "deep").unwrap();
        dir
    }

    #[test]
    fn resolves_simple_relative_path() {
        let dir = setup_dir();
        let base = dir.path().join("data");
        let result = safe_resolve(&base, "file.txt").unwrap();
        assert!(result.starts_with(base.canonicalize().unwrap()));
    }

    #[test]
    fn resolves_nested_path() {
        let dir = setup_dir();
        let base = dir.path().join("data");
        let result = safe_resolve(&base, "sub/nested/deep.txt").unwrap();
        assert!(result.starts_with(base.canonicalize().unwrap()));
    }

    #[test]
    fn rejects_dot_dot_escape() {
        let dir = setup_dir();
        let base = dir.path().join("data");
        // Create a file outside the base so canonicalize would succeed
        fs::write(dir.path().join("secret.txt"), "secret").unwrap();
        let result = safe_resolve(&base, "../secret.txt");
        assert!(matches!(result, Err(SafePathError::PathTraversal)));
    }

    #[test]
    fn rejects_encoded_traversal() {
        let dir = setup_dir();
        let base = dir.path().join("data");
        // Even sneaky traversals like sub/../../ are caught after canonicalization
        fs::write(dir.path().join("secret.txt"), "secret").unwrap();
        let result = safe_resolve(&base, "sub/../../secret.txt");
        assert!(matches!(result, Err(SafePathError::PathTraversal)));
    }

    #[test]
    fn rejects_absolute_path() {
        let dir = setup_dir();
        let base = dir.path().join("data");
        let result = safe_resolve(&base, "/etc/passwd");
        assert!(matches!(result, Err(SafePathError::PathTraversal)));
    }

    #[test]
    fn rejects_null_byte() {
        let dir = setup_dir();
        let base = dir.path().join("data");
        let result = safe_resolve(&base, "file\0.txt");
        assert!(matches!(result, Err(SafePathError::PathTraversal)));
    }

    #[test]
    fn rejects_symlink_outside_base() {
        let dir = setup_dir();
        let base = dir.path().join("data");
        // Create a file outside the base
        let outside = dir.path().join("outside.txt");
        fs::write(&outside, "outside").unwrap();
        // Create a symlink inside the base pointing outside
        unix_fs::symlink(&outside, base.join("evil_link")).unwrap();

        let result = safe_resolve(&base, "evil_link");
        assert!(matches!(result, Err(SafePathError::PathTraversal)));
    }

    #[test]
    fn allows_symlink_within_base() {
        let dir = setup_dir();
        let base = dir.path().join("data");
        // Create a symlink inside the base pointing to another file inside
        unix_fs::symlink(base.join("file.txt"), base.join("good_link")).unwrap();

        let result = safe_resolve(&base, "good_link").unwrap();
        assert!(result.starts_with(base.canonicalize().unwrap()));
    }

    #[test]
    fn returns_not_found_for_missing_path() {
        let dir = setup_dir();
        let base = dir.path().join("data");
        let result = safe_resolve(&base, "nonexistent.txt");
        assert!(matches!(result, Err(SafePathError::NotFound(_))));
    }

    #[test]
    fn returns_invalid_base_for_missing_base() {
        let result = safe_resolve(Path::new("/does/not/exist"), "file.txt");
        assert!(matches!(result, Err(SafePathError::InvalidBase(_))));
    }
}
