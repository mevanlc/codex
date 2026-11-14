# Configuration

For basic configuration instructions, see [this documentation](https://developers.openai.com/codex/config-basic).

For advanced configuration instructions, see [this documentation](https://developers.openai.com/codex/config-advanced).

For a full configuration reference, see [this documentation](https://developers.openai.com/codex/config-reference).

## Lifecycle hooks

Admins can set top-level `allow_managed_hooks_only = true` in
`requirements.toml` to ignore user, project, and session hook configs while
still allowing managed hooks from requirements and managed config layers. This
setting is only supported in `requirements.toml`; putting it in `config.toml`
does not enable managed-hooks-only mode.

## TUI

Fork-only TUI settings belong in `$CODEX_HOME/config-overlay.toml`.

To control the `← for agents · ? for shortcuts` composer hints, set:

```toml
[tui]
passive_hints = "once"
```

`never` hides them; `once` (default) shows them at startup until they first
disappear, then keeps them hidden across conversations until the CLI restarts;
`auto` shows them whenever the composer is empty; `always` also shows them while
typing. The shortcuts, status indicators, warnings, and shortcut help remain available.

To disable the rotating "tip" placeholders in the chatbox, set:

```toml
[tui]
chatbox_placeholder_tips = "off"
```

To override the default cyan accent used across TUI highlights, set:

```toml
[tui]
primary_accent = "#00AAFF" # r,g,b | #RRGGBB | 0..255
```
