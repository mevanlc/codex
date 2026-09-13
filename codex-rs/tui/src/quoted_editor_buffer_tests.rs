use super::EditorBuffer;
use pretty_assertions::assert_eq;

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
    let buffer = EditorBuffer::new("Draft prompt", /*last_agent_response*/ None);

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
