use std::cmp::Ordering;

use semver::Prerelease;
use semver::Version;

/// Reports whether `latest` has higher update precedence than `current`.
///
/// This follows SemVer except that this fork's lone hexadecimal Git suffix is
/// treated as a snapshot after the matching release. Other prerelease suffixes
/// retain their standard lower precedence.
pub fn is_newer_version(latest: &str, current: &str) -> Option<bool> {
    let latest = parse_version(latest)?;
    let current = parse_version(current)?;
    Some(compare_version_precedence(&latest, &current).is_gt())
}

fn parse_version(version: &str) -> Option<Version> {
    Version::parse(version.trim().trim_start_matches('v')).ok()
}

fn compare_version_precedence(left: &Version, right: &Version) -> Ordering {
    let core_precedence =
        (left.major, left.minor, left.patch).cmp(&(right.major, right.minor, right.patch));
    if !core_precedence.is_eq() {
        return core_precedence;
    }

    fork_release_tier(&left.pre)
        .cmp(&fork_release_tier(&right.pre))
        .then_with(|| left.cmp_precedence(right))
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ForkReleaseTier {
    Prerelease,
    Release,
    GitSnapshot,
}

fn fork_release_tier(prerelease: &Prerelease) -> ForkReleaseTier {
    if prerelease.is_empty() {
        ForkReleaseTier::Release
    } else if is_abbreviated_git_hash(prerelease.as_str()) {
        ForkReleaseTier::GitSnapshot
    } else {
        ForkReleaseTier::Prerelease
    }
}

fn is_abbreviated_git_hash(value: &str) -> bool {
    (7..=40).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
#[path = "version_tests.rs"]
mod tests;
