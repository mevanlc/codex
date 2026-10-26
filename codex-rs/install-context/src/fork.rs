//! Installation and update contracts for the maintained fork.
use std::path::Path;

use serde::Deserialize;

pub const RELEASES_URL: &str = "https://api.github.com/repos/mevanlc/codex/releases";
pub const VERSION_CACHE: &str = "mevanlc-version.json";

#[derive(Clone, Debug, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub published_at: Option<String>,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    #[serde(default)]
    pub browser_download_url: Option<String>,
    /// GitHub's digest of the asset, such as `sha256:<hex>`.
    #[serde(default)]
    pub digest: Option<String>,
}

/// Name of the installer binary and the prefix of its per-target release assets.
pub const INSTALLER_NAME: &str = "codex-fork-installer";

/// The release target this build installs and updates. Linux releases are
/// static musl builds, so local glibc builds update to them as well.
pub fn release_target() -> Option<&'static str> {
    Some(match (std::env::consts::OS, std::env::consts::ARCH) {
        ("android", "aarch64") => "aarch64-linux-android",
        ("linux", "aarch64") => "aarch64-unknown-linux-musl",
        ("linux", "x86_64") => "x86_64-unknown-linux-musl",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        _ => return None,
    })
}

pub fn release_asset_name() -> Option<String> {
    release_target().map(|target| {
        format!(
            "codex-{target}{}",
            if cfg!(windows) { ".zip" } else { ".tar.zst" }
        )
    })
}

/// The installer is versioned by this name: an incompatible change to its
/// command line or `CODEX_INSTALL_*` contract must publish under a new name.
pub fn installer_asset_name() -> Option<String> {
    release_target()
        .map(|target| format!("{INSTALLER_NAME}-{target}{}", std::env::consts::EXE_SUFFIX))
}

/// Partial platform releases, drafts and prereleases cannot become the default.
/// Commit hashes identify snapshots; they never determine chronological order.
pub fn newest_release<'a>(releases: &'a [Release], asset: &str) -> Option<&'a Release> {
    releases
        .iter()
        .filter(|r| {
            !r.draft
                && !r.prerelease
                && r.published_at.is_some()
                && r.tag_name
                    .strip_prefix("fork-v")
                    .is_some_and(|version| semver::Version::parse(version).is_ok())
                && r.assets.iter().any(|a| a.name == asset)
        })
        .max_by_key(|r| r.published_at.as_deref())
}

pub fn installed_publication_epoch(package_dir: &Path) -> Option<i64> {
    let receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(package_dir.join("install-receipt.json")).ok()?)
            .ok()?;
    (receipt["repository"] == "mevanlc/codex")
        .then(|| receipt["publishedEpoch"].as_i64())
        .flatten()
}

/// The release tag a fork installation receipt records, if it came from a release.
pub fn installed_release_tag(package_dir: &Path) -> Option<String> {
    let receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(package_dir.join("install-receipt.json")).ok()?)
            .ok()?;
    (receipt["repository"] == "mevanlc/codex")
        .then(|| receipt["tag"].as_str().map(str::to_string))
        .flatten()
}

pub fn release_is_newer(
    latest: &str,
    published: Option<i64>,
    current: &str,
    installed: Option<i64>,
) -> bool {
    if latest == current {
        return false;
    }
    if let (Some(published), Some(installed)) = (published, installed) {
        return published > installed;
    }
    let (Ok(latest), Ok(current)) = (
        semver::Version::parse(latest),
        semver::Version::parse(current),
    ) else {
        return false;
    };
    // With no installation receipt, only order different numeric release lines.
    (latest.major, latest.minor, latest.patch) > (current.major, current.minor, current.patch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots_use_publication_order() {
        assert!(release_is_newer(
            "1.2.3-aaaaaaa",
            Some(20),
            "1.2.3-fffffff",
            Some(10)
        ));
        assert!(!release_is_newer(
            "1.2.3-fffffff",
            Some(10),
            "1.2.3-aaaaaaa",
            Some(20)
        ));
        assert!(!release_is_newer(
            "1.2.3-fffffff",
            None,
            "1.2.3-aaaaaaa",
            None
        ));
        assert!(!release_is_newer(
            "1.2.3-aaaaaaa",
            Some(20),
            "1.2.3-aaaaaaa",
            Some(10)
        ));
    }

    #[test]
    fn selection_requires_a_published_platform_asset() {
        let releases: Vec<Release> = serde_json::from_value(serde_json::json!([
            {"tag_name":"fork-v1.0.0-fffffff", "published_at":"2026-10-01T00:00:00Z", "assets":[{"name":"android"}]},
            {"tag_name":"fork-v1.0.0-aaaaaaa", "published_at":"2026-10-02T00:00:00Z", "assets":[{"name":"android"}]},
            {"tag_name":"fork-v1.0.1", "published_at":"2026-10-03T00:00:00Z", "assets":[{"name":"desktop"}]},
            {"tag_name":"fork-v1.0.2", "published_at":"2026-10-04T00:00:00Z", "draft":true, "assets":[{"name":"android"}]},
            {"tag_name":"fork-v1.0.3", "published_at":"2026-10-05T00:00:00Z", "prerelease":true, "assets":[{"name":"android"}]}
        ])).unwrap();
        assert_eq!(
            newest_release(&releases, "android").unwrap().tag_name,
            "fork-v1.0.0-aaaaaaa"
        );
    }
}
