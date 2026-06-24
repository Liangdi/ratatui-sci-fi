//! **Tooltip** — a small hover hint.
//!
//! A `[ hint ]` chip (optionally with a `▼` pointer beneath) drawn in the
//! accent color — the "hover for info" callout. Shorter-lived than a
//! [`crate::Badge`], and accent-colored rather than level-colored.
//!
//! ## Implementation notes
//! - Stateless [`Widget`]: `text` is configuration.
//! - [`Pointer`](TooltipShape::Pointer) needs ≥2 rows (the bracket row + the
//!   arrow row beneath it).
//! - Color off the [`Palette`](crate::Palette): `accent`.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{Theme, Tooltip, TooltipShape};
//!
//! let t = Tooltip::new("power core").shape(TooltipShape::Pointer).theme(Theme::DeepSpace);
//! ```

use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};

use crate::Theme;

/// Arrow glyph drawn beneath a [`TooltipShape::Pointer`] tooltip.
pub const POINTER: char = '▼';

/// Visual form of a [`Tooltip`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TooltipShape {
    /// `[ text ]` only.
    #[default]
    Boxed,
    /// `[ text ]` with a `▼` beneath it (pointing down at the subject).
    Pointer,
}

/// A sci-fi tooltip.
///
/// Build with [`Tooltip::new`] (the hint text).
#[derive(Debug, Clone)]
pub struct Tooltip {
    /// The hint text.
    pub text: String,
    /// Boxed vs pointer form. Defaults to [`TooltipShape::Boxed`].
    pub shape: TooltipShape,
    /// Theme whose [`Palette`](crate::Palette) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Tooltip {
    /// Create a tooltip with `text`, default shape/theme.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            shape: TooltipShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the form (see [`TooltipShape`]).
    #[must_use]
    pub fn shape(mut self, shape: TooltipShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the tooltip.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

impl Widget for Tooltip {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let accent = self.theme.palette().accent.color();
        let style = Style::new().fg(accent);
        let content = format!("[ {} ]", self.text);
        let cw = content.chars().count() as u16;

        let row = area.y;
        let x = area.x + area.width.saturating_sub(cw.min(area.width)) / 2;
        for (i, ch) in content.chars().enumerate() {
            let col = x + i as u16;
            if col >= area.right() {
                break;
            }
            buf[(col, row)].set_char(ch).set_style(style);
        }

        // Pointer arrow beneath the bracket, centered on the text.
        if matches!(self.shape, TooltipShape::Pointer) {
            let arrow_row = row + 1;
            if arrow_row < area.bottom() {
                let arrow_x = x + cw / 2;
                if arrow_x < area.right() {
                    buf[(arrow_x, arrow_row)].set_char(POINTER).set_style(style);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 20;
    const H: u16 = 3;

    fn render(text: &str, shape: TooltipShape, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        Tooltip::new(text).shape(shape).theme(theme).render(Rect::new(0, 0, W, H), &mut buf);
        buf
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..W).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()
    }

    #[test]
    fn renders_bracketed_text() {
        let buf = render("hint", TooltipShape::Boxed, Theme::Cyberpunk);
        let text = row_text(&buf, 0);
        assert!(text.contains("[ hint ]"), "bracketed text: {text:?}");
    }

    #[test]
    fn pointer_draws_arrow_below() {
        let buf = render("hint", TooltipShape::Pointer, Theme::Cyberpunk);
        assert_eq!(buf[(0, 1)].symbol(), " ", "row 1 col 0 empty (arrow is centered)");
        // The arrow appears somewhere on row 1.
        let has_arrow = (0..W).any(|x| buf[(x, 1)].symbol() == POINTER.to_string().as_str());
        assert!(has_arrow, "Pointer draws ▼ on row 1");
    }

    #[test]
    fn boxed_has_no_arrow() {
        let buf = render("hint", TooltipShape::Boxed, Theme::Cyberpunk);
        let has_arrow = (0..W).any(|x| buf[(x, 1)].symbol() == POINTER.to_string().as_str());
        assert!(!has_arrow, "Boxed has no arrow");
    }

    #[test]
    fn accent_color() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let buf = render("hint", TooltipShape::Boxed, Theme::Cyberpunk);
        let hx = (0..W).find(|&x| buf[(x, 0)].symbol() == "h").expect("'h'");
        assert_eq!(buf[(hx, 0)].fg, accent, "tooltip is --accent");
    }

    #[test]
    fn pointer_skips_arrow_when_no_room() {
        // 1-row area can't fit the arrow row — no panic, no arrow.
        let mut buf = Buffer::empty(Rect::new(0, 0, W, 1));
        Tooltip::new("hint").shape(TooltipShape::Pointer).render(Rect::new(0, 0, W, 1), &mut buf);
        let has_arrow = (0..W).any(|x| buf[(x, 0)].symbol() == POINTER.to_string().as_str());
        assert!(!has_arrow, "no arrow row when area is 1 tall");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        Tooltip::new("hint").render(Rect::new(0, 0, 0, 0), &mut buf);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
