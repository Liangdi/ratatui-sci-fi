//! **CommLog** — sci-fi comms / chat feed with a streaming typewriter reveal
//! and an optional scrollbar for full LLM-style transcript scrolling.
//!
//! A message log: each entry is a styled speaker prefix (`NEXUS-7 ▸ …`)
//! followed by its body, wrapped to the area width. Where
//! [`ScanList`](crate::ScanList) is a selectable menu, `CommLog` is a
//! *transcript*: append-only, newest-last. Its signature trick is **streaming**
//! — the last message can be revealed character-by-character (a typewriter
//! effect) with a blinking caret, so an agent's reply appears to "type itself
//! in" like a live transmission.
//!
//! It has two render modes:
//!
//! - **Bottom-anchored** (default, [`CommLog::scrollbar`] off): the newest
//!   message is pinned to the bottom; older ones clip off the top. This is the
//!   compact "live channel" look, matching the original behavior byte-for-byte.
//! - **Scrollable / LLM transcript** ([`CommLog::scrollbar`] on): the wrapped
//!   rows are windowed top-to-bottom with a proportional scrollbar on the right
//!   edge, and [`CommLogState::scroll`] lets the user page back through history.
//!   Pushing a new message auto-follows back to the bottom — the classic chat /
//!   LLM UX.
//!
//! ## Spec
//! - Each message occupies one or more wrapped rows: a speaker prefix on the
//!   first row, continuation rows indented to align under the body.
//! - The prefix color encodes the speaker kind ([`CommKind`]); the body is the
//!   theme foreground.
//! - While the last message is streaming (`revealed < body.len()` in chars),
//!   only the revealed prefix is drawn, followed by a blinking caret glyph.
//! - Bottom-anchored mode renders newest-first from the bottom up so the latest
//!   line is always visible. Scrollbar mode windows the rows and draws a
//!   scrollbar whenever content overflows the viewport.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]; all messages + the reveal/blink/scroll clocks
//!   live in [`CommLogState`]. The app appends messages and calls
//!   [`CommLogState::tick`] once per frame.
//! - Colors come straight from [`Theme::palette`] (like [`AlertPopup`](crate::AlertPopup)):
//!   `accent` for agent prefixes + caret + scrollbar thumb, `accent2` for the
//!   user, `muted` for system notes / scrollbar track, and `fg` for bodies.
//! - The streaming caret reuses the crate's [`CaretShape`] and blinks on the
//!   shared [`DEFAULT_CURSOR_PERIOD`](crate::widgets::list::DEFAULT_CURSOR_PERIOD)
//!   cadence so it stays in lockstep with the `TextInput` / `ScanList` carets.
//! - Wrapping is char-level and assumes width-1 glyphs (crate convention #5).
//! - The scrollbar is **opt-in** so the default render is unchanged and existing
//!   tests pass without modification.
//!
//! # Example
//!
//! ```no_run
//! use ratatui_sci_fi::{CommKind, CommLog, CommLogMessage, CommLogState, Theme};
//!
//! let mut state = CommLogState::new();
//! state.push(CommLogMessage::new("ORACLE", "Pattern match at 94%.", CommKind::Agent));
//! state.push_streaming(CommLogMessage::new("NEXUS-7", "Vectors locked.", CommKind::Agent));
//! // Bottom-anchored live feed:
//! let live = CommLog::new().theme(Theme::Cyberpunk);
//! // …or a full scrollable LLM transcript with a scrollbar:
//! let transcript = CommLog::new().scrollbar(true).theme(Theme::Cyberpunk);
//! // in your event loop: state.tick(1); then render with `&mut state`.
//! // Page through history with state.scroll_up(n) / scroll_down(n) / scroll_to_bottom().
//! ```

use ratatui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};

use crate::Theme;
use crate::widgets::caret::CaretShape;
use crate::widgets::list::DEFAULT_CURSOR_PERIOD;

/// Who a [`CommLogMessage`] is from — drives its prefix color.
///
/// Maps to a [`Palette`](crate::Palette) token: agent → `accent`, user →
/// `accent2`, system note → `muted`. The body is always `fg` regardless of kind.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CommKind {
    /// A neutral system / status note — muted prefix.
    #[default]
    System,
    /// The local operator / user — `accent2` prefix.
    User,
    /// A remote agent — `accent` prefix.
    Agent,
}

/// A single comms-feed entry.
///
/// Build with [`CommLogMessage::new`] (fully revealed) or push it via
/// [`CommLogState::push_streaming`] to reveal it char-by-char. `revealed` is a
/// **char** count of the body shown so far; when it reaches
/// `body.chars().count()` the message is fully on screen.
#[derive(Debug, Clone)]
pub struct CommLogMessage {
    /// Speaker name shown as the colored prefix, e.g. `"NEXUS-7"`.
    pub speaker: String,
    /// The message body, wrapped to the feed width at render time.
    pub body: String,
    /// Speaker kind — selects the prefix color. See [`CommKind`].
    pub kind: CommKind,
    /// Chars of `body` currently shown. Equal to `body.chars().count()` once
    /// fully revealed; less while streaming.
    pub revealed: usize,
}

impl CommLogMessage {
    /// A fully-revealed message from `speaker` saying `body`.
    pub fn new(speaker: impl Into<String>, body: impl Into<String>, kind: CommKind) -> Self {
        let body = body.into();
        let revealed = body.chars().count();
        Self { speaker: speaker.into(), body, kind, revealed }
    }

    /// Same as [`new`](Self::new) but starts hidden (revealed = 0) so it streams
    /// in as [`CommLogState::tick`] advances. Convenience for callers that push
    /// directly without going through [`CommLogState::push_streaming`].
    pub fn streaming(speaker: impl Into<String>, body: impl Into<String>, kind: CommKind) -> Self {
        Self { speaker: speaker.into(), body: body.into(), kind, revealed: 0 }
    }

    /// Total char count of the body.
    fn char_count(&self) -> usize {
        self.body.chars().count()
    }

    /// Whether any body chars are still hidden.
    fn is_streaming(&self) -> bool {
        self.revealed < self.char_count()
    }
}

/// How [`CommLog`] lays out its messages.
///
/// [`Plain`](Self::Plain) (the default) is the compact bottom-anchored text
/// feed. [`Chat`](Self::Chat) renders each message as a bordered **card** with a
/// markdown-formatted body — the AI-agent chatbox look: agent cards left/full
/// in accent, user cards right/narrower in accent2, system cards muted. Requires
/// the `markdown` feature.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CommStyle {
    /// Compact bottom-anchored text feed (the original look).
    #[default]
    Plain,
    /// Bordered per-message cards with markdown bodies — the AI-agent chatbox.
    #[cfg(feature = "markdown")]
    Chat,
}

/// A sci-fi comms / chat feed.
///
/// Build with [`CommLog::new`], optionally set a theme ([`CommLog::theme`]),
/// the streaming caret's glyph ([`CommLog::caret`]), the render
/// [`style`](CommLog::style), and whether to render as a scrollable transcript
/// with a scrollbar ([`CommLog::scrollbar`]). All message state lives in the
/// companion [`CommLogState`].
#[derive(Debug, Clone, Copy, Default)]
pub struct CommLog {
    /// Theme whose [`Palette`](crate::Palette) drives the prefix / body colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
    /// Streaming caret glyph shape (drawn after a still-revealing message).
    /// Defaults to [`CaretShape::Block`] (`█`).
    pub caret: CaretShape,
    /// Render as a scrollable LLM-style transcript with a right-edge scrollbar
    /// (consulting [`CommLogState::scroll`]). Defaults to `false`, which keeps
    /// the original bottom-anchored look.
    pub scrollbar: bool,
    /// Card layout + markdown bodies ([`CommStyle::Chat`]). Defaults to
    /// [`CommStyle::Plain`].
    pub style: CommStyle,
}

impl CommLog {
    /// Create a feed, default theme + block caret, no scrollbar.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the theme whose palette drives colors.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Set the streaming caret's glyph shape (see [`CaretShape`]).
    #[must_use]
    pub fn caret(mut self, caret: CaretShape) -> Self {
        self.caret = caret;
        self
    }

    /// Enable the right-edge scrollbar + scrollable (top-anchored, windowed)
    /// render mode — the classic LLM-transcript view. Off by default, which
    /// keeps the original bottom-anchored look.
    #[must_use]
    pub fn scrollbar(mut self, on: bool) -> Self {
        self.scrollbar = on;
        self
    }

    /// Set the render style. [`CommStyle::Plain`] (default) is the compact
    /// bottom-anchored feed; [`CommStyle::Chat`] renders each message as a
    /// bordered card with a markdown-formatted body (the AI-agent chatbox,
    /// requires the `markdown` feature).
    #[must_use]
    pub fn style(mut self, style: CommStyle) -> Self {
        self.style = style;
        self
    }
}

/// Mutable state for [`CommLog`]: the message transcript plus the reveal/blink/
/// scroll clocks.
///
/// Append messages with [`push`](Self::push) (fully revealed) or
/// [`push_streaming`](Self::push_streaming) (revealed char-by-char), drive the
/// reveal + caret blink each frame with [`tick`](Self::tick), and page through
/// history with [`scroll_up`](Self::scroll_up) / [`scroll_down`](Self::scroll_down)
/// (only meaningful in scrollbar mode).
#[derive(Debug, Default, Clone)]
pub struct CommLogState {
    /// The transcript, oldest first. Bottom-anchored mode renders newest-first
    /// from the bottom up; scrollbar mode windows it by [`scroll`](Self::scroll).
    pub messages: Vec<CommLogMessage>,
    /// Blink clock for the streaming caret; advanced each tick.
    pub blink: u64,
    /// Rows scrolled up from the bottom (0 = newest pinned to the bottom).
    /// Consulted only in scrollbar mode; clamped to the available range at
    /// render time. Pushing a new message resets this to 0 (auto-follow).
    pub scroll: usize,
}

impl CommLogState {
    /// Empty transcript, clocks at zero, scroll at the bottom.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a fully-revealed message (forces `revealed` to the body length).
    /// Auto-follows: resets [`scroll`](Self::scroll) to the bottom.
    pub fn push(&mut self, mut msg: CommLogMessage) {
        msg.revealed = msg.char_count();
        self.messages.push(msg);
        self.scroll = 0;
    }

    /// Append a message that streams in char-by-char: `revealed` is set to 0 and
    /// [`tick`](Self::tick) unveils it. If the previous tail message was still
    /// streaming it is finished first, so only one message ever streams at a
    /// time. Auto-follows: resets [`scroll`](Self::scroll) to the bottom.
    pub fn push_streaming(&mut self, msg: CommLogMessage) {
        self.finish_streaming();
        let mut msg = msg;
        msg.revealed = 0;
        self.messages.push(msg);
        self.scroll = 0;
    }

    /// Fully reveal the tail message if it was still streaming. No-op otherwise.
    pub fn finish_streaming(&mut self) {
        if let Some(last) = self.messages.last_mut() {
            last.revealed = last.char_count();
        }
    }

    /// Whether the tail message is still revealing characters.
    pub fn is_streaming(&self) -> bool {
        self.messages.last().is_some_and(CommLogMessage::is_streaming)
    }

    /// The speaker name of the tail message, if any.
    pub fn last_speaker(&self) -> Option<&str> {
        self.messages.last().map(|m| m.speaker.as_str())
    }

    /// Scroll the view up (toward older messages) by `lines` rows. Only
    /// consulted in scrollbar mode; clamped to the available range at render.
    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll = self.scroll.saturating_add(lines);
    }

    /// Scroll the view down (toward the newest message) by `lines` rows.
    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll = self.scroll.saturating_sub(lines);
    }

    /// Jump the view back to the newest message (scroll = 0).
    pub fn scroll_to_bottom(&mut self) {
        self.scroll = 0;
    }

    /// Whether the view is pinned to the newest message (not scrolled up).
    pub fn at_bottom(&self) -> bool {
        self.scroll == 0
    }

    /// Drop every message.
    pub fn clear(&mut self) {
        self.messages.clear();
        self.scroll = 0;
    }

    /// Advance one frame: unveil up to `chars_per_tick` body chars of the
    /// streaming tail message and step the caret blink clock.
    pub fn tick(&mut self, chars_per_tick: usize) {
        self.blink = self.blink.wrapping_add(1);
        if let Some(last) = self.messages.last_mut()
            && last.is_streaming()
        {
            last.revealed = (last.revealed + chars_per_tick.max(1)).min(last.char_count());
        }
    }

    /// Whether the streaming caret is currently visible (shared blink cadence).
    fn caret_visible(&self) -> bool {
        (self.blink / DEFAULT_CURSOR_PERIOD.max(1)).is_multiple_of(2)
    }
}

/// A fully-built render row: a sequence of `(text, Style)` segments. Kept simple
/// (owned strings) since comms lines are short.
type Segments = Vec<(String, ratatui::style::Style)>;

impl StatefulWidget for CommLog {
    type State = CommLogState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width == 0 || area.height == 0 || state.messages.is_empty() {
            return;
        }

        // Chat style: bordered per-message cards with markdown bodies. Short-
        // circuits before the plain-feed path.
        #[cfg(feature = "markdown")]
        {
            if self.style == CommStyle::Chat {
                self.render_chat(area, buf, state);
                return;
            }
        }

        let palette = self.theme.palette();
        let prefix_style = |kind: CommKind| -> ratatui::style::Style {
            let c = match kind {
                CommKind::Agent => palette.accent.color(),
                CommKind::User => palette.accent2.color(),
                CommKind::System => palette.muted.color(),
            };
            ratatui::style::Style::new().fg(c)
        };
        let body_style = ratatui::style::Style::new().fg(palette.fg.color());
        let caret_style = ratatui::style::Style::new().fg(palette.accent.color());
        let caret_visible = state.caret_visible();

        if !self.scrollbar {
            // Default bottom-anchored mode: newest pinned to the bottom, older
            // rows clipping off the top. Render newest-first from the bottom up.
            let width = area.width as usize;
            let mut y = area.bottom() as i64 - 1;
            let top = area.top() as i64;
            let x0 = area.x;
            let right = area.x + area.width;

            'outer: for msg in state.messages.iter().rev() {
                let rows = message_rows(
                    msg,
                    width,
                    prefix_style(msg.kind),
                    body_style,
                    caret_style,
                    self.caret.glyph(),
                    caret_visible,
                );
                for segs in rows.iter().rev() {
                    if y < top {
                        break 'outer;
                    }
                    write_segments(buf, x0, y as u16, right, segs);
                    y -= 1;
                }
            }
            return;
        }

        // Scrollbar (LLM transcript) mode: window the wrapped rows and render
        // them top-to-bottom, with a proportional scrollbar on the right edge.
        // The scrollbar needs at least 3 columns (content + track); narrower
        // areas fall back to full-width content with no track drawn.
        let track_present = area.width >= 3;
        let content_w = if track_present { area.width as usize - 1 } else { area.width as usize };
        let view_h = area.height as usize;

        // Flatten every message into its wrapped rows (top-to-bottom).
        let mut all_rows: Vec<Segments> = Vec::new();
        for msg in state.messages.iter() {
            let rows = message_rows(
                msg,
                content_w,
                prefix_style(msg.kind),
                body_style,
                caret_style,
                self.caret.glyph(),
                caret_visible,
            );
            all_rows.extend(rows);
        }
        let total = all_rows.len();
        // Clamp the stored scroll into the reachable range (defensive — the app
        // may scroll past the end before a resize shrinks the history).
        let max_scroll = total.saturating_sub(view_h);
        if state.scroll > max_scroll {
            state.scroll = max_scroll;
        }
        let start = max_scroll.saturating_sub(state.scroll).min(total);

        let content_right = area.x + content_w as u16;
        for (i, segs) in all_rows[start..].iter().enumerate() {
            let y = area.y + i as u16;
            if y >= area.bottom() {
                break;
            }
            write_segments(buf, area.x, y, content_right, segs);
        }

        // Scrollbar — only when content actually overflows the viewport.
        if track_present && total > view_h {
            let track_x = area.x + area.width - 1;
            let track_style = ratatui::style::Style::new().fg(palette.muted.color());
            let thumb_style = ratatui::style::Style::new().fg(palette.accent.color());
            for ry in 0..area.height {
                buf[(track_x, area.y + ry)].set_char('│').set_style(track_style);
            }
            // Proportional thumb: height ∝ viewport/total, position ∝ start/total.
            let thumb_h = ((view_h * view_h) / total).max(1);
            let thumb_top = ((start * view_h) / total).min(view_h.saturating_sub(thumb_h));
            for t in 0..thumb_h as u16 {
                let ry = thumb_top as u16 + t;
                if ry < area.height {
                    buf[(track_x, area.y + ry)].set_char('█').set_style(thumb_style);
                }
            }
        }
    }
}

#[cfg(feature = "markdown")]
impl CommLog {
    /// Chat style: each message is a bordered card with a markdown body. User
    /// cards are narrower and right-aligned; agent/system cards span the width.
    /// Cards stack with a 1-row gap and are windowed by [`CommLogState::scroll`].
    fn render_chat(self, area: Rect, buf: &mut Buffer, state: &mut CommLogState) {
        use ratatui::text::Span;
        let palette = self.theme.palette();
        let caret_visible = state.caret_visible();

        let track_present = self.scrollbar && area.width >= 3;
        let content_w = if track_present { area.width - 1 } else { area.width };
        let view_h = area.height as usize;

        // Pre-compute each message's card geometry + wrapped markdown body.
        let mut cards: Vec<ChatCard> = Vec::new();
        let mut y = 0usize;
        for msg in &state.messages {
            let total = msg.body.chars().count();
            let shown: String = msg.body.chars().take(msg.revealed.min(total)).collect();
            let streaming = msg.revealed < total;
            let card_w = match msg.kind {
                CommKind::User => (((content_w as usize) * 7 / 10).max(20) as u16).min(content_w),
                _ => content_w,
            };
            let body_w = card_w.saturating_sub(2).max(1);
            let mut body = crate::widgets::markdown::markdown_to_lines(&shown, self.theme, body_w);
            if streaming && caret_visible
                && let Some(last) = body.last_mut()
            {
                last.spans.push(Span::styled(
                    self.caret.glyph().to_string(),
                    ratatui::style::Style::new().fg(palette.accent.color()),
                ));
            }
            let height = (2 + body.len()) as u16;
            let x = if msg.kind == CommKind::User {
                area.x + content_w.saturating_sub(card_w)
            } else {
                area.x
            };
            cards.push(ChatCard {
                top: y,
                x,
                width: card_w,
                height,
                body,
                kind: msg.kind,
                speaker: msg.speaker.clone(),
            });
            y += height as usize + 1; // 1-row gap between cards
        }
        let total_rows = y;
        let max_scroll = total_rows.saturating_sub(view_h);
        if state.scroll > max_scroll {
            state.scroll = max_scroll;
        }
        let start = max_scroll.saturating_sub(state.scroll);

        // Render visible cards. Each is drawn into an offscreen buffer then
        // blitted with clipping, so a card straddling the viewport edge simply
        // scrolls out of view instead of writing outside the feed area.
        for card in &cards {
            if card.top + card.height as usize <= start || card.top >= start + view_h {
                continue;
            }
            let buf_y = area.y as i64 + card.top as i64 - start as i64;
            if buf_y < 0 {
                continue;
            }
            Self::render_card(buf, area, buf_y as u16, card, self.theme);
        }

        // Scrollbar (same proportional look as the plain scrollbar mode).
        if track_present && total_rows > view_h {
            let track_x = area.x + area.width - 1;
            let track_style = ratatui::style::Style::new().fg(palette.muted.color());
            let thumb_style = ratatui::style::Style::new().fg(palette.accent.color());
            for ry in 0..area.height {
                buf[(track_x, area.y + ry)].set_char('│').set_style(track_style);
            }
            let thumb_h = ((view_h * view_h) / total_rows).max(1);
            let thumb_top = ((start * view_h) / total_rows).min(view_h.saturating_sub(thumb_h));
            for t in 0..thumb_h as u16 {
                let ry = thumb_top as u16 + t;
                if ry < area.height {
                    buf[(track_x, area.y + ry)].set_char('█').set_style(thumb_style);
                }
            }
        }
    }

    /// Draw one message card (rounded border + speaker title + markdown body) at
    /// its computed `y`, clipped to `clip`. Rendered into an offscreen buffer
    /// then blitted so partially-visible cards don't write outside the feed.
    fn render_card(buf: &mut Buffer, clip: Rect, y: u16, card: &ChatCard, theme: Theme) {
        use ratatui::layout::Alignment;
        use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Widget};

        let palette = theme.palette();
        let rect = Rect { x: card.x, y, width: card.width, height: card.height };
        if rect.width < 3 || rect.height < 2 {
            return;
        }
        let (border_color, align) = match card.kind {
            CommKind::Agent => (palette.accent.color(), Alignment::Left),
            CommKind::User => (palette.accent2.color(), Alignment::Right),
            CommKind::System => (palette.muted.color(), Alignment::Center),
        };
        let title = format!(" {} ", card.speaker);
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(ratatui::style::Style::new().fg(border_color))
            .title_alignment(align)
            .title(
                ratatui::text::Line::from(title)
                    .style(ratatui::style::Style::new().fg(border_color)),
            );
        let inner = block.inner(rect);

        let mut card_buf = Buffer::empty(rect);
        block.render(rect, &mut card_buf);
        Paragraph::new(ratatui::text::Text::from(card.body.clone())).render(inner, &mut card_buf);

        // Blit only the cells inside the feed area.
        for cy in rect.top()..rect.bottom() {
            if cy < clip.top() || cy >= clip.bottom() {
                continue;
            }
            for cx in rect.left()..rect.right() {
                if cx < clip.left() || cx >= clip.right() {
                    continue;
                }
                buf[(cx, cy)] = card_buf[(cx, cy)].clone();
            }
        }
    }
}

/// Layout + rendered body for one chat card (markdown feature only).
#[cfg(feature = "markdown")]
struct ChatCard {
    /// Absolute top row within the stacked feed (0-based).
    top: usize,
    x: u16,
    width: u16,
    height: u16,
    body: Vec<ratatui::text::Line<'static>>,
    kind: CommKind,
    speaker: String,
}

/// Split a message into wrapped `Segments` rows (prefix on row 0, continuation
/// rows indented to align under the body), top-to-bottom order.
fn message_rows(
    msg: &CommLogMessage,
    width: usize,
    prefix_style: ratatui::style::Style,
    body_style: ratatui::style::Style,
    caret_style: ratatui::style::Style,
    caret_glyph: char,
    caret_visible: bool,
) -> Vec<Segments> {
    // Prefix is `"<speaker> ▸ "`; its width also sets the continuation indent.
    let prefix = format!("{} ▸ ", msg.speaker);
    let prefix_w = prefix.chars().count();
    // Body content width per row: leave room for the prefix/indent. Guard so a
    // vanishingly narrow area still yields ≥1 body char rather than stalling.
    let content_w = width.saturating_sub(prefix_w).max(1);

    // Only the revealed prefix of the body is shown while streaming.
    let shown: String = msg.body.chars().take(msg.revealed).collect();
    let streaming = msg.is_streaming();

    let mut chunks: Vec<String> = Vec::new();
    let mut chars = shown.chars().peekable();
    while chars.peek().is_some() {
        let mut row = String::new();
        for _ in 0..content_w {
            match chars.next() {
                Some(c) => row.push(c),
                None => break,
            }
        }
        chunks.push(row);
    }
    if chunks.is_empty() {
        chunks.push(String::new());
    }

    let mut rows: Vec<Segments> = Vec::with_capacity(chunks.len());
    for (i, chunk) in chunks.iter().enumerate() {
        let mut segs: Segments = Vec::with_capacity(3);
        if i == 0 {
            segs.push((prefix.clone(), prefix_style));
        } else {
            // Indent continuation rows under the body, in the muted prefix tone
            // so the indent reads as structure rather than content.
            segs.push((" ".repeat(prefix_w), prefix_style));
        }
        segs.push((chunk.clone(), body_style));
        // Blinking caret at the end of the last row while still streaming.
        if streaming && i == chunks.len() - 1 && caret_visible {
            segs.push((caret_glyph.to_string(), caret_style));
        }
        rows.push(segs);
    }
    rows
}

/// Write one row's `(text, Style)` segments into `buf` at row `y`, starting at
/// `x0` and clipping at `right`.
fn write_segments(buf: &mut Buffer, x0: u16, y: u16, right: u16, segs: &[(String, ratatui::style::Style)]) {
    let mut x = x0;
    for (text, style) in segs {
        for ch in text.chars() {
            if x >= right {
                return;
            }
            buf[(x, y)].set_symbol(ch.to_string().as_str()).set_style(*style);
            x += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    const W: u16 = 24;
    const H: u16 = 8;

    fn render(state: &mut CommLogState, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let feed = CommLog::new().theme(theme);
        StatefulWidget::render(feed, Rect::new(0, 0, W, H), &mut buf, state);
        buf
    }

    fn render_scrollbar(state: &mut CommLogState, w: u16, h: u16, theme: Theme) -> Buffer {
        let area = Rect::new(0, 0, w, h);
        let mut buf = Buffer::empty(area);
        let feed = CommLog::new().scrollbar(true).theme(theme);
        StatefulWidget::render(feed, area, &mut buf, state);
        buf
    }

    fn row_string(buf: &Buffer, y: u16, x_end: u16) -> String {
        (0..x_end).map(|x| buf[(x, y)].symbol().to_string()).collect()
    }

    #[test]
    fn newest_message_anchored_to_bottom() {
        let mut s = CommLogState::new();
        s.push(CommLogMessage::new("ORACLE", "first line", CommKind::Agent));
        s.push(CommLogMessage::new("NEXUS", "second line", CommKind::Agent));
        let buf = render(&mut s, Theme::Cyberpunk);
        // The last row of the area must contain the second message's body.
        let bottom = row_string(&buf, H - 1, W);
        assert!(bottom.contains("second"), "newest message must sit on the bottom row: {bottom:?}");
    }

    #[test]
    fn agent_prefix_is_accent_colored() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let mut s = CommLogState::new();
        s.push(CommLogMessage::new("NEXUS", "hi", CommKind::Agent));
        let buf = render(&mut s, Theme::Cyberpunk);
        // First char of the prefix 'N' at the bottom row carries the accent fg.
        assert_eq!(buf[(0, H - 1)].symbol(), "N");
        assert_eq!(buf[(0, H - 1)].fg, accent, "agent prefix must be accent-colored");
    }

    #[test]
    fn user_prefix_uses_accent2() {
        let accent2 = Theme::Cyberpunk.palette().accent2.color();
        let mut s = CommLogState::new();
        s.push(CommLogMessage::new("OPERATOR", "hi", CommKind::User));
        let buf = render(&mut s, Theme::Cyberpunk);
        assert_eq!(buf[(0, H - 1)].fg, accent2, "user prefix must be accent2-colored");
    }

    #[test]
    fn streaming_reveals_chars_via_tick_and_shows_caret() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let mut s = CommLogState::new();
        s.push_streaming(CommLogMessage::new("NEXUS", "ABCDE", CommKind::Agent));
        assert!(s.is_streaming(), "freshly pushed streaming message should be streaming");

        // tick 0: blink visible at start -> caret drawn; revealed advances.
        s.tick(1);
        let buf = render(&mut s, Theme::Cyberpunk);
        let bottom = row_string(&buf, H - 1, W);
        assert!(bottom.contains("A"), "one char should be revealed after a tick: {bottom:?}");
        // The caret cell (right after the revealed char) is accent-colored.
        let caret_cell = buf[(4, H - 1)].fg; // "NEXUS ▸ " = 8 wide; 'A' at x=8; caret at x=9
        assert_eq!(caret_cell, accent, "caret should be accent-colored while streaming");

        // Reveal the rest.
        for _ in 0..10 {
            s.tick(2);
        }
        assert!(!s.is_streaming(), "message should be fully revealed once chars run out");
    }

    #[test]
    fn finish_streaming_completes_tail() {
        let mut s = CommLogState::new();
        s.push_streaming(CommLogMessage::new("NEXUS", "hello", CommKind::Agent));
        assert!(s.is_streaming());
        s.finish_streaming();
        assert!(!s.is_streaming(), "finish_streaming must fully reveal the tail");
    }

    #[test]
    fn push_streaming_finishes_previous_stream_first() {
        let mut s = CommLogState::new();
        s.push_streaming(CommLogMessage::new("A", "aaa", CommKind::Agent));
        s.push_streaming(CommLogMessage::new("B", "bbb", CommKind::Agent));
        // Only the new tail (B) should still be streaming; A is finished.
        assert!(s.is_streaming());
        assert_eq!(s.messages.len(), 2);
        assert!(!s.messages[0].is_streaming(), "previous streaming message must be finished");
        assert!(s.messages[1].is_streaming());
    }

    #[test]
    fn empty_state_is_a_noop() {
        let mut s = CommLogState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        CommLog::new().render(Rect::new(0, 0, W, H), &mut buf, &mut s);
        // Nothing drawn — every cell still the default space.
        assert_eq!(buf[(0, 0)].symbol(), " ");
    }

    #[test]
    fn tiny_area_does_not_panic() {
        let mut s = CommLogState::new();
        s.push(CommLogMessage::new("NEXUS", "overflow body that is long", CommKind::Agent));
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        CommLog::new().render(Rect::new(0, 0, 1, 1), &mut buf, &mut s);
        assert_eq!(*buf.area(), Rect::new(0, 0, 1, 1));
    }

    #[test]
    fn long_body_wraps_to_multiple_rows() {
        let mut s = CommLogState::new();
        // Width 24, prefix "SPOCK ▸ " = 8 wide -> 16 body chars per row. A body
        // longer than 16 chars must occupy at least two rows.
        s.push(CommLogMessage::new("SPOCK", "0123456789ABCDEFGHIJ", CommKind::Agent));
        let buf = render(&mut s, Theme::Cyberpunk);
        // The bottom row holds the wrapped continuation (chars past 16).
        let bottom = row_string(&buf, H - 1, W);
        assert!(bottom.contains("GHIJ") || bottom.contains("17") || bottom.contains("89AB"),
            "wrapped tail must land on the bottom row: {bottom:?}");
        // And the row above must carry the prefix + start of the body.
        assert_eq!(buf[(0, H - 2)].symbol(), "S", "prefix must start the wrapped block's first row");
    }

    #[test]
    fn tick_wraps_blink_clock_without_panic() {
        let mut s = CommLogState { messages: Vec::new(), blink: u64::MAX, scroll: 0 };
        s.tick(1);
        assert_eq!(s.blink, 0);
    }

    // ── scrollbar / scroll mode ──────────────────────────────────────────────

    #[test]
    fn scrollbar_thumb_appears_on_overflow() {
        // 30 messages in a 6-row viewport -> overflow -> a thumb is drawn in the
        // rightmost (track) column.
        let mut s = CommLogState::new();
        for i in 0..30u32 {
            s.push(CommLogMessage::new("A", format!("message number {i}"), CommKind::Agent));
        }
        let buf = render_scrollbar(&mut s, 24, 6, Theme::Cyberpunk);
        let track_x = 23; // last column
        let has_thumb = (0..6u16).any(|y| buf[(track_x, y)].symbol() == "█");
        assert!(has_thumb, "a scrollbar thumb should be present when content overflows");
        // The newest message body is still visible in the content area (x < 23).
        let bottom = row_string(&buf, 5, 23);
        assert!(bottom.contains("29"), "newest message should be visible at the bottom: {bottom:?}");
    }

    #[test]
    fn no_scrollbar_when_content_fits() {
        // Few messages in a tall viewport -> no overflow -> no thumb.
        let mut s = CommLogState::new();
        s.push(CommLogMessage::new("A", "hi", CommKind::Agent));
        let buf = render_scrollbar(&mut s, 24, 12, Theme::Cyberpunk);
        let track_x = 23;
        let has_thumb = (0..12u16).any(|y| buf[(track_x, y)].symbol() == "█");
        assert!(!has_thumb, "no scrollbar thumb should be drawn when content fits");
    }

    #[test]
    fn scroll_up_reveals_oldest_message() {
        let mut s = CommLogState::new();
        for i in 0..40u32 {
            s.push(CommLogMessage::new("A", format!("msg-{i:02}"), CommKind::Agent));
        }
        // At the bottom, the newest is visible; the oldest is not.
        let mut buf = render_scrollbar(&mut s, 24, 5, Theme::Cyberpunk);
        assert!(row_string(&buf, 4, 23).contains("39"));
        assert!(!row_string(&buf, 0, 23).contains("00"));

        // Scroll well past the top; the oldest message should now be on row 0.
        s.scroll_up(100);
        let buf2 = render_scrollbar(&mut s, 24, 5, Theme::Cyberpunk);
        let top = row_string(&buf2, 0, 23);
        assert!(top.contains("msg-00"), "scrolling up should reveal the oldest message: {top:?}");
    }

    #[test]
    fn scroll_down_moves_toward_bottom_and_clamps() {
        let mut s = CommLogState::new();
        for i in 0..40u32 {
            s.push(CommLogMessage::new("A", format!("msg-{i:02}"), CommKind::Agent));
        }
        s.scroll_up(100);
        s.scroll_down(3);
        // Still scrolled up (not at bottom); scroll_down reduced the offset.
        assert!(s.scroll > 0);
        // scroll_to_bottom pins back to the newest.
        s.scroll_to_bottom();
        assert!(s.at_bottom());
        let buf = render_scrollbar(&mut s, 24, 5, Theme::Cyberpunk);
        assert!(row_string(&buf, 4, 23).contains("39"), "newest visible again after scroll_to_bottom");
    }

    #[test]
    fn push_auto_follows_to_bottom() {
        let mut s = CommLogState::new();
        for i in 0..20u32 {
            s.push(CommLogMessage::new("A", format!("m{i}"), CommKind::Agent));
        }
        s.scroll_up(5);
        assert_eq!(s.scroll, 5);
        s.push(CommLogMessage::new("A", "new", CommKind::Agent));
        assert_eq!(s.scroll, 0, "push should auto-follow back to the bottom");
    }

    #[test]
    fn scrollbar_mode_streaming_tail_still_reveals() {
        // In scrollbar mode, a freshly-streamed tail message must still reveal
        // via tick and show its caret (and stay pinned at the bottom).
        let mut s = CommLogState::new();
        for i in 0..6 {
            s.push(CommLogMessage::new("A", format!("filler {i}"), CommKind::Agent));
        }
        s.push_streaming(CommLogMessage::new("NEXUS", "STREAMME", CommKind::Agent));
        s.tick(1);
        let buf = render_scrollbar(&mut s, 24, 6, Theme::Cyberpunk);
        let bottom = row_string(&buf, 5, 23);
        assert!(bottom.contains('S'), "streaming tail should reveal its first char at the bottom: {bottom:?}");
    }

    // ── chat (markdown card) mode ────────────────────────────────────────────

    #[cfg(feature = "markdown")]
    fn render_chat(state: &mut CommLogState, w: u16, h: u16, theme: Theme) -> Buffer {
        let area = Rect::new(0, 0, w, h);
        let mut buf = Buffer::empty(area);
        let feed = CommLog::new().style(CommStyle::Chat).scrollbar(true).theme(theme);
        StatefulWidget::render(feed, area, &mut buf, state);
        buf
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn chat_agent_card_has_accent_border() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let mut s = CommLogState::new();
        s.push(CommLogMessage::new("NEXUS", "hello", CommKind::Agent));
        let buf = render_chat(&mut s, 40, 8, Theme::Cyberpunk);
        // Rounded top-left corner of the agent card, accent-colored.
        assert_eq!(buf[(0, 0)].symbol(), "╭");
        assert_eq!(buf[(0, 0)].fg, accent, "agent card border should be accent");
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn chat_user_card_is_right_aligned() {
        let mut s = CommLogState::new();
        s.push(CommLogMessage::new("OP", "hi", CommKind::User));
        let buf = render_chat(&mut s, 40, 6, Theme::Cyberpunk);
        // User cards are narrower + right-aligned, so x=0 must be empty space
        // (not a card corner) and a top-right rounded corner must exist.
        assert_ne!(buf[(0, 0)].symbol(), "╭", "user card should not start at x=0");
        let has_tr = (0..40u16).any(|x| buf[(x, 0)].symbol() == "╮");
        assert!(has_tr, "right-aligned user card should have a top-right corner");
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn chat_markdown_list_grows_card_height() {
        // One-line card: border rows y=0/2, body y=1 → bottom corner at y=2.
        let mut a = CommLogState::new();
        a.push(CommLogMessage::new("N", "one line", CommKind::Agent));
        let buf_a = render_chat(&mut a, 40, 20, Theme::Cyberpunk);
        assert_eq!(buf_a[(0, 2)].symbol(), "╰", "single-line card bottom at y=2");

        // A 3-item markdown list renders 3 body lines → height 5, bottom at y=4.
        let mut b = CommLogState::new();
        b.push(CommLogMessage::new("N", "- a\n- b\n- c", CommKind::Agent));
        let buf_b = render_chat(&mut b, 40, 20, Theme::Cyberpunk);
        assert_eq!(buf_b[(0, 4)].symbol(), "╰", "3-item list card bottom at y=4");
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn chat_scrollbar_appears_on_overflow() {
        let mut s = CommLogState::new();
        for i in 0..20u32 {
            s.push(CommLogMessage::new("A", format!("msg {i}"), CommKind::Agent));
        }
        let buf = render_chat(&mut s, 40, 6, Theme::Cyberpunk);
        let track_x = 39;
        let has_thumb = (0..6u16).any(|y| buf[(track_x, y)].symbol() == "█");
        assert!(has_thumb, "chat scrollbar thumb should appear on overflow");
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn chat_streaming_shows_caret() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let mut s = CommLogState::new();
        s.push_streaming(CommLogMessage::new("N", "STREAMME", CommKind::Agent));
        s.tick(1);
        let buf = render_chat(&mut s, 40, 8, Theme::Cyberpunk);
        let has_caret = (0..40u16)
            .any(|x| buf[(x, 1)].symbol() == "█" && buf[(x, 1)].fg == accent);
        assert!(has_caret, "streaming chat card should show the accent caret");
    }
}
