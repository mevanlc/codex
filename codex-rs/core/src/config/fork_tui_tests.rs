//! Fork-only config resolution cases, kept outside the shared upstream suite.

use super::*;
use codex_config::types::ChatboxPlaceholderTips;
use pretty_assertions::assert_eq;

#[test]
fn tui_primary_accent_deserializes_from_toml() {
    let cfg = r##"
[tui]
primary_accent = "#00AAFF"
"##;
    let parsed = toml::from_str::<ConfigToml>(cfg).expect("TOML deserialization should succeed");
    assert_eq!(
        parsed
            .tui
            .as_ref()
            .and_then(|t| t.fork.primary_accent.as_deref()),
        Some("#00AAFF"),
    );
}

#[tokio::test]
async fn runtime_config_resolves_fork_tui_options() {
    for (source, expected) in [
        ("", ForkTuiOptions::default()),
        ("[tui]", ForkTuiOptions::default()),
        (
            "[tui]\nfile_mentions_allow_explicit_paths = false",
            ForkTuiOptions {
                file_mentions_allow_explicit_paths: false,
                ..ForkTuiOptions::default()
            },
        ),
        (
            r##"
[tui]
chatbox_placeholder_tips = "off"
file_mentions_preserve_at = true
file_mentions_allow_explicit_paths = false
primary_accent = "#00AAFF"
"##,
            ForkTuiOptions {
                chatbox_placeholder_tips: ChatboxPlaceholderTips::Off,
                file_mentions_preserve_at: true,
                file_mentions_allow_explicit_paths: false,
                primary_accent: Some("#00AAFF".to_string()),
            },
        ),
    ] {
        let config = Config::load_from_base_config_with_overrides(
            toml::from_str(source).expect("fork settings deserialize"),
            ConfigOverrides::default(),
            tempdir().expect("tempdir").abs(),
        )
        .await
        .expect("load fork settings");
        assert_eq!(config.fork_tui, expected, "{source}");
    }
}
