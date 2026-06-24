//! **Panel** — sci-fi titled container frame.
//!
//! A bordered, optionally-titled panel — the basic sci-fi container. It wraps
//! arbitrary content the way a ratatui [`Block`] does, but its look comes from
//! the theme's [`Stylesheet`](ratatui_style::Stylesheet) cascade rather than a
//! hand-built border spec.
//!
//! ## Spec
//! - A double-line bordered frame with one-cell interior padding — the classic
//!   "console viewport" look. An optional title sits on the top border.
//!
//! ## Implementation notes
//! - Stateless [`Widget`]: theme + title are per-frame config, no animation.
//! - The frame is built straight from the cascade via
//!   [`ComputedStyle::to_block`](ratatui_style::Computed::to_block): the `Frame`
//!   rule already declares `border: double; padding: 1`, so `to_block()` yields
//!   a complete double-bordered, padded block whose border color is the resolved
//!   `--muted`. No hand-rolled `Borders`/`BorderType`.
//! - [`Panel::inner`] mirrors [`ratatui::widgets::Block::inner`] (and
//!   [`crate::widgets::TargetLock::inner`]): it shrinks `area` by the border +
//!   padding so callers can render content without overwriting the frame.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{Panel, Theme};
//!
//! let panel = Panel::new().title("TELEMETRY").theme(Theme::Weyland);
//! // let inner = panel.inner(area); f.render_widget(panel, area); // … render into inner
//! ```

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::Style,
    text::Line,
    widgets::{BorderType, Widget},
};
use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Visual form of a [`Panel`]'s border.
///
/// Selects the [`BorderType`] the cascade-built [`Block`](ratatui::widgets::Block)
/// is rendered with; border color, padding, and which sides are bordered stay on
/// the CSS cascade (`Frame` rule), untouched by this enum. The
/// [`PanelShape::Double`] default reproduces the original `border: double` look
/// byte-for-byte, so existing tests pass unchanged.
///
/// Geometry is identical across all variants — only the border glyphs change —
/// so [`Panel::inner`] is unaffected by the chosen shape.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PanelShape {
    /// Double-line border (`╔` … `╝`) — the CSS-driven default.
    #[default]
    Double,
    /// Plain single-line border (`┌` … `┘`).
    Single,
    /// Rounded single-line border (`╭` … `╯`).
    Rounded,
    /// Thick single-line border (`┏` … `┛`).
    Thick,
}

impl PanelShape {
    /// The ratatui [`BorderType`] this shape maps to.
    #[must_use]
    pub const fn border_type(self) -> BorderType {
        match self {
            Self::Double => BorderType::Double,
            Self::Single => BorderType::Plain,
            Self::Rounded => BorderType::Rounded,
            Self::Thick => BorderType::Thick,
        }
    }
}

/// A sci-fi titled container panel.
///
/// Build with [`Panel::new`], optionally add a title with [`Panel::title`] and a
/// theme with [`Panel::theme`], then [`Panel::inner`] gives the content area.
/// Rendering the panel paints only its frame; render your content into the
/// rect returned by `inner` separately.
#[derive(Debug, Clone, Default)]
pub struct Panel {
    /// Optional title rendered on the top border.
    pub title: Option<String>,
    /// Border-glyph form (the [`BorderType`] override). Defaults to
    /// [`PanelShape::Double`], the CSS-driven `border: double` look.
    pub shape: PanelShape,
    /// Theme whose [`Stylesheet`](ratatui_style::Stylesheet) drives the frame's
    /// border color. Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Panel {
    /// Create an untitled panel, default theme.
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach a title rendered on the top border (left-aligned).
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the border-glyph form (see [`PanelShape`]).
    #[must_use]
    pub fn shape(mut self, shape: PanelShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme whose cascade drives the frame colors.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// The content rect inside the frame — `area` shrunk by the border +
    /// padding the `Frame` cascade declares. Safe for tiny areas: a frame too
    /// small to hold interior content yields a zero-size rect, never an
    /// out-of-bounds one.
    ///
    /// Mirrors [`ratatui::widgets::Block::inner`] in contract.
    pub fn inner(&self, area: Rect) -> Rect {
        // `to_block()` returns a `Block` borrowing the local `ComputedStyle`, so
        // it must be consumed in the same scope — drive the geometry here and
        // return the owned `Rect` only.
        let mut scratch = ComputeScratch::new();
        let computed = self.computed(&mut scratch);
        computed.to_block().inner(area)
    }

    /// The border-style `Style` (foreground) the cascade resolves for `Frame`.
    /// Used by tests; render reuses the block directly.
    #[cfg(test)]
    fn border_fg(&self) -> ratatui::style::Color {
        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        sheet
            .compute_with(&NodeRef::new("Frame"), None, &mut scratch)
            .to_style()
            .fg
            .unwrap()
    }

    /// Resolve the `Frame` cascade into an owned [`ComputedStyle`] (plus the
    /// title style and interior bg, pulled before any borrow). Returning the
    /// owned `ComputedStyle` keeps callers free to call `to_block()` locally.
    fn computed(&self, scratch: &mut ComputeScratch) -> ratatui_style::ComputedStyle {
        let sheet = self.theme.stylesheet();
        sheet.compute_with(&NodeRef::new("Frame"), None, scratch).clone()
    }
}

impl Widget for Panel {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let computed = self.computed(&mut scratch);

        // Pull the title style + interior bg from the cascade before borrowing
        // `computed` for `to_block()`.
        let title_style = sheet
            .compute_with(&NodeRef::new("Frame").classes(&["title"]), None, &mut scratch)
            .to_style();
        let block_bg = computed.to_style().bg.unwrap_or(ratatui::style::Color::Reset);

        let mut block = computed
            .to_block()
            .border_type(self.shape.border_type())
            .style(Style::default().bg(block_bg));
        if let Some(title) = &self.title {
            block = block
                .title(Line::from(title.as_str()).alignment(Alignment::Left).style(title_style));
        }
        block.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    const W: u16 = 16;
    const H: u16 = 5;

    fn render(panel: Panel, w: u16, h: u16) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        panel.render(Rect::new(0, 0, w, h), &mut buf);
        buf
    }

    #[test]
    fn renders_double_line_border_corners() {
        let buf = render(Panel::new().theme(Theme::Cyberpunk), W, H);
        // The `Frame` cascade declares `border: double`, so the corners are the
        // double-line glyphs. (Border-type strings are width-1.)
        assert_eq!(buf[(0, 0)].symbol(), "╔", "top-left must be a double corner");
        assert_eq!(buf[(W - 1, 0)].symbol(), "╗", "top-right must be a double corner");
        assert_eq!(buf[(W - 1, H - 1)].symbol(), "╝", "bottom-right must be a double corner");
    }

    #[test]
    fn rounded_shape_uses_rounded_corners() {
        use super::PanelShape;
        let buf = render(
            Panel::new().theme(Theme::Cyberpunk).shape(PanelShape::Rounded),
            W,
            H,
        );
        // The Rounded shape overrides the border type: the top-left corner must
        // be the rounded glyph `╭`, not the default double-line corner `╔`.
        assert_eq!(
            buf[(0, 0)].symbol(),
            "╭",
            "rounded shape must use the rounded top-left corner"
        );
        assert_ne!(
            buf[(0, 0)].symbol(),
            "╔",
            "rounded shape must not show the double-line corner"
        );
    }

    #[test]
    fn inner_is_strictly_smaller_and_contained() {
        let area = Rect::new(0, 0, W, H);
        let panel = Panel::new().theme(Theme::Cyberpunk);
        let inner = panel.inner(area);

        assert!(inner.width < W, "inner must be narrower than the frame");
        assert!(inner.height < H, "inner must be shorter than the frame");
        // Contained: every inner edge inside the frame.
        assert!(inner.x >= area.x && inner.right() <= area.right());
        assert!(inner.y >= area.y && inner.bottom() <= area.bottom());
    }

    #[test]
    fn inner_on_tiny_area_is_zero_size_without_panic() {
        let panel = Panel::new().theme(Theme::Cyberpunk);
        // A 2x2 frame is too small for any interior content (border eats it all).
        let inner = panel.inner(Rect::new(0, 0, 2, 2));
        // Must not panic; width/height clamp to zero rather than underflow.
        assert_eq!(inner.width, 0);
        assert_eq!(inner.height, 0);
    }

    #[test]
    fn title_renders_on_top_border() {
        let buf = render(Panel::new().title("TELEM").theme(Theme::Cyberpunk), W, H);
        // The title text should appear on the top border row (y=0), starting at
        // the first interior cell (x=1) just inside the left corner.
        assert_eq!(buf[(1, 0)].symbol(), "T", "title should sit on the top border");
        assert_eq!(buf[(2, 0)].symbol(), "E");
    }

    #[test]
    fn title_uses_accent_color() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let buf = render(Panel::new().title("T").theme(Theme::Cyberpunk), W, H);
        // The title glyph carries the `Frame.title` accent foreground.
        let title_x = (1..W).find(|&x| buf[(x, 0)].symbol() == "T").expect("title char present");
        assert_eq!(buf[(title_x, 0)].fg, accent, "title should be accent-colored");
    }

    #[test]
    fn border_color_is_muted() {
        let muted = Theme::Fallout.palette().muted.color();
        let panel = Panel::new().theme(Theme::Fallout);
        assert_eq!(panel.border_fg(), muted, "frame border should resolve to --muted");
    }

    #[test]
    fn empty_area_is_a_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        Panel::new().title("X").render(Rect::new(0, 0, 0, 0), &mut buf);
        // Didn't panic — that's the contract.
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
