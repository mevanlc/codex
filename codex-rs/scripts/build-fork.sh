#!/usr/bin/env bash
# vim: set sw=2 sts=2 noet:
#
# Build the codex CLI binary from source.
#
# Features:
#   - Auto-detects version from git tags (rust-v*)
#   - Stamps a custom version string into Cargo.toml for the build, restores on exit
#   - Termux/Android: auto-configures prebuilt V8 mirror and links C++ runtime
#   - Optional: fetch + fast-forward merge before building
#   - Optional: prune target dir if it exceeds a size cap
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CODEX_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CARGO_TOML="$CODEX_DIR/Cargo.toml"

cd "$CODEX_DIR"

# Portable in-place sed (BSD sed requires -i '', GNU sed does not).
sedi() {
  if sed --version >/dev/null 2>&1; then
    sed -i "$@"
  else
    sed -i '' "$@"
  fi
}

usage() {
  cat <<'EOF' >&2
Usage: build-fork.sh [options] [version]

Options:
  -u, --update               Fetch + ff-merge origin/main before build
  -p, --profile P            Build profile (default: lite)
  --prune-gb N               If target dir exceeds N GiB, periodically prune
  --prune-every-days D       Only prune once per D days (default: 1)
  --prune-mode MODE          MODE: incremental (default) | sweep | aggressive | auto
                             - incremental: delete target/*/incremental
                             - sweep: cargo-sweep to --maxsize NGB (may evict old-but-useful deps)
                             - aggressive: also delete target/*/deps + target/*/build
                             - auto: incremental then aggressive if still over cap

If version is omitted, auto-detects from git tags.

Platform Notes
──────────────
  macOS:
    Xcode Command Line Tools required (xcode-select --install).
    Homebrew deps: brew install cmake pkg-config openssl protobuf
    Apple Silicon (M1+) builds natively; no special flags needed.

  Linux (x86_64 / aarch64):
    Debian/Ubuntu: apt install build-essential cmake pkg-config libssl-dev
    Fedora/RHEL:   dnf install gcc cmake openssl-devel pkg-config
    Builds with default features including code-mode (V8).

  Windows:
    Requires Visual Studio Build Tools (MSVC) or the full VS installer.
    Install Rust via rustup (rustup.rs). Use the "x86_64-pc-windows-msvc"
    toolchain. Run from a "Developer Command Prompt" or ensure cl.exe is
    on PATH. See also: codex-rs/scripts/setup-windows.ps1

  Termux / Android (aarch64-linux-android):
    Auto-detected by this script. V8 prebuilt is downloaded from
    mevanlc/codex releases (via RUSTY_V8_ARCHIVE). The C++ runtime is
    linked explicitly for native deps (oboe-sys, onig_sys).
    Prereqs: pkg install rust binutils cmake openssl pkg-config
EOF
}

UPDATE_FLAG=""
PRUNE_GB=""
PRUNE_EVERY_DAYS="1"
PRUNE_MODE="incremental"
PROFILE="lite"
VERSION=""

while [[ $# -gt 0 ]]; do
  case "$1" in
	-u|--update)
	  UPDATE_FLAG="1"
	  shift
	  ;;
	-p|--profile)
	  PROFILE="${2:-}"
	  [[ -n "$PROFILE" ]] || { echo "Error: $1 requires a profile name" >&2; usage; exit 2; }
	  shift 2
	  ;;
	--prune-gb)
	  PRUNE_GB="${2:-}"
	  [[ -n "$PRUNE_GB" ]] || { echo "Error: $1 requires a number" >&2; usage; exit 2; }
	  shift 2
	  ;;
	--prune-every-days)
	  PRUNE_EVERY_DAYS="${2:-}"
	  [[ -n "$PRUNE_EVERY_DAYS" ]] || { echo "Error: $1 requires a number" >&2; usage; exit 2; }
	  shift 2
	  ;;
	--prune-mode)
	  PRUNE_MODE="${2:-}"
	  [[ -n "$PRUNE_MODE" ]] || { echo "Error: $1 requires a mode" >&2; usage; exit 2; }
	  shift 2
	  ;;
	-h|--help)
	  usage
	  exit 0
	  ;;
	*)
	  if [[ -n "$VERSION" ]]; then
		echo "Error: unexpected extra arg: $1" >&2
		usage
		exit 2
	  fi
	  VERSION="$1"
	  shift
	  ;;
  esac
done

case "$PRUNE_MODE" in
  sweep|incremental|aggressive|auto) ;;
  *) echo "Error: --prune-mode must be incremental|sweep|aggressive|auto (got: $PRUNE_MODE)" >&2; exit 2 ;;
esac

if [[ -n "$PRUNE_EVERY_DAYS" ]] && [[ ! "$PRUNE_EVERY_DAYS" =~ ^[0-9]+$ ]]; then
  echo "Error: --prune-every-days must be an integer (got: $PRUNE_EVERY_DAYS)" >&2
  exit 2
fi

if [[ -n "$PRUNE_GB" ]] && [[ ! "$PRUNE_GB" =~ ^[0-9]+([.][0-9]+)?$ ]]; then
  echo "Error: --prune-gb must be a number (got: $PRUNE_GB)" >&2
  exit 2
fi

# Handle --update: fetch, validate changes, restore, and ff-merge
if [[ -n "$UPDATE_FLAG" ]]; then
    REPO_ROOT=$(git rev-parse --show-toplevel)

    echo "Fetching from origin..."
    git fetch origin

    # Check for modified files (paths relative to repo root)
    modified_files=$(git -C "$REPO_ROOT" diff --name-only)
    if [[ -n "$modified_files" ]]; then
        # Expected: Cargo.toml (version line only), Cargo.lock (always ok)
        unexpected=""
        restore_files=""
        for file in $modified_files; do
            basename="${file##*/}"
            if [[ "$basename" == "Cargo.lock" ]]; then
                restore_files="$restore_files $file"
            elif [[ "$basename" == "Cargo.toml" ]]; then
                # Check if only version line changed
                diff_content=$(git -C "$REPO_ROOT" diff -- "$file")
                if ! echo "$diff_content" | grep -qE '^[-+]version = "'; then
                    unexpected="$unexpected $file (unexpected changes)"
                elif echo "$diff_content" | grep -E '^[-+]' | grep -vE '^[-+]version = "|^[-+]{3} ' | grep -q .; then
                    unexpected="$unexpected $file (non-version changes)"
                else
                    restore_files="$restore_files $file"
                fi
            else
                unexpected="$unexpected $file"
            fi
        done

        if [[ -n "$unexpected" ]]; then
            echo "Error: Unexpected modified files:$unexpected" >&2
            echo "Please commit or stash changes before updating." >&2
            exit 1
        fi

        if [[ -n "$restore_files" ]]; then
            echo "Restoring:$restore_files"
            git -C "$REPO_ROOT" checkout --$restore_files
        fi
    fi

    echo "Merging with --ff-only..."
    if ! git merge --ff-only origin/main; then
        echo "Error: Fast-forward merge failed. Manual intervention required." >&2
        exit 1
    fi
    echo "Update complete."
fi

# Auto-detect version if not provided
if [[ -z "$VERSION" ]]; then
    # Find newest tag whose parent is ancestor of current HEAD
    VERSION=""
    for tag in $(git for-each-ref --sort=-taggerdate --format='%(refname:short)' refs/tags/rust-v*); do
        parent=$(git rev-parse --quiet --verify "$tag^" 2>/dev/null) || continue
        if git merge-base --is-ancestor "$parent" HEAD 2>/dev/null; then
            VERSION="${tag#rust-v}"
            # Abbreviate -alpha.N as -aN and append +MMDD
            VERSION=$(echo "$VERSION" | sed 's/-alpha\./-a/')
            VERSION="${VERSION}+$(date +%m%d)"
            break
        fi
    done
    if [[ -z "$VERSION" ]]; then
        echo "Error: Could not auto-detect version from tags" >&2
        exit 1
    fi
    echo "Auto-detected version: $VERSION"
fi

# Backup original version line
ORIGINAL_VERSION=$(grep -E '^version = "' "$CARGO_TOML" | head -1)
CARGO_LOCK="Cargo.lock"
RESTORE_CARGO_LOCK=""

if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    if git diff --quiet -- "$CARGO_LOCK" && git diff --cached --quiet -- "$CARGO_LOCK"; then
        RESTORE_CARGO_LOCK="1"
    fi
fi

# Set custom version
sedi 's/^version = ".*"/version = "'"$VERSION"'"/' "$CARGO_TOML"

# Ensure we restore Cargo.toml on exit
cleanup() {
    sedi 's/^version = ".*"/'"$ORIGINAL_VERSION"'/' "$CARGO_TOML"
    if [[ -n "$RESTORE_CARGO_LOCK" ]]; then
        git checkout -- "$CARGO_LOCK"
    fi
}
trap cleanup EXIT

# Termux/Android: use prebuilt V8 from mevanlc/codex releases and link C++
# runtime for native C++ deps (oboe-sys, onig_sys).
V8_PREBUILT_BASE="https://github.com/mevanlc/codex/releases/download"
V8_PREBUILT_TAG="v8-v146.4.0"
if [[ "$(uname -m)" == "aarch64" ]] && [[ -f /system/build.prop ]]; then
	export RUSTY_V8_ARCHIVE="${V8_PREBUILT_BASE}/${V8_PREBUILT_TAG}/librusty_v8_release_aarch64-linux-android.a.gz"
	V8_BINDING="/tmp/v8_src_binding.rs"
	if [[ ! -f "$V8_BINDING" ]]; then
		curl -sL -o "$V8_BINDING" "${V8_PREBUILT_BASE}/${V8_PREBUILT_TAG}/src_binding_release_aarch64-linux-android.rs"
	fi
	export RUSTY_V8_SRC_BINDING_PATH="$V8_BINDING"
	export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-lc++_static -C link-arg=-lc++abi"
	echo "Termux detected: using prebuilt V8 from ${V8_PREBUILT_TAG}"
fi

# Build binary
echo "Building codex $VERSION ($PROFILE)..."

cargo build --bin codex --profile $PROFILE -p codex-cli

# Copy to ~/.local/bin
mkdir -p "$HOME/.local/bin"
cp "$CODEX_DIR/target/$PROFILE/codex" "$HOME/.local/bin/codex"

warn_if_installed_codex_is_not_first_on_path() {
  local installed_codex first_codex
  installed_codex="$HOME/.local/bin/codex"
  first_codex="$(type -P codex || true)"

  if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
    echo "Warning: $HOME/.local/bin is not on PATH, so running 'codex' will not use $installed_codex" >&2
    return 0
  fi

  if [[ -z "$first_codex" ]]; then
    echo "Warning: 'codex' does not resolve on PATH after installing $installed_codex" >&2
    return 0
  fi

  if [[ "$first_codex" != "$installed_codex" ]]; then
    echo "Warning: 'codex' resolves to $first_codex before $installed_codex" >&2
  fi
}

maybe_prune_target() {
  [[ -n "$PRUNE_GB" ]] || return 0
  command -v cargo >/dev/null 2>&1 || return 0

  local target_dir
  target_dir="$(
    cargo metadata --no-deps --format-version=1 2>/dev/null \
      | tr -d '\n' \
      | sed -n 's/.*"target_directory":"\\([^"]*\\)".*/\\1/p' \
      | head -n 1
  )"
  [[ -n "$target_dir" ]] || return 0
  [[ -d "$target_dir" ]] || return 0

  local limit_kib size_kib
  limit_kib="$(awk -v gb="$PRUNE_GB" 'BEGIN { printf "%.0f\n", gb * 1024 * 1024 }')"
  size_kib="$(du -sk "$target_dir" 2>/dev/null | awk '{print $1}')"

  [[ -n "$size_kib" ]] || return 0
  awk -v s="$size_kib" -v l="$limit_kib" 'BEGIN{ exit !(s > l) }' || return 0

  local stamp min_age_s now_s stamp_s
  stamp="$target_dir/.build-codex.last_prune"
  min_age_s="$(( PRUNE_EVERY_DAYS * 86400 ))"
  now_s="$(date +%s)"
  stamp_s="$(stat -c %Y "$stamp" 2>/dev/null || stat -f %m "$stamp" 2>/dev/null || echo 0)"
  if [[ $(( now_s - stamp_s )) -lt "$min_age_s" ]]; then
    return 0
  fi

  echo "Pruning Cargo target (over ${PRUNE_GB}GiB; mode=$PRUNE_MODE): $target_dir"
  case "$PRUNE_MODE" in
    sweep)
      if cargo sweep --help >/dev/null 2>&1; then
        cargo sweep --maxsize "${PRUNE_GB}GB" "$CODEX_DIR" >/dev/null 2>&1 || true
      else
        echo "Note: cargo-sweep not installed; falling back to incremental prune" >&2
        command -v prune-build-caches >/dev/null 2>&1 || return 0
        prune-build-caches --project "$CODEX_DIR" --mode incremental --no-ccache --no-sccache || true
      fi
      ;;
    incremental)
      command -v prune-build-caches >/dev/null 2>&1 || return 0
      prune-build-caches --project "$CODEX_DIR" --mode incremental --no-ccache --no-sccache || true
      ;;
    aggressive)
      command -v prune-build-caches >/dev/null 2>&1 || return 0
      prune-build-caches --project "$CODEX_DIR" --mode aggressive --no-ccache --no-sccache || true
      ;;
    auto)
      command -v prune-build-caches >/dev/null 2>&1 || return 0
      prune-build-caches --project "$CODEX_DIR" --mode incremental --no-ccache --no-sccache || true
      size_kib="$(du -sk "$target_dir" 2>/dev/null | awk '{print $1}')"
      if [[ -n "$size_kib" ]] && awk -v s="$size_kib" -v l="$limit_kib" 'BEGIN{ exit !(s > l) }'; then
        prune-build-caches --project "$CODEX_DIR" --mode aggressive --no-ccache --no-sccache || true
      fi
      ;;
  esac
  mkdir -p "$(dirname "$stamp")"
  : >"$stamp"
}

maybe_prune_target
warn_if_installed_codex_is_not_first_on_path

echo "Installed codex $VERSION to $HOME/.local/bin/codex"
