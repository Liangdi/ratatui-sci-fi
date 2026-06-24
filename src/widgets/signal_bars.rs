//! **SignalBars** — a cellular-signal strength indicator.
//!
//! A row of bars whose filled count tracks a level — the phone-signal /
//! link-quality readout. [`Ascending`](SignalBarsShape::Ascending) draws the
//! classic ramp `▁▂▃▄▅` (lit bars take their ramp glyph); [`Equal`](SignalBarsShape::Equal)
//! draws same-height `█` blocks.
//!
//! ## Spec
//! - `level` bars (out of `bars`) are `accent`; the rest are `muted`.
//!
//! ## Implementation notes
//! - Stateless [`Widget`]: `level`/`bars` are configuration.
//! - Colors off the [`Palette`](crate::Palette). All glyphs are width-1.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{SignalBars, SignalBarsShape, Theme};
//!
//! let s = SignalBars::new(3).bars(5).shape(SignalBarsShape::Ascending).theme(Theme::DeepSpace);
//! ```

use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};

use crate::Theme;

/// Default number of bars.
const DEFAULT_BARS: u8 = 5;
/// Ramp glyphs, shortest → tallest.
const RAMP: [char; 7] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇'];

/// Visual form of [`SignalBars`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SignalBarsShape {
    /// Each bar a ramp glyph (`▁▂▃▄▅…`) — the default.
    #[default]
    Ascending,
    /// Same-height `█` blocks.
    Equal,
}

impl SignalBarsShape {
    /// The glyph for bar `i` of `n`.
    fn glyph(self, i: usize, n: usize) -> char {
        match self {
            Self::Ascending => {
                if n <= 1 {
                    RAMP[RAMP.len() - 1]
                } else {
                    // Spread the ramp across the bar count.
                    let idx = i * (RAMP.len() - 1) / (n - 1);
                    RAMP[idx.min(RAMP.len() - 1)]
                }
            }
            Self::Equal => '█',
        }
    }
}

/// A sci-fi signal-strength indicator.
///
/// Build with [`SignalBars::new`] (the lit level), then set [`SignalBars::bars`]
/// (total count) and the theme.
#[derive(Debug, Clone)]
pub struct SignalBars {
    /// How many bars are lit (`0..=bars`).
    pub level: u8,
    /// Total bar count. Defaults to [`DEFAULT_BARS`] (5); capped at 7 for the ramp.
    pub bars: u8,
    /// Glyph form. Defaults to [`SignalBarsShape::Ascending`].
    pub shape: SignalBarsShape,
    /// Theme whose [`Palette`](crate::Palette) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl SignalBars {
    /// Create a signal indicator with `level` lit bars, default count/theme.
    pub fn new(level: u8) -> Self {
        Self {
            level,
            bars: DEFAULT_BARS,
            shape: SignalBarsShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the total bar count.
    #[must_use]
    pub fn bars(mut self, bars: u8) -> Self {
        self.bars = bars;
        self
    }

    /// Set the glyph form (see [`SignalBarsShape`]).
    #[must_use]
    pub fn shape(mut self, shape: SignalBarsShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the bars.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

impl Widget for SignalBars {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let p = self.theme.palette();
        let lit = Style::new().fg(p.accent.color());
        let dim = Style::new().fg(p.muted.color());

        let n = (self.bars.min(7) as usize).max(1);
        let level = (self.level as usize).min(n);
        let row = area.y + area.height / 2;

        for i in 0..n {
            let x = area.x + i as u16;
            if x >= area.right() {
                break;
            }
            let (glyph, style) = if i < level {
                (self.shape.glyph(i, n), lit)
            } else {
                (self.shape.glyph(i, n), dim)
            };
            buf[(x, row)].set_char(glyph).set_style(style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 12;
    const H: u16 = 3;

    fn render(level: u8, bars: u8, shape: SignalBarsShape, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        SignalBars::new(level)
            .bars(bars)
            .shape(shape)
            .theme(theme)
            .render(Rect::new(0, 0, W, H), &mut buf);
        buf
    }

    #[test]
    fn renders_bars_count() {
        let buf = render(5, 5, SignalBarsShape::Equal, Theme::Cyberpunk);
        let row = H / 2;
        let count = (0..W).filter(|&x| buf[(x, row)].symbol() == "█").count();
        assert_eq!(count, 5, "5 lit bars");
    }

    #[test]
    fn lit_are_accent_dim_are_muted() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let muted = Theme::Cyberpunk.palette().muted.color();
        let buf = render(2, 5, SignalBarsShape::Equal, Theme::Cyberpunk);
        let row = H / 2;
        assert_eq!(buf[(0, row)].fg, accent, "bar 0 lit → accent");
        assert_eq!(buf[(2, row)].fg, muted, "bar 2 dim → muted");
    }

    #[test]
    fn level_zero_all_dim() {
        let muted = Theme::Cyberpunk.palette().muted.color();
        let buf = render(0, 5, SignalBarsShape::Equal, Theme::Cyberpunk);
        let row = H / 2;
        for x in 0..5 {
            assert_eq!(buf[(x, row)].fg, muted, "level 0 → all muted");
        }
    }

    #[test]
    fn level_clamps_to_bars() {
        // level 99 with 5 bars → all 5 lit (no panic).
        let buf = render(99, 5, SignalBarsShape::Equal, Theme::Cyberpunk);
        let row = H / 2;
        let count = (0..5).filter(|&x| buf[(x, row)].symbol() == "█").count();
        assert_eq!(count, 5, "level clamps to bars");
    }

    #[test]
    fn ascending_uses_ramp_glyphs() {
        let buf = render(5, 5, SignalBarsShape::Ascending, Theme::Cyberpunk);
        let row = H / 2;
        // The tallest bar (last) should be a ramp glyph, not '█'.
        assert!(RAMP.contains(&buf[(4, row)].symbol().chars().next().unwrap()));
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        SignalBars::new(3).render(Rect::new(0, 0, 0, 0), &mut buf);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
