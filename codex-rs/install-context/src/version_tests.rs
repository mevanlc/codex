use pretty_assertions::assert_eq;

use super::*;

#[test]
fn git_snapshot_ranks_after_matching_release() {
    assert_eq!(is_newer_version("v0.147.0-a1b2c3d", "v0.147.0"), Some(true));
    assert_eq!(
        is_newer_version("v0.147.0", "v0.147.0-a1b2c3d"),
        Some(false)
    );
    assert_eq!(is_newer_version("v0.148.0", "v0.147.0-a1b2c3d"), Some(true));
}

#[test]
fn ordinary_prereleases_rank_before_matching_release() {
    for prerelease in [
        "v0.147.0-alpha.1",
        "v0.147.0-beta.2",
        "v0.147.0-pre.3",
        "v0.147.0-alpha.4-a1b2c3d",
        "v0.147.0-nightly-a1b2c3d",
    ] {
        assert_eq!(
            is_newer_version("v0.147.0", prerelease),
            Some(true),
            "{prerelease}"
        );
    }
}

#[test]
fn plain_semver_and_invalid_versions_are_handled() {
    assert_eq!(is_newer_version("1.2.4", "1.2.3"), Some(true));
    assert_eq!(is_newer_version("1.2.3", "1.2.4"), Some(false));
    assert_eq!(is_newer_version("not-a-version", "1.2.3"), None);
}
