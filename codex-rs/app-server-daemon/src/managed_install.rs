//! Resolves both package and legacy standalone layouts and compares installed executables.

use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use serde::Deserialize;
use serde::Serialize;
use tokio::fs;
use tokio::process::Command;
use tokio::time::timeout;

/// Fork packages are independent of CODEX_HOME and of upstream installations.
#[cfg(test)]
pub(crate) fn package_root(user_home: &Path) -> PathBuf {
    codex_utils_home_dir::fork_install_root(user_home).join("packages/app-server-daemon")
}

pub(crate) fn managed_codex_bin(root: &Path) -> PathBuf {
    root.join("current/bin").join(managed_codex_file_name())
}

/// Eligibility follows the installer's latest-channel selection, including fork
/// snapshots. Explicit pins and packages copied with --from-cli stay pinned.
pub(crate) fn is_latest_release(root: &Path, codex_bin: &Path) -> bool {
    let Ok(releases) = std::fs::canonicalize(root.join("releases")) else {
        return false;
    };
    let Ok(release) = std::fs::canonicalize(root.join("current")) else {
        return false;
    };
    release.parent() == Some(releases.as_path())
        && release.join("codex-package.json").is_file()
        && std::fs::read_to_string(root.join("auto-update-version")).is_ok_and(|selected| {
            Some(selected.as_str()) == release.file_name().and_then(|name| name.to_str())
        })
        && std::fs::canonicalize(codex_bin).is_ok_and(|bin| bin.starts_with(&release))
}

/// Older managed binaries can serve app-server requests without owning an updater.
pub(crate) async fn supports_daemon_update_loop(codex_bin: &Path) -> bool {
    supports_daemon_command(codex_bin, &["pid-update-loop", "--help"]).await
}

/// Probe an internal daemon command without running a long-lived process.
pub(crate) async fn supports_daemon_command(codex_bin: &Path, args: &[&str]) -> bool {
    let mut command = Command::new(codex_bin);
    #[cfg(windows)]
    command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    timeout(
        Duration::from_secs(5),
        command
            .args(["app-server", "daemon"])
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .status(),
    )
    .await
    .is_ok_and(|result| result.is_ok_and(|status| status.success()))
}

pub(crate) async fn resolved_managed_codex_bin(codex_bin: &Path) -> Result<PathBuf> {
    fs::canonicalize(codex_bin).await.with_context(|| {
        format!(
            "failed to resolve managed Codex binary {}",
            codex_bin.display()
        )
    })
}

pub(crate) async fn managed_codex_version(codex_bin: &Path) -> Result<String> {
    let mut command = Command::new(codex_bin);
    #[cfg(windows)]
    command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    let output = command
        .arg("--version")
        .kill_on_drop(true)
        .output()
        .await
        .with_context(|| {
            format!(
                "failed to invoke managed Codex binary {}",
                codex_bin.display()
            )
        })?;
    if !output.status.success() {
        return Err(anyhow!(
            "managed Codex binary {} exited with status {}",
            codex_bin.display(),
            output.status
        ));
    }

    let stdout = String::from_utf8(output.stdout).with_context(|| {
        format!(
            "managed Codex version was not utf-8: {}",
            codex_bin.display()
        )
    })?;
    parse_codex_version(&stdout)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ExecutableIdentity {
    pub(crate) digest: [u8; 32],
    // Distinguish release generations whose executable bytes are identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) path_digest: Option<[u8; 32]>,
}

impl ExecutableIdentity {
    pub(crate) fn same_contents(&self, other: &Self) -> bool {
        self.digest == other.digest
    }
}

pub(crate) async fn executable_identity(executable: &Path) -> Result<ExecutableIdentity> {
    let executable = executable.to_path_buf();
    // Debug executables can be hundreds of MB. Stream the digest off the async
    // runtime instead of allocating the whole file and blocking a runtime thread.
    tokio::task::spawn_blocking(move || {
        let executable = std::fs::canonicalize(&executable).unwrap_or(executable);
        let mut identity = std::fs::File::open(&executable)
            .and_then(executable_identity_from_reader)
            .with_context(|| format!("failed to read executable {}", executable.display()))?;
        identity.path_digest = Some(path_digest(&executable));
        Ok(identity)
    })
    .await
    .context("executable identity task failed")?
}

pub(crate) fn executable_identity_from_reader(
    reader: impl std::io::Read,
) -> std::io::Result<ExecutableIdentity> {
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(reader)?;
    Ok(ExecutableIdentity {
        digest: *hasher.finalize().as_bytes(),
        path_digest: None,
    })
}

pub(crate) fn path_digest(path: &Path) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        hasher.update(path.as_os_str().as_bytes());
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        for unit in path.as_os_str().encode_wide() {
            hasher.update(&unit.to_le_bytes());
        }
    }
    *hasher.finalize().as_bytes()
}

fn managed_codex_file_name() -> &'static str {
    if cfg!(windows) { "codex.exe" } else { "codex" }
}

fn parse_codex_version(output: &str) -> Result<String> {
    let version = output
        .split_whitespace()
        .nth(1)
        .filter(|version| !version.is_empty())
        .ok_or_else(|| anyhow!("managed Codex version output was malformed"))?;
    Ok(version.to_string())
}

#[cfg(test)]
#[path = "managed_install_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "managed_install_path_tests.rs"]
mod path_tests;
