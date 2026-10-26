#!/usr/bin/env bash
# Compatibility shim; install with codex-fork-installer instead (see README).
#
# Codex builds released before codex-fork-installer fetch this file from main
# and run it: `codex update` with no arguments, and the daemon updater with the
# CODEX_INSTALL_IF_LATEST, CODEX_INSTALL_IF_CURRENT, CODEX_UPDATE_FROM_RELEASE,
# and CODEX_INSTALL_DAEMON_ONLY guards in the environment. Hand both to the
# newest release's installer, which keeps the same contract. Those builds
# required an authenticated gh, so it is available to download the installer.
set -euo pipefail

repo=mevanlc/codex
kernel="$(uname -s)"
machine="$(uname -m)"
case "${MSYSTEM_CARCH:-}:$kernel:$machine" in
  aarch64:MINGW*:*|arm64:MINGW*:*|aarch64:MSYS*:*|arm64:MSYS*:*|*:MINGW*-ARM64:*|*:MSYS*-ARM64:*) target=aarch64-pc-windows-msvc ;;
  *:MINGW*:x86_64|*:MSYS*:x86_64|*:CYGWIN*:x86_64) target=x86_64-pc-windows-msvc ;;
  *:Darwin:arm64) target=aarch64-apple-darwin ;;
  *:Darwin:x86_64) target=x86_64-apple-darwin ;;
  *:Linux:aarch64)
    if [[ -n "${PREFIX:-}" ]]; then target=aarch64-linux-android; else target=aarch64-unknown-linux-musl; fi ;;
  *:Linux:x86_64) target=x86_64-unknown-linux-musl ;;
  *) target="" ;;
esac
case "${PROCESSOR_ARCHITEW6432:-${PROCESSOR_ARCHITECTURE:-}}:$kernel" in
  ARM64:MINGW*|ARM64:MSYS*|ARM64:CYGWIN*) target=aarch64-pc-windows-msvc ;;
esac
artifact="${CODEX_ARTIFACT_NAME:-codex-$target}"
target="${artifact#codex-}"
[[ -n "$target" ]] || { echo 'error: unsupported platform; set CODEX_ARTIFACT_NAME' >&2; exit 1; }
suffix=
case "$kernel" in
  MINGW*|MSYS*|CYGWIN*)
    suffix=.exe
    # The native installer resolves ~ from HOME, as Git Bash sees it.
    HOME="$(cygpath -w "$HOME")"
    export HOME
    ;;
esac
asset="codex-fork-installer-$target$suffix"
command -v gh >/dev/null || { echo "error: download $asset from https://github.com/$repo/releases and run it" >&2; exit 1; }
tag="$(gh api "repos/$repo/releases?per_page=100" --jq "[.[] | select((.draft or .prerelease) | not) | select(any(.assets[]; .name == \"$asset\"))] | max_by(.published_at) | .tag_name // empty")"
[[ -n "$tag" ]] || { echo "error: no published release of $repo carries $asset" >&2; exit 1; }
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
gh release download "$tag" -R "$repo" -p "$asset" -D "$tmp"
chmod +x "$tmp/$asset"
"$tmp/$asset" "$@"
