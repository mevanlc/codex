#![cfg(test)]
//! Verify focus during discovery and the handoff from a saved selection to the reader.

use super::*;
use crate::app::tests::make_test_app_with_channels;
use crate::app::tests::test_thread_session;
use crate::app_server_session::ThreadParamsMode;
use crate::bottom_pane::StatusLineSetupView;
use crate::chatwidget::tests::render_bottom_popup;
use crate::statusline_metrics::tests::Collector;
use crate::statusline_provider::test_support::advertised;
use crate::tui::test_support::make_test_tui;
use codex_app_server_client::AppServerClient;
use codex_app_server_client::CloudClient;
use codex_app_server_client::CloudCommand;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use tokio::sync::mpsc;

async fn picker() -> anyhow::Result<(
    Box<App>,
    AppServerSession,
    mpsc::Receiver<CloudCommand>,
    mpsc::UnboundedReceiver<AppEvent>,
)> {
    let (mut app, events, _) = make_test_app_with_channels().await;
    app.chat_widget.statusline_feature_enabled = true;
    app.chat_widget.thread_params_mode = ThreadParamsMode::Cloud;
    let (client, commands, _) = CloudClient::channel(/*capacity*/ 8);
    let owner = AppServerSession::new(
        BackendId::new(),
        AppServerClient::Cloud(client),
        ThreadParamsMode::Cloud,
    );
    Ok((app, owner, commands, events))
}

#[tokio::test]
async fn statusline_ctrl_t_marks_disabled_items_and_round_trips_through_config()
-> anyhow::Result<()> {
    let (mut app, mut owner, _commands, mut events) = picker().await?;
    let mut tui = make_test_tui()?;
    app.chat_widget.statusline_feature_enabled = false;
    let selected = vec!["model-with-reasoning-brief".to_string()];
    app.chat_widget.local_settings.tui.status_line = Some(selected.clone());
    app.chat_widget
        .restore_user_message_to_composer("/statusline".into());
    app.chat_widget.handle_key_event(KeyCode::Enter.into());
    assert!(app.chat_widget.active_view_is(StatusLineSetupView::VIEW_ID));

    let ctrl_t = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL);
    let repeat = KeyEvent::new_with_kind(
        KeyCode::Char('t'),
        KeyModifiers::CONTROL,
        KeyEventKind::Repeat,
    );
    for key in [
        KeyCode::Down.into(),
        ctrl_t,
        KeyCode::Down.into(),
        ctrl_t,
        repeat,
    ] {
        app.handle_tui_event(&mut tui, &mut owner, TuiEvent::Key(key))
            .await
            .unwrap();
    }
    assert!(app.overlay.is_none());
    let frame = render_bottom_popup(&app.chat_widget, /*width*/ 100);
    assert!(frame.contains("[b][t] model-with-reasoning"), "{frame}");
    assert!(frame.contains("› [ ][t] model "), "{frame}");

    let overlay = app
        .local_settings
        .codex_home
        .join(codex_config::CONFIG_OVERLAY_FILE);
    assert!(!overlay.exists());
    app.handle_tui_event(&mut tui, &mut owner, TuiEvent::Key(KeyCode::Enter.into()))
        .await
        .unwrap();
    let save = std::iter::from_fn(|| events.try_recv().ok())
        .find(|event| matches!(event, AppEvent::StatusLineSetup { .. }))
        .expect("Enter saves the status line");
    crate::boxed_future!(app.handle_event(&mut tui, &mut owner, save))
        .await
        .unwrap();

    let overlay: toml::Table = toml::from_str(&std::fs::read_to_string(overlay)?)?;
    assert_eq!(
        overlay,
        toml::toml! {
            [tui]
            status_line = ["model-with-reasoning-brief"]
            status_line_truncatable = ["model", "model-with-reasoning"]
        }
    );
    let shared: toml::Table = toml::from_str(&std::fs::read_to_string(
        &app.local_settings.user_config_path,
    )?)?;
    assert!(shared["tui"].get("status_line_truncatable").is_none());
    assert_eq!(
        shared["tui"]["status_line"],
        toml::Value::Array(vec!["model-with-reasoning".into()]),
    );
    app.refresh_in_memory_config_from_disk().await.unwrap();
    assert_eq!(
        app.chat_widget.local_settings.tui.status_line,
        Some(selected)
    );
    assert_eq!(
        app.chat_widget
            .local_settings
            .tui
            .fork
            .status_line_truncatable,
        ["model", "model-with-reasoning"]
    );

    app.chat_widget
        .restore_user_message_to_composer("/statusline".into());
    app.chat_widget.handle_key_event(KeyCode::Enter.into());
    let frame = render_bottom_popup(&app.chat_widget, /*width*/ 100);
    assert!(frame.contains("[ ][t] model "), "{frame}");
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn quick_second_enter_opens_the_statusline_picker_without_selecting() -> anyhow::Result<()> {
    let (mut app, mut owner, _commands, mut events) = picker().await?;
    app.chat_widget.thread_params_mode = ThreadParamsMode::Embedded;
    let mut tui = make_test_tui()?;
    app.chat_widget
        .restore_user_message_to_composer("/statusline".into());
    app.chat_widget.handle_key_event(KeyCode::Enter.into());
    app.chat_widget.handle_key_event(KeyCode::Enter.into());
    let open = std::iter::from_fn(|| events.try_recv().ok())
        .find(|event| matches!(event, AppEvent::OpenStatuslinePicker(_)))
        .expect("Enter opens the provider picker");
    crate::boxed_future!(app.handle_event(&mut tui, &mut owner, open))
        .await
        .unwrap();

    let repeat = KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Repeat);
    app.chat_widget.handle_key_event(repeat);
    app.chat_widget.handle_key_event(KeyEvent::new_with_kind(
        KeyCode::Enter,
        KeyModifiers::NONE,
        KeyEventKind::Release,
    ));
    assert!(
        !std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::ChooseStatuslineProvider { .. }))
    );
    app.chat_widget.handle_key_event(KeyCode::Enter.into());
    let choice = std::iter::from_fn(|| events.try_recv().ok())
        .find(|event| matches!(event, AppEvent::ChooseStatuslineProvider { .. }))
        .expect("fresh Enter chooses the provider");
    crate::boxed_future!(app.handle_event(&mut tui, &mut owner, choice))
        .await
        .unwrap();
    assert!(
        app.chat_widget
            .active_view_is(crate::bottom_pane::StatusLineSetupView::VIEW_ID)
    );
    app.chat_widget.handle_key_event(repeat);
    assert!(
        !std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::OpenStatuslinePicker(_)))
    );
    Ok(())
}

#[tokio::test]
async fn statusline_create_is_ignored_after_dismissing_setup() -> anyhow::Result<()> {
    let (mut app, _, _, mut events) = picker().await?;
    app.chat_widget.thread_params_mode = ThreadParamsMode::Embedded;
    app.chat_widget
        .dispatch_command(crate::slash_command::SlashCommand::Statusline);
    app.chat_widget.handle_key_event(KeyCode::Down.into());
    app.chat_widget.handle_key_event(KeyCode::Enter.into());
    let create = std::iter::from_fn(|| events.try_recv().ok())
        .find(|event| matches!(event, AppEvent::CreateCustomStatusline { .. }))
        .expect("Create selected");
    app.chat_widget.handle_key_event(KeyCode::Esc.into());

    let (client, mut requests, _) = CloudClient::channel(/*capacity*/ 1);
    let mut server = AppServerSession::new(
        BackendId::new(),
        AppServerClient::Cloud(client),
        ThreadParamsMode::Remote,
    );
    let mut tui = make_test_tui()?;
    tokio::select! {
        result = app.handle_event(&mut tui, &mut server, create) => { result.unwrap(); }
        _ = requests.recv() => panic!("dismissed Create contacted the server"),
    }
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn statusline_picker_disconnect_leaves_the_item_editor_open() -> anyhow::Result<()> {
    let (mut app, owner, mut commands, mut events) = picker().await?;
    app.app_server_target = AppServerTarget::Remote {
        endpoint: crate::resolve_remote_addr("ws://127.0.0.1:9").unwrap(),
    };
    app.chat_widget.local_settings.tui.status_line_use_colors = false;
    app.chat_widget
        .restore_user_message_to_composer("/statusline".into());
    app.chat_widget.handle_key_event(KeyCode::Enter.into());
    app.chat_widget.handle_key_event(KeyCode::Down.into());
    app.chat_widget.handle_key_event(KeyCode::Char(' ').into());
    app.open_statusline_picker(&owner);
    let mut inventory = commands.recv().await.unwrap();

    assert!(app.begin_reconnect());
    inventory.response.closed().await;
    assert!(
        app.chat_widget
            .active_view_is(crate::bottom_pane::StatusLineSetupView::VIEW_ID)
    );
    app.chat_widget.handle_key_event(KeyCode::Enter.into());
    assert!(
        std::iter::from_fn(|| events.try_recv().ok()).any(|event| matches!(
            event,
            AppEvent::StatusLineSetup {
                use_theme_colors: true,
                ..
            }
        ))
    );
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn discovery_batch_keeps_the_highlight() -> anyhow::Result<()> {
    let (mut app, owner, _commands, _events) = picker().await?;
    let collector = Collector::new();
    app.statusline_metrics = collector.metrics(&mut app.config);
    let calendar = advertised("work", "calendar", "ui://work/day");
    app.local_settings.tui.status_line_provider =
        Some(Box::new(StatusLineSelection::Mcp((&calendar.id).into())));
    app.chat_widget.local_settings.tui.status_line_provider =
        app.local_settings.tui.status_line_provider.clone();
    let update = |server: &str, providers| DiscoveryUpdate::Server(server.into(), Ok(providers));
    app.open_statusline_picker(&owner);
    let scan = app.statusline_picker.as_ref().unwrap().id;
    let retired = advertised("work", "retired", "ui://work/retired");
    app.finish_statusline_discovery(Uuid::new_v4(), vec![update("work", vec![retired])], &owner);
    let initial = render_bottom_popup(&app.chat_widget, /*width*/ 67);
    assert!(initial.contains("Discovering MCP status line providers"));
    assert!(initial.contains("Default status line"));
    assert!(initial.contains("calendar"));
    assert!(!initial.contains("retired"));
    app.chat_widget.handle_key_event(KeyCode::Down.into());
    let named = |server, tool, title: &str| {
        let mut provider = advertised(server, tool, "ui://unused");
        provider.title = title.into();
        provider
    };
    app.finish_statusline_discovery(
        scan,
        vec![
            update("usage", vec![named("usage", "limits", "Usage")]),
            update(
                "health",
                vec![
                    named("health", "quota", "Quota"),
                    named("health", "alerts", "Alerts"),
                ],
            ),
            update("work", vec![calendar]),
        ],
        &owner,
    );
    insta::assert_snapshot!(render_bottom_popup(&app.chat_widget, /*width*/ 67));
    assert_eq!(collector.snapshot(), ["picker_opened 1"]);

    app.chat_widget.handle_key_event(KeyCode::Esc.into());
    app.chat_widget.pause_unavailable_thread();
    app.reconnect.offline = true;
    app.open_statusline_picker(&owner);
    insta::assert_snapshot!(
        "statusline_providers_disconnected",
        render_bottom_popup(&app.chat_widget, /*width*/ 67)
    );
    let offline = app.statusline_picker.as_ref().unwrap().id;
    app.choose_statusline_provider(
        offline,
        /*id*/ None,
        crate::key_hint::plain(KeyCode::Enter),
        &owner,
    )
    .await;
    assert_eq!(
        app.local_settings.tui.status_line_provider.as_deref(),
        Some(&StatusLineSelection::Builtin(BuiltinStatusLine::Default))
    );
    assert_eq!(
        collector.snapshot(),
        [
            "picker_opened 1",
            "provider_changed change=returned_to_default 1"
        ]
    );
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn discovery_only_starts_a_reader_on_explicit_selection() -> anyhow::Result<()> {
    let (mut app, mut owner, _requests, mut events) = picker().await?;
    let mut tui = make_test_tui()?;
    let collector = Collector::new();
    app.statusline_metrics = collector.metrics(&mut app.config);
    let thread = ThreadId::new();
    let cwd = app.config.cwd.to_path_buf();
    app.chat_widget
        .handle_thread_session(test_thread_session(thread, cwd));
    app.chat_widget.statusline_feature_enabled = true;
    let provider = advertised("work", "calendar", "ui://work/day");

    for (uri, expected) in [
        (
            "ui://work/day",
            &[
                "provider_changed change=enabled_custom 1",
                "provider_started 1",
            ][..],
        ),
        ("ui://work/replacement", &["provider_started 1"][..]),
    ] {
        app.open_statusline_picker(&owner);
        let scan = app.statusline_picker.as_ref().unwrap().id;
        let found = advertised("work", "calendar", uri);
        let update = DiscoveryUpdate::Server(found.id.server.clone(), Ok(vec![found]));
        app.finish_statusline_discovery(scan, vec![update], &owner);
        assert!(
            !std::iter::from_fn(|| events.try_recv().ok())
                .any(|event| matches!(event, AppEvent::StatuslineRead(_)))
        );
        assert_eq!(collector.snapshot(), ["picker_opened 1"]);
        app.choose_statusline_provider(
            scan,
            Some(provider.id.clone()),
            crate::key_hint::plain(KeyCode::Enter),
            &owner,
        )
        .await;
        let read = std::iter::from_fn(|| events.try_recv().ok())
            .find(|event| matches!(event, AppEvent::StatuslineRead(_)))
            .expect("explicit choice starts a reader");
        crate::boxed_future!(app.handle_event(&mut tui, &mut owner, read))
            .await
            .unwrap();
        assert_eq!(collector.snapshot(), expected);
    }
    Ok(())
}

#[tokio::test]
async fn discovery_shows_a_bounded_warning_without_rpc_details() -> anyhow::Result<()> {
    let (mut app, owner, _requests, _events) = picker().await?;
    app.open_statusline_picker(&owner);
    let scan = app.statusline_picker.as_ref().unwrap().id;
    let private_error = color_eyre::eyre::eyre!("private credentials in an RPC message");
    let inventory = DiscoveryUpdate::InventoryFailed(private_error.into());
    let mut updates = vec![inventory];
    for server in ["\x1b[31mbeta\x1b[0m", "mail", "chat", "docs", "links"] {
        let update = DiscoveryUpdate::Server(server.into(), Err(ProviderLookupError::TimedOut));
        updates.push(update);
    }
    app.finish_statusline_discovery(scan, updates, &owner);
    app.chat_widget.handle_key_event(KeyCode::Esc.into());

    app.open_statusline_picker(&owner);
    let canceled_scan = app.statusline_picker.as_ref().unwrap().id;
    app.chat_widget.handle_key_event(KeyCode::Esc.into());
    // A canceled rescan and its queued completion leave the last warning available.
    app.finish_statusline_discovery(canceled_scan, Vec::new(), &owner);
    app.chat_widget.open_warnings(&[]);
    let warning = render_bottom_popup(&app.chat_widget, /*width*/ 80);
    insta::assert_snapshot!("statusline_discovery_warning_in_f2", warning);

    app.chat_widget.handle_key_event(KeyCode::Esc.into());
    app.open_statusline_picker(&owner);
    let successful_scan = app.statusline_picker.as_ref().unwrap().id;
    app.finish_statusline_discovery(successful_scan, Vec::new(), &owner);
    app.chat_widget.handle_key_event(KeyCode::Esc.into());
    app.chat_widget.open_warnings(&[]);
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("No warnings"));
    Ok(())
}
