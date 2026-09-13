use super::*;
use crate::config_toml::ConfigToml;
use crate::merge_toml_values;
use crate::types::Notifications;
use crate::types::Tui;
use pretty_assertions::assert_eq;

#[test]
fn absent_and_empty_tui_tables_share_fork_defaults() {
    for source in ["", "[tui]", "[tui]\nnotifications = false"] {
        let parsed: ConfigToml = toml::from_str(source).unwrap();
        assert_eq!(
            parsed.tui.map(|tui| tui.fork).unwrap_or_default(),
            ForkTuiOptions::default(),
            "{source}",
        );
    }
}

#[test]
fn fork_options_round_trip_alongside_upstream_tui_fields() {
    let source = r##"
[tui]
chatbox_placeholder_tips = "off"
file_mentions_preserve_at = true
file_mentions_allow_explicit_paths = false
primary_accent = "#00AAFF"
notifications = false
animations = false
theme = "dracula"
"##;
    let parsed: ConfigToml = toml::from_str(source).unwrap();
    let tui = parsed.tui.unwrap();
    let expected = ForkTuiOptions {
        chatbox_placeholder_tips: ChatboxPlaceholderTips::Off,
        file_mentions_preserve_at: true,
        file_mentions_allow_explicit_paths: false,
        primary_accent: Some("#00AAFF".to_string()),
    };
    assert_eq!(tui.fork, expected);
    assert_eq!(
        (
            &tui.notification_settings.notifications,
            tui.animations,
            tui.theme.as_deref(),
        ),
        (&Notifications::Enabled(false), false, Some("dracula")),
    );

    let serialized = toml::Value::try_from(&tui).unwrap();
    let fork_values = toml::Value::try_from(&expected).unwrap();
    for (key, value) in fork_values.as_table().unwrap() {
        assert_eq!(serialized.get(key), Some(value));
    }
    assert_eq!(serialized.try_into::<Tui>().unwrap(), tui);
}

#[test]
fn layered_fork_settings_keep_unoverridden_values() {
    let mut base: toml::Value = toml::from_str(
        r##"
[tui]
chatbox_placeholder_tips = "off"
file_mentions_preserve_at = true
primary_accent = "#00AAFF"
notifications = false
"##,
    )
    .unwrap();
    let overlay = toml::from_str(
        r#"
[tui]
file_mentions_preserve_at = false
file_mentions_allow_explicit_paths = false
"#,
    )
    .unwrap();
    merge_toml_values(&mut base, &overlay);
    let parsed: ConfigToml = base.try_into().unwrap();
    let tui = parsed.tui.unwrap();
    assert_eq!(
        (tui.fork, tui.notification_settings.notifications),
        (
            ForkTuiOptions {
                chatbox_placeholder_tips: ChatboxPlaceholderTips::Off,
                file_mentions_preserve_at: false,
                file_mentions_allow_explicit_paths: false,
                primary_accent: Some("#00AAFF".to_string()),
            },
            Notifications::Enabled(false),
        ),
    );
}
