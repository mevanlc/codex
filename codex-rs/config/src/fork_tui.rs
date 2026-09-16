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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ForkTuiOptions {
    /// Controls whether the chatbox placeholder shows a rotating set of tip prompts.
    ///
    /// - `on` (default): Show rotating placeholder tips.
    /// - `off`: Use a generic placeholder instead.
    #[serde(default)]
    pub chatbox_placeholder_tips: ChatboxPlaceholderTips,

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
}

impl Default for ForkTuiOptions {
    fn default() -> Self {
        Self {
            chatbox_placeholder_tips: ChatboxPlaceholderTips::default(),
            file_mentions_preserve_at: false,
            file_mentions_allow_explicit_paths: true,
            primary_accent: None,
        }
    }
}

const fn default_true() -> bool {
    true
}

#[cfg(test)]
#[path = "fork_tui_tests.rs"]
mod tests;
