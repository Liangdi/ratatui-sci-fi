//! **Speedometer** — a half-dial needle gauge.
//!
//! A semicircular dial with a needle pointing at a `0.0..=1.0` value — the
//! speed / pressure / throttle readout. Where [`crate::RadialGauge`] is a full
//! circular core, [`Speedometer`] is a 180° sweep (left = min, right = max).
//!
//! ## Implementation notes
//! - Stateless [`Widget`]: `value` is configuration.
//! - Drawn on a Braille sub-pixel canvas: the arc is sampled across 180°, and
//!   the needle is a Bresenham line from the base to the value's point on the
//!   arc. Color off the [`Palette`](crate::Palette): `accent`.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{Speedometer, Theme};
//!
//! let s = Speedometer::new(0.6).theme(Theme::Weyland);
//! ```

use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};

use crate::Theme;

/// Visual form of a [`Speedometer`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SpeedometerShape {
    /// The 180° arc plus the needle — the default.
    #[default]
    Gauge,
    /// Just the needle (no arc).
    Needle,
}

/// A sci-fi half-dial gauge.
///
/// Build with [`Speedometer::new`] (the `0.0..=1.0` value).
#[derive(Debug, Clone)]
pub struct Speedometer {
    /// The reading, `0.0..=1.0` (clamped).
    pub value: f32,
    /// Gauge vs needle-only. Defaults to [`SpeedometerShape::Gauge`].
    pub shape: SpeedometerShape,
    /// Theme whose [`Palette`](crate::Palette) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Speedometer {
    /// Create a gauge at `value`, default shape/theme.
    pub fn new(value: f32) -> Self {
        Self {
            value,
            shape: SpeedometerShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the form (see [`SpeedometerShape`]).
    #[must_use]
    pub fn shape(mut self, shape: SpeedometerShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the gauge.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

impl Widget for Speedometer {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let pw = (area.width as usize).saturating_mul(2);
        let ph = (area.height as usize).saturating_mul(4);
        if pw < 2 || ph < 2 {
            return;
        }

        let mut lit = vec![false; pw * ph];
        // Base of the dial: bottom-center. Radius spans half the width / height.
        let cx = pw as f32 / 2.0;
        let cy = (ph as f32 - 1.0).min(pw as f32);
        let r = (pw as f32 / 2.0).min(cy) - 1.0;
        if r < 1.0 {
            return;
        }

        let draw_arc = matches!(self.shape, SpeedometerShape::Gauge);
        if draw_arc {
            // Sample the 180° upper arc (y grows downward, so subtract sin).
            for deg in 0..=180 {
                let a = (deg as f32).to_radians();
                let x = cx + r * a.cos();
                let y = cy - r * a.sin();
                set_pixel(&mut lit, pw, ph, x as i64, y as i64);
            }
        }

        // Needle: value 0 → 180° (left), 1 → 0° (right).
        let v = self.value.clamp(0.0, 1.0);
        let deg = 180.0 - v * 180.0;
        let a = deg.to_radians();
        let nx = cx + r * a.cos();
        let ny = cy - r * a.sin();
        for (x, y) in line_points(cx as i64, cy as i64, nx as i64, ny as i64) {
            set_pixel(&mut lit, pw, ph, x, y);
        }

        let style = Style::new().fg(self.theme.palette().accent.color());
        for row in 0..area.height {
            for col in 0..area.width {
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
                    buf[(area.x + col, area.y + row)].set_char(ch).set_style(style);
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
fn set_pixel(grid: &mut [bool], pw: usize, ph: usize, x: i64, y: i64) {
    if x >= 0 && y >= 0 {
        let (xu, yu) = (x as usize, y as usize);
        if xu < pw && yu < ph {
            grid[yu * pw + xu] = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 20;
    const H: u16 = 10;

    fn render(value: f32, shape: SpeedometerShape, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        Speedometer::new(value).shape(shape).theme(theme).render(Rect::new(0, 0, W, H), &mut buf);
        buf
    }

    fn lit_count(buf: &Buffer) -> usize {
        let mut n = 0;
        for x in 0..W {
            for y in 0..H {
                let s = buf[(x, y)].symbol();
                if s != " " && s.chars().next().map(|c| c >= '\u{2800}').unwrap_or(false) {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn renders_arc_and_needle() {
        let buf = render(0.5, SpeedometerShape::Gauge, Theme::Cyberpunk);
        assert!(lit_count(&buf) > 0, "gauge lights cells");
    }

    #[test]
    fn needle_only_has_fewer_cells_than_gauge() {
        let gauge = render(0.5, SpeedometerShape::Gauge, Theme::Cyberpunk);
        let needle = render(0.5, SpeedometerShape::Needle, Theme::Cyberpunk);
        assert!(lit_count(&needle) < lit_count(&gauge), "needle-only < gauge");
    }

    #[test]
    fn needle_moves_with_value() {
        // Different values put the needle at different angles → different cells.
        let left = render(0.0, SpeedometerShape::Needle, Theme::Cyberpunk);
        let right = render(1.0, SpeedometerShape::Needle, Theme::Cyberpunk);
        let lit_at = |buf: &Buffer, x: u16| (0..H).any(|y| buf[(x, y)].symbol() != " ");
        // At value 0 the needle points left; at 1, right.
        assert!(lit_at(&left, 0) || lit_at(&left, 1), "value 0 → needle left");
        assert!(lit_at(&right, W - 2) || lit_at(&right, W - 1), "value 1 → needle right");
    }

    #[test]
    fn out_of_range_value_clamps() {
        // 2.0 clamps to 1.0; no panic, needle at the right.
        let buf = render(2.0, SpeedometerShape::Gauge, Theme::Cyberpunk);
        assert!(lit_count(&buf) > 0);
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        Speedometer::new(0.5).render(Rect::new(0, 0, 0, 0), &mut buf);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
