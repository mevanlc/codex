use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::models::ResponseItem;
#[cfg(test)]
use codex_protocol_test_support::ContentItemTestExt;

use crate::context::ContextualUserFragment;
use crate::context::UserShellCommand;
use crate::session::turn_context::TurnContext;
use crate::tools::format_exec_output_str;

fn user_shell_command_fragment(
    command: &str,
    exec_output: &ExecToolCallOutput,
    turn_context: &TurnContext,
) -> UserShellCommand {
    let output = format_exec_output_str(
        exec_output,
        turn_context.model_info().truncation_policy.into(),
    );
    UserShellCommand::new(command, exec_output.exit_code, exec_output.duration, output)
}

#[cfg(test)]
pub fn format_user_shell_command_record(
    command: &str,
    exec_output: &ExecToolCallOutput,
    turn_context: &TurnContext,
) -> String {
    user_shell_command_fragment(command, exec_output, turn_context)
        .render()
        .expect_text()
}

pub fn user_shell_command_record_item(
    command: &str,
    exec_output: &ExecToolCallOutput,
    turn_context: &TurnContext,
) -> ResponseItem {
    ContextualUserFragment::into(user_shell_command_fragment(
        command,
        exec_output,
        turn_context,
    ))
}

#[cfg(test)]
#[path = "user_shell_command_tests.rs"]
mod tests;

/// A managed terminal started by the user, available to subsequent model turns.
pub(crate) fn user_background_shell_started_record(command: &str, process_id: i32) -> ResponseItem {
    codex_context_fragments::RenderedFragment::new(
        "user",
        codex_context_fragments::AnnotatedContent::from_parts(
            codex_protocol::models::input_text(format!("<user_shell_command>\n<command>\n{command}\n</command>\n<result>\nStarted in background. Terminal session ID: {process_id}. Use write_stdin to poll output or send input.\n</result>\n</user_shell_command>")),
            codex_protocol::models::ContentItemKind("shell.user_command".to_string()),
            codex_protocol::models::ContentItemMetadata::harness(),
        ),
    ).into()
}
