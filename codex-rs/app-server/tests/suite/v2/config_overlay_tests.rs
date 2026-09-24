use super::*;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_migrates_fork_config_and_reports_overlay_origins() -> Result<()> {
    let home = TempDir::new()?;
    let shared = "[tui]\ntheme = 'nord'\nchatbox_placeholder_tips = 'off'\nfile_mentions_preserve_at = true\n";
    write_config(&home, shared)?;
    let overlay = AbsolutePathBuf::from_absolute_path(
        home.path()
            .canonicalize()?
            .join(codex_config::CONFIG_OVERLAY_FILE),
    )?;
    std::fs::write(&overlay, "[tui]\nchatbox_placeholder_tips = 'on'\n")?;
    let mut server = TestAppServer::builder()
        .with_codex_home(home.path())
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;
    let id = server
        .send_config_read_request(ConfigReadParams {
            include_layers: true,
            cwd: None,
        })
        .await?;
    let read: ConfigReadResponse = server.read_response(id).await?;
    for key in [
        "tui.chatbox_placeholder_tips",
        "tui.file_mentions_preserve_at",
    ] {
        assert_eq!(
            read.origins[key].name,
            ConfigLayerSource::User {
                file: overlay.clone(),
                profile: None,
            }
        );
    }
    assert_eq!(
        std::fs::read_to_string(home.path().join("config.toml"))?,
        "[tui]\ntheme = 'nord'\n"
    );
    assert_eq!(
        toml::from_str::<toml::Value>(&std::fs::read_to_string(&overlay)?)?,
        toml::from_str::<toml::Value>(
            "[tui]\nchatbox_placeholder_tips = 'on'\nfile_mentions_preserve_at = true\n"
        )?
    );

    // Config reads also repair entries reintroduced by another client after startup.
    write_config(&home, shared)?;
    let id = server
        .send_config_read_request(ConfigReadParams {
            include_layers: true,
            cwd: None,
        })
        .await?;
    let _: ConfigReadResponse = server.read_response(id).await?;
    assert_eq!(
        std::fs::read_to_string(home.path().join("config.toml"))?,
        "[tui]\ntheme = 'nord'\n"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fork_overlay_writes_preserve_shared_config_and_use_separate_versions() -> Result<()> {
    let home = TempDir::new()?;
    let shared = "[tui]\ntheme = 'nord'\n";
    write_config(&home, shared)?;
    let overlay = AbsolutePathBuf::from_absolute_path(
        home.path()
            .canonicalize()?
            .join(codex_config::CONFIG_OVERLAY_FILE),
    )?;
    let mut server = TestAppServer::builder()
        .with_codex_home(home.path())
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;
    let mut previous_version = None;
    for value in [json!(null), json!("14"), json!("15")] {
        let id = server
            .send_config_value_write_request(ConfigValueWriteParams {
                key_path: "tui.primary_accent".into(),
                value: value.clone(),
                merge_strategy: MergeStrategy::Replace,
                file_path: None,
                expected_version: previous_version.clone(),
            })
            .await?;
        let written: ConfigWriteResponse = server.read_response(id).await?;
        assert_eq!(
            (&written.file_path, &written.status),
            (&overlay, &WriteStatus::Ok)
        );
        assert_eq!(overlay.exists(), !value.is_null());
        assert_eq!(
            std::fs::read_to_string(home.path().join("config.toml"))?,
            shared
        );
        if !value.is_null() {
            let id = server
                .send_config_read_request(ConfigReadParams {
                    include_layers: true,
                    cwd: None,
                })
                .await?;
            let read: ConfigReadResponse = server.read_response(id).await?;
            assert_eq!(
                read.origins["tui.primary_accent"].name,
                ConfigLayerSource::User {
                    file: overlay.clone(),
                    profile: None
                }
            );
            assert_eq!(read.origins["tui.primary_accent"].version, written.version);
            if let Some(stale) = &previous_version {
                let id = server
                    .send_config_value_write_request(ConfigValueWriteParams {
                        key_path: "tui.primary_accent".into(),
                        value: json!("16"),
                        merge_strategy: MergeStrategy::Replace,
                        file_path: None,
                        expected_version: Some(stale.clone()),
                    })
                    .await?;
                let error = server
                    .read_stream_until_error_message(RequestId::Integer(id))
                    .await?;
                assert_eq!(
                    error.error.data.unwrap()["config_write_error_code"],
                    json!("configVersionConflict")
                );
            }
        }
        previous_version = Some(written.version);
    }
    let before = std::fs::read_to_string(&overlay)?;
    let id = server
        .send_config_batch_write_request(ConfigBatchWriteParams {
            edits: vec![
                ConfigEdit {
                    key_path: "tui.primary_accent".into(),
                    value: json!("16"),
                    merge_strategy: MergeStrategy::Replace,
                },
                ConfigEdit {
                    key_path: "tui.theme".into(),
                    value: json!("dracula"),
                    merge_strategy: MergeStrategy::Replace,
                },
            ],
            file_path: None,
            expected_version: None,
            reload_user_config: false,
        })
        .await?;
    let error = server
        .read_stream_until_error_message(RequestId::Integer(id))
        .await?;
    assert_eq!(
        error.error.data.unwrap()["config_write_error_code"],
        json!("configValidationError")
    );
    assert_eq!(
        (
            std::fs::read_to_string(&overlay)?,
            std::fs::read_to_string(home.path().join("config.toml"))?
        ),
        (before.clone(), shared.to_string())
    );
    for (path, value, file) in [
        (
            "tui.primary_accent",
            json!("16"),
            home.path().join("config.toml"),
        ),
        ("tui.theme", json!("dracula"), overlay.to_path_buf()),
        (
            "tui",
            json!({"theme": "dracula", "primary_accent": "16"}),
            home.path().join("config.toml"),
        ),
    ] {
        let id = server
            .send_config_value_write_request(ConfigValueWriteParams {
                key_path: path.into(),
                value,
                merge_strategy: MergeStrategy::Replace,
                file_path: Some(file.display().to_string()),
                expected_version: None,
            })
            .await?;
        server
            .read_stream_until_error_message(RequestId::Integer(id))
            .await?;
    }
    assert_eq!(std::fs::read_to_string(&overlay)?, before);
    let id = server
        .send_config_value_write_request(ConfigValueWriteParams {
            key_path: "tui.theme".into(),
            value: json!("dracula"),
            merge_strategy: MergeStrategy::Replace,
            file_path: None,
            expected_version: None,
        })
        .await?;
    let written: ConfigWriteResponse = server.read_response(id).await?;
    assert_eq!(
        written.file_path,
        AbsolutePathBuf::from_absolute_path(home.path().canonicalize()?.join("config.toml"))?
    );
    assert_eq!(std::fs::read_to_string(&overlay)?, before);
    let id = server
        .send_config_value_write_request(ConfigValueWriteParams {
            key_path: "tui.primary_accent".into(),
            value: json!("16"),
            merge_strategy: MergeStrategy::Replace,
            file_path: None,
            expected_version: previous_version,
        })
        .await?;
    let written: ConfigWriteResponse = server.read_response(id).await?;
    assert_eq!(written.status, WriteStatus::Ok);
    Ok(())
}
