//! One-time relocation of host-owned fork settings before loading user config.

use crate::CONFIG_OVERLAY_FILE;
use crate::CONFIG_TOML_FILE;
use crate::fork_config::FORK_CONFIG_PATHS;
use crate::fork_config::validate_config_file;
use codex_utils_path::resolve_symlink_write_paths;
use codex_utils_path::write_atomically;
use std::fs;
use std::io;
use std::path::Path;
use toml_edit::DocumentMut;
use toml_edit::Item;
use toml_edit::Table;

/// Move top-level fork settings from this host's user config into its overlay.
///
/// Call at host-local entry points, never against a remote filesystem's paths.
/// Existing overlay values win. Profiles and project files are not migrated
/// because moving their values into a global overlay would change their scope.
pub async fn migrate_user_fork_config(codex_home: &Path) -> io::Result<()> {
    let codex_home = codex_home.to_path_buf();
    tokio::task::spawn_blocking(move || migrate(&codex_home))
        .await
        .map_err(|error| io::Error::other(format!("fork config migration task failed: {error}")))?
}

fn migrate(codex_home: &Path) -> io::Result<()> {
    let shared_file = codex_home.join(CONFIG_TOML_FILE);
    let mut shared = match read_document(&shared_file) {
        Ok(shared) => shared,
        // Leave syntax diagnostics (including source spans) to the loader.
        Err(error) if error.kind() == io::ErrorKind::InvalidData => return Ok(()),
        Err(error) => return Err(error),
    };
    if !has_fork_settings(&shared) {
        return Ok(());
    }

    // Serialize concurrent startups, then reread: another process may have
    // completed the migration while we waited. Keep the lock file's inode stable.
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options.open(codex_home.join("config-overlay.toml.lock"))?;
    lock.lock()?;
    shared = read_document(&shared_file)?;
    if !has_fork_settings(&shared) {
        return Ok(());
    }

    let overlay_file = codex_home.join(CONFIG_OVERLAY_FILE);
    let mut overlay = read_document(&overlay_file)?;
    let original_overlay = overlay.to_string();
    // Validate before inserting so malformed ancestor tables cannot be replaced.
    validate_config_file(
        &overlay_file,
        &toml::from_str(&original_overlay).map_err(io::Error::other)?,
    )?;
    for path in FORK_CONFIG_PATHS {
        // toml_edit's mutable indexing can create missing ancestor tables.
        // Only traverse mutably after confirming this property exists.
        if path
            .iter()
            .try_fold(shared.as_item(), |item, key| item.get(*key))
            .is_none()
        {
            continue;
        }
        let Some((leaf, parents)) = path.split_last() else {
            continue;
        };
        let Some(source) = parents
            .iter()
            .try_fold(shared.as_item_mut(), |item, key| item.get_mut(*key))
            .and_then(Item::as_table_like_mut)
        else {
            continue;
        };
        let key = source.key(leaf).cloned();
        let Some(value) = source.remove(leaf) else {
            continue;
        };
        let mut destination = overlay.as_item_mut();
        for parent in parents {
            destination = &mut destination[parent];
            if destination.is_none() {
                let mut table = Table::new();
                table.set_implicit(/*implicit*/ true);
                *destination = Item::Table(table);
            }
        }
        if destination.get(leaf).is_none()
            && let Some(table) = destination.as_table_like_mut()
            && let Some(key) = key
        {
            table.entry_format(&key).or_insert(value);
        }
    }
    let shared_contents = shared.to_string();
    let overlay_contents = overlay.to_string();
    for (path, contents) in [
        (&shared_file, &shared_contents),
        (&overlay_file, &overlay_contents),
    ] {
        let value = toml::from_str(contents).map_err(io::Error::other)?;
        validate_config_file(path, &value)?;
    }

    let shared_paths = resolve_symlink_write_paths(&shared_file)?;
    let overlay_paths = resolve_symlink_write_paths(&overlay_file)?;
    if let (Ok(shared_target), Ok(overlay_target)) = (
        fs::canonicalize(&shared_file),
        fs::canonicalize(&overlay_file),
    ) && shared_target == overlay_target
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{CONFIG_TOML_FILE} and {CONFIG_OVERLAY_FILE} must refer to separate files"),
        ));
    }
    // Persist the overlay first. If removing the originals fails or the process
    // exits between writes, retrying keeps the already-saved overlay values.
    if overlay_contents != original_overlay {
        write_atomically(&overlay_paths.write_path, &overlay_contents).map_err(|error| {
            io::Error::new(error.kind(), format!("{}: {error}", overlay_file.display()))
        })?;
    }
    write_atomically(&shared_paths.write_path, &shared_contents).map_err(|error| {
        io::Error::new(error.kind(), format!("{}: {error}", shared_file.display()))
    })
}

fn read_document(path: &Path) -> io::Result<DocumentMut> {
    match fs::read_to_string(path) {
        Ok(contents) => contents.parse().map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: {error}", path.display()),
            )
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(DocumentMut::new()),
        Err(error) => Err(io::Error::new(
            error.kind(),
            format!("{}: {error}", path.display()),
        )),
    }
}

fn has_fork_settings(doc: &DocumentMut) -> bool {
    FORK_CONFIG_PATHS.iter().any(|path| {
        path.iter()
            .try_fold(doc.as_item(), |item, key| item.get(*key))
            .is_some()
    })
}

#[cfg(test)]
#[path = "fork_config_migration_tests.rs"]
mod tests;
