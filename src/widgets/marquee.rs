//! **Marquee** — horizontally scrolling ticker text.
//!
//! A single-row text that scrolls sideways forever — the alert ticker / news
//! crawl / banner effect. Driven by the app's tick clock, it advances one
//! column every `speed` ticks and wraps around, so a long message stays
//! readable in a narrow band.
//!
//! ## Spec
//!
//! ```text
//!   …ING SECTOR 7G   DECRYPT   (text scrolls left, a gap follows, then loops)
//! ```
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]: `text` and `speed` are immutable
//!   configuration on the widget struct (convention #3); only the animation
//!   clock lives in [`MarqueeState`], advanced each tick by the app.
//! - Scroll math: one virtual belt of length `text_len + area_width` (the text
//!   plus a one-screen gap) advances `tick / speed` columns and wraps. Each
//!   visible column maps to a belt index; in-range indices show a text char,
//!   out-of-range show a blank — so the text scrolls fully off before looping.
//! - Styling reuses the [`crate::Value`] (fg) node. `var(--…)`-driven.
//! - All glyphs are width-1.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{Marquee, MarqueeState, Theme};
//!
//! let mut state = MarqueeState::new();
//! let widget = Marquee::new("DECRYPT SECTOR 7G").speed(2).theme(Theme::DeepSpace);
//! // each frame: state.tick(); then render the widget with &mut state.
//! ```

use ratatui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};
use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Default ticks per scroll column — one column shift every 2 ticks (~30 ms
/// at 60 Hz).
pub const DEFAULT_SPEED: u32 = 2;

/// Scroll direction for a [`Marquee`].
///
/// Affects only the scroll direction; colors stay on the CSS cascade, so this
/// enum carries no glyphs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MarqueeShape {
    /// Scroll left (text enters from the right, exits left) — the default.
    #[default]
    Left,
    /// Scroll right (text enters from the left, exits right).
    Right,
}

/// A sci-fi scrolling marquee.
///
/// Build with [`Marquee::new`] (the text), then set the scroll speed with
/// [`Marquee::speed`] and the theme with [`Marquee::theme`].
#[derive(Debug, Clone)]
pub struct Marquee {
    /// The text to scroll.
    pub text: String,
    /// Ticks per column shift. Defaults to [`DEFAULT_SPEED`].
    pub speed: u32,
    /// Scroll direction. Defaults to [`MarqueeShape::Left`].
    pub shape: MarqueeShape,
    /// Theme whose [`Stylesheet`](ratatui_style::Stylesheet) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Default for Marquee {
    fn default() -> Self {
        Self {
            text: String::new(),
            speed: DEFAULT_SPEED,
            shape: MarqueeShape::default(),
            theme: Theme::Cyberpunk,
        }
    }
}

impl Marquee {
    /// Create a marquee for `text`, default speed and theme.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            speed: DEFAULT_SPEED,
            shape: MarqueeShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the ticks-per-column scroll speed.
    #[must_use]
    pub fn speed(mut self, speed: u32) -> Self {
        self.speed = speed;
        self
    }

    /// Set the scroll direction (see [`MarqueeShape`]).
    #[must_use]
    pub fn shape(mut self, shape: MarqueeShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the marquee.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

/// Mutable state for [`Marquee`].
///
/// `tick` is the animation clock; the app advances it each frame (or calls
/// [`Self::tick`]).
#[derive(Debug, Default, Clone)]
pub struct MarqueeState {
    /// Animation clock, advanced once per frame.
    pub tick: u64,
}

impl MarqueeState {
    /// Create a state at tick 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance the clock one tick.
    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }
}

impl StatefulWidget for Marquee {
    type State = MarqueeState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }

        let chars: Vec<char> = self.text.chars().collect();
        let text_len = chars.len() as u64;
        // Empty text — nothing to scroll. Leave the area untouched.
        if text_len == 0 {
            return;
        }

        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let style = sheet.compute_with(&NodeRef::new("Value"), None, &mut scratch).to_style();

        // One belt = the text + a one-screen gap, so the text scrolls fully off
        // before it re-enters. Left scrolls the text leftward (column + pos);
        // Right runs the belt backwards (period - pos) so it scrolls rightward.
        let width = area.width as u64;
        let period = text_len + width;
        let raw = state.tick / self.speed.max(1) as u64;
        let pos = match self.shape {
            MarqueeShape::Left => raw % period,
            MarqueeShape::Right => (period - raw % period) % period,
        };

        let row = area.y + area.height / 2;
        for col in 0..area.width {
            let i = (col as u64 + pos) % period;
            let ch = if i < text_len { chars[i as usize] } else { ' ' };
            buf[(area.x + col, row)].set_char(ch).set_style(style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 10;
    const H: u16 = 3;

    fn render(text: &str, speed: u32, theme: Theme, tick: u64, shape: MarqueeShape) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let mut state = MarqueeState { tick };
        StatefulWidget::render(
            Marquee::new(text).speed(speed).shape(shape).theme(theme),
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
    fn starts_with_text_left_aligned_at_tick_zero() {
        // tick 0, speed 1 → pos 0; the text begins left-aligned (col 0 = text[0]).
        let buf = render("ABC", 1, Theme::Cyberpunk, 0, MarqueeShape::Left);
        let text = row_text(&buf, H / 2);
        assert!(text.starts_with("ABC"), "text left-aligned at tick 0: {text:?}");
    }

    #[test]
    fn scrolls_left_as_tick_advances() {
        // tick 0 → col0='A'; tick 1 (speed 1, pos 1) → col0='B' (belt shifted).
        let buf0 = render("ABC", 1, Theme::Cyberpunk, 0, MarqueeShape::Left);
        let buf1 = render("ABC", 1, Theme::Cyberpunk, 1, MarqueeShape::Left);
        assert_eq!(buf0[(0, H / 2)].symbol(), "A");
        assert_eq!(buf1[(0, H / 2)].symbol(), "B", "tick advances the scroll");
    }

    #[test]
    fn wraps_without_panic() {
        // A huge tick must wrap via the modulo, not overflow.
        let buf = render("ABC", 1, Theme::Cyberpunk, u64::MAX / 2, MarqueeShape::Left);
        // Just confirm it renders without panicking.
        let _ = row_text(&buf, H / 2);
    }

    #[test]
    fn text_eventually_scrolls_fully_off() {
        // period = text_len(3) + width(10) = 13. At pos = text_len (3), col 0
        // maps to belt index 3 → out of text → blank.
        let buf = render("ABC", 1, Theme::Cyberpunk, 3, MarqueeShape::Left);
        assert_eq!(buf[(0, H / 2)].symbol(), " ", "text scrolled off the left edge");
    }

    #[test]
    fn loops_back_after_full_period() {
        // pos = period (13) wraps to 0 → same as tick 0 (text left-aligned).
        let buf0 = render("ABC", 1, Theme::Cyberpunk, 0, MarqueeShape::Left);
        let buf_period = render("ABC", 1, Theme::Cyberpunk, 13, MarqueeShape::Left);
        assert_eq!(buf0[(0, H / 2)].symbol(), buf_period[(0, H / 2)].symbol());
    }

    #[test]
    fn right_direction_scrolls_the_other_way() {
        // Left at tick 1 → col0='B'; Right at tick 1 → col0 is a different char
        // (the belt runs backwards), so they differ.
        let left = render("ABC", 1, Theme::Cyberpunk, 1, MarqueeShape::Left);
        let right = render("ABC", 1, Theme::Cyberpunk, 1, MarqueeShape::Right);
        assert_ne!(
            left[(0, H / 2)].symbol(),
            right[(0, H / 2)].symbol(),
            "Left and Right scroll in opposite directions"
        );
    }

    #[test]
    fn text_uses_fg_color() {
        let fg = Theme::Cyberpunk.palette().fg.color();
        let buf = render("ABC", 1, Theme::Cyberpunk, 0, MarqueeShape::Left);
        assert_eq!(buf[(0, H / 2)].fg, fg, "scrolled text should be --fg");
    }

    #[test]
    fn empty_text_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let mut state = MarqueeState::new();
        StatefulWidget::render(
            Marquee::new(""),
            Rect::new(0, 0, W, H),
            &mut buf,
            &mut state,
        );
        assert_eq!(buf[(0, 0)].symbol(), " ", "empty text leaves the area blank");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let mut state = MarqueeState::new();
        StatefulWidget::render(Marquee::new("ABC"), Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }

    #[test]
    fn tick_wraps_without_panic() {
        let mut s = MarqueeState { tick: u64::MAX };
        s.tick();
        assert_eq!(s.tick, 0);
    }
}
