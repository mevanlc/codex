#![cfg(test)]

use crate::AppServerTarget;
use crate::app::test_support::make_test_app;
use crate::bottom_pane::StatusLineItem;
use crate::chatwidget::tests::render_bottom_popup;
use crate::statusline_metrics::tests::Collector;
use codex_config::types::BuiltinStatusLine;
use codex_config::types::StatusLineProviderId;
use codex_config::types::StatusLineSelection;
use codex_utils_absolute_path::AbsolutePathBuf;
use crossterm::event::KeyCode;
use pretty_assertions::assert_eq;
use std::collections::BTreeSet;

#[tokio::test]
async fn status_line_overlay_write_failure_keeps_the_previous_selection() -> anyhow::Result<()> {
    let mut app = make_test_app().await;
    let shared = app.local_settings.user_config_path.clone();
    let shared_initial = "tui.status_line = ['current-dir']\n";
    std::fs::write(&shared, shared_initial)?;
    let overlay = app
        .local_settings
        .codex_home
        .join(codex_config::CONFIG_OVERLAY_FILE);
    let initial =
        "[tui]\nstatus_line = ['hostname-brief']\nstatus_line_truncatable = ['current-dir']\n";
    std::fs::write(&overlay, initial)?;
    app.refresh_in_memory_config_from_disk().await.unwrap();
    let broken = "[broken";
    std::fs::write(&overlay, broken)?;

    app.save_status_line_settings(vec![StatusLineItem::ModelName], false, BTreeSet::new())
        .await;
    assert_eq!(std::fs::read_to_string(&overlay)?, broken);
    assert_eq!(std::fs::read_to_string(&shared)?, shared_initial);
    for settings in [&app.local_settings, &app.chat_widget.local_settings] {
        assert_eq!(
            settings.tui.status_line,
            Some(vec!["hostname-brief".into()])
        );
        assert!(settings.tui.status_line_use_colors);
        assert_eq!(settings.tui.fork.status_line_truncatable, ["current-dir"]);
    }

    std::fs::write(&overlay, initial)?;
    app.save_status_line_settings(vec![StatusLineItem::ModelName], false, BTreeSet::new())
        .await;
    app.refresh_in_memory_config_from_disk().await.unwrap();
    assert!(
        app.chat_widget
            .local_settings
            .tui
            .fork
            .status_line_truncatable
            .is_empty()
    );
    let parsed: toml::Table = toml::from_str(&std::fs::read_to_string(overlay)?)?;
    assert_eq!(
        parsed,
        toml::toml! {
            [tui]
            status_line = ["model"]
            status_line_truncatable = []
        },
    );
    Ok(())
}

#[tokio::test]
async fn status_line_mirror_failure_keeps_the_fork_save_and_can_be_retried() -> anyhow::Result<()> {
    let mut app = make_test_app().await;
    let shared = app.local_settings.user_config_path.clone();
    std::fs::write(&shared, "[broken")?;
    let items = vec![StatusLineItem::ModelNameBrief, StatusLineItem::CurrentDir];
    let truncatable = BTreeSet::from([StatusLineItem::CurrentDir]);
    app.save_status_line_settings(items.clone(), false, truncatable.clone())
        .await;

    assert_eq!(std::fs::read_to_string(&shared)?, "[broken");
    let overlay = app
        .local_settings
        .codex_home
        .join(codex_config::CONFIG_OVERLAY_FILE);
    let saved: toml::Table = toml::from_str(&std::fs::read_to_string(overlay)?)?;
    assert_eq!(
        saved,
        toml::toml! {
            [tui]
            status_line = ["model-brief", "current-dir"]
            status_line_truncatable = ["current-dir"]
        },
    );
    for settings in [&app.local_settings, &app.chat_widget.local_settings] {
        assert_eq!(
            settings.tui.status_line,
            Some(vec!["model-brief".into(), "current-dir".into()]),
        );
        assert!(settings.tui.status_line_use_colors);
        assert_eq!(settings.tui.fork.status_line_truncatable, ["current-dir"]);
    }

    std::fs::write(&shared, "tui.theme = 'nord'\n")?;
    app.save_status_line_settings(items, false, truncatable)
        .await;
    assert_eq!(
        toml::from_str::<toml::Table>(&std::fs::read_to_string(shared)?)?,
        toml::toml! {
            [tui]
            theme = "nord"
            status_line = ["model", "current-dir"]
            status_line_use_colors = false
        },
    );
    assert!(!app.chat_widget.local_settings.tui.status_line_use_colors);
    Ok(())
}

#[tokio::test]
async fn status_line_save_with_profile_mirrors_to_base_and_reloads_overlay() -> anyhow::Result<()> {
    let mut app = make_test_app().await;
    let base = app.local_settings.user_config_path.clone();
    let profile = app.local_settings.codex_home.join("work.config.toml");
    std::fs::write(&base, "tui.status_line = ['current-dir']\n")?;
    std::fs::write(&profile, "tui.status_line = ['thread-name']\n")?;
    app.loader_overrides.user_config_path = Some(profile.clone());
    app.loader_overrides.user_config_profile = Some("work".parse()?);
    app.refresh_in_memory_config_from_disk().await.unwrap();

    app.save_status_line_settings(
        vec![StatusLineItem::HostnameBrief, StatusLineItem::ModelName],
        false,
        BTreeSet::new(),
    )
    .await;
    app.refresh_in_memory_config_from_disk().await.unwrap();
    assert_eq!(
        app.chat_widget.local_settings.tui.status_line,
        Some(vec!["hostname-brief".into(), "model".into()]),
    );
    assert_eq!(
        toml::from_str::<toml::Table>(&std::fs::read_to_string(&base)?)?,
        toml::toml! { [tui] status_line = ["hostname", "model"] },
    );
    assert_eq!(
        toml::from_str::<toml::Table>(&std::fs::read_to_string(&profile)?)?,
        toml::toml! {
            [tui]
            status_line = ["thread-name"]
            status_line_use_colors = false
        },
    );

    app.save_status_line_settings(Vec::new(), false, BTreeSet::new())
        .await;
    app.refresh_in_memory_config_from_disk().await.unwrap();
    assert_eq!(
        app.chat_widget.local_settings.tui.status_line,
        Some(Vec::new())
    );
    assert_eq!(
        toml::from_str::<toml::Table>(&std::fs::read_to_string(&base)?)?,
        toml::toml! { [tui] status_line = [] },
    );

    let overlay = app
        .local_settings
        .codex_home
        .join(codex_config::CONFIG_OVERLAY_FILE);
    std::fs::write(&overlay, "tui.status_line = ['reasoning-brief']\n")?;
    app.refresh_in_memory_config_from_disk().await.unwrap();
    assert_eq!(
        app.chat_widget.local_settings.tui.status_line,
        Some(vec!["reasoning-brief".into()]),
    );
    assert_eq!(
        toml::from_str::<toml::Table>(&std::fs::read_to_string(&base)?)?,
        toml::toml! { [tui] status_line = [] },
    );

    app.chat_widget
        .restore_user_message_to_composer("/statusline --clear".into());
    app.chat_widget.handle_key_event(KeyCode::Enter.into());
    app.refresh_in_memory_config_from_disk().await.unwrap();
    assert_eq!(
        app.chat_widget.local_settings.tui.status_line,
        Some(Vec::new())
    );
    assert_eq!(
        app.chat_widget.config_panel_tui.status_line,
        Some(vec!["reasoning-brief".into()]),
    );
    Ok(())
}

#[tokio::test]
async fn remote_session_saves_to_the_active_local_profile() -> anyhow::Result<()> {
    let mut app = make_test_app().await;
    let base = app.local_settings.user_config_path.clone();
    let base_text = "[tui]\nstatus_line_provider = { server = 'personal', tool = 'usage' }\n";
    std::fs::write(&base, base_text)?;
    let profile = app.local_settings.codex_home.join("work.config.toml");
    std::fs::write(&profile, "[tui]\ntheme = 'nord'\n")?;
    app.loader_overrides.user_config_path = Some(profile.clone());
    app.loader_overrides.user_config_profile = Some("work".parse()?);

    app.cli_kv_overrides = vec![("tui.status_line_provider".into(), "default".into())];
    app.refresh_in_memory_config_from_disk()
        .await
        .expect("load the initial provider from -c");
    assert_eq!(
        app.local_settings.tui.status_line_provider.as_deref(),
        Some(&StatusLineSelection::Builtin(BuiltinStatusLine::Default)),
    );

    // The running thread's config and cwd belong to the remote app server.
    let remote = tempfile::tempdir()?;
    let remote_file = remote.path().join("config.toml");
    std::fs::write(&remote_file, "[tui]\ntheme = 'remote'\n")?;
    app.config.codex_home = AbsolutePathBuf::from_absolute_path(remote.path())?;
    app.config.cwd = app.config.codex_home.clone();
    app.app_server_target = AppServerTarget::Remote {
        endpoint: crate::resolve_remote_addr("ws://127.0.0.1:8765").expect("test endpoint"),
    };
    for (selection, expected) in [
        (
            StatusLineSelection::Builtin(BuiltinStatusLine::Default),
            toml::toml! {
                [tui]
                theme = "nord"
                status_line_provider = "default"
            },
        ),
        (
            StatusLineSelection::Mcp(StatusLineProviderId {
                server: "work".into(),
                tool: "usage".into(),
            }),
            toml::toml! {
                [tui]
                theme = "nord"
                status_line_provider = { server = "work", tool = "usage" }
            },
        ),
    ] {
        app.save_status_line_provider(selection.clone(), /*provider_from_picker*/ None)
            .await?;
        let saved: toml::Table = toml::from_str(&std::fs::read_to_string(&profile)?)?;
        assert_eq!(saved, expected);
        assert_eq!(
            (
                app.local_settings.tui.status_line_provider.as_deref(),
                app.chat_widget
                    .local_settings
                    .tui
                    .status_line_provider
                    .as_deref()
            ),
            (Some(&selection), Some(&selection)),
        );
    }
    app.refresh_in_memory_config_from_disk()
        .await
        .expect("reload the CLI override");
    let default = StatusLineSelection::Builtin(BuiltinStatusLine::Default);
    assert_eq!(
        (
            app.local_settings.tui.status_line_provider.as_deref(),
            app.chat_widget
                .local_settings
                .tui
                .status_line_provider
                .as_deref(),
        ),
        (Some(&default), Some(&default)),
    );
    assert_eq!(std::fs::read_to_string(base)?, base_text);
    assert_eq!(
        std::fs::read_to_string(remote_file)?,
        "[tui]\ntheme = 'remote'\n"
    );
    Ok(())
}

#[tokio::test]
async fn failed_write_preserves_the_file_and_the_live_selection() -> anyhow::Result<()> {
    let mut app = make_test_app().await;
    let collector = Collector::new();
    app.statusline_metrics = collector.metrics(&mut app.config);
    let path = app.local_settings.user_config_path.clone();
    for tool in ["usage", "other", "other"] {
        let selection = StatusLineSelection::Mcp(StatusLineProviderId {
            server: "work".into(),
            tool: tool.into(),
        });
        app.save_status_line_provider(selection, /*provider_from_picker*/ None)
            .await?;
    }
    let before = (
        app.local_settings.clone(),
        app.chat_widget.local_settings.clone(),
    );
    for contents in ["[malformed", "[[tui]]\ntheme = 'nord'\n"] {
        std::fs::write(&path, contents)?;
        let selection = StatusLineSelection::Builtin(BuiltinStatusLine::Default);
        assert!(
            app.save_status_line_provider(selection, /*provider_from_picker*/ None)
                .await
                .is_err()
        );
        assert_eq!(std::fs::read_to_string(&path)?, contents);
        assert_eq!(
            (&app.local_settings, &app.chat_widget.local_settings),
            (&before.0, &before.1)
        );
    }
    assert_eq!(
        collector.snapshot(),
        [
            "provider_changed change=enabled_custom 1",
            "provider_changed change=switched_custom 1"
        ]
    );
    Ok(())
}

#[tokio::test]
async fn reloading_the_local_provider_preserves_the_server_feature_gate() {
    let mut app = make_test_app().await;
    let path = app.local_settings.user_config_path.clone();
    let configured = "[features]\ntui_extensions_statusline = true\n[tui]\nstatus_line = []\nstatus_line_provider = { tool='line', server='fixture' } # local formatting\n";
    std::fs::write(&path, configured).unwrap();
    app.cli_kv_overrides = vec![(
        "tui.status_line_provider".into(),
        toml::toml! {
            server = "fixture"
            tool = "line"
        }
        .into(),
    )];
    app.refresh_in_memory_config_from_disk().await.unwrap();
    assert!(!render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("Initializing…"));
    app.chat_widget.statusline_feature_enabled = true;
    app.chat_widget.refresh_status_line();
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("Initializing…"));

    let selected = app
        .local_settings
        .tui
        .status_line_provider
        .as_deref()
        .unwrap()
        .clone();
    std::fs::write(&path, configured.replace("tool='line'", "tool='elsewhere'")).unwrap();
    app.save_status_line_provider(selected, /*provider_from_picker*/ None)
        .await
        .unwrap();
    let stored: toml::Table = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        stored["tui"]["status_line_provider"]["tool"].as_str(),
        Some("line")
    );
    std::fs::write(&path, configured.replace("= true", "= false")).unwrap();
    app.refresh_in_memory_config_from_disk().await.unwrap();
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("Initializing…"));
}
