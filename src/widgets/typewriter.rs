//! **Typewriter** — reveal text one character at a time.
//!
//! A single-row text that types itself out, char by char, driven by the app's
//! tick clock — the classic boot-narrative / AI-dialogue / decode effect. Where
//! [`crate::BootSequence`] reveals whole lines, [`Typewriter`] reveals
//! characters within one line, and finishes with a blinking cursor.
//!
//! ## Spec
//!
//! ```text
//!   DECRYPT█         (mid-type, cursor blinks at the reveal frontier)
//!   DECRYPTING       (done — no cursor)
//! ```
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]: `text` and `ticks_per_char` are immutable
//!   configuration on the widget struct (convention #3); only the animation
//!   clock lives in [`TypewriterState`], advanced each tick by the app.
//! - No `handle_key` — a typewriter auto-advances; the app just calls
//!   [`TypewriterState::tick`] each frame. [`TypewriterState::reset`] replays it.
//! - Reveal math: `revealed = tick / ticks_per_char` (clamped to the text
//!   length). The cursor blinks on the same `DEFAULT_CURSOR_PERIOD` cadence as
//!   [`crate::ScanList`] / [`crate::TextInput`], so all cursors flash in sync.
//! - Styling reuses the [`crate::Value`] (fg) node for the revealed text and the
//!   `Cursor` (accent) node for the blinking caret. `var(--…)`-driven.
//! - Left-aligned (a growing typewriter would jump if centered). All glyphs are
//!   width-1.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{Theme, Typewriter, TypewriterState};
//!
//! let mut state = TypewriterState::new();
//! let widget = Typewriter::new("DECRYPTING SECTOR 7G").ticks_per_char(2).theme(Theme::DeepSpace);
//! // each frame: state.tick(); then render the widget with &mut state.
//! ```

use ratatui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};
use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;
use crate::widgets::list::DEFAULT_CURSOR_PERIOD;

/// Cursor glyph for the [`TypewriterShape::Block`] default.
pub const CURSOR_BLOCK: char = '█';
/// Cursor glyph for the [`TypewriterShape::Bar`] variant.
pub const CURSOR_BAR: char = '_';

/// Visual form of a [`Typewriter`]'s blinking caret.
///
/// Selects the caret glyph drawn at the reveal frontier (or none at all);
/// colors stay on the CSS cascade (`Cursor` = accent), untouched by this enum.
/// The [`TypewriterShape::Block`] default draws the original `█` caret.
///
/// Every caret glyph is Unicode width-1 (see convention #5 at the crate root).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TypewriterShape {
    /// `█` caret — the original look.
    #[default]
    Block,
    /// `_` caret.
    Bar,
    /// No caret (the text just stops growing).
    None,
}

impl TypewriterShape {
    /// The caret glyph for this shape, or `None` for [`TypewriterShape::None`].
    #[must_use]
    pub const fn glyph(self) -> Option<char> {
        match self {
            Self::Block => Some(CURSOR_BLOCK),
            Self::Bar => Some(CURSOR_BAR),
            Self::None => None,
        }
    }
}

/// A sci-fi typewriter text.
///
/// Build with [`Typewriter::new`] (the text), then set the reveal speed with
/// [`Typewriter::ticks_per_char`] and the theme with [`Typewriter::theme`].
#[derive(Debug, Clone)]
pub struct Typewriter {
    /// The full text to type out.
    pub text: String,
    /// Ticks elapsed per revealed character. Defaults to [`DEFAULT_TICKS_PER_CHAR`].
    pub ticks_per_char: u32,
    /// Caret-glyph form. Defaults to [`TypewriterShape::Block`].
    pub shape: TypewriterShape,
    /// Theme whose [`Stylesheet`](ratatui_style::Stylesheet) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

/// Default ticks-per-character — one new char every 3 ticks (~180 ms at 60 Hz).
pub const DEFAULT_TICKS_PER_CHAR: u32 = 3;

impl Default for Typewriter {
    fn default() -> Self {
        Self {
            text: String::new(),
            ticks_per_char: DEFAULT_TICKS_PER_CHAR,
            shape: TypewriterShape::default(),
            theme: Theme::Cyberpunk,
        }
    }
}

impl Typewriter {
    /// Create a typewriter for `text`, default speed and theme.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ticks_per_char: DEFAULT_TICKS_PER_CHAR,
            shape: TypewriterShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the ticks-per-character reveal speed.
    #[must_use]
    pub fn ticks_per_char(mut self, ticks: u32) -> Self {
        self.ticks_per_char = ticks;
        self
    }

    /// Set the caret-glyph form (see [`TypewriterShape`]).
    #[must_use]
    pub fn shape(mut self, shape: TypewriterShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the typewriter.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// How many characters of `text` are currently revealed at this state's
    /// tick. Clamped to the text length, so it never overflows the text.
    #[must_use]
    pub fn revealed_chars(&self, state: &TypewriterState) -> usize {
        let total = self.text.chars().count();
        let per = self.ticks_per_char.max(1) as u64;
        ((state.tick / per) as usize).min(total)
    }

    /// Whether the full text has been revealed.
    #[must_use]
    pub fn done(&self, state: &TypewriterState) -> bool {
        self.revealed_chars(state) >= self.text.chars().count()
    }
}

/// Mutable state for [`Typewriter`].
///
/// `tick` is the animation clock; the app advances it each frame (or calls
/// [`Self::tick`]). [`Self::reset`] rewinds to the start to replay the reveal.
#[derive(Debug, Default, Clone)]
pub struct TypewriterState {
    /// Animation clock, advanced once per frame.
    pub tick: u64,
}

impl TypewriterState {
    /// Create a state at tick 0 (nothing revealed yet).
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance the clock one tick.
    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }

    /// Rewind to tick 0 to replay the reveal from the start.
    pub fn reset(&mut self) {
        self.tick = 0;
    }

    /// Whether the blinking caret is currently visible (shared cadence with
    /// [`crate::ScanListState`] / [`crate::TextInputState`]).
    fn cursor_visible(&self) -> bool {
        (self.tick / DEFAULT_CURSOR_PERIOD.max(1)).is_multiple_of(2)
    }
}

impl StatefulWidget for Typewriter {
    type State = TypewriterState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }

        let revealed = self.revealed_chars(state);
        let done = revealed >= self.text.chars().count();

        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let text_style = sheet.compute_with(&NodeRef::new("Value"), None, &mut scratch).to_style();
        let cursor_style =
            sheet.compute_with(&NodeRef::new("Cursor"), None, &mut scratch).to_style();

        let row = area.y + area.height / 2;
        let right = area.right();

        // Revealed characters, left-aligned.
        let mut x = area.x;
        for ch in self.text.chars().take(revealed) {
            if x >= right {
                break;
            }
            buf[(x, row)].set_char(ch).set_style(text_style);
            x += 1;
        }

        // Blinking caret at the reveal frontier while still typing.
        if !done
            && state.cursor_visible()
            && let Some(glyph) = self.shape.glyph()
            && x < right
        {
            buf[(x, row)].set_char(glyph).set_style(cursor_style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 24;
    const H: u16 = 3;

    fn render(text: &str, tpc: u32, theme: Theme, tick: u64, shape: TypewriterShape) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let mut state = TypewriterState { tick };
        StatefulWidget::render(
            Typewriter::new(text).ticks_per_char(tpc).shape(shape).theme(theme),
            Rect::new(0, 0, W, H),
            &mut buf,
            &mut state,
        );
        buf
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..W).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()
    }

    #[test]
    fn revealed_chars_advances_with_tick() {
        let w = Typewriter::new("ABCDE").ticks_per_char(2);
        let s0 = TypewriterState { tick: 0 };
        let s1 = TypewriterState { tick: 2 };
        let s2 = TypewriterState { tick: 4 };
        assert_eq!(w.revealed_chars(&s0), 0);
        assert_eq!(w.revealed_chars(&s1), 1);
        assert_eq!(w.revealed_chars(&s2), 2);
    }

    #[test]
    fn revealed_clamps_at_text_length() {
        let w = Typewriter::new("AB").ticks_per_char(1);
        let s = TypewriterState { tick: 100 };
        assert_eq!(w.revealed_chars(&s), 2, "never overflows the text");
        assert!(w.done(&s));
    }

    #[test]
    fn empty_text_is_immediately_done() {
        let w = Typewriter::new("");
        let s = TypewriterState::new();
        assert_eq!(w.revealed_chars(&s), 0);
        assert!(w.done(&s), "empty text is done at tick 0");
    }

    #[test]
    fn reset_rewinds_reveal() {
        let mut w_state = TypewriterState { tick: 10 };
        let w = Typewriter::new("ABCDE").ticks_per_char(1);
        assert_eq!(w.revealed_chars(&w_state), 5);
        w_state.reset();
        assert_eq!(w.revealed_chars(&w_state), 0, "reset rewinds to start");
    }

    #[test]
    fn cursor_shown_at_reveal_frontier() {
        // tick 0, tpc 3 → revealed 0, not done; cursor cadence at tick 0 is on.
        let buf = render("HELLO", 3, Theme::Cyberpunk, 0, TypewriterShape::Block);
        let y = H / 2;
        // Column 0 (the frontier, nothing revealed yet) carries the block cursor.
        assert_eq!(buf[(0, y)].symbol(), "█", "cursor sits at the reveal frontier");
    }

    #[test]
    fn cursor_hidden_on_done() {
        // Fully revealed → no cursor.
        let buf = render("AB", 1, Theme::Cyberpunk, 10, TypewriterShape::Block);
        let y = H / 2;
        assert_eq!(buf[(2, y)].symbol(), " ", "no cursor past the fully-revealed text");
    }

    #[test]
    fn revealed_text_uses_fg_color() {
        let fg = Theme::Cyberpunk.palette().fg.color();
        let buf = render("AB", 1, Theme::Cyberpunk, 10, TypewriterShape::Block);
        let y = H / 2;
        assert_eq!(buf[(0, y)].fg, fg, "revealed char should be --fg");
    }

    #[test]
    fn cursor_uses_accent_color() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let buf = render("HELLO", 3, Theme::Cyberpunk, 0, TypewriterShape::Block);
        let y = H / 2;
        assert_eq!(buf[(0, y)].fg, accent, "cursor should be --accent");
    }

    #[test]
    fn bar_shape_uses_underscore_cursor() {
        let buf = render("HELLO", 3, Theme::Cyberpunk, 0, TypewriterShape::Bar);
        let y = H / 2;
        assert_eq!(buf[(0, y)].symbol(), "_", "Bar cursor is '_'");
    }

    #[test]
    fn none_shape_draws_no_cursor() {
        let buf = render("HELLO", 3, Theme::Cyberpunk, 0, TypewriterShape::None);
        let y = H / 2;
        assert_eq!(buf[(0, y)].symbol(), " ", "None shape draws no cursor");
    }

    #[test]
    fn tick_wraps_without_panic() {
        let mut s = TypewriterState { tick: u64::MAX };
        s.tick();
        // wrapping_add → no panic.
        let _ = Typewriter::new("X").revealed_chars(&s);
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let mut state = TypewriterState::new();
        StatefulWidget::render(
            Typewriter::new("X"),
            Rect::new(0, 0, 0, 0),
            &mut buf,
            &mut state,
        );
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }

    #[test]
    fn reveals_chars_left_to_right() {
        // tpc 1: tick 2 → 2 chars revealed ("HE"), cursor/blank after.
        let buf = render("HELLO", 1, Theme::Cyberpunk, 2, TypewriterShape::None);
        let text = row_text(&buf, H / 2);
        assert!(text.starts_with("HE"), "first two chars revealed: {text:?}");
        assert!(!text.contains("L"), "third char not yet revealed: {text:?}");
    }
}
