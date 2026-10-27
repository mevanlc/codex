//! Shell-mode state and presentation shared by the composer and input queue.

use ratatui::style::Stylize;
use ratatui::text::Line;

/// Prompt submitted automatically after an armed user shell command finishes.
pub(crate) const SHELL_FOLLOW_UP_PROMPT: &str = "Review the shell command result above. If it failed, investigate the failure(s). If the fixes are compatible with any work in progress and do not conflict with the goals of the session, then fix the failures; otherwise, just explain the results and any relevant next steps.";

/// Invocation selected for this shell draft. Tab advances only on key presses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ShellInvocationMode {
    #[default]
    Normal,
    FollowUp,
    Background,
}

impl ShellInvocationMode {
    pub(super) fn cycle(&mut self) {
        *self = match self {
            Self::Normal => Self::FollowUp,
            Self::FollowUp => Self::Background,
            Self::Background => Self::Normal,
        };
    }

    pub(crate) fn requests_follow_up(self) -> bool {
        self == Self::FollowUp
    }

    pub(super) fn footer_line(self) -> Line<'static> {
        let state = match self {
            Self::Normal => "normal".dim(),
            Self::FollowUp => "follow-up".green(),
            Self::Background => "background".green(),
        };
        vec![
            "Shell mode".light_red(),
            " · ".dim(),
            state,
            " · ".dim(),
            "Tab".cyan(),
            " to change mode".dim(),
        ]
        .into()
    }
}
