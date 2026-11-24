use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn overlay_is_optional_and_preserves_shared_profile_and_cli_layers() -> io::Result<()> {
    let home = tempdir()?;
    let overlay_file =
        AbsolutePathBuf::resolve_path_against_base(crate::CONFIG_OVERLAY_FILE, home.path());
    let shared_file = AbsolutePathBuf::resolve_path_against_base(CONFIG_TOML_FILE, home.path());
    let mut options = LoaderOverrides::without_managed_config_for_tests();
    let missing = load_config_layers_state(
        &TestFileSystem,
        home.path(),
        /*cwd*/ None,
        &[],
        options.clone(),
        &crate::NoopThreadConfigLoader,
    )
    .await?;
    assert_eq!(missing.get_user_config_file(), Some(&shared_file));
    assert!(!overlay_file.exists());
    std::fs::write(&shared_file, "[tui]\nanimations = false")?;
    std::fs::write(
        &overlay_file,
        "[tui]\nprimary_accent = '14'\nfile_mentions_preserve_at = true",
    )?;
    let profile = AbsolutePathBuf::resolve_path_against_base("work.config.toml", home.path());
    std::fs::write(&profile, "[tui]\ntheme = 'nord'")?;
    options.user_config_path = Some(profile.clone());
    options.user_config_profile = Some("work".parse().unwrap());
    let layers = load_config_layers_state(
        &TestFileSystem,
        home.path(),
        /*cwd*/ None,
        &[],
        ConfigLoadOptions {
            loader_overrides: options.clone(),
            strict_config: true,
            ..Default::default()
        },
        &crate::NoopThreadConfigLoader,
    )
    .await?;
    let expected: TomlValue = toml::from_str("animations = false\nprimary_accent = '14'\nfile_mentions_preserve_at = true\ntheme = 'nord'").unwrap();
    // Packaged defaults may also contribute TUI preferences.
    let tui = layers.effective_config()["tui"].clone();
    for (key, value) in expected.as_table().unwrap() {
        assert_eq!(tui.get(key), Some(value));
    }
    assert_eq!(layers.get_user_config_file(), Some(&profile));
    assert_eq!(
        layers.origins()["tui.primary_accent"].name,
        ConfigLayerSource::User {
            file: overlay_file.clone(),
            profile: None
        }
    );
    let overridden = load_config_layers_state(
        &TestFileSystem,
        home.path(),
        /*cwd*/ None,
        &[("tui.primary_accent".into(), "15".into())],
        options.clone(),
        &crate::NoopThreadConfigLoader,
    )
    .await?;
    assert_eq!(
        overridden.effective_config()["tui"]["primary_accent"],
        TomlValue::from("15")
    );
    let cwd = AbsolutePathBuf::from_absolute_path(home.path())?;
    let local = super::super::local::load_local_config_layers_with_overrides(
        &TestFileSystem,
        home.path(),
        &cwd,
        &options,
    )
    .await?;
    assert!(local.config.layers.iter().any(|layer| matches!(&layer.source, ConfigLayerSource::User { file, .. } if file == &overlay_file)));
    options.ignore_user_config = true;
    std::fs::write(&overlay_file, "malformed = [")?;
    load_config_layers_state(
        &TestFileSystem,
        home.path(),
        /*cwd*/ None,
        &[],
        options,
        &crate::NoopThreadConfigLoader,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn status_line_overlay_wins_over_profile_and_project_with_cli_override_available()
-> io::Result<()> {
    let home = tempdir()?;
    let project = tempdir()?;
    let cwd = AbsolutePathBuf::try_from(project.path())?;
    let dot_codex = cwd.join(".codex");
    let trust_key = TomlValue::String(project_trust_key(project.path()));
    let profile = AbsolutePathBuf::resolve_path_against_base("work.config.toml", home.path());
    let overlay = home.path().join(crate::CONFIG_OVERLAY_FILE);
    std::fs::create_dir(&dot_codex)?;
    std::fs::write(
        home.path().join(CONFIG_TOML_FILE),
        format!(
            "[tui]\nstatus_line = ['model']\n[projects.{trust_key}]\ntrust_level = 'trusted'\n"
        ),
    )?;
    std::fs::write(&profile, "tui.status_line = ['current-dir']")?;
    std::fs::write(
        dot_codex.join(CONFIG_TOML_FILE),
        "tui.status_line = ['git-branch']",
    )?;
    let options = ConfigLoadOptions {
        loader_overrides: LoaderOverrides {
            user_config_path: Some(profile.clone()),
            user_config_profile: Some("work".parse().unwrap()),
            ..LoaderOverrides::without_managed_config_for_tests()
        },
        strict_config: true,
        ..Default::default()
    };
    let cli = [(
        "tui.status_line".into(),
        TomlValue::Array(vec!["weekly-limit".into()]),
    )];
    for (contents, cwd, cli, expected) in [
        (
            "tui.primary_accent = '14'",
            None,
            &[][..],
            &["current-dir"][..],
        ),
        (
            "tui.primary_accent = '14'",
            Some(cwd.clone()),
            &[][..],
            &["git-branch"][..],
        ),
        (
            "tui.status_line = ['model-brief']",
            Some(cwd.clone()),
            &[][..],
            &["model-brief"][..],
        ),
        ("tui.status_line = []", Some(cwd.clone()), &[][..], &[][..]),
        (
            "tui.status_line = []",
            Some(cwd.clone()),
            cli.as_slice(),
            &["weekly-limit"][..],
        ),
    ] {
        std::fs::write(&overlay, contents)?;
        let stack = load_config_layers_state(
            &TestFileSystem,
            home.path(),
            cwd,
            cli,
            options.clone(),
            &crate::NoopThreadConfigLoader,
        )
        .await?;
        assert_eq!(stack.get_user_config_file(), Some(&profile));
        assert_eq!(
            stack.effective_config()["tui"]["status_line"],
            TomlValue::Array(expected.iter().copied().map(TomlValue::from).collect()),
        );
    }
    Ok(())
}

#[tokio::test]
async fn file_errors_identify_the_overlay_and_misplaced_keys() -> io::Result<()> {
    let home = tempdir()?;
    for (file, input, expected) in [
        (
            crate::CONFIG_OVERLAY_FILE,
            "[tui]\ntheme = 'nord'",
            "tui.theme",
        ),
        (
            crate::CONFIG_OVERLAY_FILE,
            "[tui]\nfile_mentions_preserve_at = 'yes'",
            "file_mentions_preserve_at",
        ),
        (
            CONFIG_TOML_FILE,
            "[tui]\nprimary_accent = '14'",
            "Move fork setting `tui.primary_accent`",
        ),
    ] {
        let path = home.path().join(file);
        std::fs::write(&path, input)?;
        let error = load_config_layers_state(
            &TestFileSystem,
            home.path(),
            /*cwd*/ None,
            &[],
            LoaderOverrides::without_managed_config_for_tests(),
            &crate::NoopThreadConfigLoader,
        )
        .await
        .unwrap_err()
        .to_string();
        assert!(error.contains(file) && error.contains(expected), "{error}");
        std::fs::remove_file(path)?;
    }
    Ok(())
}
