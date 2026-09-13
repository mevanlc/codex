//! Draft editing with a removable quote of the last agent response.
//!
//! Keep this fork-specific buffer policy separate from the upstream editor launcher.

use std::ops::Range;

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

#[cfg(test)]
#[path = "quoted_editor_buffer_tests.rs"]
mod tests;
