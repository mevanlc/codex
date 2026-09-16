//! Storage ownership for fork settings. Runtime configuration still uses the usual TOML shape.

use crate::config_toml::ConfigToml;
use std::io;
use std::path::Path;
use toml::Value;

pub const CONFIG_OVERLAY_FILE: &str = "config-overlay.toml";

pub(crate) const FORK_CONFIG_PATHS: &[&[&str]] = &[
    &["tui", "primary_accent"],
    &["tui", "chatbox_placeholder_tips"],
    &["tui", "file_mentions_preserve_at"],
    &["tui", "file_mentions_allow_explicit_paths"],
    &["tui", "keymap", "global", "open_external_editor_with_quote"],
];

/// Persistent destination of a configuration edit, independent of its value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigFileKind {
    Shared,
    ForkOverlay,
}

impl ConfigFileKind {
    pub fn for_file(path: &Path) -> Self {
        if path
            .file_name()
            .is_some_and(|name| name == CONFIG_OVERLAY_FILE)
        {
            Self::ForkOverlay
        } else {
            Self::Shared
        }
    }

    /// Classify an edit, including leaf properties inside table-valued edits.
    /// Clearing a parent table affects only that table in the shared file.
    pub fn for_edit(path: &[String], value: Option<&Value>) -> Result<Self, String> {
        if FORK_CONFIG_PATHS.iter().any(|fork| {
            path.iter()
                .take(fork.len())
                .map(String::as_str)
                .eq(fork.iter().copied())
        }) {
            return Ok(Self::ForkOverlay);
        }
        let mut destination = None;
        if let Some(Value::Table(table)) = value {
            for (key, value) in table {
                let mut child = path.to_vec();
                child.push(key.clone());
                let kind = Self::for_edit(&child, Some(value))?;
                if destination.is_some_and(|previous| previous != kind) {
                    return Err("Write shared and fork settings in separate requests".to_string());
                }
                destination = Some(kind);
            }
        }
        Ok(destination.unwrap_or(Self::Shared))
    }

    /// Validate a raw file without inserting default values into its contents.
    pub fn validate(self, config: &Value) -> Result<(), String> {
        match self {
            Self::Shared => {
                for path in FORK_CONFIG_PATHS {
                    if path
                        .iter()
                        .try_fold(config, |value, key| value.get(*key))
                        .is_some()
                    {
                        return Err(format!(
                            "Move fork setting `{}` to $CODEX_HOME/{CONFIG_OVERLAY_FILE}",
                            path.join(".")
                        ));
                    }
                }
                if let Some(profiles) = config.get("profiles").and_then(Value::as_table) {
                    for profile in profiles.values() {
                        self.validate(profile)?;
                    }
                }
                Ok(())
            }
            Self::ForkOverlay => validate_overlay_paths(config, &mut Vec::new()),
        }
    }
}

fn validate_overlay_paths(value: &Value, path: &mut Vec<String>) -> Result<(), String> {
    if FORK_CONFIG_PATHS
        .iter()
        .any(|allowed| path.iter().map(String::as_str).eq(allowed.iter().copied()))
    {
        let config = path.iter().rev().fold(value.clone(), |value, key| {
            Value::Table([(key.clone(), value)].into_iter().collect())
        });
        return config
            .try_into::<ConfigToml>()
            .map(|_| ())
            .map_err(|error| format!("Invalid `{}`: {error}", path.join(".")));
    }
    if !FORK_CONFIG_PATHS.iter().any(|allowed| {
        allowed
            .iter()
            .copied()
            .take(path.len())
            .eq(path.iter().map(String::as_str))
    }) {
        return Err(format!(
            "`{}` is not a fork setting; {CONFIG_OVERLAY_FILE} accepts only fork settings",
            path.join(".")
        ));
    }
    let table = value
        .as_table()
        .ok_or_else(|| format!("`{}` must be a table", path.join(".")))?;
    for (key, value) in table {
        path.push(key.clone());
        validate_overlay_paths(value, path)?;
        path.pop();
    }
    Ok(())
}

pub(crate) fn validate_config_file(path: &Path, config: &Value) -> io::Result<()> {
    ConfigFileKind::for_file(path)
        .validate(config)
        .map_err(|message| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: {message}", path.display()),
            )
        })
}

#[cfg(test)]
#[path = "fork_config_tests.rs"]
mod tests;
