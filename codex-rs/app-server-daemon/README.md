# codex-app-server-daemon

> `codex-app-server-daemon` is experimental and its lifecycle contract may
> change while the remote-management flow is still being developed.

`codex-app-server-daemon` backs the machine-readable `codex app-server`
lifecycle commands used by remote clients such as the desktop and mobile apps.
It is intended for Codex instances launched over SSH, including fresh developer
machines that should expose app-server with `remote_control` enabled.

## Platform support

The fork daemon supports Android/Termux, Linux, macOS, and Windows using platform-specific process
and file-locking primitives. Windows startup requires a non-elevated terminal
whose host permits detached child processes.

Windows automatic attachment requires the canonical socket address to fit the
108-byte AF_UNIX limit (including its terminator). A short junction alias whose
resolved address exceeds that limit falls back to the embedded server. Use a
shorter `CODEX_HOME` to share the daemon; discovery does not trust a mutable alias.

Shared clients use the environment inherited when the daemon started. Opening a
new terminal or clearing variables there does not clear the running daemon's
environment; per-client environment isolation is not provided.
An invocation that sets `CODEX_EXEC_SERVER_URL` skips implicit daemon attachment
so its executor selection is preserved. If an implicitly discovered daemon cannot
initialize the connection, the TUI starts an embedded server instead. Explicit
`--remote` endpoints remain authoritative and report connection failures.

## Commands

```sh
codex app-server daemon start
codex app-server daemon restart
codex app-server daemon update
codex app-server daemon enable-remote-control
codex app-server daemon disable-remote-control
codex app-server daemon stop
codex app-server daemon version
codex app-server daemon bootstrap --remote-control
```

On success, every command writes exactly one JSON object to stdout. Consumers
should parse that JSON rather than relying on human-readable text. Lifecycle
responses report the resolved backend, socket path, local CLI version, and
running app-server version when applicable.

Eligible managed daemons check for updates after five minutes, then hourly by
default. Edit `$CODEX_HOME/mevanlc/app-server-daemon/settings.json` to change this:

```json
{
  "remoteControlEnabled": false,
  "shutdownGraceSeconds": 60,
  "updater": { "autoUpdateEnabled": false, "updateIntervalMinutes": 120 }
}
```

Positive minute intervals have no configured cap. `daemon restart` applies the
enabled state; the next updater wait reads a new interval. The preference does
not affect an explicit `codex update` command or `daemon update`.

`daemon update` selects the newest published fork release carrying this platform's
package, including fork snapshots, even with automatic updates disabled. It
returns pinned or local managed packages to latest-channel eligibility while
preserving the automatic-update preference. JSON reports `updated`, `noUpdate`,
or `unsupported`, with installed and running versions. A running daemon restarts,
so active or queued work may be interrupted; a stopped daemon stays stopped.
Installer errors return nonzero. The updater uses saved network settings; CLI
`-c` overrides do not reach it. Every update uses **mevanlc/codex**.

For all managed app-server shutdowns, including explicit stop and restart and
updater-triggered restarts, `shutdownGraceSeconds` defaults to 60 and accepts
an integer from 0 through 300. Zero forces shutdown immediately after requesting
a graceful exit; the five-minute maximum bounds the wait even if a turn is still
running.

## Bootstrap flow

Install the fork's complete CLI package with `codex-fork-installer` (see the
[repository README](../../README.md#install)), then run:

```sh
codex app-server daemon bootstrap --remote-control
```

On Windows, run the lifecycle command in a non-elevated terminal whose host
permits detached children. The updater runs the newest release's
`codex-fork-installer` directly and needs no Bash.

If no daemon package is selected, `bootstrap` copies the invoking complete CLI
package into `~/.local/share/mevanlc-codex/packages/app-server-daemon` without
asking for confirmation. Existing fork selections are reused; broken selections
require repair. A bare executable cannot supply a new installation. Upstream
packages and daemon state are never adopted or migrated.

Settings, logs, PIDs, locks and recovery files use
`$CODEX_HOME/mevanlc/app-server-daemon/`. Default control sockets and startup
locks use `$CODEX_HOME/mevanlc/app-server-control/`. The detached updater runs
only when automatic updates are enabled and the selected package follows the
fork's latest channel. Explicit pins remain pinned.

On Unix the advertised socket is an alias into a fixed private socket directory.
Android uses `/data/data/com.termux/files/mc-daemon/` because Termux cannot write
`/tmp`; the physical socket names remain short enough for Unix socket addresses.

Ordinary interactive launches use the embedded server by default. Set
`[features].daemon_auto_start = true` or use `--enable daemon_auto_start` to opt
in; `false`, `--disable daemon_auto_start`, and `--no-daemon` force embedded mode.
Explicit daemon commands and `codex agents` request the shared server directly.
Android can stop background processes, so embedded mode is often preferable there.

## Installation and update cases

The selected executable is
`~/.local/share/mevanlc-codex/packages/app-server-daemon/current/bin/codex`
(`codex.exe` on Windows). The package includes its helpers. All OSs use this same
home-relative layout; `CODEX_HOME` and `XDG_DATA_HOME` do not redirect packages.
CLI packages use a separate `packages/cli/current` selection. Daemon-only
installer updates leave the CLI command and shell setup alone. Old releases
remain available for recovery; neither installer nor daemon prunes them.

| Situation                                                                                       | What starts                                                            | Does this daemon fetch new binaries?                                      | Does a running app-server eventually move to a newer binary on its own?                                                                       |
| ----------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- | ------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| Latest-channel installer has run; `start` or `bootstrap` is used with automatic updates enabled | Managed binary and detached updater when supported                     | When supported, the platform's installer runs on the configured cadence.  | When supported, the running server restarts with the new binary before the updater replaces itself.                                           |
| Installer selected an explicit release; `bootstrap` is used                                     | Managed binary only                                                    | No; the selected release stays pinned.                                    | No; an explicit restart uses the selected binary.                                                                                             |
| Another tool updates the managed binary                                                         | A fresh start or explicit restart uses it; a running server is reused. | Yes, when a latest-channel updater is running, on the configured cadence. | An updater that was running through the change compares binary contents on its next successful installer pass and refreshes the server first. |

### Managed packages

For fork daemon installations:

- lifecycle commands use the selected daemon package, regardless of the invoking
  CLI version; they do not implicitly replace an existing package
- `bootstrap` is supported
- managed `start`, `restart`, and `bootstrap` ensure a single detached pid-backed
  updater loop only when automatic updates are enabled for a latest-channel
  release whose managed binary supports the updater command
- the installer records the latest-channel selection alongside `current`;
  selecting an explicit release clears it, even if that version is currently
  latest. The updater checks the selection again while holding the install lock
  so an in-flight update cannot override a new pin
- after a successful refresh, if app-server is running and the managed binary
  contents changed, the updater restarts app-server with that binary first and
  only then replaces its own process image
- the updater loop is not reboot-persistent; a managed start after reboot
  starts it again

### Out-of-band updates

This daemon does not watch arbitrary executable files for replacement. If some
other tool updates the managed binary path:

- an updater that was already running notices a changed managed
  binary on its next successful scheduled installer pass; if
  app-server is running, it refreshes app-server first and then refreshes itself
  once that replacement starts successfully
- if the updater was absent during a same-version binary replacement, a later
  managed start recovers it but cannot infer the running server's previous
  executable identity; use `codex app-server daemon restart` to refresh the server

## Lifecycle semantics

`start` is idempotent and returns after app-server is ready to answer the normal
JSON-RPC initialize handshake on the Unix control socket.

`restart` stops any managed daemon and starts it again.

`enable-remote-control` and `disable-remote-control` persist the launch setting
for future starts. If a managed app-server is already running, they restart it
so the new setting takes effect immediately.

Top-level `codex remote-control start` enables and persists remote control for
the managed daemon, overriding a saved disabled value. It starts or bootstraps
the daemon as needed. Plain `codex remote-control` uses the managed daemon when
eligible; `codex remote-control --no-daemon` runs a separate foreground server.
`codex remote-control stop` stops the managed daemon without clearing its saved
remote-control preference.
`daemon start` and `daemon restart` use that saved preference. `daemon bootstrap`
sets it according to `--remote-control` (disabled when omitted).

`stop` sends a graceful termination request first, then force-terminates the
process after the configured grace window if it is still alive.

All mutating lifecycle commands are serialized per `CODEX_HOME`, so a concurrent
`start`, `restart`, `enable-remote-control`, `disable-remote-control`, `stop`,
or `bootstrap` does not race another in-flight lifecycle operation.

## State

The daemon stores its local state under `CODEX_HOME/app-server-daemon/`:

- `settings.json` for remote-control launch settings and updater preferences
- `app-server.pid` for the app-server process record
- `app-server-updater.pid` for the pid-backed standalone updater loop
- `daemon.lock` for daemon-wide lifecycle serialization
