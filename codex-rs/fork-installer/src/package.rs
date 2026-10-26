//! Extracts and validates complete Codex package archives.

use std::fs::File;
use std::fs::OpenOptions;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use serde_json::json;

use crate::cancel::Cancel;
use crate::layout::Target;
use crate::probe::Probe;

pub(crate) fn extract(archive: &Path, destination: &Path, cancel: &Cancel) -> Result<()> {
    let name = archive
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();
    if name.ends_with(".zip") {
        extract_zip(archive, destination, cancel)
    } else if name.ends_with(".tar.zst") {
        extract_tar_zst(archive, destination, cancel)
    } else {
        anyhow::bail!(
            "expected a complete .tar.zst or .zip package; loose binaries are no longer supported"
        )
    }
}

fn extract_zip(archive: &Path, destination: &Path, cancel: &Cancel) -> Result<()> {
    let mut package = zip::ZipArchive::new(File::open(archive)?)
        .with_context(|| format!("failed to read {}", archive.display()))?;
    for index in 0..package.len() {
        cancel.check()?;
        let mut member = package.by_index(index)?;
        let name = member.name()?.into_owned();
        let path = member_path(destination, &name)?;
        let mode = member.unix_mode();
        if member.is_symlink() || mode.is_some_and(|mode| mode & 0o170000 == 0o120000) {
            anyhow::bail!("package links are not supported");
        }
        if member.is_dir() {
            std::fs::create_dir_all(&path)?;
        } else {
            write_member(
                &mut member,
                &path,
                mode.map(|mode| mode & 0o777)
                    .filter(|mode| *mode != 0)
                    .unwrap_or(0o644),
            )?;
        }
    }
    Ok(())
}

fn extract_tar_zst(archive: &Path, destination: &Path, cancel: &Cancel) -> Result<()> {
    let decoder = zstd::stream::read::Decoder::new(File::open(archive)?)?;
    let mut package = tar::Archive::new(decoder);
    for member in package.entries().context("package decompression failed")? {
        cancel.check()?;
        let mut member = member.context("package decompression failed")?;
        let name = String::from_utf8(member.path_bytes().into_owned())
            .context("package member name is not UTF-8")?;
        let path = member_path(destination, &name)?;
        let kind = member.header().entry_type();
        if kind.is_dir() {
            std::fs::create_dir_all(&path)?;
        } else if kind.is_file() {
            let mode = member.header().mode()? & 0o777;
            write_member(&mut member, &path, mode)?;
        } else {
            anyhow::bail!("unsupported package member: {name}");
        }
    }
    Ok(())
}

/// Map an archive member onto the destination, refusing anything that could escape it.
fn member_path(destination: &Path, name: &str) -> Result<PathBuf> {
    let parts: Vec<&str> = name
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect();
    anyhow::ensure!(
        !name.contains('\\')
            && !name.contains(':')
            && !name.starts_with('/')
            && !parts.contains(&".."),
        "unsafe package member: {name}"
    );
    Ok(parts
        .iter()
        .fold(destination.to_path_buf(), |path, part| path.join(part)))
}

fn write_member(source: &mut impl Read, path: &Path, mode: u32) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut out = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("failed to create {}", path.display()))?;
    std::io::copy(source, &mut out).context("package decompression failed")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
    }
    #[cfg(windows)]
    let _ = mode;
    Ok(())
}

/// Check the package manifest and required executables, optionally running
/// the entrypoint and `rg` to prove they work here. Returns the package version.
pub(crate) fn validate(
    target: &Target,
    package: &Path,
    expected_version: Option<&str>,
    probe: Option<&dyn Probe>,
) -> Result<String> {
    let manifest_path = package.join("codex-package.json");
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&manifest_path)
            .with_context(|| format!("failed to read {}", manifest_path.display()))?,
    )
    .context("invalid codex-package.json")?;
    let entrypoint = format!("bin/{}", target.exe("codex"));
    for (key, expected) in [
        ("layoutVersion", json!(1)),
        ("variant", json!("codex")),
        ("target", json!(target.triple())),
        ("entrypoint", json!(entrypoint)),
        ("pathDir", json!("codex-path")),
        ("resourcesDir", json!("codex-resources")),
    ] {
        anyhow::ensure!(
            manifest[key] == expected,
            "invalid package {key}: expected {expected}, got {}",
            manifest[key]
        );
    }
    let version = manifest["version"].as_str().unwrap_or_default();
    anyhow::ensure!(
        is_package_version(version) && expected_version.is_none_or(|expected| expected == version),
        "invalid or mismatched package version: {:?}",
        manifest["version"]
    );
    for directory in ["bin", "codex-path", "codex-resources"] {
        anyhow::ensure!(
            package.join(directory).is_dir(),
            "package is missing directory {directory}"
        );
    }
    let names = target.required_executables();
    for name in &names {
        let metadata = std::fs::symlink_metadata(package.join(name));
        anyhow::ensure!(
            metadata
                .as_ref()
                .is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0),
            "package is missing required executable {name}"
        );
        #[cfg(unix)]
        if let Ok(metadata) = &metadata {
            use std::os::unix::fs::PermissionsExt;
            anyhow::ensure!(
                target.windows() || metadata.permissions().mode() & 0o111 != 0,
                "package file {name} is not executable"
            );
        }
    }
    if let Some(probe) = probe {
        let output = probe
            .version(&package.join(&names[0]))
            .context("packaged Codex failed --version")?;
        anyhow::ensure!(
            output.split_whitespace().last() == Some(version),
            "package version {version} does not match executable: {output}"
        );
        probe
            .version(&package.join(&names[2]))
            .context("packaged rg failed --version")?;
    }
    Ok(version.to_string())
}

/// `MAJOR.MINOR.PATCH[-PRE][+BUILD]` using only `[0-9A-Za-z.-]` after the core.
/// Looser than SemVer on purpose: a fork snapshot suffix is a commit hash,
/// which may be all digits with a leading zero.
pub(crate) fn is_package_version(version: &str) -> bool {
    let tail_ok = |tail: &str| {
        !tail.is_empty()
            && tail
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    };
    let (rest, build) = match version.split_once('+') {
        Some((rest, build)) => (rest, Some(build)),
        None => (version, None),
    };
    let (core, pre) = match rest.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (rest, None),
    };
    let numbers: Vec<&str> = core.split('.').collect();
    numbers.len() == 3
        && numbers
            .iter()
            .all(|number| !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()))
        && pre.is_none_or(tail_ok)
        && build.is_none_or(tail_ok)
}

#[cfg(test)]
#[path = "package_tests.rs"]
mod tests;
