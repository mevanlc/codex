use super::*;

#[test]
fn background_launch_record_keeps_shell_context_attribution() {
    let record = user_background_shell_started_record("sleep 60", 123);
    assert!(crate::context::is_guardian_context_message(&record));
    assert!(!crate::context::is_user_authorization_message(&record));
    let serialized = serde_json::to_string(&record).expect("serialize launch record");
    assert!(serialized.contains("Terminal session ID: 123"));
    assert!(serialized.contains("shell.user_command"));
}
use crate::context::ContextualUserFragment;
use crate::context::UserShellCommand;
use crate::session::tests::make_session_and_context;
use codex_protocol::exec_output::StreamOutput;
use codex_protocol::models::ContentItem;
use pretty_assertions::assert_eq;
use std::time::Duration;

#[test]
fn detects_user_shell_command_text_variants() {
    assert!(UserShellCommand::matches_text(
        "<user_shell_command>\necho hi\n</user_shell_command>"
    ));
    assert!(!UserShellCommand::matches_text("echo hi"));
}

#[tokio::test]
async fn formats_basic_record() {
    let exec_output = ExecToolCallOutput {
        exit_code: 0,
        stdout: StreamOutput::new("hi".to_string()),
        stderr: StreamOutput::new(String::new()),
        aggregated_output: StreamOutput::new("hi".to_string()),
        duration: Duration::from_secs(1),
        timed_out: false,
    };
    let (_, turn_context) = make_session_and_context().await;
    let item = user_shell_command_record_item("echo hi", &exec_output, &turn_context);
    let ResponseItem::Message { content, .. } = item else {
        panic!("expected message");
    };
    let [ContentItem::InputText { text }] = content.as_slice() else {
        panic!("expected input text");
    };
    assert_eq!(
        text,
        "<user_shell_command>\n<command>\necho hi\n</command>\n<result>\nExit code: 0\nDuration: 1.0000 seconds\nOutput:\nhi\n</result>\n</user_shell_command>"
    );
}

#[tokio::test]
async fn uses_aggregated_output_over_streams() {
    let exec_output = ExecToolCallOutput {
        exit_code: 42,
        stdout: StreamOutput::new("stdout-only".to_string()),
        stderr: StreamOutput::new("stderr-only".to_string()),
        aggregated_output: StreamOutput::new("combined output wins".to_string()),
        duration: Duration::from_millis(120),
        timed_out: false,
    };
    let (_, turn_context) = make_session_and_context().await;
    let record = format_user_shell_command_record("false", &exec_output, &turn_context);
    assert_eq!(
        record,
        "<user_shell_command>\n<command>\nfalse\n</command>\n<result>\nExit code: 42\nDuration: 0.1200 seconds\nOutput:\ncombined output wins\n</result>\n</user_shell_command>"
    );
}
