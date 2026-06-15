//! **Spinner** — braille activity spinner.
//!
//! A single-row activity indicator: a rotating braille glyph that advances one
//! frame per tick, with an optional muted label beside it. It's the loading
//! companion to [`crate::widgets::EnergyGauge`].
//!
//! ## Spec
//! - Cycle through a braille glyph set: `⠋ ⠙ ⠹ ⠸ ⠴ ⠦ ⠧ ⠇ ⠏` (repeat).
//! - Advance one glyph per `tick()`.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]; only a frame counter lives in
//!   [`SpinnerState`], advanced by [`SpinnerState::tick`]. No RNG — the glyph is
//!   `tick % glyphs.len()` — so output is trivially deterministic.
//! - Styling goes through the theme's
//!   [`Stylesheet`](ratatui_style::Stylesheet) cascade: the glyph resolves via
//!   the `Value` rule (theme foreground — a bright readout), the optional label
//!   via the `Label` rule (muted). Both are `var(--…)`-driven off the same
//!   palette.
//!
//! # Example
//!
//! ```no_run
//! use ratatui_sci_fi::{Spinner, SpinnerState, Theme};
//!
//! let mut state = SpinnerState::default();
//! let spinner = Spinner::new().label("SYNC").theme(Theme::Cyberpunk);
//! // in your event loop: state.tick(); each frame before render.
//! ```

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::StatefulWidget,
};
use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Default braille glyph set. All width-1, so they slot cleanly into one cell.
pub const SPINNER_GLYPHS: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// A braille activity spinner.
///
/// Build with [`Spinner::new`], optionally add a label with [`Spinner::label`]
/// and a theme with [`Spinner::theme`]. Animation lives in the companion
/// [`SpinnerState`], advanced by the app's event loop each tick.
#[derive(Debug, Clone, Default)]
pub struct Spinner {
    /// Optional left-aligned muted label, e.g. `"SYNC"`.
    pub label: Option<String>,
    /// Theme whose [`Stylesheet`](ratatui_style::Stylesheet) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Spinner {
    /// Create an unlabeled spinner, default theme.
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach a left-aligned muted label.
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the theme whose cascade drives colors.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

/// Mutable state for [`Spinner`].
///
/// Just a wrapping frame counter — call [`SpinnerState::tick`] once per frame.
/// [`SpinnerState::current_glyph`] picks the glyph for a given glyph-set length.
#[derive(Debug, Default, Clone)]
pub struct SpinnerState {
    /// Increments each tick; wraps at `u64::MAX` so a long run never panics.
    pub tick: u64,
}

impl SpinnerState {
    /// Advance one frame.
    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }

    /// The glyph to display for a glyph set of `len` glyphs at the current tick.
    pub fn current_glyph(&self, len: usize) -> char {
        // Guard against an empty glyph set (would divide by zero) — fall back to
        // SPINNER_GLYPHS[0].
        let len = len.max(1);
        SPINNER_GLYPHS[(self.tick as usize) % len]
    }
}

impl StatefulWidget for Spinner {
    type State = SpinnerState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let label_style =
            sheet.compute_with(&NodeRef::new("Label"), None, &mut scratch).to_style();
        let glyph_style =
            sheet.compute_with(&NodeRef::new("Value"), None, &mut scratch).to_style();

        let y = area.y + area.height / 2;
        let right = area.x + area.width;
        let mut x = area.x;

        // Optional muted label + one-cell gap.
        if let Some(label) = &self.label {
            for ch in label.chars() {
                if x >= right {
                    break;
                }
                buf[(x, y)].set_symbol(ch.to_string().as_str()).set_style(label_style);
                x += 1;
            }
            if x < right {
                buf[(x, y)].set_style(label_style);
                x += 1;
            }
        }

        // Current glyph in the foreground value color.
        if x < right {
            let glyph = state.current_glyph(SPINNER_GLYPHS.len());
            buf[(x, y)].set_symbol(glyph.to_string().as_str()).set_style(glyph_style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    const W: u16 = 12;
    const H: u16 = 1;

    fn render(label: Option<&str>, theme: Theme, tick: u64) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let mut spinner = Spinner::new().theme(theme);
        if let Some(l) = label {
            spinner = spinner.label(l);
        }
        let mut state = SpinnerState { tick };
        StatefulWidget::render(spinner, Rect::new(0, 0, W, H), &mut buf, &mut state);
        buf
    }

    #[test]
    fn tick_zero_shows_first_glyph() {
        let buf = render(None, Theme::Cyberpunk, 0);
        assert_eq!(buf[(0, 0)].symbol(), SPINNER_GLYPHS[0].to_string());
    }

    #[test]
    fn advancing_tick_advances_glyph() {
        let buf = render(None, Theme::Cyberpunk, 1);
        assert_eq!(buf[(0, 0)].symbol(), SPINNER_GLYPHS[1].to_string());
        let buf = render(None, Theme::Cyberpunk, 3);
        assert_eq!(buf[(0, 0)].symbol(), SPINNER_GLYPHS[3].to_string());
    }

    #[test]
    fn glyph_wraps_at_len() {
        // tick == len -> wraps back to glyph[0].
        let len = SPINNER_GLYPHS.len() as u64;
        let buf = render(None, Theme::Cyberpunk, len);
        assert_eq!(buf[(0, 0)].symbol(), SPINNER_GLYPHS[0].to_string());
        let buf = render(None, Theme::Cyberpunk, len + 2);
        assert_eq!(buf[(0, 0)].symbol(), SPINNER_GLYPHS[2].to_string());
    }

    #[test]
    fn label_renders_left_in_muted() {
        let muted = Theme::Cyberpunk.palette().muted.color();
        let buf = render(Some("SYNC"), Theme::Cyberpunk, 0);
        assert_eq!(buf[(0, 0)].symbol(), "S");
        assert_eq!(buf[(0, 0)].fg, muted);
        // Gap at x=4, glyph at x=5.
        assert_eq!(buf[(5, 0)].symbol(), SPINNER_GLYPHS[0].to_string());
    }

    #[test]
    fn glyph_uses_foreground() {
        let fg = Theme::Cyberpunk.palette().fg.color();
        let buf = render(None, Theme::Cyberpunk, 0);
        assert_eq!(buf[(0, 0)].fg, fg, "glyph should be --fg via the Value rule");
    }

    #[test]
    fn tick_wrapping_does_not_panic_at_max() {
        let mut s = SpinnerState { tick: u64::MAX };
        s.tick(); // wraps to 0
        assert_eq!(s.tick, 0);
        // Glyph lookup still works after the wrap.
        let _ = s.current_glyph(SPINNER_GLYPHS.len());
    }

    #[test]
    fn empty_area_is_a_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let mut state = SpinnerState::default();
        Spinner::new().label("X").render(Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
