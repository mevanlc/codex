use super::*;
use crate::legacy_core::config::ConfigBuilder;
use crate::legacy_core::config::edit::ConfigEditsBuilder;
use codex_config::LoaderOverrides;
use codex_config::types::SessionPickerViewMode;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn local_load_preserves_defaults_and_resolved_overrides() -> anyhow::Result<()> {
    for config_text in [
        "",
        r#"
[tui]
animations = false
whimsy = false
show_tooltips = false
auto_recap = false
vim_mode_default = true
terminal_resize_reflow_max_rows = 0
session_picker_view = "comfortable"
[history]
persistence = "none"
max_bytes = 4096
[notice]
fast_default_opt_out = true
"#,
    ] {
        let home = tempfile::tempdir()?;
        std::fs::write(home.path().join("config.toml"), config_text)?;
        if !config_text.is_empty() {
            std::fs::write(
                home.path().join(codex_config::CONFIG_OVERLAY_FILE),
                r#"[tui]
chatbox_placeholder_tips = "off"
file_mentions_preserve_at = true
file_mentions_allow_explicit_paths = false
primary_accent = "3,4,5"
"#,
            )?;
        }
        let config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .loader_overrides(LoaderOverrides {
                ignore_project_config: true,
                ..LoaderOverrides::without_managed_config_for_tests()
            })
            .cli_overrides(vec![("tui.disable_paste_burst".into(), true.into())])
            .build()
            .await?;
        let local = LocalSettings::from(&config);
        let mut expected: Tui = toml::from_str("")?;
        expected.disable_paste_burst = Some(true);
        expected.session_picker_view = Some(SessionPickerViewMode::Dense);
        if !config_text.is_empty() {
            expected.animations = false;
            expected.whimsy = false;
            expected.show_tooltips = false;
            expected.auto_recap = false;
            expected.vim_mode_default = true;
            expected.terminal_resize_reflow_max_rows = Some(0);
            expected.session_picker_view = Some(SessionPickerViewMode::Comfortable);
            expected.fork = codex_config::types::ForkTuiOptions {
                chatbox_placeholder_tips: codex_config::types::ChatboxPlaceholderTips::Off,
                file_mentions_preserve_at: true,
                file_mentions_allow_explicit_paths: false,
                primary_accent: Some("3,4,5".to_string()),
            };
        }
        assert_eq!(local.tui, expected);
        assert_eq!(
            local.terminal_resize_reflow(),
            config.terminal_resize_reflow
        );
        assert_eq!(
            (&local.history, &local.notices),
            (&config.history, &config.notices)
        );
    }
    Ok(())
}

#[tokio::test]
async fn local_writes_preserve_selected_user_file_and_home_destinations() -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    let selected = AbsolutePathBuf::from_absolute_path(home.path().join("work.config.toml"))?;
    std::fs::write(&selected, "[tui]\ntheme = \"dracula\"\n")?;
    let overrides = LoaderOverrides {
        user_config_path: Some(selected.clone()),
        user_config_profile: Some("work".parse()?),
        ignore_project_config: true,
        ..LoaderOverrides::without_managed_config_for_tests()
    };
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(overrides.clone())
        .build()
        .await?;
    let local = LocalSettings::from(&config);
    assert_eq!(local.user_config_path, selected);
    ConfigEditsBuilder::for_config_path(local.user_config_path.as_path())
        .with_edits([crate::legacy_core::config::edit::syntax_theme_edit("nord")])
        .apply()
        .await?;
    ConfigEditsBuilder::new(local.codex_home.as_path())
        .set_session_picker_view(SessionPickerViewMode::Comfortable)
        .apply()
        .await?;
    let reloaded = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(overrides)
        .build()
        .await?;
    assert_eq!(
        LocalSettings::from(&reloaded).tui.theme.as_deref(),
        Some("nord")
    );
    let home_config: toml::Value =
        toml::from_str(&std::fs::read_to_string(home.path().join("config.toml"))?)?;
    assert_eq!(
        home_config["tui"]["session_picker_view"].as_str(),
        Some("comfortable")
    );
    assert_eq!(home_config["tui"].get("theme"), None);
    let overlay = local.keymap_config_path("global", "open_external_editor_with_quote");
    let clear = crate::legacy_core::config::edit::keymap_binding_clear_edit(
        "global",
        "open_external_editor_with_quote",
    );
    ConfigEditsBuilder::for_config_path(overlay.as_path())
        .with_edits([clear.clone()])
        .apply()
        .await?;
    assert!(!overlay.exists());
    ConfigEditsBuilder::for_config_path(overlay.as_path())
        .with_edits([crate::legacy_core::config::edit::keymap_binding_edit(
            "global",
            "open_external_editor_with_quote",
            "ctrl-x g",
        )])
        .apply()
        .await?;
    assert_eq!(
        std::fs::read_to_string(&selected)?,
        "[tui]\ntheme = \"nord\"\n"
    );
    let reloaded = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(LoaderOverrides {
            ignore_project_config: true,
            ..LoaderOverrides::without_managed_config_for_tests()
        })
        .build()
        .await?;
    assert_eq!(
        reloaded.tui_keymap.global.open_external_editor_with_quote,
        Some(
            toml::from_str::<codex_config::types::Tui>(
                "[keymap.global]\nopen_external_editor_with_quote = 'ctrl-x g'"
            )?
            .keymap
            .global
            .open_external_editor_with_quote
            .unwrap()
        )
    );
    ConfigEditsBuilder::for_config_path(overlay.as_path())
        .with_edits([clear])
        .apply()
        .await?;
    let cleared: toml::Value = toml::from_str(&std::fs::read_to_string(overlay)?)?;
    let cleared: codex_config::config_toml::ConfigToml = cleared.try_into()?;
    assert_eq!(
        cleared.tui.unwrap_or_default().keymap,
        codex_config::types::TuiKeymap::default()
    );
    Ok(())
}

#[test]
fn misplaced_fork_setting_diagnostic() {
    let value = toml::from_str("[tui]\nprimary_accent = '14'").unwrap();
    insta::assert_snapshot!(
        codex_config::ConfigFileKind::Shared
            .validate(&value)
            .unwrap_err()
    );
}
