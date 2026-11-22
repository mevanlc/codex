//! Measure and paint native and extension status lines within the caller's content rectangle.

use super::status_line_style::STATUS_LINE_SEPARATOR;
use crate::line_truncation::line_width;
use crate::line_truncation::span_width;
use crate::line_truncation::truncate_line_to_width;
use crate::line_truncation::truncate_line_with_ellipsis;
use crate::render::highlight::with_syntax_theme;
use crate::render::line_utils::line_to_static;
use crate::style::secondary_text_style;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::project_hyperlink_columns;
use crate::ui_tree::PreparedUiTree;
use crate::ui_tree::UiTree;
use crate::width::display_width;
use crate::wrapping::adaptive_wrap_line_to_width;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::layout::Size;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use std::borrow::Cow;
use std::cell::Ref;
use std::cell::RefCell;
use std::sync::Arc;
use std::sync::OnceLock;
use unicode_segmentation::UnicodeSegmentation;

const MIN_TRUNCATED_COMPONENT_WIDTH: usize = 5;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatusLineFocus {
    Focused,
    Unfocused,
}

/// A status line and the cached layout for its content.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StatusLine(StatusContent);

#[derive(Clone, Debug, PartialEq)]
enum StatusContent {
    Native(CachedLayout<Arc<NativeStatusLine>, NativeLayout>),
    Extension(CachedLayout<Arc<PreparedUiTree>, u16>),
}

#[derive(Debug, PartialEq)]
enum NativeStatusLine {
    Plain(HyperlinkLine),
    Components(NativeComponents),
}

#[derive(Debug, PartialEq)]
pub(super) struct StatusLineComponent {
    pub(super) span: Span<'static>,
    pub(super) destination: Option<String>,
    pub(super) truncatable: bool,
}

#[derive(Debug)]
struct NativeComponents {
    items: Arc<[StatusLineComponent]>,
    separator: Span<'static>,
    suffix: Line<'static>,
    full_line: OnceLock<HyperlinkLine>,
}

impl PartialEq for NativeComponents {
    fn eq(&self, other: &Self) -> bool {
        self.items == other.items
            && self.separator == other.separator
            && self.suffix == other.suffix
    }
}

#[derive(Debug)]
enum NativeLayout {
    Original,
    Fitted(HyperlinkLine),
    Wrapped(Vec<HyperlinkLine>),
}

#[derive(Debug)]
struct CachedLayout<C, L> {
    content: C,
    layout: RefCell<Option<(Size, L)>>,
}

impl<C, L> CachedLayout<C, L> {
    fn new(content: C) -> Self {
        Self {
            content,
            layout: RefCell::default(),
        }
    }

    fn get(&self, bounds: Size, compute: impl FnOnce(&C) -> L) -> Ref<'_, L> {
        if self
            .layout
            .borrow()
            .as_ref()
            .is_none_or(|(previous, _)| *previous != bounds)
        {
            self.layout.replace(Some((bounds, compute(&self.content))));
        }
        Ref::map(self.layout.borrow(), |entry| {
            let Some((_, layout)) = entry else {
                unreachable!("layout cache was populated");
            };
            layout
        })
    }
}

// Cached geometry belongs to the current surface. Clones and comparisons use only the content.
impl<C: Clone, L> Clone for CachedLayout<C, L> {
    fn clone(&self) -> Self {
        Self::new(self.content.clone())
    }
}

impl<C: PartialEq, L> PartialEq for CachedLayout<C, L> {
    fn eq(&self, other: &Self) -> bool {
        self.content == other.content
    }
}

impl From<UiTree> for StatusLine {
    fn from(tree: UiTree) -> Self {
        Self(StatusContent::Extension(CachedLayout::new(Arc::new(
            tree.prepare(),
        ))))
    }
}

impl From<HyperlinkLine> for StatusLine {
    fn from(line: HyperlinkLine) -> Self {
        Self::native(NativeStatusLine::Plain(line))
    }
}

impl From<Line<'static>> for StatusLine {
    fn from(line: Line<'static>) -> Self {
        Self::from(HyperlinkLine::from(line))
    }
}

impl From<&'static str> for StatusLine {
    fn from(text: &'static str) -> Self {
        Self::from(Line::from(text))
    }
}

impl StatusLine {
    fn native(line: NativeStatusLine) -> Self {
        Self(StatusContent::Native(CachedLayout::new(Arc::new(line))))
    }

    pub(super) fn from_components(items: Vec<StatusLineComponent>) -> Option<Self> {
        if items.is_empty() {
            return None;
        }
        let separator = Span::styled(STATUS_LINE_SEPARATOR, secondary_text_style());
        if items.iter().any(|item| item.truncatable) {
            return Some(Self::native(NativeStatusLine::Components(
                NativeComponents {
                    items: items.into(),
                    separator,
                    suffix: Line::default(),
                    full_line: OnceLock::new(),
                },
            )));
        }
        let mut line = HyperlinkLine::default();
        for item in items {
            append_component(
                &mut line,
                &separator,
                item.span,
                item.destination.as_deref(),
            );
        }
        Some(line.into())
    }

    pub(super) fn is_native(&self) -> bool {
        matches!(self.0, StatusContent::Native(_))
    }

    #[cfg(test)]
    pub(crate) fn as_native(&self) -> Option<&HyperlinkLine> {
        match &self.0 {
            StatusContent::Native(native) => Some(native.content.full_line()),
            StatusContent::Extension(_) => None,
        }
    }

    pub(super) fn with_suffix(&self, suffix: Line<'static>) -> Self {
        if suffix.spans.is_empty() {
            return self.clone();
        }
        let StatusContent::Native(native) = &self.0 else {
            return self.clone();
        };
        match native.content.as_ref() {
            NativeStatusLine::Plain(line) => {
                let mut line = line.clone();
                let separator = Span::styled(STATUS_LINE_SEPARATOR, secondary_text_style());
                append_suffix(&mut line, &separator, &suffix);
                line.into()
            }
            NativeStatusLine::Components(components) => {
                Self::native(NativeStatusLine::Components(NativeComponents {
                    items: Arc::clone(&components.items),
                    separator: components.separator.clone(),
                    suffix,
                    full_line: OnceLock::new(),
                }))
            }
        }
    }

    /// Fit a native row before applying the one-row reasoning-effort transition.
    pub(super) fn native_line_for_width(&self, width: u16) -> Option<HyperlinkLine> {
        let StatusContent::Native(native) = &self.0 else {
            return None;
        };
        let bounds = Size { width, height: 1 };
        match &*native_layout(native, bounds) {
            NativeLayout::Original => Some(native.content.full_line().clone()),
            NativeLayout::Fitted(line) => Some(line.clone()),
            NativeLayout::Wrapped(rows) => rows.first().cloned(),
        }
    }

    /// Measure within the space available for content after the caller applies padding.
    pub(crate) fn height(&self, bounds: Size) -> u16 {
        if bounds.width == 0 || bounds.height == 0 {
            return 0;
        }
        match &self.0 {
            StatusContent::Native(native) => match &*native_layout(native, bounds) {
                NativeLayout::Original => u16::from(native.content.full_line().width() > 0),
                NativeLayout::Fitted(line) => u16::from(line.width() > 0),
                NativeLayout::Wrapped(rows) => {
                    rows.len().max(1).min(usize::from(bounds.height)) as u16
                }
            },
            StatusContent::Extension(extension) => {
                *extension.get(bounds, |tree| tree.measure(bounds).height)
            }
        }
    }

    /// Paint within the final content rectangle after the caller applies padding.
    pub(crate) fn render(&self, area: Rect, buf: &mut Buffer, focus: StatusLineFocus) {
        if area.is_empty() {
            return;
        }
        match &self.0 {
            StatusContent::Native(native) => {
                let layout = native_layout(native, area.as_size());
                let rows = match &*layout {
                    NativeLayout::Original => std::slice::from_ref(native.content.full_line()),
                    NativeLayout::Fitted(line) => std::slice::from_ref(line),
                    NativeLayout::Wrapped(rows) => rows.as_slice(),
                };
                let more_rows = rows.len() > usize::from(area.height);
                for (row, line) in area.rows().zip(rows) {
                    render_status_row(
                        line,
                        row,
                        buf,
                        focus,
                        more_rows && row.bottom() == area.bottom(),
                    );
                }
            }
            StatusContent::Extension(extension) => {
                let plain_style =
                    (focus == StatusLineFocus::Unfocused).then(|| Style::reset().dim());
                with_syntax_theme(|theme| extension.content.render(area, theme, buf, plain_style));
            }
        }
    }
}

impl NativeStatusLine {
    fn full_line(&self) -> &HyperlinkLine {
        match self {
            Self::Plain(line) => line,
            Self::Components(components) => components.full_line.get_or_init(|| {
                let mut line = HyperlinkLine::default();
                for item in components.items.iter() {
                    append_component(
                        &mut line,
                        &components.separator,
                        item.span.clone(),
                        item.destination.as_deref(),
                    );
                }
                append_suffix(&mut line, &components.separator, &components.suffix);
                line
            }),
        }
    }
}

impl NativeComponents {
    fn fit(&self, width: usize) -> Option<HyperlinkLine> {
        let separator_width = span_width(&self.separator);
        let suffix_width = line_width(&self.suffix);
        let mut count = self.items.len();
        let mut body_width = self
            .items
            .iter()
            .map(|item| span_width(&item.span))
            .sum::<usize>()
            + count.saturating_sub(1) * separator_width;
        let combined_width = |body| {
            body + suffix_width
                + if body > 0 && suffix_width > 0 {
                    separator_width
                } else {
                    0
                }
        };
        let mut total = combined_width(body_width);
        if total <= width {
            return None;
        }

        let mut dropped = Vec::new();
        let mut shortened = None;
        for (index, item) in self
            .items
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, item)| item.truncatable)
        {
            let item_width = span_width(&item.span);
            let available = width.saturating_sub(total - item_width);
            if available >= MIN_TRUNCATED_COMPONENT_WIDTH {
                // The ellipsis takes one terminal column. Grapheme boundaries can leave
                // another column unused, so check the actual prefix before keeping it.
                let prefix = truncate_line_to_width(Line::from(item.span.clone()), available - 1);
                if let Some(span) = prefix.spans.into_iter().next()
                    && span_width(&span) + 1 >= MIN_TRUNCATED_COMPONENT_WIDTH
                {
                    shortened = Some((index, span));
                    break;
                }
            }

            dropped.push(index);
            count -= 1;
            body_width -= item_width + if count > 0 { separator_width } else { 0 };
            total = combined_width(body_width);
            if total <= width {
                break;
            }
        }

        let mut line = HyperlinkLine::default();
        for (index, item) in self.items.iter().enumerate() {
            if dropped.last() == Some(&index) {
                dropped.pop();
                continue;
            }
            let prefix = shortened.as_ref().filter(|(target, _)| *target == index);
            let span = prefix.map_or(&item.span, |(_, span)| span);
            append_component(
                &mut line,
                &self.separator,
                span.clone(),
                item.destination.as_deref(),
            );
            if prefix.is_some() {
                line.line.push_span(Span::styled("…", item.span.style));
            }
        }
        append_suffix(&mut line, &self.separator, &self.suffix);
        Some(line)
    }
}

fn append_component(
    line: &mut HyperlinkLine,
    separator: &Span<'static>,
    span: Span<'static>,
    destination: Option<&str>,
) {
    if !line.line.spans.is_empty() {
        line.line.push_span(separator.clone());
    }
    line.push_span(span, destination);
}

fn append_suffix(line: &mut HyperlinkLine, separator: &Span<'static>, suffix: &Line<'static>) {
    if suffix.spans.is_empty() {
        return;
    }
    if line.width() > 0 && line_width(suffix) > 0 {
        line.line.push_span(separator.clone());
    }
    line.line.spans.extend(suffix.spans.iter().cloned());
}

fn native_layout(
    native: &CachedLayout<Arc<NativeStatusLine>, NativeLayout>,
    mut bounds: Size,
) -> Ref<'_, NativeLayout> {
    // Reuse the same wrapped prefix across small height changes.
    if bounds.height > 1 {
        bounds.height = bounds.height.max(4);
    }
    native.get(bounds, |native| {
        let width = usize::from(bounds.width);
        let fitted = match native.as_ref() {
            NativeStatusLine::Plain(_) => None,
            NativeStatusLine::Components(components) => components.fit(width),
        };
        let line = fitted.as_ref().unwrap_or_else(|| native.full_line());
        if bounds.height > 1 && line.width() > width {
            NativeLayout::Wrapped(wrap_rows(line, bounds))
        } else {
            fitted.map_or(NativeLayout::Original, NativeLayout::Fitted)
        }
    })
}

fn render_status_row(
    line: &HyperlinkLine,
    area: Rect,
    buf: &mut Buffer,
    focus: StatusLineFocus,
    more_rows: bool,
) {
    let mut line = Cow::Borrowed(line);
    let width = usize::from(area.width);
    if more_rows || line.width() > width {
        let line = line.to_mut();
        line.line = truncate_line_with_ellipsis(std::mem::take(&mut line.line), width);
        line.clip_hyperlinks_to_width(line.width().saturating_sub(1));
    }
    if focus == StatusLineFocus::Unfocused {
        let line = line.to_mut();
        line.hyperlinks.clear();
        line.line.style = Style::reset().dim();
        for span in &mut line.line.spans {
            span.style = Style::default();
        }
    }
    line.render_fitted(area, buf);
}

fn wrap_rows(status: &HyperlinkLine, bounds: Size) -> Vec<HyperlinkLine> {
    let line = &status.line;
    let width = usize::from(bounds.width);
    // One row of lookahead settles the last visible break. Whitespace discarded by
    // wrapping cannot count toward that budget or it could hide a later word.
    let limit = usize::from(bounds.height) + 1;
    let mut remaining = width * limit;
    let mut prefix = Line::default().style(line.style);
    for span in &line.spans {
        let mut end = 0;
        for grapheme in span.content.graphemes(true) {
            end += grapheme.len();
            if !grapheme.chars().all(char::is_whitespace) {
                remaining = remaining.saturating_sub(display_width(grapheme).min(width));
                if remaining == 0 {
                    break;
                }
            }
        }
        prefix.push_span(Span::styled(&span.content[..end], span.style));
        if remaining == 0 {
            break;
        }
    }
    // Native link columns are counted per grapheme within each painted span.
    let mut graphemes = prefix
        .spans
        .iter()
        .flat_map(|span| span.content.graphemes(true));
    let (mut byte, mut column) = (0, 0);
    let mut column_at = |target| {
        while byte < target
            && let Some(grapheme) = graphemes.next()
        {
            byte += grapheme.len();
            column += display_width(grapheme);
        }
        column
    };
    adaptive_wrap_line_to_width(&prefix, width.into())
        .into_iter()
        .take(limit)
        .map(|row| {
            let source = column_at(row.range.start)..column_at(row.range.end);
            HyperlinkLine {
                line: line_to_static(&row.line),
                hyperlinks: project_hyperlink_columns(
                    &status.hyperlinks,
                    source,
                    /*prefix*/ 0,
                ),
                source: None,
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "status_line_tests.rs"]
mod tests;
