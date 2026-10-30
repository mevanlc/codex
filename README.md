# Codex CLI — Termux Fork

A fork of [OpenAI's Codex CLI](https://github.com/openai/codex) with Android/Termux support.

> For full upstream documentation, see the [official Codex CLI README](https://github.com/openai/codex#readme) and [docs](https://developers.openai.com/codex).

## What this fork does

This fork maintains a build of Codex CLI that runs natively on Android via [Termux](https://termux.dev), while keeping the codebase compilable for other platforms. It stays close to upstream through frequent merges.

### Key changes

- **Cross-platform fork releases** — GitHub-hosted runners build Linux, macOS, and Windows for amd64 and arm64, while the self-hosted A1 runner builds `aarch64-linux-android`
- **Android code-mode / JS REPL** — Android builds link a prebuilt `librusty_v8` and ship `codex-code-mode-host` alongside `codex`
- **Local source installs** — `just build-and-install-fork` builds and installs a complete managed package; `build-and-install.sh` also supports loose-binary installs with Termux auto-detection and target-dir pruning
- **`[tui].primary_accent`** — replace Codex's cyan accent with a color of your choosing
- **`[tui].chatbox_placeholder_tips`** — turn off the rotating composer tips
- **File mention settings** — optionally preserve the `@` prefix and complete explicit paths
- **Shell invocation modes** — press Tab in `!` mode to cycle normal execution, agent follow-up, and managed background execution
- **`--x-no-mcp`** — disable every MCP server and orchestrator MCP access for one invocation
- **Retractable steer messages** — pull a still-pending steer back into the composer; backed by a new `turn/retract` app-server method
- **Context usage on resume and fork** — restore the saved context fullness before the next turn, including paginated forks and turns that crossed a compaction checkpoint with their identity saved only in the following turn context
- **Unrestricted reasoning shortcuts** — the reasoning hotkeys step all the way into Max and Ultra
- **Brief statusline components** — selected components cycle through off, full, and brief in `/statusline`
- **Platform fixes** — `SHELL`-based shell detection on Android, vendored OpenSSL, fork-aware update checks

Everything else behaves like upstream Codex.

## Goals

- A mostly-vanilla build of Codex that runs on Termux
- Codebase continues to compile for other platforms
- Can be compiled within Termux on devices with high enough specs (e.g. Galaxy S25)
- Frequent merges of upstream into the fork

## Non-goals

- **Substantial features not in upstream** — this is a platform port, not a feature fork

## Install

Install the newest complete package available for your platform from
[mevanlc/codex releases](https://github.com/mevanlc/codex/releases) with
`codex-fork-installer`, a single self-contained program published for every
platform. `install-fork.sh` and `install-fork.ps1` detect your platform, download
the newest published `codex-fork-installer-<target>` binary, verify its SHA-256
digest, and run it (needing no Python, `gh`, or `zstd`).

macOS, Linux, and Android/Termux (`pkg install curl` if needed):

```shell
curl -fsSL https://raw.githubusercontent.com/mevanlc/codex/main/install-fork.sh | sh
```

PowerShell 7 or Windows PowerShell 5.1:

```powershell
irm https://raw.githubusercontent.com/mevanlc/codex/main/install-fork.ps1 | iex
```

Pass installer flags with `sh -s -- <flags>` (for example, `... | sh -s -- --list-available`)
or by running `./install-fork.sh <flags>` / `.\install-fork.ps1 <flags>` directly.
You can also download `codex-fork-installer-<target>` directly from the releases
page or with `gh release download -R mevanlc/codex -p 'codex-fork-installer-<target>*'`:

| Platform | `<target>` |
| --- | --- |
| macOS Apple silicon / Intel | `aarch64-apple-darwin` / `x86_64-apple-darwin` |
| Linux arm64 / x86-64 | `aarch64-unknown-linux-musl` / `x86_64-unknown-linux-musl` |
| Android/Termux | `aarch64-linux-android` |
| Windows Arm / x86-64 | `aarch64-pc-windows-msvc` / `x86_64-pc-windows-msvc` (assets end in `.exe`) |

Command-line downloads are normally not quarantined; if you download with a
browser instead, macOS Gatekeeper and Windows SmartScreen may ask before running it.

Add `~/.local/bin` to `PATH` (also on Windows). After the first install,
`codex update` and the daemon updater download and run the newest release's
installer themselves, so installer fixes reach existing installs.
`codex-rs/scripts/install-from-gh.sh` remains only so that builds released before
the installer can update to it.

All platforms use the same installation root, independently of `CODEX_HOME` and
`XDG_DATA_HOME`:

```text
~/.local/share/mevanlc-codex/
  packages/
    cli/
      releases/<version-target-digest>/
      current
    app-server-daemon/
      releases/<version-target-digest>/
      current
```

Commands in `~/.local/bin` select the CLI package through `current`. Unix uses
symlinks; Windows uses Bash and `.cmd` launchers plus an NTFS directory junction,
without administrator symlink privileges. Replaced loose commands are saved in
`~/.local/share/mevanlc-codex/backups/`. Old releases are retained for recovery;
the installer never prunes them automatically or changes your configuration.

Every archive contains `codex-package.json`, `bin/codex`,
`bin/codex-code-mode-host`, `codex-path/rg`, and the applicable sandbox resources
(with `.exe` suffixes on Windows). Android ripgrep is built for Android without
Termux library dependencies; Android does not include Linux bwrap. Keep this
whole directory together: extracting just the binaries loses required resources.

- Android: `codex-aarch64-linux-android.tar.zst`
- Linux: `codex-{x86_64,aarch64}-unknown-linux-musl.tar.zst`
- macOS: `codex-{x86_64,aarch64}-apple-darwin.tar.zst`
- Windows: `codex-{x86_64,aarch64}-pc-windows-msvc.zip`

Each build also uploads one Actions artifact named `codex-<target>`, containing
that archive. Pass `--artifact` to install the newest non-expired artifact (it
needs `GH_TOKEN` or `gh auth login`), `--list-available` to list platform
releases, `--version <tag-or-version>` to pin an exact release (including
downgrades), `--archive <path>` to pin a downloaded archive, or `--force` to
reinstall. The default chooses the newest published non-prerelease carrying your
platform's package; other platforms may still be building. Downloads must match
the SHA-256 digest GitHub publishes for them, and incomplete and loose-binary
archives are rejected before changing the installation. Release metadata is read
without credentials; set `GH_TOKEN` if you hit GitHub's anonymous rate limit.

### Embedded server or managed daemon

The embedded app server is the default on **every platform**, including Android.
To opt into the shared background daemon, put this in `config.toml`:

```toml
[features]
daemon_auto_start = true
```

`false` always uses the embedded server, even if a daemon is running. For a single
invocation use `--enable daemon_auto_start`, `--disable daemon_auto_start`, or
`--no-daemon` (which forces embedded mode). These choices also apply to `resume`
and `fork`. Explicit remote connections and daemon/agents commands keep their
own behavior. MCP, OSS, and execution-environment exclusions still apply.

The first opted-in launch copies the complete CLI package to the daemon's own
release directory. CLI and daemon selections are independent. Fork daemon state,
logs, PIDs, locks, recovery files, and default sockets live under
`$CODEX_HOME/mevanlc/` (normally `~/.codex/mevanlc/`), separate from OpenAI's
installation and daemon. On Android the daemon can share loaded sessions across
clients, but Android may stop background processes; embedded mode needs fewer
persistent processes.

`codex update` updates the CLI from **mevanlc/codex**. Daemon updates also use the
fork installer and retain their own auto-update preference. Explicit version
pins do not auto-update; `codex app-server daemon update` explicitly returns the
daemon to the latest fork channel. `codex app-server daemon update --from-cli
--yes` selects and pins this CLI's package instead. Only daemon updates restart
the daemon. No fork update path installs OpenAI releases, npm packages, or Brew
casks. A separate upstream-version notification is deferred.

Or [build from source](#building).

## Fork-only settings and behavior

Store fork-only settings in **`$CODEX_HOME/config-overlay.toml`** (normally
`~/.codex/config-overlay.toml`). This optional file is read automatically by the
fork; without it, all fork defaults apply. Keep shared settings in `config.toml`
so official clients can read that file without encountering fork extensions.

The overlay accepts only `tui.primary_accent`, `tui.chatbox_placeholder_tips`,
`tui.file_mentions_preserve_at`, `tui.file_mentions_allow_explicit_paths`, and
`tui.keymap.global.open_external_editor_with_quote`. On startup, the fork
automatically moves these entries from the top level of `config.toml` into the
overlay, preserving existing overlay values when both files set the same key.
It saves the overlay before removing the original entries, so interrupted
migrations can be retried safely. With no entries to move, no files are created.
Profile/project entries still require manual relocation because the overlay is
global. The examples below belong in the overlay.

Fork-setting saves create the overlay when needed; standard saves still use the
shared config or selected profile. Command-line `-c` overrides remain available.
The corresponding schemas are `codex-rs/core/config.schema.json` and
`codex-rs/core/config-overlay.schema.json`.

Config-write RPCs infer the destination from the edited properties when no file
is specified. Each request edits one file; send shared and fork changes in
separate requests and use the version reported for the destination file.

To customize the quoted-response editor shortcut (default Meta+G, written `alt-g`):

```toml
[tui.keymap.global]
open_external_editor_with_quote = "alt-e"
```

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

### Shell invocation modes

While composing a `!` shell command, press Tab to cycle **normal → follow-up → background → normal**, then Enter to run it. The selected mode applies only to the current command and requires no configuration.

Normal runs the command as usual. Follow-up starts an agent turn after the command exits; the agent receives the result and is instructed to investigate and fix failures, or explain the result and relevant next steps.

Background launches the command into the same terminal manager used by agent commands and immediately returns control. `/ps` lists it with a `user` marker; `/stop` stops it along with other managed terminals. The agent can poll its output or send stdin through the normal terminal permission checks. User commands retain `!`'s full access and local-host shell behavior. Background commands have no overall timeout and remain managed until they exit, are stopped, or the backend cleans up the session; completing an agent turn does not stop them. Starting or completing one does not automatically start an agent turn.

### Disable all MCPs for an invocation

```shell
codex --x-no-mcp
codex exec --x-no-mcp 'Explain this project'
codex resume --last --x-no-mcp
```

`--x-no-mcp` sets every registered MCP server's effective `enabled` value to
`false` and forces `orchestrator.mcp.enabled = false`. This includes servers from
config, plugins, extensions, and selected execution environments, including
registrations added during config reloads. It leaves saved configuration untouched.
Interactive launches use an embedded backend; the flag cannot be combined with
`--remote` or `codex agents`.

### Retracting a pending steer

Upstream binds `Alt+Up` (or `Shift+Left`) to "edit the most recently queued message", and it only reaches messages still queued locally. This fork extends the same binding to _steers_ — messages already handed to an in-flight turn but not yet consumed — and pops the steer back into the composer for editing. The pending-input preview now shows the edit hint whenever anything is retractable, not just for locally queued messages. If the turn consumes the message first, Codex warns that it was already submitted and can no longer be edited.

The underlying mechanism is an experimental `turn/retract` app-server request. Clients with `capabilities.experimentalApi` can retract a pending steer using its `clientUserMessageId` and the exact active turn ID:

```json
{ "method": "turn/retract", "id": 33, "params": {
    "threadId": "thr_123",
    "expectedTurnId": "turn_456",
    "clientUserMessageId": "client_msg_124"
} }
{ "id": 33, "result": { "status": "retracted" } }
```

The response status is `notPending` if the input was already consumed, was already retracted, or does not match the active turn. A steer with additional context or Responses API client metadata returns `notRetractable` because those side effects are applied when the steer is accepted.

### Reasoning shortcuts reach Max and Ultra

`Alt+.` / `Shift+Up` and `Alt+,` / `Shift+Down` step the active model's reasoning effort. Upstream now allows these shortcuts to reach Max, but still requires `/model → … → More reasoning…` for Ultra. This fork walks the full list of efforts the model advertises, with Max and Ultra last. Plan mode's Ultra concurrency warning still applies.

### Brief statusline components

In `/statusline`, Space cycles supported rows through `[ ]` off, `[x]` full, and
`[b]` brief. Other rows toggle on/off. Brief choices keep their position and save
as independent IDs in the shared `[tui].status_line` list in `config.toml`:

| Brief ID | Display change |
| --- | --- |
| `model-brief` | Remove a leading `GPT-`, `GPT_`, or `GPT `, ignoring case |
| `reasoning-brief` | Show the first two characters of the effort label |
| `model-with-reasoning-brief` | Apply both changes above; keep the processing tier |
| `context-remaining-brief` | `Context 75% left` becomes `ctx < 75%` |
| `context-used-brief` | `Context 25% used` becomes `ctx > 25%` |
| `five-hour-limit-brief` | `5h 65% left` becomes `5h < 65%` |
| `weekly-limit-brief` | `weekly 82% left` becomes `7d < 82%` |
| `hostname-brief` | Remove only the last dotted suffix: `host.example.com` becomes `host.example` |

Brief usage limits keep their actual window labels when the service reports a
different window, such as `monthly < 65%`.

The IDs also work in `/statusline` arguments and presets. If a manually edited
list includes both forms, both render; opening and saving the picker keeps only
the first occurrence of each pair, using its form and position. Cancelling leaves
the list unchanged.

### Platform fixes

- Shell detection prefers `$SHELL` over the `passwd` entry on Android, where Termux's shell is not in `/etc/passwd`.
- OpenSSL is vendored for `aarch64-linux-android`.
- Fork update checks use platform availability and publication time, including `X.Y.Z-<sha>` snapshots; commit hashes are identities, not version order.

## Building

Build this checkout and install it into the managed
`~/.local/share/mevanlc-codex/` layout:

```shell
just build-and-install-fork          # lite profile (default)
just build-and-install-fork release  # optimized
```

The command builds a complete package, including `codex-code-mode-host` and
platform resources, then runs the fork installer on it. Commands in
`~/.local/bin` select the new CLI package; previous releases are retained. The
staged package and archive are replaced under `dist/codex-local` and
`dist/codex-local.zip` on each run. Local archive installs are pinned against
automatic updates. This uses the version in `codex-rs/Cargo.toml` without
modifying it.

For loose-binary installs, `codex-rs/scripts/build-and-install.sh` builds
`codex` and `codex-code-mode-host` and copies both into `~/.local/bin`:

```shell
git clone https://github.com/mevanlc/codex.git
cd codex
./codex-rs/scripts/build-and-install.sh               # lite profile: fast, unoptimized
./codex-rs/scripts/build-and-install.sh -p release    # optimized
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
