//! Where packages, selections, and commands live for one installation.

use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;

pub(crate) const REPO: &str = "mevanlc/codex";
/// Commands published into `~/.local/bin` for the CLI package.
pub(crate) const COMMANDS: [&str; 2] = ["codex", "codex-code-mode-host"];

/// A Rust target triple with the package conventions that depend on it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Target(String);

impl Target {
    pub(crate) fn new(triple: impl Into<String>) -> Self {
        Self(triple.into())
    }

    pub(crate) fn triple(&self) -> &str {
        &self.0
    }

    pub(crate) fn windows(&self) -> bool {
        self.0.ends_with("-pc-windows-msvc")
    }

    pub(crate) fn suffix(&self) -> &'static str {
        if self.windows() { ".exe" } else { "" }
    }

    pub(crate) fn exe(&self, name: &str) -> String {
        format!("{name}{}", self.suffix())
    }

    /// Executables every complete package must carry; the first is the entrypoint.
    pub(crate) fn required_executables(&self) -> Vec<String> {
        let mut names = vec![
            format!("bin/{}", self.exe("codex")),
            format!("bin/{}", self.exe("codex-code-mode-host")),
            format!("codex-path/{}", self.exe("rg")),
        ];
        if self.windows() {
            names.push("codex-resources/codex-command-runner.exe".to_string());
            names.push("codex-resources/codex-windows-sandbox-setup.exe".to_string());
        } else if self.0.contains("-unknown-linux-") {
            names.push("codex-resources/bwrap".to_string());
        }
        names
    }

    pub(crate) fn default_asset(&self) -> String {
        format!(
            "codex-{}{}",
            self.0,
            if self.windows() { ".zip" } else { ".tar.zst" }
        )
    }
}

/// Paths for the CLI or daemon-owned package root and the command directory.
#[derive(Clone, Debug)]
pub(crate) struct Layout {
    pub(crate) target: Target,
    /// Actions artifact name, `codex-<target>`.
    pub(crate) artifact: String,
    /// Release asset (and artifact member) holding the package archive.
    pub(crate) asset: String,
    pub(crate) install_root: PathBuf,
    pub(crate) root: PathBuf,
    pub(crate) bin: PathBuf,
    pub(crate) daemon_only: bool,
}

impl Layout {
    pub(crate) fn new(
        user_home: &Path,
        artifact: Option<&str>,
        asset: Option<&str>,
        daemon_only: bool,
    ) -> Result<Self> {
        let artifact = match artifact.filter(|name| !name.is_empty()) {
            Some(artifact) => artifact.to_string(),
            None => format!(
                "codex-{}",
                codex_install_context::fork::release_target().ok_or_else(|| anyhow::anyhow!(
                    "unsupported platform; set CODEX_ARTIFACT_NAME"
                ))?
            ),
        };
        let triple = artifact.strip_prefix("codex-").unwrap_or(&artifact);
        anyhow::ensure!(
            !triple.is_empty(),
            "unsupported platform; set CODEX_ARTIFACT_NAME"
        );
        let target = Target::new(triple);
        anyhow::ensure!(
            target.windows() == cfg!(windows),
            "{triple} packages cannot be installed on this platform"
        );
        let install_root = codex_utils_home_dir::fork_install_root(user_home);
        let root = install_root.join("packages").join(if daemon_only {
            "app-server-daemon"
        } else {
            "cli"
        });
        Ok(Self {
            asset: asset
                .filter(|name| !name.is_empty())
                .map_or_else(|| target.default_asset(), str::to_string),
            target,
            artifact,
            install_root,
            root,
            bin: user_home.join(".local").join("bin"),
            daemon_only,
        })
    }

    pub(crate) fn current(&self) -> PathBuf {
        self.root.join("current")
    }

    pub(crate) fn releases(&self) -> PathBuf {
        self.root.join("releases")
    }

    pub(crate) fn marker(&self) -> PathBuf {
        self.root.join("auto-update-version")
    }

    /// Launchers for one command: path and exact contents (or link target on Unix).
    pub(crate) fn launchers(&self, command: &str) -> Vec<(PathBuf, String)> {
        let executable = self.current().join("bin").join(self.target.exe(command));
        if !cfg!(windows) {
            return vec![(
                self.bin.join(command),
                executable.to_string_lossy().into_owned(),
            )];
        }
        let posix = executable.to_string_lossy().replace('\\', "/");
        vec![
            (
                self.bin.join(command),
                format!("#!/usr/bin/env bash\nexec {} \"$@\"\n", shell_quote(&posix)),
            ),
            (
                self.bin.join(format!("{command}.cmd")),
                format!(
                    "@echo off\r\nsetlocal DisableDelayedExpansion\r\n\"%~dp0..\\share\\mevanlc-codex\\packages\\cli\\current\\bin\\{command}.exe\" %*\r\n"
                ),
            ),
        ]
    }

    /// Command files that would shadow the Windows launchers in cmd or PowerShell.
    pub(crate) fn stale_commands(&self, command: &str) -> Vec<PathBuf> {
        if !cfg!(windows) {
            return Vec::new();
        }
        [".exe", ".ps1", ".bat"]
            .iter()
            .map(|extension| self.bin.join(format!("{command}{extension}")))
            .collect()
    }
}

/// Quote for POSIX shells exactly as Python's `shlex.quote` does.
pub(crate) fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn shell_quote_matches_python() {
        assert_eq!(shell_quote(""), "''");
        assert_eq!(
            shell_quote("C:/Users/me/bin/codex.exe"),
            "C:/Users/me/bin/codex.exe"
        );
        assert_eq!(shell_quote("C:/h & 1%/x"), "'C:/h & 1%/x'");
        assert_eq!(shell_quote("it's é"), "'it'\"'\"'s é'");
    }

    #[test]
    fn package_requirements_follow_the_target() {
        let linux = Target::new("x86_64-unknown-linux-musl").required_executables();
        assert!(linux.contains(&"codex-resources/bwrap".to_string()));
        let mac = Target::new("aarch64-apple-darwin").required_executables();
        assert_eq!(
            mac,
            ["bin/codex", "bin/codex-code-mode-host", "codex-path/rg"]
        );
        let windows = Target::new("x86_64-pc-windows-msvc").required_executables();
        assert_eq!(windows[0], "bin/codex.exe");
        assert!(windows.contains(&"codex-resources/codex-windows-sandbox-setup.exe".to_string()));
        assert_eq!(
            Target::new("aarch64-linux-android").default_asset(),
            "codex-aarch64-linux-android.tar.zst"
        );
    }
}
