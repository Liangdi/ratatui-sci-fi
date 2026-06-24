//! **Barcode** — a 1D barcode strip.
//!
//! A row of black bars and white spaces encoding a string — the shipping /
//! inventory scan line. Each character's 8 bits drive a run of bars: a `1` bit
//! is a 2-cell `█` bar, a `0` bit is a 1-cell gap. It reads as a barcode at a
//! glance (not a spec-exact Code39/128 — those would need encoder tables).
//!
//! ## Implementation notes
//! - Stateless [`Widget`]: `data` is configuration.
//! - Bars take `accent`, gaps stay blank. The text can optionally render below.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{Barcode, Theme};
//!
//! let b = Barcode::new("SCN-7G-2049").theme(Theme::Fallout);
//! ```

use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};

use crate::Theme;

/// Default bar glyph.
pub const BAR: char = '█';
/// Bar width for a `1` bit (cells); `0` bits are 1-cell gaps.
const ON_W: u16 = 2;

/// Visual form of a [`Barcode`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BarcodeShape {
    /// Bars only (no caption) — the default.
    #[default]
    Bars,
    /// Bars + the data text rendered on the row beneath.
    WithCaption,
}

/// A sci-fi 1D barcode.
///
/// Build with [`Barcode::new`] (the data string).
#[derive(Debug, Clone)]
pub struct Barcode {
    /// The data to encode.
    pub data: String,
    /// Bars vs bars+caption. Defaults to [`BarcodeShape::Bars`].
    pub shape: BarcodeShape,
    /// Theme whose [`Palette`](crate::Palette) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Barcode {
    /// Create a barcode for `data`, default shape/theme.
    pub fn new(data: impl Into<String>) -> Self {
        Self {
            data: data.into(),
            shape: BarcodeShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the form (see [`BarcodeShape`]).
    #[must_use]
    pub fn shape(mut self, shape: BarcodeShape) -> Self {
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

impl Widget for Barcode {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() || self.data.is_empty() {
            return;
        }
        let style = Style::new().fg(self.theme.palette().accent.color());
        let right = area.right();
        let bar_row = area.y;
        let caption_row = area.y + area.height / 2 + 1;

        // Encode each char's 8 bits as a run of bars (1-bit → 2 cells) / gaps
        // (0-bit → 1 cell).
        let mut col = area.x;
        for ch in self.data.chars() {
            let mut code = ch as u32;
            for _ in 0..8 {
                let on = code & 1 == 1;
                let w = if on { ON_W } else { 1 };
                if on {
                    for _ in 0..w {
                        if col >= right {
                            break;
                        }
                        buf[(col, bar_row)].set_char(BAR).set_style(style);
                        col += 1;
                    }
                } else {
                    col += w;
                }
                code >>= 1;
            }
            if col >= right {
                break;
            }
        }

        // Optional caption beneath the bars.
        if matches!(self.shape, BarcodeShape::WithCaption) && caption_row < area.bottom() {
            let w = self.data.chars().count() as u16;
            let x = area.x + area.width.saturating_sub(w) / 2;
            for (i, ch) in self.data.chars().enumerate() {
                let c = x + i as u16;
                if c >= right {
                    break;
                }
                buf[(c, caption_row)].set_char(ch).set_style(style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 60;
    const H: u16 = 3;

    fn render(data: &str, shape: BarcodeShape, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        Barcode::new(data).shape(shape).theme(theme).render(Rect::new(0, 0, W, H), &mut buf);
        buf
    }

    #[test]
    fn renders_some_bars() {
        let buf = render("ABC", BarcodeShape::Bars, Theme::Cyberpunk);
        let bars = (0..W).filter(|&x| buf[(x, 0)].symbol() == BAR.to_string().as_str()).count();
        assert!(bars > 0, "some bars drawn");
    }

    #[test]
    fn different_data_different_bars() {
        let a = render("A", BarcodeShape::Bars, Theme::Cyberpunk);
        let b = render("B", BarcodeShape::Bars, Theme::Cyberpunk);
        assert_ne!(a[(0, 0)].symbol(), b[(0, 0)].symbol(), "A vs B differ at col 0");
    }

    #[test]
    fn bars_are_accent() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let buf = render("A", BarcodeShape::Bars, Theme::Cyberpunk);
        let bar_x = (0..W).find(|&x| buf[(x, 0)].symbol() == BAR.to_string().as_str()).expect("a bar");
        assert_eq!(buf[(bar_x, 0)].fg, accent, "bar is --accent");
    }

    #[test]
    fn caption_renders_data() {
        let buf = render("HI", BarcodeShape::WithCaption, Theme::Cyberpunk);
        let caption_row = H / 2 + 1;
        let text: String = (0..W).map(|x| buf[(x, caption_row)].symbol().to_string()).collect();
        assert!(text.contains('H') && text.contains('I'), "caption shows data: {text:?}");
    }

    #[test]
    fn bars_only_no_caption() {
        let buf = render("HI", BarcodeShape::Bars, Theme::Cyberpunk);
        let caption_row = H / 2 + 1;
        assert_eq!(buf[(0, caption_row)].symbol(), " ", "no caption in Bars shape");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        Barcode::new("A").render(Rect::new(0, 0, 0, 0), &mut buf);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
