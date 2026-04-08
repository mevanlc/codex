# Codex CLI — Termux Fork

A fork of [OpenAI's Codex CLI](https://github.com/openai/codex) with Android/Termux support.

> For full upstream documentation, see the [official Codex CLI README](https://github.com/openai/codex#readme) and [docs](https://developers.openai.com/codex).

## What this fork does

This fork maintains a build of Codex CLI that runs natively on Android via [Termux](https://termux.dev), while keeping the codebase compilable for other platforms. It stays close to upstream through frequent merges.

### Key changes

- **Android aarch64 target** — CI workflow produces release binaries for `aarch64-linux-android`
- **Android code-mode/V8** — Android builds now include working `code-mode` and JS REPL support via prebuilt `rusty_v8` artifacts
- **Termux-compatible build script** — `build-fork.sh` works across macOS, Linux, and Termux
- **Build optimizations for constrained devices** — swap file management, thin LTO, job limits to avoid OOM on device
- **Platform-specific fixes** — file locking fallback, `SHELL` env handling, voice input deps disabled on Android
- **Small QOL additions** — chatbox placeholder tips toggle, model switch while MCP servers connect, PATH shadow warning

## Goals

- A mostly-vanilla build of Codex that runs on Termux
- Codebase continues to compile for other platforms
- Can be compiled within Termux on devices with high enough specs (e.g. Galaxy S25)
- Frequent merges of upstream into the fork

## Non-goals

- **Substantial features not in upstream** — this is a platform port, not a feature fork

## Install

Grab the latest Android aarch64 binary from [Releases](https://github.com/mevanlc/codex/releases), or build from source. Current Android release builds include working code-mode / JS REPL support:

```shell
# In Termux
git clone https://github.com/mevanlc/codex.git
cd codex
./codex-rs/scripts/build-fork.sh
```

## Status

| Target | CI | Notes |
|--------|----|-------|
| Android aarch64 | Passing | Release binaries published automatically, including code-mode / JS REPL |
| Other platforms | Failing | Investigating — upstream code-mode changes may need additional gating |

## Fork stats

- ~30 original commits on top of upstream
- 13 upstream merges to date
- Tracking upstream actively

## License

Same as upstream — [Apache-2.0](LICENSE).
