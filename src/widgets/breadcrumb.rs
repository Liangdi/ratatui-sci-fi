//! **Breadcrumb** — a `>` separated navigation path.
//!
//! A left-aligned `HOME > SECTOR 7G > DOCK` trail — the "where am I" path
//! header. The final (current) segment is `accent`; the rest are `muted`.
//!
//! ## Implementation notes
//! - Stateless [`Widget`]: the path is configuration.
//! - Styling reuses the `Value` (fg/accent for the current segment) and `Label`
//!   (muted for ancestors + separators) cascade nodes.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{Breadcrumb, BreadcrumbShape, Theme};
//!
//! let bc = Breadcrumb::new(["HOME", "SECTOR 7G", "DOCK"])
//!     .shape(BreadcrumbShape::Chevron)
//!     .theme(Theme::DeepSpace);
//! ```

use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};
use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Separator glyph for the [`BreadcrumbShape::Chevron`] default.
pub const SEP_CHEVRON: char = '>';

/// Visual form of a [`Breadcrumb`]'s separator.
///
/// Selects the separator glyph; colors stay on the cascade, untouched by this
/// enum. Every glyph is Unicode width-1.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BreadcrumbShape {
    /// `>` separator — the default.
    #[default]
    Chevron,
    /// `/` separator.
    Slash,
    /// `►` separator.
    Arrow,
}

impl BreadcrumbShape {
    /// The separator glyph.
    #[must_use]
    pub const fn sep(self) -> char {
        match self {
            Self::Chevron => SEP_CHEVRON,
            Self::Slash => '/',
            Self::Arrow => '►',
        }
    }
}

/// A sci-fi breadcrumb trail.
///
/// Build with [`Breadcrumb::new`] (an iterator of path segments).
#[derive(Debug, Clone)]
pub struct Breadcrumb {
    /// Path segments, root → current.
    pub path: Vec<String>,
    /// Separator form. Defaults to [`BreadcrumbShape::Chevron`].
    pub shape: BreadcrumbShape,
    /// Theme whose [`Stylesheet`](ratatui_style::Stylesheet) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Breadcrumb {
    /// Create a breadcrumb from an iterator of path segments.
    pub fn new(path: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            path: path.into_iter().map(Into::into).collect(),
            shape: BreadcrumbShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the separator form (see [`BreadcrumbShape`]).
    #[must_use]
    pub fn shape(mut self, shape: BreadcrumbShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the breadcrumb.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

impl Widget for Breadcrumb {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() || self.path.is_empty() {
            return;
        }

        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let cur_style = sheet.compute_with(&NodeRef::new("Value"), None, &mut scratch).to_style();
        let anc_style = sheet.compute_with(&NodeRef::new("Label"), None, &mut scratch).to_style();
        let sep = self.shape.sep();
        let last = self.path.len() - 1;

        let row = area.y + area.height / 2;
        let mut col = area.x;
        for (i, seg) in self.path.iter().enumerate() {
            // Separator before every segment but the first: " > ".
            if i > 0 {
                for ch in [' ', sep, ' '] {
                    if col >= area.right() {
                        return;
                    }
                    buf[(col, row)].set_char(ch).set_style(anc_style);
                    col += 1;
                }
            }
            // Segment text; the last one is the current (accent).
            let style = if i == last { cur_style } else { anc_style };
            for ch in seg.chars() {
                if col >= area.right() {
                    return;
                }
                buf[(col, row)].set_char(ch).set_style(style);
                col += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 40;
    const H: u16 = 3;

    fn render(path: &[&str], shape: BreadcrumbShape, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        Breadcrumb::new(path.iter().copied())
            .shape(shape)
            .theme(theme)
            .render(Rect::new(0, 0, W, H), &mut buf);
        buf
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..W).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()
    }

    #[test]
    fn renders_path_with_separators() {
        let buf = render(&["HOME", "DOCK"], BreadcrumbShape::Chevron, Theme::Cyberpunk);
        let text = row_text(&buf, H / 2);
        assert!(text.starts_with("HOME"), "root segment first: {text:?}");
        assert!(text.contains(SEP_CHEVRON), "separator present");
        assert!(text.contains("DOCK"), "current segment present");
    }

    #[test]
    fn last_segment_is_accent_ancestors_muted() {
        let fg = Theme::Cyberpunk.palette().fg.color();
        let muted = Theme::Cyberpunk.palette().muted.color();
        let buf = render(&["A", "B"], BreadcrumbShape::Chevron, Theme::Cyberpunk);
        let row = H / 2;
        let a_x = (0..W).find(|&x| buf[(x, row)].symbol() == "A").expect("'A'");
        let b_x = (0..W).find(|&x| buf[(x, row)].symbol() == "B").expect("'B'");
        assert_eq!(buf[(a_x, row)].fg, muted, "ancestor A is muted");
        assert_eq!(buf[(b_x, row)].fg, fg, "current B is fg/accent");
    }

    #[test]
    fn single_segment_no_separator() {
        let buf = render(&["SOLO"], BreadcrumbShape::Chevron, Theme::Cyberpunk);
        let text = row_text(&buf, H / 2);
        assert!(text.starts_with("SOLO"));
        assert!(!text.contains(SEP_CHEVRON), "no separator with one segment");
    }

    #[test]
    fn slash_shape_uses_slash() {
        let buf = render(&["A", "B"], BreadcrumbShape::Slash, Theme::Cyberpunk);
        let text = row_text(&buf, H / 2);
        assert!(text.contains('/'), "Slash separator: {text:?}");
    }

    #[test]
    fn empty_path_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        Breadcrumb::new(std::iter::empty::<&str>()).render(Rect::new(0, 0, W, H), &mut buf);
        assert_eq!(buf[(0, H / 2)].symbol(), " ");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        Breadcrumb::new(["A"]).render(Rect::new(0, 0, 0, 0), &mut buf);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
