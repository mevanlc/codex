#!/usr/bin/env sh
# Download the newest published codex-fork-installer for this platform and run it.
set -eu

repo="${CODEX_FORK_REPO:-mevanlc/codex}"
api_url="${CODEX_GITHUB_API_URL:-https://api.github.com}"
releases_url="${CODEX_GITHUB_RELEASES_URL:-https://github.com/$repo/releases}"

kernel="$(uname -s)"
machine="$(uname -m)"
if [ "$kernel" = "Darwin" ] && { [ "$machine" = "x86_64" ] || [ "$machine" = "amd64" ]; }; then
  if [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || true)" = "1" ]; then
    machine="arm64"
  fi
fi

target=""
case "${MSYSTEM_CARCH:-}:$kernel:$machine" in
  aarch64:MINGW*:*|arm64:MINGW*:*|aarch64:MSYS*:*|arm64:MSYS*:*|*:MINGW*-ARM64:*|*:MSYS*-ARM64:*)
    target="aarch64-pc-windows-msvc"
    ;;
  *:MINGW*:x86_64|*:MSYS*:x86_64|*:CYGWIN*:x86_64|*:MINGW*:amd64|*:MSYS*:amd64|*:CYGWIN*:amd64)
    target="x86_64-pc-windows-msvc"
    ;;
  *:Darwin:arm64|*:Darwin:aarch64)
    target="aarch64-apple-darwin"
    ;;
  *:Darwin:x86_64|*:Darwin:amd64)
    target="x86_64-apple-darwin"
    ;;
  *:Linux:aarch64|*:Linux:arm64)
    if [ -n "${TERMUX_VERSION:-}" ] || [ -n "${PREFIX:-}" ] || [ -f /system/build.prop ] || [ "$(uname -o 2>/dev/null || true)" = "Android" ]; then
      target="aarch64-linux-android"
    else
      target="aarch64-unknown-linux-musl"
    fi
    ;;
  *:Linux:x86_64|*:Linux:amd64)
    target="x86_64-unknown-linux-musl"
    ;;
esac

case "${PROCESSOR_ARCHITEW6432:-${PROCESSOR_ARCHITECTURE:-}}:$kernel" in
  ARM64:MINGW*|ARM64:MSYS*|ARM64:CYGWIN*)
    target="aarch64-pc-windows-msvc"
    ;;
esac

artifact="${CODEX_ARTIFACT_NAME:-${target:+codex-$target}}"
target="${artifact#codex-}"
if [ -z "$target" ]; then
  echo "error: unsupported platform ($kernel/$machine); set CODEX_ARTIFACT_NAME" >&2
  exit 1
fi

suffix=""
case "$kernel:$target" in
  MINGW*:*|MSYS*:*|CYGWIN*:*|*-pc-windows-msvc)
    suffix=".exe"
    ;;
esac

case "$kernel" in
  MINGW*|MSYS*|CYGWIN*)
    if command -v cygpath >/dev/null 2>&1; then
      HOME="$(cygpath -w "$HOME")"
      export HOME
    fi
    ;;
esac

asset="codex-fork-installer-$target$suffix"

command -v curl >/dev/null 2>&1 || {
  echo "error: curl is required to download $asset" >&2
  exit 1
}

if [ -z "${TMPDIR:-}" ] && [ -n "${PREFIX:-}" ] && [ -d "$PREFIX/tmp" ]; then
  TMPDIR="$PREFIX/tmp"
  export TMPDIR
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT INT TERM HUP

token="${GH_TOKEN:-${GITHUB_TOKEN:-}}"
api_endpoint="${api_url%/}/repos/$repo/releases?per_page=100"
metadata_file="$tmp_dir/releases.json"

fetch_releases_metadata() {
  if [ -n "$token" ]; then
    curl -fsSL \
      -H "Accept: application/vnd.github+json" \
      -H "User-Agent: codex-fork-installer-bootstrap" \
      -H "Authorization: Bearer $token" \
      -o "$metadata_file" \
      "$api_endpoint" 2>/dev/null
  else
    curl -fsSL \
      -H "Accept: application/vnd.github+json" \
      -H "User-Agent: codex-fork-installer-bootstrap" \
      -o "$metadata_file" \
      "$api_endpoint" 2>/dev/null
  fi
}

resolve_release_asset() {
  awk -v asset="$asset" '
    {
      s = $0
      while (match(s, /"[^"\\]*(\\.[^"\\]*)*"|\{|\}|\[|\]|:|,|true|false|null|-?[0-9.eE+-]+/)) {
        tok = substr(s, RSTART, RLENGTH)
        s = substr(s, RSTART + RLENGTH)
        if (tok == "{") {
          depth++
          if (depth == 1 && bracket_depth == 1) {
            tag = ""
            draft = "false"
            prerelease = "false"
            published = ""
            in_assets = 0
            rel_url = ""
            rel_digest = ""
            key1 = ""
            expect_val1 = 0
          } else if (depth == 2 && bracket_depth == 2 && in_assets) {
            a_name = ""
            a_url = ""
            a_digest = ""
            key2 = ""
            expect_val2 = 0
          } else {
            if (depth == 2) expect_val1 = 0
            if (depth == 3) expect_val2 = 0
          }
        } else if (tok == "}") {
          if (depth == 2 && bracket_depth == 2 && in_assets) {
            if (a_name == asset && a_url != "") {
              rel_url = a_url
              rel_digest = a_digest
            }
          } else if (depth == 1 && bracket_depth == 1) {
            if (draft == "false" && prerelease == "false" && published != "" && tag ~ /^fork-v[0-9]+\.[0-9]+\.[0-9]+/ && rel_url != "") {
              if (best_published == "" || published > best_published) {
                best_published = published
                best_url = rel_url
                best_digest = rel_digest
              }
            }
            in_assets = 0
          }
          depth--
        } else if (tok == "[") {
          bracket_depth++
          if (depth == 1 && bracket_depth == 2 && key1 == "assets" && expect_val1) {
            in_assets = 1
            expect_val1 = 0
          } else {
            if (depth == 1) expect_val1 = 0
            if (depth == 2) expect_val2 = 0
          }
        } else if (tok == "]") {
          if (depth == 1 && bracket_depth == 2 && in_assets) {
            in_assets = 0
          }
          bracket_depth--
        } else if (tok == ":") {
          if (depth == 1 && bracket_depth == 1) {
            expect_val1 = 1
          } else if (depth == 2 && bracket_depth == 2 && in_assets) {
            expect_val2 = 1
          }
        } else if (tok == ",") {
          if (depth == 1 && bracket_depth == 1) {
            expect_val1 = 0
          } else if (depth == 2 && bracket_depth == 2 && in_assets) {
            expect_val2 = 0
          }
        } else if (depth == 1 && bracket_depth == 1) {
          if (!expect_val1) {
            key1 = substr(tok, 2, length(tok) - 2)
          } else {
            val = (substr(tok, 1, 1) == "\"") ? substr(tok, 2, length(tok) - 2) : tok
            if (key1 == "tag_name") tag = val
            else if (key1 == "draft") draft = val
            else if (key1 == "prerelease") prerelease = val
            else if (key1 == "published_at" && val != "null") published = val
            expect_val1 = 0
          }
        } else if (depth == 2 && bracket_depth == 2 && in_assets) {
          if (!expect_val2) {
            key2 = substr(tok, 2, length(tok) - 2)
          } else {
            val = (substr(tok, 1, 1) == "\"") ? substr(tok, 2, length(tok) - 2) : tok
            if (key2 == "name") a_name = val
            else if (key2 == "browser_download_url" && val != "null") a_url = val
            else if (key2 == "digest" && val != "null") a_digest = val
            expect_val2 = 0
          }
        }
      }
    }
    END {
      if (best_url != "") {
        print best_url
        print best_digest
      }
    }
  ' "$metadata_file"
}

verify_sha256() {
  file_path="$1"
  expected_hex="$(printf '%s' "$2" | tr '[:upper:]' '[:lower:]')"
  if command -v sha256sum >/dev/null 2>&1; then
    actual_hex="$(sha256sum "$file_path" | awk '{print tolower($1)}')"
  elif command -v shasum >/dev/null 2>&1; then
    actual_hex="$(shasum -a 256 "$file_path" | awk '{print tolower($1)}')"
  elif command -v openssl >/dev/null 2>&1; then
    actual_hex="$(openssl dgst -sha256 "$file_path" | awk '{print tolower($NF)}')"
  else
    echo "error: sha256sum, shasum, or openssl is required to verify $asset" >&2
    exit 1
  fi
  if [ "$actual_hex" != "$expected_hex" ]; then
    echo "error: $asset does not match its published digest (expected sha256:$expected_hex, got sha256:$actual_hex)" >&2
    exit 1
  fi
}

download_url=""
expected_digest=""
if fetch_releases_metadata; then
  resolved="$(resolve_release_asset)"
  if [ -z "$resolved" ]; then
    echo "error: no published release of $repo carries $asset" >&2
    exit 1
  fi
  download_url="$(printf '%s\n' "$resolved" | sed -n '1p')"
  expected_digest="$(printf '%s\n' "$resolved" | sed -n '2p')"
  case "$expected_digest" in
    sha256:*)
      expected_digest="${expected_digest#sha256:}"
      ;;
    *)
      echo "error: GitHub published no SHA-256 digest for $asset" >&2
      exit 1
      ;;
  esac
else
  download_url="${releases_url%/}/latest/download/$asset"
fi

installer_path="$tmp_dir/$asset"
if [ -n "$token" ]; then
  curl -fsSL \
    -H "User-Agent: codex-fork-installer-bootstrap" \
    -H "Authorization: Bearer $token" \
    -o "$installer_path" \
    "$download_url"
else
  curl -fsSL \
    -H "User-Agent: codex-fork-installer-bootstrap" \
    -o "$installer_path" \
    "$download_url"
fi

if [ -n "$expected_digest" ]; then
  verify_sha256 "$installer_path" "$expected_digest"
fi

chmod +x "$installer_path"
"$installer_path" "$@"
