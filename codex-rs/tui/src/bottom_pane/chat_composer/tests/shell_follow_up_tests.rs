//! Fork shell-follow-up presentation, using the upstream composer snapshot fixture.

use super::*;
use pretty_assertions::assert_eq;

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

#[test]
fn repeated_tab_preserves_the_armed_shell_follow_up() {
    let (mut composer, _rx) = new_test_composer();
    composer.set_text_content("!ls".to_string(), Vec::new(), Vec::new());
    composer.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

    let (result, _) = composer.handle_key_event(KeyEvent::new_with_kind(
        KeyCode::Tab,
        KeyModifiers::NONE,
        KeyEventKind::Repeat,
    ));
    assert_eq!(result, InputResult::None);
    assert_eq!(
        composer.draft.shell_invocation_mode,
        ShellInvocationMode::FollowUp
    );

    let (result, _) = composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(
        result,
        InputResult::SubmittedShell {
            mode: ShellInvocationMode::FollowUp,
            ..
        }
    ));
}

#[test]
fn importing_a_slash_draft_clears_the_shell_follow_up() {
    let (mut composer, _rx) = new_test_composer();
    composer.set_text_content("!ls".to_string(), Vec::new(), Vec::new());
    composer.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(
        composer.draft.shell_invocation_mode,
        ShellInvocationMode::FollowUp
    );

    composer.set_text_content("/model".to_string(), Vec::new(), Vec::new());
    assert_eq!(composer.draft.textarea.text(), "/model");
    assert!(!composer.draft.is_bash_mode);
    assert_eq!(
        composer.draft.shell_invocation_mode,
        ShellInvocationMode::Normal
    );

    composer.set_text_content("!pwd".to_string(), Vec::new(), Vec::new());
    let (result, _) = composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(
        result,
        InputResult::SubmittedShell {
            mode: ShellInvocationMode::Normal,
            ..
        }
    ));
}

#[test]
fn shell_background_submission_and_repeat() {
    let (mut composer, _rx) = new_test_composer();
    composer.set_text_content("!echo hi".to_string(), Vec::new(), Vec::new());
    for _ in 0..2 {
        composer.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    }
    composer.handle_key_event(KeyEvent::new_with_kind(
        KeyCode::Tab,
        KeyModifiers::NONE,
        KeyEventKind::Repeat,
    ));
    let (result, _) = composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(
        result,
        InputResult::SubmittedShell {
            mode: ShellInvocationMode::Background,
            ..
        }
    ));
    assert_eq!(
        composer.draft.shell_invocation_mode,
        ShellInvocationMode::Normal
    );
}

#[test]
fn shell_background_footer_snapshot() {
    snapshot_composer_state("footer_mode_shell_background", true, |composer| {
        composer.set_text_content("!sleep 60".to_string(), Vec::new(), Vec::new());
        for _ in 0..2 {
            composer.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        }
    });
}
