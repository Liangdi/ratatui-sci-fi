//! **ImageView** — a centered ASCII-art display.
//!
//! Renders a multi-line ASCII string centered in its area — the logo / banner
//! / ship-schematic slot. The app supplies the art; the widget centers it and
//! colors it.
//!
//! ## Implementation notes
//! - Stateless [`Widget`]: `art` is configuration.
//! - Color off the [`Palette`](crate::Palette): [`Accent`](ImageViewShape::Accent)
//!   by default.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{ImageView, Theme};
//!
//! let v = ImageView::new("  /\\\n /  \\\n/____\\").theme(Theme::DeepSpace);
//! ```

use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};

use crate::Theme;

/// Visual form of an [`ImageView`]: the art's color.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ImageViewShape {
    /// `--accent` — the default.
    #[default]
    Accent,
    /// `--muted`.
    Muted,
    /// `--fg`.
    Fg,
}

/// A sci-fi ASCII-art viewer.
///
/// Build with [`ImageView::new`] (the multi-line art string).
#[derive(Debug, Clone)]
pub struct ImageView {
    /// The ASCII art (lines separated by `\n`).
    pub art: String,
    /// Color form. Defaults to [`ImageViewShape::Accent`].
    pub shape: ImageViewShape,
    /// Theme whose [`Palette`](crate::Palette) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl ImageView {
    /// Create a viewer for `art`, default shape/theme.
    pub fn new(art: impl Into<String>) -> Self {
        Self {
            art: art.into(),
            shape: ImageViewShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the color form (see [`ImageViewShape`]).
    #[must_use]
    pub fn shape(mut self, shape: ImageViewShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the art.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

impl Widget for ImageView {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() || self.art.is_empty() {
            return;
        }
        let p = self.theme.palette();
        let color = match self.shape {
            ImageViewShape::Accent => p.accent.color(),
            ImageViewShape::Muted => p.muted.color(),
            ImageViewShape::Fg => p.fg.color(),
        };
        let style = Style::new().fg(color);

        let lines: Vec<&str> = self.art.lines().collect();
        let h = lines.len() as u16;
        let start_y = area.y + area.height.saturating_sub(h.min(area.height)) / 2;

        for (i, line) in lines.iter().enumerate() {
            let y = start_y + i as u16;
            if y >= area.bottom() {
                break;
            }
            let w = line.chars().count() as u16;
            let start = area.x + area.width.saturating_sub(w.min(area.width)) / 2;
            for (i, ch) in line.chars().enumerate() {
                let col = start + i as u16;
                if col >= area.right() {
                    break;
                }
                if ch != ' ' {
                    buf[(col, y)].set_char(ch).set_style(style);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 16;
    const H: u16 = 6;

    fn render(art: &str, shape: ImageViewShape, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        ImageView::new(art).shape(shape).theme(theme).render(Rect::new(0, 0, W, H), &mut buf);
        buf
    }

    #[test]
    fn renders_art() {
        let buf = render("/\\\n__", ImageViewShape::Accent, Theme::Cyberpunk);
        let has_art = (0..W).any(|x| (0..H).any(|y| buf[(x, y)].symbol() == "/"));
        assert!(has_art, "art rendered");
    }

    #[test]
    fn art_is_centered() {
        // A single short line is centered → leading spaces on the left.
        let buf = render("X", ImageViewShape::Accent, Theme::Cyberpunk);
        let first_x = (0..W).find(|&x| (0..H).any(|y| buf[(x, y)].symbol() == "X")).expect("'X'");
        assert!(first_x > 0, "art is centered (not at col 0): col {first_x}");
    }

    #[test]
    fn accent_color() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let buf = render("X", ImageViewShape::Accent, Theme::Cyberpunk);
        let y = (0..H).find(|&y| (0..W).any(|x| buf[(x, y)].symbol() == "X")).expect("row with X");
        let x = (0..W).find(|&x| buf[(x, y)].symbol() == "X").expect("'X'");
        assert_eq!(buf[(x, y)].fg, accent, "default shape is accent");
    }

    #[test]
    fn muted_shape() {
        let muted = Theme::Cyberpunk.palette().muted.color();
        let buf = render("X", ImageViewShape::Muted, Theme::Cyberpunk);
        let y = (0..H).find(|&y| (0..W).any(|x| buf[(x, y)].symbol() == "X")).expect("row with X");
        let x = (0..W).find(|&x| buf[(x, y)].symbol() == "X").expect("'X'");
        assert_eq!(buf[(x, y)].fg, muted, "Muted shape");
    }

    #[test]
    fn multiline_renders_each_line() {
        let buf = render("A\nB\nC", ImageViewShape::Accent, Theme::Cyberpunk);
        // Three lines, centered vertically: find each.
        let rows_with: Vec<u16> = (0..H).filter(|&y| (0..W).any(|x| buf[(x, y)].symbol() != " ")).collect();
        assert_eq!(rows_with.len(), 3, "three art lines");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        ImageView::new("X").render(Rect::new(0, 0, 0, 0), &mut buf);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
