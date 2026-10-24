//! Coordinates package selection with the fork installer using OS locks.

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use std::fs::OpenOptions;
use std::path::Path;
use std::time::Duration;
use tokio::time::sleep;

pub(crate) struct InstallLock {
    _file: tokio::fs::File,
}

pub(crate) async fn acquire_install_lock(root: &Path) -> Result<InstallLock> {
    let lock = root.join("install.lock");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    let mut options = OpenOptions::new();
    options.create(true).truncate(false).write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    loop {
        let acquired = options.open(&lock);
        #[cfg(unix)]
        let acquired = acquired.and_then(|file| {
            {
                use std::os::fd::AsRawFd;
                let result =
                    unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
                if result != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(file)
        });
        match acquired {
            Ok(file) => {
                return Ok(InstallLock {
                    _file: tokio::fs::File::from_std(file),
                });
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || cfg!(windows)
                        && (error.raw_os_error() == Some(32)
                            || error.kind() == std::io::ErrorKind::PermissionDenied) => {}
            Err(error) => return Err(error).context("failed to acquire daemon installer lock"),
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(anyhow!(
                "timed out waiting for daemon installer lock {}",
                lock.display()
            ));
        }
        sleep(Duration::from_millis(50)).await;
    }
}
