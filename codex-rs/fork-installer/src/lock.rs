//! The `install.lock` protocol shared with the daemon's package preparation:
//! an exclusive open on Windows, `flock` elsewhere. The file is never removed,
//! since unlinking it would let concurrent installers lock different inodes.

use std::fs::File;
use std::fs::OpenOptions;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;

use crate::cancel::Cancel;

pub(crate) struct InstallLock {
    _file: File,
}

pub(crate) fn acquire(root: &Path, timeout: Duration, cancel: &Cancel) -> Result<InstallLock> {
    std::fs::create_dir_all(root)
        .with_context(|| format!("failed to create {}", root.display()))?;
    let path = root.join("install.lock");
    let deadline = Instant::now() + timeout;
    let mut options = OpenOptions::new();
    options.create(true).truncate(false).write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    loop {
        cancel.check()?;
        let acquired = options.open(&path);
        #[cfg(unix)]
        let acquired = acquired.and_then(|file| {
            use std::os::fd::AsRawFd;
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(file)
        });
        match acquired {
            Ok(file) => return Ok(InstallLock { _file: file }),
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || cfg!(windows)
                        && (error.raw_os_error() == Some(32)
                            || error.kind() == std::io::ErrorKind::PermissionDenied) => {}
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to acquire installer lock {}", path.display())
                });
            }
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "timed out waiting for installer lock {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}
