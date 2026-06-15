//! **CommLog** — sci-fi comms / chat feed with a streaming typewriter reveal.
//!
//! A bottom-anchored message log: each entry is a styled speaker prefix
//! (`NEXUS-7 ▸ …`) followed by its body, wrapped to the area width. The newest
//! message is always pinned to the bottom; older ones clip off the top when the
//! feed overflows — the natural shape of a live comms channel or agent chat.
//!
//! Where [`ScanList`](crate::ScanList) is a selectable menu, `CommLog` is a
//! *transcript*: append-only, newest-last, no selection. Its signature trick is
//! **streaming**: the last message can be revealed character-by-character (a
//! typewriter effect) with a blinking caret, so an agent's reply appears to
//! "type itself in" like a live transmission.
//!
//! ## Spec
//! - Each message occupies one or more wrapped rows: a speaker prefix on the
//!   first row, continuation rows indented to align under the body.
//! - The prefix color encodes the speaker kind ([`CommKind`]); the body is the
//!   theme foreground.
//! - While the last message is streaming (`revealed < body.len()` in chars),
//!   only the revealed prefix is drawn, followed by a blinking caret glyph.
//! - The feed is bottom-anchored: render newest first from the bottom up so the
//!   latest line is always visible regardless of how many entries precede it.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]; all messages + the reveal/blink clocks live in
//!   [`CommLogState`]. The app appends messages and calls
//!   [`CommLogState::tick`] once per frame.
//! - Colors come straight from [`Theme::palette`] (like [`AlertPopup`](crate::AlertPopup)):
//!   `accent` for agent prefixes + caret, `accent2` for the user, `muted` for
//!   system notes, and `fg` for bodies. This keeps every speaker visually
//!   distinct without a per-message style field.
//! - The streaming caret reuses the crate's [`CaretShape`] and blinks on the
//!   shared [`DEFAULT_CURSOR_PERIOD`](crate::widgets::list::DEFAULT_CURSOR_PERIOD)
//!   cadence so it stays in lockstep with the `TextInput` / `ScanList` carets.
//! - Wrapping is char-level and assumes width-1 glyphs (crate convention #5);
//!   that holds for the ASCII/border content comms logs carry.
//!
//! # Example
//!
//! ```no_run
//! use ratatui_sci_fi::{CommKind, CommLog, CommLogMessage, CommLogState, Theme};
//!
//! let mut state = CommLogState::new();
//! // a fully-revealed line:
//! state.push(CommLogMessage::new("ORACLE", "Pattern match at 94%.", CommKind::Agent));
//! // a line that streams in char-by-char as you call `tick`:
//! state.push_streaming(CommLogMessage::new("NEXUS-7", "Vectors locked.", CommKind::Agent));
//! let feed = CommLog::new().theme(Theme::Cyberpunk);
//! // in your event loop: state.tick(1); then render `feed` with `&mut state`.
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

/// A sci-fi comms / chat feed.
///
/// Build with [`CommLog::new`], optionally set a theme ([`CommLog::theme`]) and
/// the streaming caret's glyph ([`CommLog::caret`]). All message state lives in
/// the companion [`CommLogState`].
#[derive(Debug, Clone, Default)]
pub struct CommLog {
    /// Theme whose [`Palette`](crate::Palette) drives the prefix / body colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
    /// Streaming caret glyph shape (drawn after a still-revealing message).
    /// Defaults to [`CaretShape::Block`] (`█`).
    pub caret: CaretShape,
}

impl CommLog {
    /// Create a feed, default theme + block caret.
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
}

/// Mutable state for [`CommLog`]: the message transcript plus the reveal/blink
/// clocks.
///
/// Append messages with [`push`](Self::push) (fully revealed) or
/// [`push_streaming`](Self::push_streaming) (revealed char-by-char), and drive
/// the reveal + caret blink each frame with [`tick`](Self::tick).
#[derive(Debug, Default, Clone)]
pub struct CommLogState {
    /// The transcript, oldest first. Rendered bottom-up so the last entry is
    /// always at the bottom of the feed.
    pub messages: Vec<CommLogMessage>,
    /// Blink clock for the streaming caret; advanced each tick.
    pub blink: u64,
}

impl CommLogState {
    /// Empty transcript, clocks at zero.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a fully-revealed message (forces `revealed` to the body length).
    pub fn push(&mut self, mut msg: CommLogMessage) {
        msg.revealed = msg.char_count();
        self.messages.push(msg);
    }

    /// Append a message that streams in char-by-char: `revealed` is set to 0 and
    /// [`tick`](Self::tick) unveils it. If the previous tail message was still
    /// streaming it is finished first, so only one message ever streams at a
    /// time.
    pub fn push_streaming(&mut self, msg: CommLogMessage) {
        self.finish_streaming();
        let mut msg = msg;
        msg.revealed = 0;
        self.messages.push(msg);
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

    /// Drop every message.
    pub fn clear(&mut self) {
        self.messages.clear();
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

/// A fully-built render row: a sequence of `(text, Style)` segments plus the
/// row's overall right clip. Kept simple (owned strings) since comms lines are
/// short.
type Segments = Vec<(String, ratatui::style::Style)>;

impl StatefulWidget for CommLog {
    type State = CommLogState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width == 0 || area.height == 0 || state.messages.is_empty() {
            return;
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
        let width = area.width as usize;

        // Build each message's wrapped rows, then place them bottom-up so the
        // newest message is always flush with the bottom edge and older ones
        // clip off the top when the feed overflows.
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
    }
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

    #[test]
    fn newest_message_anchored_to_bottom() {
        let mut s = CommLogState::new();
        s.push(CommLogMessage::new("ORACLE", "first line", CommKind::Agent));
        s.push(CommLogMessage::new("NEXUS", "second line", CommKind::Agent));
        let buf = render(&mut s, Theme::Cyberpunk);
        // The last row of the area must contain the second message's body.
        let bottom = (0..W).map(|x| buf[(x, H - 1)].symbol().to_string()).collect::<String>();
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
        let bottom: String = (0..W).map(|x| buf[(x, H - 1)].symbol().to_string()).collect();
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
        let bottom: String = (0..W).map(|x| buf[(x, H - 1)].symbol().to_string()).collect();
        assert!(bottom.contains("GHIJ") || bottom.contains("17") || bottom.contains("89AB"),
            "wrapped tail must land on the bottom row: {bottom:?}");
        // And the row above must carry the prefix + start of the body.
        assert_eq!(buf[(0, H - 2)].symbol(), "S", "prefix must start the wrapped block's first row");
    }

    #[test]
    fn tick_wraps_blink_clock_without_panic() {
        let mut s = CommLogState { messages: Vec::new(), blink: u64::MAX };
        s.tick(1);
        assert_eq!(s.blink, 0);
    }
}
