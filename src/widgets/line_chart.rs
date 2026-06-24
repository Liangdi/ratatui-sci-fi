//! **LineChart** — an axis-labelled trend line.
//!
//! A single-series line plot with X/Y axes (`│` / `─` / `└`) — the standard
//! trend chart. Where [`crate::Sparkline`] is a compact axis-less mini-line
//! and [`crate::StripChart`] is a rolling multi-trace monitor, [`LineChart`]
//! draws a static series with reference axes.
//!
//! ## Implementation notes
//! - Stateless [`Widget`]: `data` is configuration.
//! - The axes occupy the leftmost column + bottom row (muted); the line is
//!   rasterized on a Braille sub-pixel canvas inside the remaining area.
//!   Color off the [`Palette`](crate::Palette): line `accent`, axes `muted`.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{LineChart, Theme};
//!
//! let lc = LineChart::new([0.0, 1.0, 0.5, 2.0, 1.5]).theme(Theme::DeepSpace);
//! ```

use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};

use crate::Theme;

/// Visual form of a [`LineChart`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LineChartShape {
    /// Draw the `│`/`─`/`└` axes — the default.
    #[default]
    Axes,
    /// No axes; the line fills the whole area.
    Bare,
}

/// A sci-fi axis-labelled line chart.
///
/// Build with [`LineChart::new`] (an iterator of `f32` samples).
#[derive(Debug, Clone)]
pub struct LineChart {
    /// The series samples, left → right.
    pub data: Vec<f32>,
    /// Axes vs bare. Defaults to [`LineChartShape::Axes`].
    pub shape: LineChartShape,
    /// Theme whose [`Palette`](crate::Palette) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl LineChart {
    /// Create a chart from an iterator of samples.
    pub fn new(data: impl IntoIterator<Item = f32>) -> Self {
        Self {
            data: data.into_iter().collect(),
            shape: LineChartShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the form (see [`LineChartShape`]).
    #[must_use]
    pub fn shape(mut self, shape: LineChartShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the chart.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

impl Widget for LineChart {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() || self.data.len() < 2 {
            return;
        }
        let p = self.theme.palette();
        let muted = Style::new().fg(p.muted.color());
        let line = Style::new().fg(p.accent.color());

        let draw_axes = matches!(self.shape, LineChartShape::Axes);
        // Plot area shrinks by 1 col (left) + 1 row (bottom) when axes are on.
        let plot = if draw_axes && area.width > 1 && area.height > 1 {
            Rect::new(area.x + 1, area.y, area.width - 1, area.height - 1)
        } else {
            area
        };

        // Axes.
        if draw_axes {
            for row in 0..area.height {
                buf[(area.x, area.y + row)].set_char('│').set_style(muted);
            }
            let bottom = area.bottom() - 1;
            for col in 0..area.width {
                buf[(area.x + col, bottom)].set_char('─').set_style(muted);
            }
            buf[(area.x, bottom)].set_char('└').set_style(muted);
        }

        if plot.width == 0 || plot.height == 0 {
            return;
        }

        // Value range.
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for &v in &self.data {
            min = min.min(v);
            max = max.max(v);
        }
        let span = (max - min).max(1e-9);
        let n = self.data.len();

        let pw = (plot.width as usize).saturating_mul(2);
        let ph = (plot.height as usize).saturating_mul(4);
        let mut lit = vec![false; pw * ph];

        // Map data index → x pixel, value → y pixel (inverted: high value = small y).
        let point = |i: usize| -> (i64, i64) {
            let x = if n == 1 { 0.0 } else { i as f32 * (pw as f32 - 1.0) / (n - 1) as f32 };
            let v = self.data[i];
            let y = (max - v) / span * (ph as f32 - 1.0);
            (x as i64, y as i64)
        };

        for i in 1..n {
            let (x0, y0) = point(i - 1);
            let (x1, y1) = point(i);
            for (x, y) in line_points(x0, y0, x1, y1) {
                set_pixel(&mut lit, pw, ph, x, y);
            }
        }

        // Braille combine.
        for row in 0..plot.height {
            for col in 0..plot.width {
                let mut bits = 0u8;
                for sy in 0..4u16 {
                    for sx in 0..2u16 {
                        let px = (col * 2 + sx) as usize;
                        let py = (row * 4 + sy) as usize;
                        if lit[py * pw + px] {
                            bits |= 1 << (sx + sy * 2);
                        }
                    }
                }
                if bits != 0 {
                    let ch = char::from_u32(0x2800 + bits as u32).unwrap_or(' ');
                    buf[(plot.x + col, plot.y + row)].set_char(ch).set_style(line);
                }
            }
        }
    }
}

/// Bresenham's line between two pixel endpoints.
fn line_points(x0: i64, y0: i64, x1: i64, y1: i64) -> Vec<(i64, i64)> {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;
    let mut x = x0;
    let mut y = y0;
    let mut pts = Vec::new();
    loop {
        pts.push((x, y));
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }
    pts
}

/// Bounds-checked pixel set.
fn set_pixel(grid: &mut [bool], pw: usize, _ph: usize, x: i64, y: i64) {
    if x >= 0 && y >= 0 {
        let (xu, yu) = (x as usize, y as usize);
        if xu < pw && yu < grid.len() / pw {
            grid[yu * pw + xu] = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 20;
    const H: u16 = 8;

    fn render(data: &[f32], shape: LineChartShape, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        LineChart::new(data.iter().copied())
            .shape(shape)
            .theme(theme)
            .render(Rect::new(0, 0, W, H), &mut buf);
        buf
    }

    fn has_axis(buf: &Buffer) -> bool {
        (0..H).any(|y| buf[(0, y)].symbol() == "│")
    }

    #[test]
    fn renders_axes_and_line() {
        let buf = render(&[0.0, 1.0, 0.0, 1.0], LineChartShape::Axes, Theme::Cyberpunk);
        assert!(has_axis(&buf), "axes drawn");
        // Some braille line cell exists.
        let mut has_line = false;
        'scan: for x in 1..W {
            for y in 0..H - 1 {
                if buf[(x, y)].symbol().chars().next().map(|c| c >= '\u{2800}').unwrap_or(false) {
                    has_line = true;
                    break 'scan;
                }
            }
        }
        assert!(has_line, "line drawn in plot area");
    }

    #[test]
    fn bare_has_no_axes() {
        let buf = render(&[0.0, 1.0, 0.0], LineChartShape::Bare, Theme::Cyberpunk);
        assert!(!has_axis(&buf), "Bare draws no axis");
    }

    #[test]
    fn line_is_accent() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let buf = render(&[0.0, 1.0], LineChartShape::Bare, Theme::Cyberpunk);
        let mut lit = None;
        'scan: for x in 0..W {
            for y in 0..H {
                if buf[(x, y)].symbol().chars().next().map(|c| c >= '\u{2800}').unwrap_or(false) {
                    lit = Some((x, y));
                    break 'scan;
                }
            }
        }
        let lit = lit.expect("a lit cell");
        assert_eq!(buf[lit].fg, accent, "line is --accent");
    }

    #[test]
    fn single_point_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        LineChart::new([1.0]).render(Rect::new(0, 0, W, H), &mut buf);
        assert_eq!(buf[(0, 0)].symbol(), " ", "need ≥2 points to draw a line");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        LineChart::new([0.0, 1.0]).render(Rect::new(0, 0, 0, 0), &mut buf);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
