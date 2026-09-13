//! Fork shell-follow-up presentation, using the upstream composer snapshot fixture.

use super::*;

#[test]
fn shell_follow_up_enabled_snapshot() {
    snapshot_composer_state(
        "footer_mode_shell_follow_up_enabled",
        /*enhanced_keys_supported*/ true,
        |composer| {
            composer.set_status_line_enabled(/*enabled*/ true);
            composer.set_status_line(Some(Line::from(
                "gpt-5.4 high fast · ~/code/codex-1 · Context 0% used",
            )));
            composer.set_text_content("!git status".to_string(), Vec::new(), Vec::new());
            let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        },
    );
}
