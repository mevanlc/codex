//! Small filesystem helpers shared by the installation transaction.

use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;

/// True for any directory entry, including dangling links.
pub(crate) fn lexists(path: &Path) -> std::io::Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub(crate) fn remove_file_if_exists(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// Read a small marker file, or None when it does not exist.
pub(crate) fn read_marker(path: &Path) -> std::io::Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Replace `path` with `contents` without readers ever seeing a partial file.
pub(crate) fn atomic_write(path: &Path, contents: &[u8]) -> Result<()> {
    let name = path
        .file_name()
        .with_context(|| format!("{} has no file name", path.display()))?;
    let temporary = path.with_file_name(format!(".{}-{}", name.to_string_lossy(), random_hex(32)));
    let result = std::fs::write(&temporary, contents)
        .and_then(|()| std::fs::rename(&temporary, path))
        .with_context(|| format!("failed to write {}", path.display()));
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

/// Rename, retrying while Windows scanners briefly hold freshly written files.
pub(crate) fn rename_with_retry(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut retries = 0;
    loop {
        match std::fs::rename(from, to) {
            Err(error)
                if cfg!(windows)
                    && retries < 100
                    && (error.kind() == ErrorKind::PermissionDenied
                        || error.raw_os_error() == Some(32)) =>
            {
                retries += 1;
                std::thread::sleep(Duration::from_millis(50));
            }
            result => return result,
        }
    }
}

/// Resolve links in the longest existing prefix and keep the remaining
/// components, so missing paths still compare equal to their eventual form.
pub(crate) fn resolve_lenient(path: &Path) -> std::io::Result<PathBuf> {
    let mut missing = Vec::new();
    let mut existing = path;
    loop {
        match std::fs::canonicalize(existing) {
            Ok(mut resolved) => {
                resolved.extend(missing.iter().rev());
                return Ok(resolved);
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                let (Some(parent), Some(name)) = (existing.parent(), existing.file_name()) else {
                    return Err(error);
                };
                missing.push(name);
                existing = parent;
            }
            Err(error) => return Err(error),
        }
    }
}

pub(crate) fn random_hex(length: usize) -> String {
    let mut hex = uuid::Uuid::new_v4().simple().to_string();
    hex.truncate(length);
    hex
}
