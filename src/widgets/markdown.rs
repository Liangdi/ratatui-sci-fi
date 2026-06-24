//! **Markdown** — render CommonMark into styled, word-wrapped ratatui lines.
//!
//! A small renderer backed by [pulldown-cmark] that turns a markdown string into
//! a `Vec<Line>` of styled spans, wrapped to a given width. It powers
//! [`CommLog`](crate::CommLog)'s chat style (the AI-agent chatbox) and is also
//! exposed as a standalone [`Markdown`] widget.
//!
//! ## Supported subset
//!
//! CommonMark paragraphs, ATX headings (`#`–`######`), **strong**, *emphasis*,
//! ~~strikethrough~~, `` `inline code` ``, fenced ```` ``` ```` code blocks,
//! ordered / unordered lists (one level indented per nest), block quotes, and
//! thematic breaks (`---`). Links render as their (underlined) text. Anything
//! more exotic degrades to plain paragraph text.
//!
//! ## Styling
//!
//! All colors come from [`Theme::palette`] — headings + bullets are `accent`,
//! inline/code are `accent2` on a `panel` backdrop, the quote bar is `muted`,
//! body text is `fg`. So the output recolors cleanly across every theme.
//!
//! [pulldown-cmark]: https://crates.io/crates/pulldown-cmark

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::Theme;

/// Render `md` into styled, word-wrapped lines no wider than `width` (in cells).
///
/// Pure function — no terminal I/O. Use it directly, or via the [`Markdown`]
/// widget. An empty / whitespace-only `md` yields a single empty line so callers
/// always get at least one row to render into.
#[must_use]
pub fn markdown_to_lines(md: &str, theme: Theme, width: u16) -> Vec<Line<'static>> {
    let mut r = Renderer {
        palette: theme.palette(),
        width: width.max(1) as usize,
        out: Vec::new(),
        inline: Vec::new(),
        bold: false,
        italic: false,
        strike: false,
        link: false,
        heading: None,
        quote_depth: 0,
        list_stack: Vec::new(),
        item_active: false,
        code_block: None,
    };
    r.run(md);
    if r.out.is_empty() {
        r.out.push(Line::default());
    }
    r.out
}

/// A stateless markdown widget — render a markdown string into an area.
///
/// ```no_run
/// use ratatui_sci_fi::{Markdown, Theme};
/// let w = Markdown::new("# Hello\nrendered **bold**").theme(Theme::Cyberpunk);
/// // f.render_widget(w, area);
/// ```
#[derive(Debug, Clone, Default)]
pub struct Markdown {
    /// The markdown source.
    pub text: String,
    /// Theme whose palette drives colors. Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Markdown {
    /// Create a widget rendering `text`.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), theme: Theme::default() }
    }

    /// Set the theme whose palette drives colors.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

impl Widget for Markdown {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let lines = markdown_to_lines(&self.text, self.theme, area.width);
        for (i, line) in lines.iter().enumerate() {
            let y = area.y + i as u16;
            if y >= area.bottom() {
                break;
            }
            let mut x = area.x;
            let right = area.x + area.width;
            for span in &line.spans {
                for ch in span.content.chars() {
                    if x >= right {
                        break;
                    }
                    buf[(x, y)].set_char(ch).set_style(span.style);
                    x += 1;
                }
            }
        }
    }
}

// ─── renderer ────────────────────────────────────────────────────────────────

type CharStyle = (char, Style);

struct Renderer {
    palette: crate::Palette,
    width: usize,
    out: Vec<Line<'static>>,
    /// Accumulated inline content for the current paragraph / heading / list
    /// item, as `(char, Style)` pairs. `'\n'` marks a hard line break.
    inline: Vec<CharStyle>,
    bold: bool,
    italic: bool,
    strike: bool,
    link: bool,
    /// Active heading level (1..=6), if any.
    heading: Option<u8>,
    quote_depth: usize,
    list_stack: Vec<(bool, usize)>, // (is_ordered, counter)
    item_active: bool,
    /// Accumulating fenced-code-block text (raw, including newlines).
    code_block: Option<String>,
}

impl Renderer {
    fn run(&mut self, md: &str) {
        use pulldown_cmark::{Options, Parser};

        let mut opts = Options::empty();
        opts.insert(Options::ENABLE_STRIKETHROUGH);
        for event in Parser::new_ext(md, opts) {
            self.event(event);
        }
        // Flush any trailing inline / code block.
        self.flush_inline();
        if let Some(code) = self.code_block.take() {
            self.push_code_block(&code);
        }
    }

    fn event(&mut self, event: pulldown_cmark::Event) {
        use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Tag, TagEnd};

        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => {}
                Tag::Heading { level, .. } => {
                    self.heading = Some(match level {
                        HeadingLevel::H1 => 1,
                        HeadingLevel::H2 => 2,
                        HeadingLevel::H3 => 3,
                        HeadingLevel::H4 => 4,
                        HeadingLevel::H5 => 5,
                        HeadingLevel::H6 => 6,
                    });
                }
                Tag::Strong => self.bold = true,
                Tag::Emphasis => self.italic = true,
                Tag::Strikethrough => self.strike = true,
                Tag::CodeBlock(kind) => {
                    // End any pending inline before the code block.
                    self.flush_inline();
                    let _lang = match kind {
                        CodeBlockKind::Fenced(l) => l.into_string(),
                        CodeBlockKind::Indented => String::new(),
                    };
                    self.code_block = Some(String::new());
                }
                Tag::List(start) => {
                    let ordered = start.is_some();
                    // Start the counter one below the first number so the first
                    // item increments to the user's start value.
                    let mut begin = 0usize;
                    if let Some(n) = start {
                        begin = list_start_to_usize(n).saturating_sub(1);
                    }
                    self.list_stack.push((ordered, begin));
                }
                Tag::Item => {
                    self.flush_inline();
                    self.item_active = true;
                    if let Some(top) = self.list_stack.last_mut()
                        && top.0
                    {
                        top.1 += 1;
                    }
                }
                Tag::BlockQuote(kind) => {
                    let _ = kind;
                    self.flush_inline();
                    self.quote_depth += 1;
                }
                Tag::Link { .. } => self.link = true,
                _ => {}
            },
            Event::End(end) => match end {
                TagEnd::Paragraph | TagEnd::Heading(_) => self.flush_inline(),
                TagEnd::Strong => self.bold = false,
                TagEnd::Emphasis => self.italic = false,
                TagEnd::Strikethrough => self.strike = false,
                TagEnd::CodeBlock => {
                    if let Some(code) = self.code_block.take() {
                        self.push_code_block(&code);
                    }
                }
                TagEnd::List(_) => {
                    self.flush_inline();
                    self.list_stack.pop();
                }
                TagEnd::Item => {
                    self.flush_inline();
                    self.item_active = false;
                }
                TagEnd::BlockQuote(_) => {
                    self.flush_inline();
                    self.quote_depth = self.quote_depth.saturating_sub(1);
                }
                TagEnd::Link => self.link = false,
                _ => {}
            },
            Event::Text(s) => {
                if let Some(buf) = self.code_block.as_mut() {
                    buf.push_str(s.as_ref());
                } else {
                    let style = self.text_style();
                    for ch in s.chars() {
                        self.inline.push((ch, style));
                    }
                }
            }
            Event::Code(s) => {
                let style = self.code_style();
                for ch in s.chars() {
                    self.inline.push((ch, style));
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if self.code_block.is_none() {
                    // Hard line break inside the current inline block.
                    self.inline.push(('\n', self.text_style()));
                }
            }
            Event::Rule => {
                self.flush_inline();
                let rule = "─".repeat(self.width.max(1));
                self.out.push(Line::from(vec![Span::styled(
                    rule,
                    Style::new().fg(self.palette.muted.color()),
                )]));
            }
            _ => {}
        }
    }

    /// The style for a run of body text given the current inline flags.
    fn text_style(&self) -> Style {
        let fg = match self.heading {
            Some(_) => self.palette.accent.color(),
            None if self.quote_depth > 0 => self.palette.fg.color(),
            None => self.palette.fg.color(),
        };
        let mut style = Style::new().fg(fg);
        if self.heading.is_some() || self.bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        if self.italic {
            style = style.add_modifier(Modifier::ITALIC);
        }
        if self.strike {
            style = style.add_modifier(Modifier::CROSSED_OUT);
        }
        if self.link {
            style = style.add_modifier(Modifier::UNDERLINED);
        }
        if self.quote_depth > 0 {
            style = style.add_modifier(Modifier::DIM);
        }
        style
    }

    /// Inline `` `code` `` style: bright accent2 on the panel backdrop.
    fn code_style(&self) -> Style {
        Style::new()
            .fg(self.palette.accent2.color())
            .bg(self.palette.panel.color())
            .add_modifier(Modifier::BOLD)
    }

    fn flush_inline(&mut self) {
        if self.inline.is_empty() && self.heading.is_none() && !self.item_active {
            // Nothing accumulated and not a heading/item start — nothing to draw.
            return;
        }
        let inline = std::mem::take(&mut self.inline);

        // Two independent prefix parts: the quote bar prefixes *every* wrapped
        // line, while the list bullet/number prefixes only the first line
        // (continuation lines align under the body with spaces).
        let quote_pfx = "▏ ".repeat(self.quote_depth);
        let list_pfx: String = if let Some((ordered, counter)) = self.list_stack.last().copied() {
            let depth = self.list_stack.len();
            let mut s = String::new();
            for _ in 0..depth.saturating_sub(1) {
                s.push_str("  ");
            }
            if ordered {
                s.push_str(&format!("{counter}. "));
            } else {
                s.push_str("• ");
            }
            s
        } else {
            String::new()
        };
        let first_pfx = format!("{quote_pfx}{list_pfx}");
        let cont_pfx = format!("{quote_pfx}{}", " ".repeat(list_pfx.chars().count()));
        let pfx_w = first_pfx.chars().count();
        let body_w = self.width.saturating_sub(pfx_w).max(1);
        let quote_style = Style::new().fg(self.palette.muted.color());
        let list_style = Style::new().fg(self.palette.accent.color());

        // Heading decoration: `#` markers + a space before the text.
        let decorated: Vec<CharStyle> = if let Some(level) = self.heading {
            let hashes = "#".repeat(level as usize);
            let hs = self.text_style();
            hashes
                .chars()
                .map(|c| (c, hs))
                .chain(std::iter::once((' ', hs)))
                .chain(inline.iter().copied())
                .collect()
        } else {
            inline
        };

        let wrapped = wrap_styled(&decorated, body_w);
        for (i, line) in wrapped.into_iter().enumerate() {
            let (pfx_text, pfx_style) = if i == 0 {
                (first_pfx.clone(), if self.quote_depth > 0 { quote_style } else { list_style })
            } else {
                (cont_pfx.clone(), quote_style)
            };
            let mut spans: Vec<Span<'static>> = Vec::with_capacity(line.spans.len() + 1);
            if !pfx_text.is_empty() {
                spans.push(Span::styled(pfx_text, pfx_style));
            }
            spans.extend(line.spans);
            self.out.push(Line::from(spans));
        }
        self.heading = None;
    }

    fn push_code_block(&mut self, code: &str) {
        let body_w = self.width.max(1);
        let code_style = Style::new().fg(self.palette.accent2.color()).bg(self.palette.panel.color());
        let bg = self.palette.panel.color();
        for raw in code.split('\n') {
            // Wrap long code lines by char (code doesn't word-wrap).
            let mut remaining = raw;
            loop {
                let (chunk, rest): (&str, Option<&str>) = if remaining.chars().count() <= body_w {
                    (remaining, None)
                } else {
                    let (a, b) = remaining.split_at(byte_floor(remaining, body_w));
                    (a, Some(b))
                };
                // Paint the whole row's background so the code block reads as a
                // solid panel, then overlay the chunk.
                let line = Line::from(vec![
                    Span::styled(chunk.to_string(), code_style),
                    Span::styled(
                        " ".repeat(body_w.saturating_sub(chunk.chars().count())),
                        Style::new().bg(bg),
                    ),
                ]);
                self.out.push(line);
                match rest {
                    Some(r) => remaining = r,
                    None => break,
                }
            }
        }
    }
}

/// Return the largest byte offset ≤ `max_chars` chars into `s` (don't split a
/// multibyte char). Used to hard-wrap code lines by display width.
fn byte_floor(s: &str, max_chars: usize) -> usize {
    s.char_indices().nth(max_chars).map(|(b, _)| b).unwrap_or(s.len())
}

/// Coerce a list-start marker (pulldown-cmark's `Option<u64>` start value) to a
/// `usize`. Kept as a free function so the exact start type can change between
/// pulldown-cmark versions without touching the event handler.
fn list_start_to_usize(n: u64) -> usize {
    n as usize
}

/// Word-wrap a `(char, Style)` stream (with `'\n'` hard breaks) into `Line`s no
/// wider than `width` cells, regrouping consecutive same-styled chars into spans.
fn wrap_styled(content: &[CharStyle], width: usize) -> Vec<Line<'static>> {
    // Split on hard breaks first.
    let mut all_lines: Vec<Line<'static>> = Vec::new();
    for segment in content.split(|(c, _)| *c == '\n') {
        all_lines.extend(wrap_segment(segment, width));
    }
    if all_lines.is_empty() {
        all_lines.push(Line::default());
    }
    all_lines
}

fn wrap_segment(segment: &[CharStyle], width: usize) -> Vec<Line<'static>> {
    if width == 0 {
        return vec![chars_to_line(segment)];
    }
    // Tokenize into words (runs of non-space chars) — keep each word's chars
    // with their styles. Multiple spaces collapse to one separator.
    let mut words: Vec<Vec<CharStyle>> = Vec::new();
    let mut cur: Vec<CharStyle> = Vec::new();
    for &pair in segment {
        if pair.0 == ' ' {
            if !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(pair);
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }

    let mut rows: Vec<Vec<CharStyle>> = Vec::new();
    let mut row: Vec<CharStyle> = Vec::new();
    let mut space_style = Style::new();
    for word in words {
        space_style = word.first().map(|(_, s)| *s).unwrap_or(space_style);
        let needed = word.len() + usize::from(!row.is_empty());
        if !row.is_empty() && row.len() + needed > width {
            rows.push(std::mem::take(&mut row));
        }
        if row.is_empty() && word.len() > width {
            // Hard-split an over-long word.
            for chunk in word.chunks(width) {
                rows.push(chunk.to_vec());
            }
        } else {
            if !row.is_empty() {
                row.push((' ', space_style));
            }
            row.extend(word);
        }
    }
    if !row.is_empty() || rows.is_empty() {
        rows.push(row);
    }
    rows.into_iter().map(|r| chars_to_line(&r)).collect()
}

/// Group consecutive same-styled chars into a single styled span.
fn chars_to_line(chars: &[CharStyle]) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut buf = String::new();
    let mut cur: Option<Style> = None;
    for &(c, s) in chars {
        if cur != Some(s) {
            if !buf.is_empty() {
                spans.push(Span::styled(std::mem::take(&mut buf), cur.unwrap()));
            }
            cur = Some(s);
        }
        buf.push(c);
    }
    if !buf.is_empty() {
        spans.push(Span::styled(buf, cur.unwrap()));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_strings(lines: &[Line]) -> Vec<String> {
        lines
            .iter()
            .map(|l| {
                l.spans.iter().map(|s| s.content.as_ref()).collect::<String>()
            })
            .collect()
    }

    #[test]
    fn empty_yields_one_blank_line() {
        let lines = markdown_to_lines("", Theme::Cyberpunk, 40);
        assert_eq!(lines.len(), 1);
        assert!(line_strings(&lines)[0].is_empty());
    }

    #[test]
    fn paragraph_wraps_to_width() {
        let md = "the quick brown fox jumps over the lazy dog again";
        let lines = markdown_to_lines(md, Theme::Cyberpunk, 20);
        for s in line_strings(&lines) {
            assert!(s.chars().count() <= 20, "line exceeded width: {s:?} ({})", s.chars().count());
        }
        assert!(lines.len() > 1, "long paragraph should wrap to multiple lines");
        // No content lost.
        let joined: String = line_strings(&lines).join(" ");
        assert!(joined.contains("quick") && joined.contains("dog"));
    }

    #[test]
    fn heading_is_bold_accent_and_prefixed() {
        let lines = markdown_to_lines("# Title", Theme::Cyberpunk, 40);
        let accent = Theme::Cyberpunk.palette().accent.color();
        let bodies = line_strings(&lines);
        assert_eq!(bodies.len(), 1);
        assert!(bodies[0].contains('#'), "heading should carry its # marker: {:?}", bodies[0]);
        assert!(bodies[0].contains("Title"));
        // The heading run (all one style → one span) carries accent + bold.
        let first_span = &lines[0].spans[0];
        assert!(first_span.content.starts_with('#'), "heading span should start with '#'");
        assert_eq!(first_span.style.fg, Some(accent));
        assert!(first_span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn inline_code_uses_accent2_on_panel() {
        let p = Theme::Cyberpunk.palette();
        let lines = markdown_to_lines("run `evasive` now", Theme::Cyberpunk, 40);
        // Find the 'evasive' code span.
        let code_span = lines[0].spans.iter().find(|s| s.content.contains("evasive")).unwrap();
        assert_eq!(code_span.style.fg, Some(p.accent2.color()));
        assert_eq!(code_span.style.bg, Some(p.panel.color()));
    }

    #[test]
    fn fenced_code_block_each_line_on_panel_bg() {
        let md = "```\nfn main() {}\nlet x = 1;\n```";
        let p = Theme::Cyberpunk.palette();
        let lines = markdown_to_lines(md, Theme::Cyberpunk, 40);
        let bodies: Vec<String> = line_strings(&lines);
        assert!(bodies.iter().any(|s| s.contains("fn main")), "code line present");
        // Code lines carry the panel background.
        let code_line = lines.iter().find(|l| line_strings(std::slice::from_ref(l))[0].contains("fn main")).unwrap();
        assert_eq!(code_line.spans[0].style.bg, Some(p.panel.color()));
    }

    #[test]
    fn unordered_list_emits_bullets() {
        let lines = markdown_to_lines("- one\n- two\n- three", Theme::Cyberpunk, 40);
        let bodies = line_strings(&lines);
        assert_eq!(bodies.len(), 3);
        assert!(bodies[0].starts_with("• one"), "first item should be bulleted: {:?}", bodies[0]);
        assert!(bodies[2].starts_with("• three"));
    }

    #[test]
    fn ordered_list_numbers_items() {
        let lines = markdown_to_lines("1. alpha\n2. beta", Theme::Cyberpunk, 40);
        let bodies = line_strings(&lines);
        assert!(bodies[0].starts_with("1. alpha"));
        assert!(bodies[1].starts_with("2. beta"));
    }

    #[test]
    fn blockquote_bars_each_line() {
        let lines = markdown_to_lines("> wisdom\n> more wisdom", Theme::Cyberpunk, 40);
        let bodies = line_strings(&lines);
        assert!(bodies.iter().all(|s| s.starts_with("▏")), "every quote line gets a bar");
    }

    #[test]
    fn thematic_break_is_a_rule_line() {
        let lines = markdown_to_lines("above\n\n---\n\nbelow", Theme::Cyberpunk, 12);
        let bodies = line_strings(&lines);
        assert!(bodies.iter().any(|s| s.chars().all(|c| c == '─') && !s.is_empty()), "a full-width rule line exists");
    }

    #[test]
    fn markdown_widget_renders_into_buffer() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 5));
        Markdown::new("# Hi").theme(Theme::Fallout).render(Rect::new(0, 0, 20, 5), &mut buf);
        // First cell is the heading '#'.
        assert_eq!(buf[(0, 0)].symbol(), "#");
    }

    #[test]
    fn tiny_width_does_not_panic() {
        let md = "# Heading\n\n- a long item that must wrap hard\n\n```\ncode\n```";
        let _ = markdown_to_lines(md, Theme::Cyberpunk, 1);
        let _ = markdown_to_lines(md, Theme::Cyberpunk, 2);
    }
}
