use super::*;
use crate::config::load_config_toml_with_layer_stack;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn startup_migrates_before_strict_validation_and_keeps_cli_precedence() -> anyhow::Result<()>
{
    let home = tempdir()?;
    let shared = home.path().join(CONFIG_TOML_FILE);
    let overlay =
        AbsolutePathBuf::from_absolute_path(home.path().join(codex_config::CONFIG_OVERLAY_FILE))?;
    tokio::fs::write(&shared, "[tui]\nchatbox_placeholder_tips = 'off'\n").await?;
    let options = ConfigLoadOptions {
        loader_overrides: LoaderOverrides::without_managed_config_for_tests(),
        strict_config: true,
        ..Default::default()
    };
    let loaded =
        load_config_toml_with_layer_stack(home.path(), /*cwd*/ None, vec![], options.clone())
            .await?;
    assert_eq!(
        loaded.config_layer_stack.origins()["tui.chatbox_placeholder_tips"].name,
        ConfigLayerSource::User {
            file: overlay,
            profile: None,
        }
    );
    let overridden = load_config_toml_with_layer_stack(
        home.path(),
        /*cwd*/ None,
        vec![(
            "tui.chatbox_placeholder_tips".to_string(),
            TomlValue::String("on".to_string()),
        )],
        options,
    )
    .await?;
    assert_eq!(
        overridden.config_layer_stack.effective_config()["tui"]["chatbox_placeholder_tips"],
        TomlValue::String("on".to_string())
    );
    assert_eq!(tokio::fs::read_to_string(shared).await?, "[tui]\n");
    Ok(())
}

#[tokio::test]
async fn ignoring_user_config_does_not_migrate_it() -> anyhow::Result<()> {
    let home = tempdir()?;
    let shared = home.path().join(CONFIG_TOML_FILE);
    let original = "[tui]\nchatbox_placeholder_tips = 'off'\n";
    tokio::fs::write(&shared, original).await?;
    let mut options = LoaderOverrides::without_managed_config_for_tests();
    options.ignore_user_config = true;
    load_config_toml_with_layer_stack(home.path(), /*cwd*/ None, vec![], options).await?;
    assert_eq!(tokio::fs::read_to_string(shared).await?, original);
    assert_eq!(std::fs::read_dir(home.path())?.count(), 1);
    Ok(())
}
