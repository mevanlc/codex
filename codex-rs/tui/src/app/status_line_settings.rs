//! Save status line preferences to the local user configuration.

use super::App;
use crate::app_event::AppEvent;
use crate::bottom_pane::StatusLineItem;
use crate::config_update::format_config_error;
use crate::legacy_core::config::edit::ConfigEdit;
use crate::legacy_core::config::edit::ConfigEditsBuilder;
use crate::legacy_core::config::edit::save_status_line_config;
use crate::statusline_provider::StatuslineProvider;
use anyhow::Context;
use codex_config::types::BuiltinStatusLine;
use codex_config::types::StatusLineSelection;
use codex_config::types::StatusLineSelection::Builtin;
use codex_config::types::StatusLineSelection::Mcp;
use std::collections::BTreeSet;

impl App {
    pub(super) async fn save_status_line_settings(
        &mut self,
        items: Vec<StatusLineItem>,
        use_theme_colors: bool,
        truncatable: BTreeSet<StatusLineItem>,
    ) {
        let ids = items.iter().map(ToString::to_string).collect::<Vec<_>>();
        let truncatable = truncatable
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let result = save_status_line_config(
            self.local_settings.codex_home.as_path(),
            self.local_settings.user_config_path.as_path(),
            &ids,
            &truncatable,
            use_theme_colors,
        )
        .await;
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(err) => {
                self.report_status_line_save_error("Failed to save status line settings", &err);
                return;
            }
        };

        let saved_colors = outcome
            .profile_colors
            .as_ref()
            .unwrap_or(&outcome.mirror)
            .is_ok()
            .then_some(use_theme_colors);
        self.local_settings.tui.status_line = Some(ids.clone());
        self.local_settings.tui.fork.status_line_truncatable = truncatable.clone();
        self.chat_widget.set_status_line_truncatable(truncatable);
        if let Some(colors) = saved_colors {
            self.local_settings.tui.status_line_use_colors = colors;
        }
        self.chat_widget.apply_saved_status_line(ids, saved_colors);
        self.chat_widget.refresh_config_panel(/*notice*/ None);
        if let Err(err) = outcome.mirror {
            self.report_status_line_save_error(
                "Saved fork status line. Failed to update the shared status line",
                &err,
            );
        }
        if let Some(Err(err)) = outcome.profile_colors {
            self.report_status_line_save_error(
                "Saved fork status line. Failed to save status line colors",
                &err,
            );
        }
    }

    fn report_status_line_save_error(&mut self, message: &str, err: &anyhow::Error) {
        let error = format_config_error(err);
        tracing::error!(%error, "{message}");
        self.app_event_tx.send(AppEvent::FollowTranscript);
        self.chat_widget
            .report_config_save_error(format!("{message}: {error}"));
    }

    pub(super) async fn save_status_line_provider(
        &mut self,
        provider: StatusLineSelection,
        provider_from_picker: Option<StatuslineProvider>,
    ) -> anyhow::Result<()> {
        let value = match &provider {
            Builtin(BuiltinStatusLine::Default) => toml_edit::value("default"),
            Mcp(id) => {
                let table = toml_edit::InlineTable::from_iter([
                    ("server", id.server.as_str()),
                    ("tool", id.tool.as_str()),
                ]);
                toml_edit::value(table)
            }
        };
        ConfigEditsBuilder::for_config_path(self.local_settings.user_config_path.as_path())
            .with_edits([ConfigEdit::SetPath {
                segments: vec!["tui".into(), "status_line_provider".into()],
                value,
            }])
            .apply()
            .await
            .context("Failed to save status line provider")?;

        let previous = self
            .local_settings
            .tui
            .status_line_provider
            .as_deref()
            .unwrap_or(&Builtin(BuiltinStatusLine::Default));
        let change = match (previous, &provider) {
            (Builtin(_), Mcp(_)) => Some("enabled_custom"),
            (Mcp(_), Builtin(_)) => Some("returned_to_default"),
            (Mcp(old), Mcp(new)) if old != new => Some("switched_custom"),
            (Builtin(_), Builtin(_)) | (Mcp(_), Mcp(_)) => None,
        };
        if let Some(change) = change {
            self.statusline_metrics.record(
                &self.config,
                "codex.statusline.provider_changed",
                &[("change", change)],
            );
        }
        self.local_settings.tui.status_line_provider = Some(Box::new(provider.clone()));
        self.chat_widget.local_settings.tui.status_line_provider = Some(Box::new(provider));
        self.chat_widget
            .reselect_statusline_provider(provider_from_picker);
        Ok(())
    }
}

#[cfg(test)]
#[path = "status_line_settings_tests.rs"]
mod tests;
