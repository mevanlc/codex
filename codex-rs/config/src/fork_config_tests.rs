use super::*;
use pretty_assertions::assert_eq;

#[test]
fn edits_route_by_property_including_nested_tables_and_resets() {
    let path = ["tui", "keymap", "global", "open_external_editor_with_quote"].map(str::to_string);
    assert_eq!(
        ConfigFileKind::for_edit(&path, /*value*/ None),
        Ok(ConfigFileKind::ForkOverlay)
    );
    let fork: Value = toml::from_str("primary_accent = '14'").unwrap();
    assert_eq!(
        ConfigFileKind::for_edit(&["tui".into()], Some(&fork)),
        Ok(ConfigFileKind::ForkOverlay)
    );
    let mixed: Value = toml::from_str("primary_accent = '14'\ntheme = 'nord'").unwrap();
    assert_eq!(
        ConfigFileKind::for_edit(&["tui".into()], Some(&mixed)),
        Err("Write shared and fork settings in separate requests".into())
    );
}

#[test]
fn overlay_rejects_non_fork_fields_and_invalid_values() {
    for input in [
        "model = 'x'",
        "[tui]\ntheme = 'nord'",
        "[tui]\nprimary_accent = false",
        "[tui.keymap.global]\ncopy = 'ctrl-g'",
        "[tui.keymap.global]\nopen_external_editor_with_quote = 'invalid-key'",
    ] {
        assert!(
            ConfigFileKind::ForkOverlay
                .validate(&toml::from_str(input).unwrap())
                .is_err(),
            "{input}"
        );
    }
    let value = toml::from_str("[tui]\nfile_mentions_preserve_at = true").unwrap();
    assert_eq!(
        ConfigFileKind::Shared.validate(&value),
        Err(
            "Move fork setting `tui.file_mentions_preserve_at` to $CODEX_HOME/config-overlay.toml"
                .into()
        )
    );
    assert_eq!(ConfigFileKind::ForkOverlay.validate(&value), Ok(()));
}
