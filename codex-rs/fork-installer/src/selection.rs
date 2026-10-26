//! Reads and moves the `current` selection: a symlink on Unix, and on Windows
//! a directory junction, which needs no symlink privilege.

use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;

use crate::fsutil::lexists;
use crate::fsutil::resolve_lenient;

/// The selected release directory, if any. Refuses selections this installer
/// did not create, so it never retargets something it does not own.
pub(crate) fn selected(root: &Path) -> Result<Option<PathBuf>> {
    let current = root.join("current");
    if !lexists(&current)? {
        return Ok(None);
    }
    let release = match std::fs::canonicalize(&current) {
        Ok(release) => release,
        // A dangling selection is still ours to repair.
        Err(_) => {
            let target = std::fs::read_link(&current).with_context(|| {
                format!(
                    "current must be an installer-owned link: {}",
                    current.display()
                )
            })?;
            resolve_lenient(&root.join(target))?
        }
    };
    anyhow::ensure!(
        release.parent() == Some(resolve_lenient(&root.join("releases"))?.as_path()),
        "refusing unmanaged current selection: {}",
        current.display()
    );
    #[cfg(windows)]
    anyhow::ensure!(
        windows::is_junction(&current)?,
        "current must be an installer-owned directory junction"
    );
    #[cfg(unix)]
    anyhow::ensure!(
        std::fs::symlink_metadata(&current)?
            .file_type()
            .is_symlink(),
        "current must be an installer-owned symlink"
    );
    Ok(Some(release))
}

/// Point `current` at `release`, replacing any previous selection atomically.
pub(crate) fn set_current(root: &Path, release: &Path) -> Result<()> {
    let current = root.join("current");
    #[cfg(unix)]
    {
        let temporary = root.join(format!(".current-{}", crate::fsutil::random_hex(32)));
        std::os::unix::fs::symlink(release, &temporary)?;
        if let Err(error) = std::fs::rename(&temporary, &current) {
            let _ = std::fs::remove_file(&temporary);
            return Err(error).context("failed to select release");
        }
        Ok(())
    }
    #[cfg(windows)]
    {
        let release = release.canonicalize()?;
        if lexists(&current)? {
            // Retarget in place so concurrent readers never see current missing.
            return windows::retarget_junction(&current, &release);
        }
        let temporary = tempfile::Builder::new()
            .prefix(".current-")
            .tempdir_in(root)?;
        let junction = temporary.path().join("current");
        std::fs::create_dir(&junction)?;
        windows::retarget_junction(&junction, &release)?;
        std::fs::rename(&junction, &current).context("failed to select release")?;
        Ok(())
    }
}

/// Remove the selection link itself, never its target.
pub(crate) fn remove_current(root: &Path) -> std::io::Result<()> {
    let current = root.join("current");
    if !lexists(&current)? {
        return Ok(());
    }
    if cfg!(windows) {
        std::fs::remove_dir(current)
    } else {
        std::fs::remove_file(current)
    }
}

#[cfg(windows)]
mod windows {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use std::path::Path;

    use anyhow::Context;
    use anyhow::Result;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_TAG_INFO;
    use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;
    use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
    use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_DELETE;
    use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
    use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE;
    use windows_sys::Win32::Storage::FileSystem::FileAttributeTagInfo;
    use windows_sys::Win32::Storage::FileSystem::GetFileInformationByHandleEx;
    use windows_sys::Win32::System::IO::DeviceIoControl;

    const IO_REPARSE_TAG_MOUNT_POINT: u32 = 0xa000_0003;

    fn open_link(path: &Path, access: u32) -> std::io::Result<std::fs::File> {
        std::fs::OpenOptions::new()
            .access_mode(access)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
    }

    pub(super) fn is_junction(path: &Path) -> Result<bool> {
        let handle = open_link(path, 0)?;
        let mut info = FILE_ATTRIBUTE_TAG_INFO {
            FileAttributes: 0,
            ReparseTag: 0,
        };
        if unsafe {
            GetFileInformationByHandleEx(
                handle.as_raw_handle(),
                FileAttributeTagInfo,
                (&raw mut info).cast(),
                std::mem::size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error())
                .with_context(|| format!("failed to inspect {}", path.display()));
        }
        Ok(info.ReparseTag == IO_REPARSE_TAG_MOUNT_POINT)
    }

    /// Set the mount-point reparse data of an existing junction or empty directory.
    pub(super) fn retarget_junction(current: &Path, release: &Path) -> Result<()> {
        const FSCTL_SET_REPARSE_POINT: u32 = 0x0009_00a4;
        const GENERIC_WRITE: u32 = 0x4000_0000;
        let release: Vec<u16> = release.as_os_str().encode_wide().collect();
        let prefix: Vec<u16> = r"\\?\".encode_utf16().collect();
        let release = release.strip_prefix(prefix.as_slice()).unwrap_or(&release);
        let substitute: Vec<u8> = r"\??\"
            .encode_utf16()
            .chain(release.iter().copied())
            .flat_map(u16::to_le_bytes)
            .collect();
        let length = u16::try_from(substitute.len())
            .ok()
            .filter(|length| usize::from(*length) + 20 <= usize::from(u16::MAX))
            .context("release path is too long for a junction")?;
        // REPARSE_DATA_BUFFER: an 8-byte header, then u16 offsets/lengths for the
        // substitute and print names. Both names end with NUL; the print name is empty.
        let mut data = vec![0; substitute.len() + 20];
        data[0..4].copy_from_slice(&IO_REPARSE_TAG_MOUNT_POINT.to_le_bytes());
        data[4..6].copy_from_slice(&(length + 12).to_le_bytes());
        data[10..12].copy_from_slice(&length.to_le_bytes());
        data[12..14].copy_from_slice(&(length + 2).to_le_bytes());
        data[16..16 + substitute.len()].copy_from_slice(&substitute);
        let handle = open_link(current, GENERIC_WRITE)?;
        let mut returned = 0;
        if unsafe {
            DeviceIoControl(
                handle.as_raw_handle(),
                FSCTL_SET_REPARSE_POINT,
                data.as_ptr().cast(),
                data.len() as u32,
                std::ptr::null_mut(),
                0,
                &mut returned,
                std::ptr::null_mut(),
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error())
                .with_context(|| format!("failed to point {} at the release", current.display()));
        }
        Ok(())
    }
}
