#![cfg(test)]

use super::StatusLine;
use super::StatusLineFocus;
use crate::bottom_pane::StatusLineItem;
use crate::bottom_pane::status_line_from_segments;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::osc8_hyperlink;
use crate::terminal_hyperlinks::strip_osc8;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::layout::Size;
use ratatui::text::Line;

fn status_from_items(
    items: &[(StatusLineItem, &str)],
    truncatable: &[StatusLineItem],
    pull_request_url: Option<&str>,
) -> StatusLine {
    status_line_from_segments(
        items.iter().map(|(item, text)| (*item, (*text).to_owned())),
        &truncatable.iter().copied().collect(),
        /*use_theme_colors*/ true,
        /*thread_id*/ None,
        pull_request_url,
    )
    .unwrap()
}

fn paint(status: &StatusLine, width: u16, height: u16) -> Buffer {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    status.render(area, &mut buf, StatusLineFocus::Focused);
    buf
}

fn text_rows(buf: &Buffer) -> Vec<String> {
    buf.content
        .chunks(usize::from(buf.area.width))
        .map(|row| {
            row.iter()
                .map(|cell| strip_osc8(cell.symbol()))
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect()
}

#[test]
fn status_line_drops_rightmost_before_shortening_an_earlier_marked_item() {
    use StatusLineItem::*;

    let status = status_from_items(
        &[
            (ThreadName, "left-left"),
            (ModelName, "FIXED"),
            (CurrentDir, "right-right"),
        ],
        &[ThreadName, CurrentDir],
        /*pull_request_url*/ None,
    );
    for (width, expected) in [
        (31, "left-left · FIXED · right-right"),
        (26, "left-left · FIXED · right…"),
        (25, "left-left · FIXED · righ…"),
        (24, "left-left · FIXED"),
        (13, "left… · FIXED"),
        (7, "FIXED"),
        (31, "left-left · FIXED · right-right"),
    ] {
        assert_eq!(status.height(Size { width, height: 4 }), 1);
        assert_eq!(text_rows(&paint(&status, width, /*height*/ 1)), [expected]);
    }
}

#[test]
fn status_line_minimum_uses_the_actual_grapheme_width_with_ellipsis() {
    let item = StatusLineItem::ThreadName;
    for (input, width, expected) in [
        ("abcde", 5, "abcde"),
        ("abcde", 4, ""),
        ("abcd", 4, "abcd"),
        ("ABCD界", 5, "ABCD…"),
        ("abc界xyz", 5, ""),
        ("ab👩‍💻xyz", 5, "ab👩‍💻 …"),
    ] {
        let status = status_from_items(&[(item, input)], &[item], /*pull_request_url*/ None);
        let height = u16::from(!expected.is_empty());
        assert_eq!(status.height(Size { width, height: 2 }), height);
        assert_eq!(text_rows(&paint(&status, width, /*height*/ 1)), [expected]);
    }
}

#[test]
fn status_line_wraps_only_the_remaining_items_when_they_still_overflow() {
    use StatusLineItem::*;

    let status = status_from_items(
        &[(ThreadName, "one two three four"), (CurrentDir, "hide-me")],
        &[CurrentDir],
        /*pull_request_url*/ None,
    );
    assert_eq!(
        status.height(Size {
            width: 10,
            height: 2
        }),
        2
    );
    assert_eq!(
        text_rows(&paint(&status, /*width*/ 10, /*height*/ 2)),
        ["one two", "three four"],
    );
}

#[test]
fn fitted_status_line_rebuilds_links_and_separators_around_the_agent_label() {
    use StatusLineItem::*;

    let url = "https://example.com/pr/20252";
    let items = [(CurrentDir, "left-left"), (PullRequestNumber, "PR #20252")];
    let status = status_from_items(&items, &[CurrentDir, PullRequestNumber], Some(url))
        .with_suffix(Line::from("bot"));
    let buf = paint(&status, /*width*/ 23, /*height*/ 1);
    assert_eq!(text_rows(&buf), ["left-left · PR #… · bot"]);
    for (x, ch) in (12..16).zip("PR #".chars()) {
        assert_eq!(buf[(x, 0)].symbol(), osc8_hyperlink(url, &ch.to_string()));
    }
    assert_eq!(buf[(16, 0)].symbol(), "…");
    assert_eq!(buf[(16, 0)].style(), buf[(15, 0)].style());
    assert_eq!(
        text_rows(&paint(&status, /*width*/ 4, /*height*/ 1)),
        ["bot"]
    );

    let status = status_from_items(&items, &[CurrentDir], Some(url)).with_suffix(Line::from("bot"));
    let buf = paint(&status, /*width*/ 15, /*height*/ 1);
    assert_eq!(text_rows(&buf), ["PR #20252 · bot"]);
    for (x, ch) in (0..9).zip("PR #20252".chars()) {
        assert_eq!(buf[(x, 0)].symbol(), osc8_hyperlink(url, &ch.to_string()));
    }
}

#[test]
fn clipped_links_keep_their_cells_after_a_halfwidth_katakana_grapheme() {
    let project_url = "https://example.com/project";
    let pr_url = "https://example.com/pr/42";
    let mut line = HyperlinkLine::default();
    line.push_span("ｶﾞ".into(), Some(project_url));
    line.push_span(" · ".into(), /*destination*/ None);
    line.push_span("PR #42".into(), Some(pr_url));

    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 10, /*height*/ 1,
    );
    let mut buf = Buffer::empty(area);
    StatusLine::from(line).render(area, &mut buf, StatusLineFocus::Focused);
    assert_eq!(buf[(0, 0)].symbol(), osc8_hyperlink(project_url, "ｶﾞ"));
    for (x, ch) in (2..5).zip(" · ".chars()) {
        assert_eq!(buf[(x, 0)].symbol(), ch.to_string());
    }
    for (x, ch) in (5..9).zip("PR #".chars()) {
        assert_eq!(
            buf[(x, 0)].symbol(),
            osc8_hyperlink(pr_url, &ch.to_string())
        );
    }
    assert_eq!(buf[(9, 0)].symbol(), "…");
}

#[test]
fn arabic_prefix_keeps_link_on_the_painted_cells_at_exact_fit_and_overflow() {
    let url = "https://example.com/pr/42";
    let mut line = HyperlinkLine::from("لا · ");
    line.push_span("PR #42".into(), Some(url));
    assert_eq!(line.width(), 11);
    let content = StatusLine::from(line);

    for (width, visible_link) in [(11, "PR #42"), (10, "PR #")] {
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, /*height*/ 1);
        let mut buf = Buffer::empty(area);
        content.render(area, &mut buf, StatusLineFocus::Focused);
        assert_eq!(buf[(4, 0)].symbol(), " ");
        for (x, ch) in (5..width).zip(visible_link.chars()) {
            assert_eq!(buf[(x, 0)].symbol(), osc8_hyperlink(url, &ch.to_string()));
        }
        if width < 11 {
            assert_eq!(buf[(width - 1, 0)].symbol(), "…");
        }
    }
}

#[test]
fn prepared_tree_paints_only_the_footer_viewport() {
    use crate::ui_tree::UiTree;
    use ratatui::buffer::Cell;

    let url = "https://example.com/build";
    let tree: UiTree = serde_json::json!({
        "type":"link","props":{"href":url},"children":["ok\nlarge build\na\nb\nc\nd"]
    })
    .to_string()
    .parse()
    .unwrap();
    let prepared = StatusLine::from(tree);

    let area = Rect::new(
        /*x*/ 1, /*y*/ 1, /*width*/ 6, /*height*/ 4,
    );
    let mut buffer = Buffer::filled(
        Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 8, /*height*/ 7,
        ),
        Cell::new("."),
    );
    prepared.render(area, &mut buffer, StatusLineFocus::Focused);
    let rows: Vec<String> = buffer
        .content
        .chunks(8)
        .map(|row| row.iter().map(|cell| strip_osc8(cell.symbol())).collect())
        .collect();
    insta::assert_snapshot!(rows.join("\n"));

    assert_eq!(buffer[(1, 4)].symbol(), osc8_hyperlink(url, "a"));
    assert_eq!(buffer[(2, 4)].symbol(), "…");
}
