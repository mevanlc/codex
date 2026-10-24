//! The host-local rendezvous root for privileged app-server RPC sockets.
//!
//! Every listener uses this root, which sandboxes hide even before a daemon
//! starts. It must not depend on HOME, TMPDIR, CODEX_HOME, or command settings.

use std::fs;
use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

// Termux cannot write /tmp. Keep this independent of environment variables and
// short enough for a 64-character socket hash within sockaddr_un.sun_path.
#[cfg(any(test, target_os = "android"))]
const ANDROID_SOCKET_DIRECTORY: &str = "/data/data/com.termux/files/mc-daemon";

/// Returns the fixed executor-local directory that every sandbox must hide.
pub fn shared_daemon_socket_directory() -> io::Result<PathBuf> {
    #[cfg(target_os = "android")]
    {
        // Android gives each app a private data directory, so no UID suffix is
        // needed. prepare_shared_daemon_socket_directory still verifies ownership.
        Ok(PathBuf::from(ANDROID_SOCKET_DIRECTORY))
    }
    #[cfg(not(target_os = "android"))]
    {
        // Resolve the system alias /tmp -> /private/tmp on macOS.
        let temporary_root = fs::canonicalize("/tmp")?;
        let uid = unsafe { libc::geteuid() };
        Ok(temporary_root.join(format!("codex-daemon-{uid}")))
    }
}

/// Creates the reserved directory, rejecting symlinks and unsafe existing owners or modes.
pub fn prepare_shared_daemon_socket_directory() -> io::Result<PathBuf> {
    let directory = shared_daemon_socket_directory()?;
    match fs::DirBuilder::new().mode(0o700).create(&directory) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    let metadata = fs::symlink_metadata(&directory)?;
    if !metadata.is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o777 != 0o700
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "app-server socket directory must be a user-owned directory with mode 0700",
        ));
    }
    Ok(directory)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_daemon_socket_address_fits_the_platform_limit() {
        let socket = PathBuf::from(ANDROID_SOCKET_DIRECTORY).join("f".repeat(64));
        std::os::unix::net::SocketAddr::from_pathname(socket)
            .expect("Termux socket root and full SHA-256 name must fit sockaddr_un");
    }
}
