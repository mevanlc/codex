# Codex CLI — Termux Fork

A fork of [OpenAI's Codex CLI](https://github.com/openai/codex) with Android/Termux support.

> For full upstream documentation, see the [official Codex CLI README](https://github.com/openai/codex#readme) and [docs](https://developers.openai.com/codex).

## What this fork does

This fork maintains a build of Codex CLI that runs natively on Android via [Termux](https://termux.dev), while keeping the codebase compilable for other platforms. It stays close to upstream through frequent merges.

### Key changes

- **Android aarch64 target** — a self-hosted CI workflow publishes `aarch64-linux-android` release binaries
- **Android code-mode / JS REPL** — Android builds link a prebuilt `librusty_v8` and ship `codex-code-mode-host` alongside `codex`
- **`build-fork.sh`** — one build-and-install script for macOS, Linux, and Termux, with Termux auto-detection and target-dir pruning
- **`[tui].primary_accent`** — replace Codex's cyan accent with a color of your choosing
- **`[tui].chatbox_placeholder_tips`** — turn off the rotating composer tips
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

Grab the latest Android aarch64 build from [Releases](https://github.com/mevanlc/codex/releases). Each release ships two zstd-compressed binaries; install both into the same directory on `PATH`, since the code-mode runtime resolves its host beside `codex`:

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

### Retracting a pending steer

Upstream binds `Alt+Up` (or `Shift+Left`) to "edit the most recently queued message", and it only reaches messages still queued locally. This fork extends the same binding to _steers_ — messages already handed to an in-flight turn but not yet consumed — and pops the steer back into the composer for editing. The pending-input preview now shows the edit hint whenever anything is retractable, not just for locally queued messages. If the turn consumes the message first, Codex warns that it was already submitted and can no longer be edited.

The underlying mechanism is a new experimental `turn/retract` app-server request taking `threadId`, `expectedTurnId`, and `clientUserMessageId`, and returning `retracted`, `notPending`, or `notRetractable`. Steers that carried additional context or Responses API client metadata are not retractable, because those side effects are applied when the steer is accepted. See [`codex-rs/app-server/README.md`](codex-rs/app-server/README.md) for the request/response shapes.

### Reasoning shortcuts reach Max and Ultra

`Alt+.` / `Shift+Up` and `Alt+,` / `Shift+Down` step the active model's reasoning effort. Upstream refuses to raise into Max or Ultra from the keyboard and instead points at `/model → … → More reasoning…`; this fork walks the full list of efforts the model advertises, with Max and Ultra last. Plan mode's Ultra concurrency warning still applies.

### Platform fixes

- `flock` is best-effort where the filesystem rejects it (some Android f2fs kernels return `EOPNOTSUPP`): the installation-id lock is skipped, and the per-session PATH directory falls back to a `/proc`-based liveness record so stale-directory cleanup still works.
- Shell detection prefers `$SHELL` over the `passwd` entry on Android, where Termux's shell is not in `/etc/passwd`.
- OpenSSL is vendored for `aarch64-linux-android`.
- Update checks parse full semver, so the fork's `X.Y.Z-<sha>` release versions compare correctly instead of being ignored.

## Building

`codex-rs/scripts/build-fork.sh` builds `codex` and `codex-code-mode-host` and installs both into `~/.local/bin`:

```shell
git clone https://github.com/mevanlc/codex.git
cd codex
./codex-rs/scripts/build-fork.sh               # lite profile: fast, unoptimized
./codex-rs/scripts/build-fork.sh -p release    # optimized
```

The script stamps a version derived from the newest `rust-v*` tag reachable from `HEAD` (restoring `Cargo.toml` on exit), re-signs the binaries on macOS, and warns if `~/.local/bin/codex` is not what `codex` actually resolves to on `PATH`.

| Flag                   | Effect                                                                                  |
| ---------------------- | --------------------------------------------------------------------------------------- |
| `-p, --profile P`      | Cargo profile; default `lite` (this fork's unoptimized, thin-LTO, no-debuginfo profile) |
| `-u, --update`         | `git fetch` and `--ff-only` merge `origin/main` before building                         |
| `--prune-gb N`         | Prune the Cargo target dir when it exceeds N GiB                                        |
| `--prune-mode MODE`    | `incremental` (default), `sweep`, `aggressive`, or `auto`                               |
| `--prune-every-days D` | Prune at most once per D days (default `1`)                                             |

`--help` also prints per-platform prerequisites. On Termux (aarch64 with `/system/build.prop` present) the script auto-detects the platform, points `RUSTY_V8_ARCHIVE` at the prebuilt V8 hosted on this repo's releases, and adds the link flags needed for `libc++` and `__clear_cache`. Termux prerequisites: `pkg install rust binutils cmake openssl pkg-config`.

This fork also raises `[profile.release]` to `opt-level = 3` with fat LTO.

## Status

| Target          | CI      | Notes                                                                    |
| --------------- | ------- | ------------------------------------------------------------------------ |
| Android aarch64 | Active  | Release binaries published automatically from a self-hosted ARM64 runner |
| Other platforms | Not run | Build from source; validated locally                                     |

Upstream's own CI workflows are disabled in this fork, so the Android release workflow is the only build that runs here. Other targets still compile — that is an explicit goal — but nothing verifies them automatically.

## License

Same as upstream — [Apache-2.0](LICENSE).
