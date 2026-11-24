use super::*;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

#[tokio::test]
async fn moves_all_fork_properties_preserving_shared_settings_and_comments() -> io::Result<()> {
    let home = tempdir()?;
    let shared = home.path().join(CONFIG_TOML_FILE);
    let overlay = home.path().join(CONFIG_OVERLAY_FILE);
    fs::write(
        &shared,
        r#"# shared settings
model = "example"
[tui]
theme = "nord" # keep this
primary_accent = '#7a81ff'
# no rotating tips
chatbox_placeholder_tips = "off"
passive_hints = "always"
file_mentions_preserve_at = true
file_mentions_allow_explicit_paths = false
status_line_truncatable = ["current-dir", "thread-name"]
[tui.keymap.global]
open_external_editor_with_quote = "ctrl-x g" # quoted response
"#,
    )?;
    migrate_user_fork_config(home.path()).await?;
    let actual_shared = fs::read_to_string(&shared)?;
    let actual_overlay = fs::read_to_string(&overlay)?;
    assert_eq!(
        actual_shared,
        "# shared settings\nmodel = \"example\"\n[tui]\ntheme = \"nord\" # keep this\n[tui.keymap.global]\n"
    );
    assert_eq!(
        toml::from_str::<toml::Value>(&actual_overlay).unwrap(),
        toml::from_str::<toml::Value>(
            r#"
[tui]
primary_accent = '#7a81ff'
chatbox_placeholder_tips = "off"
passive_hints = "always"
file_mentions_preserve_at = true
file_mentions_allow_explicit_paths = false
status_line_truncatable = ["current-dir", "thread-name"]
[tui.keymap.global]
open_external_editor_with_quote = "ctrl-x g"
"#
        )
        .unwrap()
    );
    assert!(actual_overlay.contains("# no rotating tips"));
    assert!(actual_overlay.contains("# quoted response"));
    migrate_user_fork_config(home.path()).await?;
    assert_eq!(
        (fs::read_to_string(shared)?, fs::read_to_string(overlay)?),
        (actual_shared, actual_overlay)
    );
    Ok(())
}

#[tokio::test]
async fn existing_overlay_wins_and_inline_and_dotted_keys_migrate() -> io::Result<()> {
    for shared in [
        "tui.chatbox_placeholder_tips = 'off'\ntui.file_mentions_preserve_at = true\n",
        "tui = { chatbox_placeholder_tips = 'off', file_mentions_preserve_at = true }\n",
        "[tui]\nchatbox_placeholder_tips = 'off'\nfile_mentions_preserve_at = true\n",
    ] {
        let home = tempdir()?;
        fs::write(home.path().join(CONFIG_TOML_FILE), shared)?;
        fs::write(
            home.path().join(CONFIG_OVERLAY_FILE),
            "# existing overlay\ntui = { chatbox_placeholder_tips = 'on' }\n",
        )?;
        migrate_user_fork_config(home.path()).await?;
        let overlay = fs::read_to_string(home.path().join(CONFIG_OVERLAY_FILE))?;
        assert!(overlay.starts_with("# existing overlay"));
        assert_eq!(
            toml::from_str::<toml::Value>(&overlay).unwrap(),
            toml::from_str::<toml::Value>(
                "[tui]\nchatbox_placeholder_tips = 'on'\nfile_mentions_preserve_at = true"
            )
            .unwrap()
        );
        assert!(!has_fork_settings(&read_document(
            &home.path().join(CONFIG_TOML_FILE)
        )?));
    }
    Ok(())
}

#[tokio::test]
async fn brief_status_line_migrates_once_and_keeps_the_shared_copy() -> io::Result<()> {
    let home = tempdir()?;
    let shared = home.path().join(CONFIG_TOML_FILE);
    let overlay = home.path().join(CONFIG_OVERLAY_FILE);
    let legacy = "tui = { status_line = ['model-brief', 'current-dir', 'hostname-brief'], theme = 'nord' } # keep\n";
    fs::write(&shared, legacy)?;

    migrate_user_fork_config(home.path()).await?;

    let saved_overlay = fs::read_to_string(&overlay)?;
    assert_eq!(
        toml::from_str::<toml::Table>(&saved_overlay).unwrap(),
        toml::toml! { [tui] status_line = ["model-brief", "current-dir", "hostname-brief"] },
    );
    let saved_shared = fs::read_to_string(&shared)?;
    assert_eq!(
        toml::from_str::<toml::Table>(&saved_shared).unwrap(),
        toml::toml! {
            [tui]
            status_line = ["model", "current-dir", "hostname"]
            theme = "nord"
        },
    );
    assert!(saved_shared.contains("# keep"));

    let stock_edit = "tui.status_line = ['thread-name']\n";
    fs::write(&shared, stock_edit)?;
    migrate_user_fork_config(home.path()).await?;
    assert_eq!(fs::read_to_string(shared)?, stock_edit);
    assert_eq!(fs::read_to_string(overlay)?, saved_overlay);
    Ok(())
}

#[tokio::test]
async fn existing_status_line_overlay_supplies_the_migrated_shared_copy() -> io::Result<()> {
    let home = tempdir()?;
    let shared = home.path().join(CONFIG_TOML_FILE);
    let overlay = home.path().join(CONFIG_OVERLAY_FILE);
    let saved_overlay = "tui.status_line = [] # hidden\n";
    fs::write(&shared, "[tui]\nstatus_line = ['model-brief']\n")?;
    fs::write(&overlay, saved_overlay)?;

    migrate_user_fork_config(home.path()).await?;

    assert_eq!(fs::read_to_string(overlay)?, saved_overlay);
    assert_eq!(
        toml::from_str::<toml::Table>(&fs::read_to_string(shared)?).unwrap(),
        toml::toml! { [tui] status_line = [] },
    );
    Ok(())
}

#[tokio::test]
async fn no_migration_creates_no_files_or_reformats_shared_config() -> io::Result<()> {
    let home = tempdir()?;
    migrate_user_fork_config(home.path()).await?;
    assert_eq!(fs::read_dir(home.path())?.count(), 0);
    let original = "# mine\n[tui]\nanimations=false\nstatus_line=['current-dir']\n";
    fs::write(home.path().join(CONFIG_TOML_FILE), original)?;
    migrate_user_fork_config(home.path()).await?;
    assert_eq!(fs::read_dir(home.path())?.count(), 1);
    assert_eq!(
        fs::read_to_string(home.path().join(CONFIG_TOML_FILE))?,
        original
    );
    Ok(())
}

#[tokio::test]
async fn invalid_overlay_or_migrated_values_leave_both_files_unchanged() -> io::Result<()> {
    for (shared, overlay) in [
        ("[tui]\nchatbox_placeholder_tips = 'off'", "malformed = ["),
        (
            "[tui]\nchatbox_placeholder_tips = 'off'",
            "[tui]\ntheme = 'nord'",
        ),
        ("[tui]\nchatbox_placeholder_tips = 'off'", "tui = false"),
        ("[tui]\nfile_mentions_preserve_at = 'yes'", "# existing\n"),
    ] {
        let home = tempdir()?;
        let shared_file = home.path().join(CONFIG_TOML_FILE);
        let overlay_file = home.path().join(CONFIG_OVERLAY_FILE);
        fs::write(&shared_file, shared)?;
        fs::write(&overlay_file, overlay)?;
        assert!(migrate_user_fork_config(home.path()).await.is_err());
        assert_eq!(
            (
                fs::read_to_string(shared_file)?,
                fs::read_to_string(overlay_file)?
            ),
            (shared.to_string(), overlay.to_string())
        );
    }
    Ok(())
}

#[tokio::test]
async fn concurrent_startups_complete_the_same_migration() -> io::Result<()> {
    let home = tempdir()?;
    fs::write(
        home.path().join(CONFIG_TOML_FILE),
        "[tui]\nchatbox_placeholder_tips = 'off'\n",
    )?;
    let (first, second) = tokio::join!(
        migrate_user_fork_config(home.path()),
        migrate_user_fork_config(home.path())
    );
    first?;
    second?;
    assert_eq!(
        fs::read_to_string(home.path().join(CONFIG_TOML_FILE))?,
        "[tui]\n"
    );
    assert_eq!(
        toml::from_str::<toml::Value>(&fs::read_to_string(home.path().join(CONFIG_OVERLAY_FILE))?)
            .unwrap(),
        toml::from_str::<toml::Value>("[tui]\nchatbox_placeholder_tips = 'off'").unwrap()
    );
    Ok(())
}

#[tokio::test]
async fn creates_nested_keymap_inside_inline_overlay_tables() -> io::Result<()> {
    for original_overlay in [
        "tui = {}",
        "tui = { keymap = {} }",
        "tui.keymap.global = {}",
    ] {
        let home = tempdir()?;
        let shared = "tui.keymap.global.open_external_editor_with_quote = 'alt-e'\n";
        fs::write(home.path().join(CONFIG_TOML_FILE), shared)?;
        fs::write(home.path().join(CONFIG_OVERLAY_FILE), original_overlay)?;
        migrate_user_fork_config(home.path()).await?;
        assert_eq!(
            toml::from_str::<toml::Value>(&fs::read_to_string(
                home.path().join(CONFIG_OVERLAY_FILE)
            )?)
            .unwrap(),
            toml::from_str::<toml::Value>(shared).unwrap()
        );
    }
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn follows_config_symlinks_and_writes_private_files() -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::fs::symlink;
    let home = tempdir()?;
    let shared_target = home.path().join("shared-target.toml");
    let overlay_target = home.path().join("overlay-target.toml");
    fs::write(&shared_target, "[tui]\nchatbox_placeholder_tips = 'off'\n")?;
    symlink(&shared_target, home.path().join(CONFIG_TOML_FILE))?;
    symlink(&overlay_target, home.path().join(CONFIG_OVERLAY_FILE))?;
    migrate_user_fork_config(home.path()).await?;
    assert_eq!(
        fs::read_link(home.path().join(CONFIG_TOML_FILE))?,
        shared_target
    );
    assert_eq!(
        fs::read_link(home.path().join(CONFIG_OVERLAY_FILE))?,
        overlay_target
    );
    for file in [shared_target, overlay_target] {
        assert_eq!(fs::metadata(file)?.permissions().mode() & 0o777, 0o600);
    }
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn overlapping_symlinks_cannot_erase_fork_settings() -> io::Result<()> {
    let home = tempdir()?;
    let shared = home.path().join(CONFIG_TOML_FILE);
    let original = "[tui]\nchatbox_placeholder_tips = 'off'\n";
    fs::write(&shared, original)?;
    std::os::unix::fs::symlink(&shared, home.path().join(CONFIG_OVERLAY_FILE))?;
    assert!(migrate_user_fork_config(home.path()).await.is_err());
    assert_eq!(fs::read_to_string(shared)?, original);
    Ok(())
}
