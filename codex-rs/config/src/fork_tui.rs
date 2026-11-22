//! Fork-owned TUI configuration, stored under `[tui]` in `config-overlay.toml`.
//!
//! Keeping these options together limits changes to upstream config declarations while
//! preserving their user-facing keys. The same value is carried into the resolved config.

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum ChatboxPlaceholderTips {
    #[default]
    On,
    Off,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum PassiveHints {
    Never,
    #[default]
    Once,
    Auto,
    Always,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ForkTuiOptions {
    /// Controls whether the chatbox placeholder shows a rotating set of tip prompts.
    ///
    /// - `on` (default): Show rotating placeholder tips.
    /// - `off`: Use a generic placeholder instead.
    #[serde(default)]
    pub chatbox_placeholder_tips: ChatboxPlaceholderTips,

    /// Visibility of the agents and shortcuts hints below the composer.
    ///
    /// - `never`: Hide the hints.
    /// - `once` (default): Show at startup until first hidden, then keep hidden for the session.
    /// - `auto`: Show whenever the composer is empty (the original behavior).
    /// - `always`: Also show while the composer contains a draft.
    #[serde(default)]
    pub passive_hints: PassiveHints,

    /// Keep the leading `@` when a file-search completion is inserted into the composer.
    /// Defaults to `false`.
    #[serde(default)]
    pub file_mentions_preserve_at: bool,

    /// Let `@` file search resolve absolute paths and paths beginning with `./` or `../`.
    /// Defaults to `true`.
    #[serde(default = "default_true")]
    pub file_mentions_allow_explicit_paths: bool,

    /// Overrides the default cyan accent used throughout the TUI.
    ///
    /// Supported formats:
    /// - `r,g,b` where each channel is `0..255` (for example `0,170,255`)
    /// - `#RRGGBB` (for example `#00AAFF`)
    /// - `0..255` ANSI palette index (for example `14`)
    #[serde(default)]
    pub primary_accent: Option<String>,

    /// Built-in status line items that may be shortened or dropped to fit the terminal.
    /// Use the same identifier for the full and brief forms, even for disabled items.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub status_line_truncatable: Vec<String>,
}

impl Default for ForkTuiOptions {
    fn default() -> Self {
        Self {
            chatbox_placeholder_tips: ChatboxPlaceholderTips::default(),
            passive_hints: PassiveHints::default(),
            file_mentions_preserve_at: false,
            file_mentions_allow_explicit_paths: true,
            primary_accent: None,
            status_line_truncatable: Vec::new(),
        }
    }
}

const fn default_true() -> bool {
    true
}

#[cfg(test)]
#[path = "fork_tui_tests.rs"]
mod tests;
