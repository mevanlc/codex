//! Shell-mode state and presentation shared by the composer and input queue.

use ratatui::style::Stylize;
use ratatui::text::Line;

/// Prompt submitted automatically after an armed user shell command finishes.
pub(crate) const SHELL_FOLLOW_UP_PROMPT: &str = "Review the shell command result above. If it failed, investigate the failure(s). If the fixes are compatible with any work in progress and do not conflict with the spirit of the session, then fix the failures; otherwise, just explain the results and any relevant next steps.";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ShellFollowUp {
    #[default]
    Disabled,
    Enabled,
}

impl ShellFollowUp {
    pub(super) fn toggle(&mut self) {
        *self = match self {
            Self::Disabled => Self::Enabled,
            Self::Enabled => Self::Disabled,
        };
    }

    pub(crate) fn is_enabled(self) -> bool {
        self == Self::Enabled
    }

    pub(super) fn footer_line(self) -> Line<'static> {
        let state = match self {
            Self::Disabled => "off".dim(),
            Self::Enabled => "on".green(),
        };
        vec![
            "Shell mode".light_red(),
            " · ".dim(),
            "Tab".cyan(),
            " follow-up ".dim(),
            state,
        ]
        .into()
    }
}
