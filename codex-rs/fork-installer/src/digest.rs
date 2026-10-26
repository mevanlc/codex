//! Content digest of an installed package tree.
//!
//! Format 2: BLAKE3 over a domain tag, then for every regular file in
//! bytewise order of its `/`-separated UTF-8 relative path: the path length
//! (u64 LE), the path, its permission bits (u32 LE; always 0 on Windows), and
//! the BLAKE3 hash of its contents. The receipt itself is excluded. Releases
//! are compared only on the machine that installed them, so the Unix/Windows
//! difference in permission bits never matters.

use std::path::Path;

use anyhow::Context;
use anyhow::Result;

pub(crate) const DIGEST_FORMAT: u32 = 2;
pub(crate) const RECEIPT: &str = "install-receipt.json";

pub(crate) fn tree_digest(root: &Path) -> Result<String> {
    let mut files = Vec::new();
    collect(root, "", &mut files)?;
    files.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codex-package-tree-v2\0");
    for (relative, metadata) in files {
        let path = root.join(&relative);
        let mut contents = blake3::Hasher::new();
        contents
            .update_reader(
                std::fs::File::open(&path)
                    .with_context(|| format!("failed to open {}", path.display()))?,
            )
            .with_context(|| format!("failed to read {}", path.display()))?;
        hasher.update(&(relative.len() as u64).to_le_bytes());
        hasher.update(relative.as_bytes());
        hasher.update(&permission_bits(&metadata).to_le_bytes());
        hasher.update(contents.finalize().as_bytes());
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn collect(root: &Path, prefix: &str, files: &mut Vec<(String, std::fs::Metadata)>) -> Result<()> {
    let directory = root.join(prefix);
    for entry in std::fs::read_dir(&directory)
        .with_context(|| format!("failed to list {}", directory.display()))?
    {
        let entry = entry?;
        let name = entry.file_name();
        let name = name
            .to_str()
            .with_context(|| format!("package file name is not UTF-8: {name:?}"))?;
        let relative = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}/{name}")
        };
        let metadata = std::fs::symlink_metadata(entry.path())?;
        let file_type = metadata.file_type();
        if file_type.is_symlink() {
            // The daemon adds this alias beside the package layout on Unix.
            if cfg!(unix)
                && relative == "codex"
                && std::fs::read_link(entry.path())
                    .is_ok_and(|target| target == Path::new("bin/codex"))
            {
                continue;
            }
            anyhow::bail!("package contains a link: {relative}");
        } else if file_type.is_dir() {
            collect(root, &relative, files)?;
        } else if file_type.is_file() {
            if relative != RECEIPT {
                files.push((relative, metadata));
            }
        } else {
            anyhow::bail!("package contains an unsupported file: {relative}");
        }
    }
    Ok(())
}

#[cfg(unix)]
fn permission_bits(metadata: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o777
}

#[cfg(windows)]
fn permission_bits(_metadata: &std::fs::Metadata) -> u32 {
    0
}

#[cfg(test)]
#[path = "digest_tests.rs"]
mod tests;
