use std::env;
use std::fs;
use std::ops::Range;
use std::process::Stdio;

use color_eyre::eyre::Report;
use color_eyre::eyre::Result;
use tempfile::Builder;
use thiserror::Error;
use tokio::process::Command;

#[derive(Debug, Error)]
pub(crate) enum EditorError {
    #[error("neither VISUAL nor EDITOR is set")]
    MissingEditor,
    #[cfg(not(windows))]
    #[error("failed to parse editor command")]
    ParseFailed,
    #[error("editor command is empty")]
    EmptyCommand,
}

pub(crate) struct EditorBuffer {
    initial_text: String,
    agent_quote: Option<String>,
}

impl EditorBuffer {
    pub(crate) fn new(draft: &str, last_agent_response: Option<&str>) -> Self {
        let agent_quote = last_agent_response
            .filter(|response| !response.is_empty())
            .map(|response| {
                response
                    .lines()
                    .map(|line| format!("> {line}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            });
        let initial_text = match (draft.is_empty(), agent_quote.as_deref()) {
            (_, None) => draft.to_string(),
            (true, Some(agent_quote)) => agent_quote.to_string(),
            (false, Some(agent_quote)) => format!("{draft}\n\n{agent_quote}"),
        };
        Self {
            initial_text,
            agent_quote,
        }
    }

    pub(crate) fn initial_text(&self) -> &str {
        &self.initial_text
    }

    pub(crate) fn edited_prompt(&self, edited_text: &str) -> String {
        let mut prompt = edited_text.to_string();
        if let Some(agent_quote) = self.agent_quote.as_deref()
            && let Some(range) = find_whitespace_insensitive_block(&prompt, agent_quote)
        {
            prompt.replace_range(range, "");
        }
        prompt.trim_end().to_string()
    }
}

fn find_whitespace_insensitive_block(haystack: &str, needle: &str) -> Option<Range<usize>> {
    haystack
        .char_indices()
        .filter_map(|(start, _)| whitespace_insensitive_match_at(haystack, needle, start))
        .filter(|range| match_starts_on_line(haystack, range.start))
        .rfind(|range| match_ends_on_line(haystack, range.end))
}

fn match_starts_on_line(text: &str, start: usize) -> bool {
    text[..start].rsplit_once('\n').map_or_else(
        || text[..start].chars().all(char::is_whitespace),
        |(_, prefix)| prefix.chars().all(char::is_whitespace),
    )
}

fn match_ends_on_line(text: &str, end: usize) -> bool {
    text[end..].split_once('\n').map_or_else(
        || text[end..].chars().all(char::is_whitespace),
        |(suffix, _)| suffix.chars().all(char::is_whitespace),
    )
}

fn whitespace_insensitive_match_at(
    haystack: &str,
    needle: &str,
    start: usize,
) -> Option<Range<usize>> {
    let mut haystack_chars = haystack[start..].char_indices().peekable();
    let mut needle_chars = needle.chars().peekable();
    let mut end = start;

    while let Some(needle_char) = needle_chars.next() {
        if needle_char.is_whitespace() {
            while needle_chars.peek().is_some_and(|ch| ch.is_whitespace()) {
                needle_chars.next();
            }
            let (offset, haystack_char) = haystack_chars.next()?;
            if !haystack_char.is_whitespace() {
                return None;
            }
            end = start + offset + haystack_char.len_utf8();
            while let Some((offset, haystack_char)) =
                haystack_chars.next_if(|(_, haystack_char)| haystack_char.is_whitespace())
            {
                end = start + offset + haystack_char.len_utf8();
            }
        } else {
            let (offset, haystack_char) = haystack_chars.next()?;
            if haystack_char != needle_char {
                return None;
            }
            end = start + offset + haystack_char.len_utf8();
        }
    }

    Some(start..end)
}

/// Tries to resolve the full path to a Windows program, respecting PATH + PATHEXT.
/// Falls back to the original program name if resolution fails.
#[cfg(windows)]
fn resolve_windows_program(program: &str) -> std::path::PathBuf {
    // On Windows, `Command::new("code")` will not resolve `code.cmd` shims on PATH.
    // Use `which` so we respect PATH + PATHEXT (e.g., `code` -> `code.cmd`).
    which::which(program).unwrap_or_else(|_| std::path::PathBuf::from(program))
}

/// Resolve the editor command from environment variables.
/// Prefers `VISUAL` over `EDITOR`.
pub(crate) fn resolve_editor_command() -> std::result::Result<Vec<String>, EditorError> {
    let raw = env::var("VISUAL")
        .or_else(|_| env::var("EDITOR"))
        .map_err(|_| EditorError::MissingEditor)?;
    let parts = {
        #[cfg(windows)]
        {
            winsplit::split(&raw)
        }
        #[cfg(not(windows))]
        {
            shlex::split(&raw).ok_or(EditorError::ParseFailed)?
        }
    };
    if parts.is_empty() {
        return Err(EditorError::EmptyCommand);
    }
    Ok(parts)
}

/// Write `seed` to a temp file, launch the editor command, and return the updated content.
pub(crate) async fn run_editor(seed: &str, editor_cmd: &[String]) -> Result<String> {
    if editor_cmd.is_empty() {
        return Err(Report::msg("editor command is empty"));
    }

    // Convert to TempPath immediately so no file handle stays open on Windows.
    let temp_path = Builder::new().suffix(".md").tempfile()?.into_temp_path();
    fs::write(&temp_path, seed)?;

    let mut cmd = {
        #[cfg(windows)]
        {
            // handles .cmd/.bat shims
            Command::new(resolve_windows_program(&editor_cmd[0]))
        }
        #[cfg(not(windows))]
        {
            Command::new(&editor_cmd[0])
        }
    };
    if editor_cmd.len() > 1 {
        cmd.args(&editor_cmd[1..]);
    }
    let status = cmd
        .arg(&temp_path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await?;

    if !status.success() {
        return Err(Report::msg(format!("editor exited with status {status}")));
    }

    let contents = fs::read_to_string(&temp_path)?;
    Ok(contents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serial_test::serial;
    #[cfg(unix)]
    use tempfile::tempdir;

    struct EnvGuard {
        visual: Option<String>,
        editor: Option<String>,
    }

    impl EnvGuard {
        fn new() -> Self {
            Self {
                visual: env::var("VISUAL").ok(),
                editor: env::var("EDITOR").ok(),
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            restore_env("VISUAL", self.visual.take());
            restore_env("EDITOR", self.editor.take());
        }
    }

    fn restore_env(key: &str, value: Option<String>) {
        match value {
            Some(val) => unsafe { env::set_var(key, val) },
            None => unsafe { env::remove_var(key) },
        }
    }

    #[test]
    #[serial]
    fn resolve_editor_prefers_visual() {
        let _guard = EnvGuard::new();
        unsafe {
            env::set_var("VISUAL", "vis");
            env::set_var("EDITOR", "ed");
        }
        let cmd = resolve_editor_command().unwrap();
        assert_eq!(cmd, vec!["vis".to_string()]);
    }

    #[test]
    #[serial]
    fn resolve_editor_errors_when_unset() {
        let _guard = EnvGuard::new();
        unsafe {
            env::remove_var("VISUAL");
            env::remove_var("EDITOR");
        }
        assert!(matches!(
            resolve_editor_command(),
            Err(EditorError::MissingEditor)
        ));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn run_editor_returns_updated_content() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let script_path = dir.path().join("edit.sh");
        fs::write(&script_path, "#!/bin/sh\nprintf \"edited\" > \"$1\"\n").unwrap();
        let mut perms = fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).unwrap();

        let cmd = vec![script_path.to_string_lossy().to_string()];
        let result = run_editor("seed", &cmd).await.unwrap();
        assert_eq!(result, "edited".to_string());
    }

    #[test]
    fn quoted_buffer_places_draft_above_line_prefixed_agent_response() {
        let buffer = EditorBuffer::new(
            "Please revise the explanation.",
            Some("First paragraph.\n\n- first item\n- second item"),
        );

        assert_eq!(
            buffer.initial_text(),
            "Please revise the explanation.\n\n> First paragraph.\n> \n> - first item\n> - second item"
        );
    }

    #[test]
    fn quoted_buffer_omits_separator_when_draft_is_empty() {
        let buffer = EditorBuffer::new("", Some("Agent response"));

        assert_eq!(buffer.initial_text(), "> Agent response");
        assert_eq!(buffer.edited_prompt(buffer.initial_text()), "");
    }

    #[test]
    fn draft_only_buffer_preserves_existing_external_editor_behavior() {
        let buffer = EditorBuffer::new("Draft prompt", None);

        assert_eq!(buffer.initial_text(), "Draft prompt");
        assert_eq!(buffer.edited_prompt("Edited prompt\n\n"), "Edited prompt");
    }

    #[test]
    fn edited_prompt_removes_agent_quote_with_unicode_whitespace_differences() {
        let buffer = EditorBuffer::new("Original prompt", Some("First  line\nSecond\tline"));
        let edited = "Revised prompt\n\n> First\u{2003}line \n\t> Second line\n";

        assert_eq!(buffer.edited_prompt(edited), "Revised prompt");
    }

    #[test]
    fn edited_prompt_keeps_quote_when_non_whitespace_content_changes() {
        let buffer = EditorBuffer::new("", Some("Agent response"));
        let edited = "New prompt\n\n> Agent changed response";

        assert_eq!(buffer.edited_prompt(edited), edited);
    }

    #[test]
    fn edited_prompt_does_not_remove_quote_prefix_from_a_longer_line() {
        let buffer = EditorBuffer::new("", Some("Agent response"));
        let edited = "> Agent response with an addition";

        assert_eq!(buffer.edited_prompt(edited), edited);
    }
}
