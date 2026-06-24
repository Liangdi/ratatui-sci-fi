//! **BatteryIndicator** — a battery level gauge.
//!
//! A `[████░░]▐` cell with a positive terminal nub, filled by a ratio and
//! colored by urgency — the laptop / device battery readout.
//!
//! ## Spec
//! - [`Cells`](BatteryShape::Cells): a fixed number of segments (default 6).
//! - [`Bar`](BatteryShape::Bar): a continuous bar spanning the area.
//! - Color: `< 0.2` → `alert`, `< 0.5` → `warn`, else `ok`.
//!
//! ## Implementation notes
//! - Stateless [`Widget`]: `ratio` is configuration.
//! - Colors off the [`Palette`](crate::Palette).
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{BatteryIndicator, BatteryShape, Theme};
//!
//! let b = BatteryIndicator::new(0.75).shape(BatteryShape::Cells).theme(Theme::Fallout);
//! ```

use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};

use crate::Theme;

/// Segment count for the [`Cells`](BatteryShape::Cells) shape.
const CELLS: u16 = 6;

/// Visual form of a [`BatteryIndicator`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BatteryShape {
    /// Fixed-segment cells (`[████░░]▐`) — the default.
    #[default]
    Cells,
    /// A continuous bar spanning the area.
    Bar,
}

/// A sci-fi battery indicator.
///
/// Build with [`BatteryIndicator::new`] (the `0.0..=1.0` ratio).
#[derive(Debug, Clone)]
pub struct BatteryIndicator {
    /// Charge level, `0.0..=1.0`.
    pub ratio: f32,
    /// Cells vs continuous bar. Defaults to [`BatteryShape::Cells`].
    pub shape: BatteryShape,
    /// Theme whose [`Palette`](crate::Palette) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl BatteryIndicator {
    /// Create a battery at `ratio` charge, default shape/theme.
    pub fn new(ratio: f32) -> Self {
        Self {
            ratio,
            shape: BatteryShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the shape (see [`BatteryShape`]).
    #[must_use]
    pub fn shape(mut self, shape: BatteryShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the battery.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// The level color for a given (clamped) ratio.
    fn level_color(&self) -> ratatui::style::Color {
        let p = self.theme.palette();
        let r = self.ratio.clamp(0.0, 1.0);
        if r < 0.2 {
            p.alert.color()
        } else if r < 0.5 {
            p.warn.color()
        } else {
            p.ok.color()
        }
    }
}

impl Widget for BatteryIndicator {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let p = self.theme.palette();
        let muted = Style::new().fg(p.muted.color());
        let on = Style::new().fg(self.level_color());
        let row = area.y + area.height / 2;
        let right = area.right();
        let ratio = self.ratio.clamp(0.0, 1.0);

        let mut x = area.x;
        // Opening bracket.
        if x < right {
            buf[(x, row)].set_char('[').set_style(muted);
            x += 1;
        }
        let seg_w: u16 = match self.shape {
            BatteryShape::Cells => CELLS,
            BatteryShape::Bar => area.width.saturating_sub(3), // [ ] + nub
        };
        let filled = (ratio * seg_w as f32).round() as u16;
        for i in 0..seg_w {
            if x >= right {
                break;
            }
            let (glyph, style) = if i < filled { ('█', on) } else { ('░', muted) };
            buf[(x, row)].set_char(glyph).set_style(style);
            x += 1;
        }
        // Closing bracket + positive-terminal nub.
        if x < right {
            buf[(x, row)].set_char(']').set_style(muted);
            x += 1;
        }
        if x < right {
            buf[(x, row)].set_char('▐').set_style(on);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 16;
    const H: u16 = 3;

    fn render(ratio: f32, shape: BatteryShape, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        BatteryIndicator::new(ratio)
            .shape(shape)
            .theme(theme)
            .render(Rect::new(0, 0, W, H), &mut buf);
        buf
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..W).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()
    }

    #[test]
    fn renders_bracket_and_nub() {
        let buf = render(0.5, BatteryShape::Cells, Theme::Cyberpunk);
        let text = row_text(&buf, H / 2);
        assert!(text.contains('['), "opening bracket: {text:?}");
        assert!(text.contains(']'), "closing bracket");
        assert!(text.contains('▐'), "positive-terminal nub");
    }

    #[test]
    fn full_ratio_all_filled() {
        let buf = render(1.0, BatteryShape::Cells, Theme::Cyberpunk);
        let text = row_text(&buf, H / 2);
        assert!(!text.contains('░'), "full → no empty cells: {text:?}");
    }

    #[test]
    fn empty_ratio_all_empty() {
        let buf = render(0.0, BatteryShape::Cells, Theme::Cyberpunk);
        let text = row_text(&buf, H / 2);
        assert!(text.contains('░'), "empty → all dim cells");
        assert!(!text.contains('█'));
    }

    #[test]
    fn high_ratio_uses_ok_color() {
        let ok = Theme::Cyberpunk.palette().ok.color();
        let buf = render(0.8, BatteryShape::Cells, Theme::Cyberpunk);
        let row = H / 2;
        // First filled cell (index 1, after '[') is ok.
        assert_eq!(buf[(1, row)].fg, ok, "ratio ≥ 0.5 → ok");
    }

    #[test]
    fn low_ratio_uses_alert_color() {
        let alert = Theme::Cyberpunk.palette().alert.color();
        let buf = render(0.1, BatteryShape::Cells, Theme::Cyberpunk);
        let row = H / 2;
        // The nub (last cell of the content) carries the level color.
        let nub_x = (0..W).find(|&x| buf[(x, row)].symbol() == "▐").expect("nub present");
        assert_eq!(buf[(nub_x, row)].fg, alert, "ratio < 0.2 → alert");
    }

    #[test]
    fn mid_ratio_uses_warn_color() {
        let warn = Theme::Cyberpunk.palette().warn.color();
        let buf = render(0.3, BatteryShape::Cells, Theme::Cyberpunk);
        let row = H / 2;
        let nub_x = (0..W).find(|&x| buf[(x, row)].symbol() == "▐").expect("nub present");
        assert_eq!(buf[(nub_x, row)].fg, warn, "0.2 ≤ ratio < 0.5 → warn");
    }

    #[test]
    fn bar_shape_spans_area() {
        // Bar uses the area width; a full bar has no ░.
        let buf = render(1.0, BatteryShape::Bar, Theme::Cyberpunk);
        let text = row_text(&buf, H / 2);
        assert!(!text.contains('░'), "full bar → no empty");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        BatteryIndicator::new(0.5).render(Rect::new(0, 0, 0, 0), &mut buf);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
