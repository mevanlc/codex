# Codex CLI — Termux Fork

A fork of [OpenAI's Codex CLI](https://github.com/openai/codex) with Android/Termux support.

> For full upstream documentation, see the [official Codex CLI README](https://github.com/openai/codex#readme) and [docs](https://developers.openai.com/codex).

## What this fork does

This fork maintains a build of Codex CLI that runs natively on Android via [Termux](https://termux.dev), while keeping the codebase compilable for other platforms. It stays close to upstream through frequent merges.

### Key changes

- **Cross-platform fork releases** — GitHub-hosted runners build Linux, macOS, and Windows for amd64 and arm64, while the self-hosted A1 runner builds `aarch64-linux-android`
- **Android code-mode / JS REPL** — Android builds link a prebuilt `librusty_v8` and ship `codex-code-mode-host` alongside `codex`
- **`install-fork.sh`** — one build-and-install script for macOS, Linux, and Termux, with Termux auto-detection and target-dir pruning
- **`[tui].primary_accent`** — replace Codex's cyan accent with a color of your choosing
- **`[tui].chatbox_placeholder_tips`** — turn off the rotating composer tips
- **File mention settings** — optionally preserve the `@` prefix and complete explicit paths
- **Shell follow-ups** — press Tab in `!` mode to start an agent turn after the command finishes
- **Retractable steer messages** — pull a still-pending steer back into the composer; backed by a new `turn/retract` app-server method
- **Unrestricted reasoning shortcuts** — the reasoning hotkeys step all the way into Max and Ultra
- **Platform fixes** — file-lock fallbacks, `SHELL`-based shell detection on Android, vendored OpenSSL, fork-aware update checks

Everything else behaves like upstream Codex.

## Goals

- A mostly-vanilla build of Codex that runs on Termux
- Codebase continues to compile for other platforms
- Can be compiled within Termux on devices with high enough specs (e.g. Galaxy S25)
- Frequent merges of upstream into the fork

## Non-goals

- **Substantial features not in upstream** — this is a platform port, not a feature fork

## Install

Grab the archive for your platform from [Releases](https://github.com/mevanlc/codex/releases). Desktop archives use the Rust target triple in their names:

- `codex-{x86_64,aarch64}-unknown-linux-musl.tar.zst`
- `codex-{x86_64,aarch64}-apple-darwin.tar.zst`
- `codex-{x86_64,aarch64}-pc-windows-msvc.zip`

Each desktop archive contains a complete Codex package, including `codex`, `codex-code-mode-host`, ripgrep, and the platform-specific sandbox helpers where applicable.

The Android/Termux build remains two zstd-compressed binaries. Install both into the same directory on `PATH`, since the code-mode runtime resolves its host beside `codex`:

```shell
zstd -d codex-aarch64-linux-android.zst -o codex
zstd -d codex-code-mode-host-aarch64-linux-android.zst -o codex-code-mode-host
chmod +x codex codex-code-mode-host
```

Or [build from source](#building).

## Fork-only settings and behavior

### Primary accent color

Codex uses cyan as its accent color throughout the TUI. `[tui].primary_accent` remaps every cyan cell on the way to the terminal — composer, popups, and scrollback transcript alike:

```toml
[tui]
primary_accent = "#7a81ff"
```

Accepted forms are `r,g,b` with each channel `0..255`, `#RRGGBB` (exactly six hex digits), or a bare `0..255` ANSI palette index. An unparsable value is reported as a startup warning and the default cyan is kept. Only the exact `Cyan` color is substituted; every other color passes through unchanged.

### Chatbox placeholder tips

```toml
[tui]
chatbox_placeholder_tips = "off"
```

`on` (the default) keeps upstream's rotating tip prompts in the composer; `off` leaves the placeholder blank.

### File mentions

```toml
[tui]
file_mentions_preserve_at = true
file_mentions_allow_explicit_paths = true
```

`file_mentions_preserve_at` keeps the leading `@` in the composer when a file-search result is completed, so `@program` remains `@program` in the prompt sent to the model. It defaults to `false`. When a directory is selected, Tab inserts its trailing slash and continues path completion; Enter inserts the trailing slash and finishes the completion. The intermediate Tab completion retains `@` while search remains active, and the final result honors `file_mentions_preserve_at`. `file_mentions_allow_explicit_paths` makes file search recognize absolute paths, home-relative paths beginning with `~/`, and relative paths beginning with `./` or `../`, including paths with repeated `.` and `..` components. Home-relative searches resolve `~` to the current user's home directory while preserving the lexical form you typed in the completed path. Explicit paths are enabled by default; set this option to `false` to disable them. When fuzzy matches have the same score, entries closer to the searched directory appear first.

The unified mention picker offers `Filesystem` for the ordinary file-search scope and `Filesystem (All)` for hidden and ignored entries. `All Results` continues to combine plugins with only the ordinary filesystem scope; expanded filesystem matches appear only under `Filesystem (All)`.

### Shell command follow-ups

While composing a `!` shell command, press Tab to toggle an automatic follow-up on or off. With the follow-up on, Enter runs the command as usual, then starts an agent turn after the command exits. The agent receives the command result and is instructed to investigate and fix failures, or explain a successful result and relevant next steps. The toggle applies only to the current shell command and requires no configuration.

### Retracting a pending steer

Upstream binds `Alt+Up` (or `Shift+Left`) to "edit the most recently queued message", and it only reaches messages still queued locally. This fork extends the same binding to _steers_ — messages already handed to an in-flight turn but not yet consumed — and pops the steer back into the composer for editing. The pending-input preview now shows the edit hint whenever anything is retractable, not just for locally queued messages. If the turn consumes the message first, Codex warns that it was already submitted and can no longer be edited.

The underlying mechanism is a new experimental `turn/retract` app-server request taking `threadId`, `expectedTurnId`, and `clientUserMessageId`, and returning `retracted`, `notPending`, or `notRetractable`. Steers that carried additional context or Responses API client metadata are not retractable, because those side effects are applied when the steer is accepted. See [`codex-rs/app-server/README.md`](codex-rs/app-server/README.md) for the request/response shapes.

### Reasoning shortcuts reach Max and Ultra

`Alt+.` / `Shift+Up` and `Alt+,` / `Shift+Down` step the active model's reasoning effort. Upstream refuses to raise into Max or Ultra from the keyboard and instead points at `/model → … → More reasoning…`; this fork walks the full list of efforts the model advertises, with Max and Ultra last. Plan mode's Ultra concurrency warning still applies.

### Platform fixes

- `flock` is best-effort where the filesystem rejects it (some Android f2fs kernels return `EOPNOTSUPP`): the installation-id lock is skipped, and the per-session PATH directory falls back to a `/proc`-based liveness record so stale-directory cleanup still works.
- Shell detection prefers `$SHELL` over the `passwd` entry on Android, where Termux's shell is not in `/etc/passwd`.
- OpenSSL is vendored for `aarch64-linux-android`.
- Update checks parse full semver and rank the fork's `X.Y.Z-<sha>` snapshots after the matching `X.Y.Z` release. Ordinary prereleases, including `X.Y.Z-alpha.N-<sha>`, remain before the release.

## Building

`codex-rs/scripts/install-fork.sh` builds `codex` and `codex-code-mode-host` and installs both into `~/.local/bin`:

```shell
git clone https://github.com/mevanlc/codex.git
cd codex
./codex-rs/scripts/install-fork.sh               # lite profile: fast, unoptimized
./codex-rs/scripts/install-fork.sh -p release    # optimized
```

The script stamps a version derived from the newest `rust-v*` tag reachable from `HEAD` (restoring `Cargo.toml` on exit), re-signs the binaries on macOS, and warns if `~/.local/bin/codex` is not what `codex` actually resolves to on `PATH`.

| Flag                   | Effect                                                                                  |
| ---------------------- | --------------------------------------------------------------------------------------- |
| `-p, --profile P`      | Cargo profile; default `lite` (this fork's unoptimized, thin-LTO, no-debuginfo profile) |
| `-u, --update`         | `git fetch` and `--ff-only` merge `origin/main` before building                         |
| `--prune-gb N`         | Prune the Cargo target dir when it exceeds N GiB                                        |
| `--prune-mode MODE`    | `incremental` (default), `sweep`, `aggressive`, or `auto`                               |
| `--prune-every-days D` | Prune at most once per D days (default `1`)                                             |

`--help` also prints per-platform prerequisites. Desktop builds use the exact-version sandboxed V8 archive and binding published by `openai/codex`. On Termux (aarch64 with `/system/build.prop` present), the script instead points `RUSTY_V8_ARCHIVE` at the Android prebuilt hosted on this repo's releases and adds the link flags needed for `libc++` and `__clear_cache`. Termux prerequisites: `pkg install rust binutils cmake openssl pkg-config`.

This fork also raises `[profile.release]` to `opt-level = 3` with fat LTO.

## Status

| Target          | CI     | Runner                                                |
| --------------- | ------ | ----------------------------------------------------- |
| Linux amd64     | Active | GitHub-hosted `ubuntu-24.04`                          |
| Linux arm64     | Active | GitHub-hosted `ubuntu-24.04-arm`                      |
| macOS amd64     | Active | GitHub-hosted `macos-15-intel`                        |
| macOS arm64     | Active | GitHub-hosted `macos-15`                              |
| Windows amd64   | Active | GitHub-hosted `windows-2022`                          |
| Windows arm64   | Active | GitHub-hosted `windows-11-arm`                        |
| Android aarch64 | Active | Self-hosted A1 runner (`self-hosted`, `Linux`, `ARM64`) |

The release workflow runs on every push to `main`. The first completed platform build publishes the release with its artifact; the other builds add their artifacts as they finish. Matrix failures do not cancel the remaining platform builds.

## License

Same as upstream — [Apache-2.0](LICENSE).
