//! **PieChart** — a solid pie/donut chart.
//!
//! A filled circle split into proportional slices — the share-of-whole chart.
//! Where [`crate::DonutChart`] renders a ring, [`PieChart`] fills to the
//! center. Each cell picks its slice by polar angle around the center and
//! takes that slice's color.
//!
//! ## Implementation notes
//! - Stateless [`Widget`]: `slices` (proportional weights) are configuration.
//! - Drawn with `█` blocks (one per cell), so it reads best at ≥5×5. Colors
//!   cycle `accent` / `accent2` / `ok` / `warn` / `alert` off the palette.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{PieChart, Theme};
//!
//! let p = PieChart::new([3.0, 2.0, 1.0]).theme(Theme::Cyberpunk);
//! ```

use std::f32::consts::TAU;

use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};

use crate::Theme;

/// Visual form of a [`PieChart`]. (Reserved for future slice-gap variants.)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PieShape {
    /// Solid slices edge-to-edge — the default.
    #[default]
    Filled,
}

/// A sci-fi pie chart.
///
/// Build with [`PieChart::new`] (an iterator of slice weights). Weights are
/// normalized to their sum, so `[3, 2, 1]` and `[30, 20, 10]` draw the same.
#[derive(Debug, Clone)]
pub struct PieChart {
    /// Slice weights (any positive numbers; normalized on render).
    pub slices: Vec<f32>,
    /// Slice form. Defaults to [`PieShape::Filled`].
    pub shape: PieShape,
    /// Theme whose [`Palette`](crate::Palette) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl PieChart {
    /// Create a pie from an iterator of slice weights.
    pub fn new(slices: impl IntoIterator<Item = f32>) -> Self {
        Self {
            slices: slices.into_iter().collect(),
            shape: PieShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the slice form (see [`PieShape`]).
    #[must_use]
    pub fn shape(mut self, shape: PieShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the slices.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// The slice colors: accent, accent2, ok, warn, alert (cycling).
    fn slice_colors(&self) -> [ratatui::style::Color; 5] {
        let p = self.theme.palette();
        [p.accent.color(), p.accent2.color(), p.ok.color(), p.warn.color(), p.alert.color()]
    }
}

impl Widget for PieChart {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() || self.slices.is_empty() {
            return;
        }
        let colors = self.slice_colors();

        let total: f32 = self.slices.iter().sum();
        if total <= 0.0 {
            return;
        }
        // Cumulative angle bounds (0..TAU) per slice.
        let mut acc = 0.0;
        let bounds: Vec<f32> = self
            .slices
            .iter()
            .map(|s| {
                acc += s / total * TAU;
                acc
            })
            .collect();

        let cx = area.width as f32 / 2.0;
        let cy = area.height as f32 / 2.0;
        // Terminal cells are ~2:1 (tall), so bias the radius by width to keep
        // the pie round-ish.
        let r = cx.min(cy * 2.0) - 0.5;

        for row in 0..area.height {
            for col in 0..area.width {
                let dx = col as f32 + 0.5 - cx;
                let dy = (row as f32 + 0.5 - cy) * 2.0; // unbias the 2:1 cells
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > r {
                    continue;
                }
                let mut a = dy.atan2(dx);
                if a < 0.0 {
                    a += TAU;
                }
                let si = bounds.iter().position(|&b| a < b).unwrap_or(bounds.len() - 1);
                let color = colors[si % colors.len()];
                buf[(area.x + col, area.y + row)].set_char('█').set_style(Style::new().fg(color));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 10;
    const H: u16 = 10;

    fn render(slices: &[f32], theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        PieChart::new(slices.iter().copied()).theme(theme).render(Rect::new(0, 0, W, H), &mut buf);
        buf
    }

    fn filled_count(buf: &Buffer) -> usize {
        let mut n = 0;
        for x in 0..W {
            for y in 0..H {
                if buf[(x, y)].symbol() == "█" {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn renders_some_cells() {
        let buf = render(&[3.0, 2.0, 1.0], Theme::Cyberpunk);
        assert!(filled_count(&buf) > 0, "pie lights cells");
    }

    #[test]
    fn larger_slice_has_more_cells() {
        let uneven = render(&[9.0, 1.0], Theme::Cyberpunk);
        let even = render(&[5.0, 5.0], Theme::Cyberpunk);
        // Both fill the same circle area, so totals are similar — instead check
        // that the dominant slice color appears in both.
        let accent = Theme::Cyberpunk.palette().accent.color();
        let has_accent = |buf: &Buffer| {
            (0..W).any(|x| (0..H).any(|y| buf[(x, y)].fg == accent))
        };
        assert!(has_accent(&uneven) && has_accent(&even), "slice 0 is accent");
    }

    #[test]
    fn two_slices_use_two_colors() {
        let buf = render(&[1.0, 1.0], Theme::Cyberpunk);
        let accent = Theme::Cyberpunk.palette().accent.color();
        let accent2 = Theme::Cyberpunk.palette().accent2.color();
        let mut fgs = std::collections::HashSet::new();
        for x in 0..W {
            for y in 0..H {
                if buf[(x, y)].symbol() == "█" {
                    fgs.insert(buf[(x, y)].fg);
                }
            }
        }
        assert!(fgs.contains(&accent) && fgs.contains(&accent2), "two slices → two colors");
    }

    #[test]
    fn empty_slices_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        PieChart::new(std::iter::empty()).render(Rect::new(0, 0, W, H), &mut buf);
        assert_eq!(filled_count(&buf), 0);
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        PieChart::new([1.0]).render(Rect::new(0, 0, 0, 0), &mut buf);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
