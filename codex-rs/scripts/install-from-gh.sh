#!/usr/bin/env bash

set -euo pipefail

set -x

usage() {
  cat <<'EOF'
Usage: install-codex-from-gh.sh [--artifact|--release] [--force]
       install-codex-from-gh.sh [--release] --list-available
       install-codex-from-gh.sh [--release] --version <version>

Installs the current platform's Codex binaries from the latest GitHub artifact
or the latest GitHub release in mevanlc/codex. Supports both legacy releases
containing only codex and newer releases that also contain codex-code-mode-host.
All installed binaries are placed in $HOME/.local/bin.

Options:
  --artifact         Download the latest GitHub Actions artifact
  --release          Download the latest GitHub release (default)
  --force            Bypass mtime and version checks; always download and install
  --list-available   List releases that carry this platform's asset, then exit
  --version <ver>    Install a specific release instead of the latest one

--list-available and --version are release-mode only; neither can be combined
with --artifact.

--version accepts either a full release tag (fork-v0.147.0-alpha.7-9b2fff8) or
the bare version that `codex --version` prints (0.147.0-alpha.7-9b2fff8). Because
an explicit version is a deliberate pin, it skips the mtime currency check and
installs even when that means downgrading.
EOF
}

step() {
  printf '==> %s\n' "$1"
}

fail() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "$1 is required"
  fi
}

python_cmd() {
  if command -v python3 >/dev/null 2>&1; then
    printf 'python3\n'
    return
  fi

  if command -v python >/dev/null 2>&1; then
    printf 'python\n'
    return
  fi

  fail "python3 or python is required"
}

mode=""
force=false
list_available=false
version=""

while (($# > 0)); do
  case "$1" in
    --artifact)
      [[ -z "$mode" ]] || fail "choose exactly one of --artifact or --release"
      mode="artifact"
      ;;
    --release)
      [[ -z "$mode" ]] || fail "choose exactly one of --artifact or --release"
      mode="release"
      ;;
    --force)
      force=true
      ;;
    --list-available)
      list_available=true
      ;;
    --version)
      (($# >= 2)) || fail "--version requires a value"
      [[ -z "$version" ]] || fail "--version given more than once"
      version="$2"
      shift
      ;;
    --version=*)
      [[ -z "$version" ]] || fail "--version given more than once"
      version="${1#--version=}"
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      usage >&2
      fail "unknown argument: $1"
      ;;
  esac
  shift
done

# --list-available and --version only make sense against releases, which are the
# only source with stable, addressable tags. Artifacts are transient and expire.
if [[ "$mode" == "artifact" ]]; then
  if "$list_available"; then
    fail "--list-available cannot be combined with --artifact"
  fi
  if [[ -n "$version" ]]; then
    fail "--version cannot be combined with --artifact"
  fi
fi

if "$list_available" && [[ -n "$version" ]]; then
  fail "choose either --list-available or --version, not both"
fi

mode="${mode:-release}"

python_bin="$(python_cmd)"
tmp_root="${TMPDIR:-/tmp}"
tmp_dir=""

cleanup() {
  if [[ -n "$tmp_dir" && -d "$tmp_dir" ]]; then
    rm -rf "$tmp_dir"
  fi
}

trap cleanup EXIT INT TERM HUP

detect_artifact_name() {
  if [[ -n "${CODEX_ARTIFACT_NAME:-}" ]]; then
    printf '%s\n' "$CODEX_ARTIFACT_NAME"
    return
  fi

  local kernel_name=""
  local machine_name=""

  kernel_name="$(uname -s 2>/dev/null || true)"
  machine_name="$(uname -m 2>/dev/null || true)"

  # Git for Windows can itself be x86_64 while running on ARM64 Windows. In
  # that case `uname -m` describes Git Bash, not the native binaries Windows
  # can run. MSYSTEM_CARCH and the ARM64 suffix in `uname -s` describe the
  # host, so prefer them when they are available.
  case "${MSYSTEM_CARCH:-}:$kernel_name:$machine_name" in
    aarch64:*|arm64:*|*:MINGW*-ARM64:*|*:MSYS*-ARM64:*|*:CYGWIN*-ARM64:*)
      printf 'aarch64-pc-windows-msvc\n'
      return
      ;;
  esac

  case "$kernel_name:$machine_name" in
    MINGW*:x86_64|MSYS*:x86_64|CYGWIN*:x86_64)
      printf 'x86_64-pc-windows-msvc\n'
      return
      ;;
    Darwin:arm64)
      printf 'aarch64-apple-darwin\n'
      return
      ;;
    Darwin:x86_64)
      printf 'x86_64-apple-darwin\n'
      return
      ;;
    Linux:aarch64)
      if [[ -n "${PREFIX:-}" ]]; then
        printf 'codex-aarch64-linux-android\n'
      else
        printf 'aarch64-unknown-linux-musl\n'
      fi
      return
      ;;
    Linux:x86_64)
      printf 'x86_64-unknown-linux-musl\n'
      return
      ;;
  esac

  fail "set CODEX_ARTIFACT_NAME for this platform"
}

resolve_bin_path() {
  case "$artifact_name" in
    *-pc-windows-msvc)
      printf '%s\n' "$HOME/.local/bin/codex.exe"
      ;;
    *)
      printf '%s\n' "$HOME/.local/bin/codex"
      ;;
  esac
}

detect_release_asset_name() {
  if [[ -n "${CODEX_RELEASE_ASSET_NAME:-}" ]]; then
    printf '%s\n' "$CODEX_RELEASE_ASSET_NAME"
    return
  fi

  case "$1" in
    codex-*-apple-darwin|codex-*-unknown-linux-musl)
      printf '%s.tar.zst\n' "$1"
      ;;
    codex-*-pc-windows-msvc)
      printf '%s.zip\n' "$1"
      ;;
    *-apple-darwin|*-unknown-linux-musl)
      printf 'codex-%s.tar.zst\n' "$1"
      ;;
    *-pc-windows-msvc)
      printf 'codex-%s.zip\n' "$1"
      ;;
    *) printf '%s.zst\n' "$1" ;;
  esac
}

detect_code_mode_host_asset_name() {
  if [[ -n "${CODEX_CODE_MODE_HOST_RELEASE_ASSET_NAME:-}" ]]; then
    printf '%s\n' "$CODEX_CODE_MODE_HOST_RELEASE_ASSET_NAME"
    return
  fi

  case "$1" in
    codex-*) printf 'codex-code-mode-host-%s.zst\n' "${1#codex-}" ;;
    *) printf 'codex-code-mode-host-%s.zst\n' "$1" ;;
  esac
}

resolve_code_mode_host_bin_path() {
  case "$artifact_name" in
    *-pc-windows-msvc)
      printf '%s\n' "$HOME/.local/bin/codex-code-mode-host.exe"
      ;;
    *)
      printf '%s\n' "$HOME/.local/bin/codex-code-mode-host"
      ;;
  esac
}

iso_to_epoch() {
  "$python_bin" - "$1" <<'PY'
from datetime import datetime, timezone
import sys

created_at = datetime.strptime(sys.argv[1], "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)
print(int(created_at.timestamp()))
PY
}

file_mtime_epoch() {
  "$python_bin" - "$1" <<'PY'
from pathlib import Path
import sys

print(int(Path(sys.argv[1]).stat().st_mtime))
PY
}

list_available_releases() {
  local asset_name="$1"
  local host_asset_name="$2"
  local latest_tag=""

  # `releases/latest` is what a bare --release run installs, so mark it. It can
  # legitimately fail (e.g. every release is a prerelease), which is not fatal here.
  latest_tag="$(gh api "repos/$repo/releases/latest" --jq '.tag_name' 2>/dev/null || true)"

  # The releases payload is megabytes; Linux caps a single environment variable at
  # 128 KiB (MAX_ARG_STRLEN), so passing it via the environment fails on Android
  # with "Argument list too long". Hand it over as a file instead. Using tmp_dir
  # means the existing EXIT trap cleans it up.
  tmp_dir="$(mktemp -d "$tmp_root/codex-gh-list.XXXXXX")"
  local releases_file="$tmp_dir/releases.json"
  gh api "repos/$repo/releases?per_page=100" >"$releases_file"

  LATEST_TAG="$latest_tag" \
    "$python_bin" - "$asset_name" "$host_asset_name" "$releases_file" <<'PY'
import json
import os
import sys

asset_name, host_asset_name = sys.argv[1], sys.argv[2]
latest_tag = os.environ.get("LATEST_TAG", "")
with open(sys.argv[3], encoding="utf-8") as handle:
    releases = json.load(handle)

rows = []
for release in releases:
    if release.get("draft"):
        continue
    names = {asset.get("name") for asset in release.get("assets", [])}
    # Releases that carry no binary for this platform (e.g. the v8-v* prebuilts)
    # are not installable targets, so they are not listed.
    if asset_name not in names:
        continue
    layout = (
        "package"
        if asset_name.endswith((".tar.zst", ".zip"))
        else "dual" if host_asset_name in names else "single"
    )
    rows.append((
        release["tag_name"],
        release.get("published_at") or release.get("created_at") or "",
        layout,
        " (default)" if release["tag_name"] == latest_tag else "",
    ))

if not rows:
    print(f"no releases carry {asset_name}", file=sys.stderr)
    raise SystemExit(1)

# GitHub returns releases in its own order, which is not publish order, so sort
# newest-first: that is the end people pick from. ISO-8601 Z sorts chronologically.
rows.sort(key=lambda row: row[1], reverse=True)

width = max(len(row[0]) for row in rows)
print(f"{'TAG'.ljust(width)}  {'PUBLISHED'.ljust(20)}  LAYOUT")
for tag, published, layout, marker in rows:
    print(f"{tag.ljust(width)}  {published.ljust(20)}  {layout}{marker}")
PY
}

install_payload() {
  local payload_path="$1"
  local bin_path="$2"
  local binary_name="$3"
  local unpacked_binary="$tmp_dir/$binary_name"

  if zstd -q -t "$payload_path" >/dev/null 2>&1; then
    step "Unpacking $(basename "$payload_path")"
    zstd -d -f "$payload_path" -o "$unpacked_binary" >/dev/null
  else
    step "Using downloaded binary payload directly"
    cp "$payload_path" "$unpacked_binary"
  fi

  mkdir -p "$(dirname "$bin_path")"
  step "Installing updated $binary_name"
  install -m 700 "$unpacked_binary" "$bin_path"
}

extract_package_archive() {
  local archive_path="$1"
  local package_dir="$tmp_dir/package"

  case "$archive_path" in
    *.tar.zst)
      step "Unpacking $(basename "$archive_path")"
      mkdir -p "$package_dir"
      zstd -q -d -c "$archive_path" | tar -xf - -C "$package_dir"

      [[ -f "$package_dir/bin/codex" ]] ||
        fail "package archive did not contain bin/codex"
      main_payload="$package_dir/bin/codex"

      if [[ -f "$package_dir/bin/codex-code-mode-host" ]]; then
        code_mode_host_payload="$package_dir/bin/codex-code-mode-host"
      fi
      ;;
    *.zip)
      step "Unpacking $(basename "$archive_path")"
      mkdir -p "$package_dir"
      unzip -q "$archive_path" -d "$package_dir"

      [[ -f "$package_dir/bin/codex.exe" ]] ||
        fail "package archive did not contain bin/codex.exe"
      main_payload="$package_dir/bin/codex.exe"

      if [[ -f "$package_dir/bin/codex-code-mode-host.exe" ]]; then
        code_mode_host_payload="$package_dir/bin/codex-code-mode-host.exe"
      fi
      ;;
  esac
}

require_command gh
require_command install
require_command mktemp
require_command zstd
require_command cp
require_command tar
mkdir -p "$tmp_root"

repo="mevanlc/codex"
artifact_name="$(detect_artifact_name)"
release_asset_name="$(detect_release_asset_name "$artifact_name")"
code_mode_host_asset_name="$(detect_code_mode_host_asset_name "$artifact_name")"

case "$release_asset_name" in
  *.zip) require_command unzip ;;
esac

if "$list_available"; then
  list_available_releases "$release_asset_name" "$code_mode_host_asset_name"
  exit 0
fi

bin_path="$(resolve_bin_path)"
code_mode_host_bin_path="$(resolve_code_mode_host_bin_path)"

step "Repository: $repo"
step "Mode: $mode"
if [[ -n "$version" ]]; then
  step "Requested version: $version"
fi
step "Artifact name: $artifact_name"
step "Release asset: $release_asset_name"
step "Optional code-mode host asset: $code_mode_host_asset_name"
step "Target binary: $bin_path"
step "Target code-mode host: $code_mode_host_bin_path"

download_description=""
download_cmd=()
source_epoch=""

case "$mode" in
  artifact)
    artifact_json="$(gh api "repos/$repo/actions/artifacts")"
    artifact_record="$(
      ARTIFACT_JSON="$artifact_json" "$python_bin" - "$artifact_name" <<'PY'
import json
import os
import sys

artifact_name = sys.argv[1]
payload = json.loads(os.environ["ARTIFACT_JSON"])

for artifact in payload.get("artifacts", []):
    if artifact.get("name") == artifact_name and not artifact.get("expired", False):
        workflow_run = artifact.get("workflow_run") or {}
        print("\t".join([
            str(artifact["id"]),
            str(workflow_run["id"]),
            artifact["created_at"],
        ]))
        break
PY
    )"

    if [[ -z "$artifact_record" ]]; then
      fail "no non-expired artifact named $artifact_name found in $repo"
    fi

    IFS=$'\t' read -r artifact_id run_id artifact_created_at <<<"$artifact_record"
    source_epoch="$(iso_to_epoch "$artifact_created_at")"
    download_description="artifact $artifact_id from run $run_id"
    download_cmd=(gh run download "$run_id" -R "$repo" -n "$artifact_name")
    ;;
  release)
    if [[ -n "$version" ]]; then
      # Accept a full tag or the bare version `codex --version` prints. Fork
      # releases are tagged fork-v<version>; other series (android-a1a-*) must be
      # given as a full tag.
      release_json=""
      for candidate_tag in "$version" "fork-v$version" "v$version"; do
        if release_json="$(gh api "repos/$repo/releases/tags/$candidate_tag" 2>/dev/null)"; then
          step "Resolved version to tag $candidate_tag"
          break
        fi
        release_json=""
      done
      if [[ -z "$release_json" ]]; then
        fail "no release matching '$version' in $repo (use --list-available to see options)"
      fi
    else
      release_json="$(gh api "repos/$repo/releases/latest")"
    fi
    release_record="$(
      RELEASE_JSON="$release_json" "$python_bin" - \
        "$release_asset_name" "$code_mode_host_asset_name" <<'PY'
import json
import os
import sys

asset_name = sys.argv[1]
host_asset_name = sys.argv[2]
release = json.loads(os.environ["RELEASE_JSON"])
assets = {asset.get("name"): asset for asset in release.get("assets", [])}
asset = assets.get(asset_name)

if asset is not None:
    host_asset = assets.get(host_asset_name)
    timestamps = [asset["updated_at"]]
    if host_asset is not None:
        timestamps.append(host_asset["updated_at"])
    print("\t".join([
        release["tag_name"],
        max(timestamps),
        "dual" if host_asset is not None else "single",
    ]))
PY
    )"

    if [[ -z "$release_record" ]]; then
      fail "latest release in $repo does not include asset $release_asset_name"
    fi

    IFS=$'\t' read -r release_tag release_updated_at release_layout <<<"$release_record"
    source_epoch="$(iso_to_epoch "$release_updated_at")"
    download_description="release assets from tag $release_tag"
    download_cmd=(gh release download "$release_tag" -R "$repo" -p "$release_asset_name")
    if [[ "$release_layout" == "dual" ]]; then
      download_cmd+=(-p "$code_mode_host_asset_name")
    fi
    ;;
esac

tmp_dir="$(mktemp -d "$tmp_root/codex-gh-install.XXXXXX")"
download_dir="$tmp_dir/download"
mkdir -p "$download_dir"

step "Downloading $download_description"
"${download_cmd[@]}" -D "$download_dir"

main_payload=""
code_mode_host_payload=""
payload_files=()
while IFS= read -r -d '' payload_file; do
  payload_files+=("$payload_file")
  case "$(basename "$payload_file")" in
    "$release_asset_name"|codex)
      main_payload="$payload_file"
      ;;
    "$code_mode_host_asset_name"|codex-code-mode-host)
      code_mode_host_payload="$payload_file"
      ;;
  esac
done < <(find "$download_dir" -maxdepth 1 -type f -print0)

if [[ -z "$main_payload" && ${#payload_files[@]} -eq 1 ]]; then
  main_payload="${payload_files[0]}"
fi
if [[ -z "$main_payload" ]]; then
  fail "downloaded payload did not contain $release_asset_name or codex"
fi
if [[ ${#payload_files[@]} -eq 0 ]]; then
  fail "downloaded artifact did not contain a file"
fi

extract_package_archive "$main_payload"

if [[ -n "$code_mode_host_payload" ]]; then
  step "Detected dual-binary payload layout"
else
  step "Detected legacy single-binary payload layout"
fi

if [[ -n "$version" ]]; then
  # An explicit version is a deliberate pin, so the "is the installed copy newer?"
  # test would be wrong here ΓÇö it would refuse every downgrade.
  step "Explicit version requested; skipping mtime currency check"
fi

if ! "$force" && [[ -z "$version" ]] && [[ -e "$bin_path" ]]; then
  current_epoch="$(file_mtime_epoch "$bin_path")"
  installation_is_current=false
  if (( current_epoch >= source_epoch )); then
    installation_is_current=true
    if [[ -n "$code_mode_host_payload" ]]; then
      if [[ ! -e "$code_mode_host_bin_path" ]]; then
        installation_is_current=false
      else
        code_mode_host_epoch="$(file_mtime_epoch "$code_mode_host_bin_path")"
        if (( code_mode_host_epoch < source_epoch )); then
          installation_is_current=false
        fi
      fi
    fi
  fi

  if "$installation_is_current"; then
    step "Installed Codex layout is already as new or newer than requested source"
    step "Source epoch: $source_epoch"
    step "Use --force to install anyway"
    exit 0
  fi
fi

install_payload "$main_payload" "$bin_path" codex
if [[ -n "$code_mode_host_payload" ]]; then
  install_payload \
    "$code_mode_host_payload" \
    "$code_mode_host_bin_path" \
    codex-code-mode-host
fi

step "Installed version: $("$bin_path" --version)"

