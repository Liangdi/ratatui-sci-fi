//! **Thermometer** — a vertical fill thermometer.
//!
//! A vertical tube with a bulb at the bottom and a mercury column that rises
//! with a ratio — the reactor-temp / coolant gauge. Tall areas read better;
//! give it ≥ 3 rows.
//!
//! ## Spec
//! - [`Bulb`](ThermometerShape::Bulb): bottom row is a `●` bulb, the column
//!   rises above it — the default.
//! - [`Tube`](ThermometerShape::Tube): no bulb, just a plain tube.
//! - Color: `> 0.8` → `alert` (hot), `< 0.2` → `--ok` (cold), else `accent`.
//!
//! ## Implementation notes
//! - Stateless [`Widget`]: `ratio` is configuration.
//! - Colors off the [`Palette`](crate::Palette).
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{Theme, Thermometer, ThermometerShape};
//!
//! let t = Thermometer::new(0.6).shape(ThermometerShape::Bulb).theme(Theme::Weyland);
//! ```

use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};

use crate::Theme;

/// Visual form of a [`Thermometer`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ThermometerShape {
    /// A `●` bulb at the bottom, column above — the default.
    #[default]
    Bulb,
    /// A plain tube (no bulb).
    Tube,
}

/// A sci-fi thermometer.
///
/// Build with [`Thermometer::new`] (the `0.0..=1.0` ratio).
#[derive(Debug, Clone)]
pub struct Thermometer {
    /// Fill level, `0.0..=1.0`.
    pub ratio: f32,
    /// Bulb vs plain tube. Defaults to [`ThermometerShape::Bulb`].
    pub shape: ThermometerShape,
    /// Theme whose [`Palette`](crate::Palette) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Thermometer {
    /// Create a thermometer at `ratio`, default shape/theme.
    pub fn new(ratio: f32) -> Self {
        Self {
            ratio,
            shape: ThermometerShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the shape (see [`ThermometerShape`]).
    #[must_use]
    pub fn shape(mut self, shape: ThermometerShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the thermometer.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// The fill color for a given (clamped) ratio.
    fn fill_color(&self) -> ratatui::style::Color {
        let p = self.theme.palette();
        let r = self.ratio.clamp(0.0, 1.0);
        if r > 0.8 {
            p.alert.color()
        } else if r < 0.2 {
            p.ok.color()
        } else {
            p.accent.color()
        }
    }
}

impl Widget for Thermometer {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let muted = Style::new().fg(self.theme.palette().muted.color());
        let fill = Style::new().fg(self.fill_color());
        let h = area.height;
        let col = area.x + area.width / 2;
        let has_bulb = matches!(self.shape, ThermometerShape::Bulb);
        // Column height (excluding the bulb row).
        let tube_h = if has_bulb { h.saturating_sub(1) } else { h };
        let filled = (self.ratio.clamp(0.0, 1.0) * tube_h as f32).round() as u16;

        for row in 0..h {
            let y = area.y + row;
            let from_bottom = h - 1 - row; // 0 at the bottom row
            let (glyph, style) = if has_bulb && from_bottom == 0 {
                ('●', fill)
            } else {
                // Column rows are 1..=tube_h above the bottom (or 0.. if no bulb).
                let col_row = if has_bulb { from_bottom } else { from_bottom + 1 };
                if col_row <= filled {
                    ('█', fill)
                } else {
                    ('░', muted)
                }
            };
            buf[(col, y)].set_char(glyph).set_style(style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 5;
    const H: u16 = 7;

    fn render(ratio: f32, shape: ThermometerShape, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        Thermometer::new(ratio)
            .shape(shape)
            .theme(theme)
            .render(Rect::new(0, 0, W, H), &mut buf);
        buf
    }

    #[test]
    fn bulb_at_bottom() {
        let buf = render(0.0, ThermometerShape::Bulb, Theme::Cyberpunk);
        let col = W / 2;
        // Bottom row (y = H-1) is the bulb.
        assert_eq!(buf[(col, H - 1)].symbol(), "●", "bulb at the bottom");
    }

    #[test]
    fn tube_has_no_bulb() {
        let buf = render(0.0, ThermometerShape::Tube, Theme::Cyberpunk);
        let col = W / 2;
        assert_ne!(buf[(col, H - 1)].symbol(), "●", "Tube has no bulb");
    }

    #[test]
    fn full_ratio_fills_column() {
        let buf = render(1.0, ThermometerShape::Bulb, Theme::Cyberpunk);
        let col = W / 2;
        // Every row above the bulb is filled (█), no ░.
        for row in 0..H - 1 {
            assert_eq!(buf[(col, row)].symbol(), "█", "row {row} filled at ratio 1.0");
        }
    }

    #[test]
    fn empty_ratio_only_bulb_filled() {
        let buf = render(0.0, ThermometerShape::Bulb, Theme::Cyberpunk);
        let col = W / 2;
        assert_eq!(buf[(col, H - 1)].symbol(), "●", "bulb always present");
        // Row above the bulb is empty tube.
        assert_eq!(buf[(col, H - 2)].symbol(), "░", "empty tube above bulb at ratio 0");
    }

    #[test]
    fn high_ratio_uses_alert_color() {
        let alert = Theme::Cyberpunk.palette().alert.color();
        let buf = render(0.9, ThermometerShape::Bulb, Theme::Cyberpunk);
        let col = W / 2;
        assert_eq!(buf[(col, H - 1)].fg, alert, "ratio > 0.8 → alert");
    }

    #[test]
    fn low_ratio_uses_ok_color() {
        let ok = Theme::Cyberpunk.palette().ok.color();
        let buf = render(0.1, ThermometerShape::Bulb, Theme::Cyberpunk);
        let col = W / 2;
        assert_eq!(buf[(col, H - 1)].fg, ok, "ratio < 0.2 → ok");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        Thermometer::new(0.5).render(Rect::new(0, 0, 0, 0), &mut buf);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
