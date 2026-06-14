//! **TargetLock** — crosshair + corner-brackets HUD frame (PRD §3 目标锁定容器).
//!
//! A target-lock container: a HUD frame that draws four corner brackets and a
//! center crosshair, framing the most important data on screen.
//!
//! ## Spec
//! - Four **corner brackets** at the rectangle corners in a broken / gap-toothed
//!   style: short horizontal arms run inward from each corner along the top and
//!   bottom edges, leaving the middle of each edge open (the classic targeting
//!   reticle silhouette).
//! - A **center crosshair** glyph at the geometric center of the area, drawn in
//!   a dim/muted color so content rendered on top remains readable.
//! - An optional **title** rendered near the top edge.
//! - [`TargetLock::inner`] mirrors [`ratatui::widgets::Block::inner`]: it
//!   shrinks `area` by the bracket thickness so callers can render content
//!   inside without overwriting the frame.
//!
//! ## Glyphs (all width-1)
//! - Corners: `┏` (top-left), `┓` (top-right), `┗` (bottom-left), `┛` (bottom-right)
//! - Corner arms: `━` (heavy horizontal)
//! - Crosshair: `✛`
//!
//! ## Implementation notes
//! - Stateless [`Widget`]: `render(self, area, buf)` by value. The theme and
//!   title are per-frame configuration, not animation state.
//! - Styled with [`Theme::palette`] → bare [`ratatui::style::Color`] /
//!   [`ratatui::style::Style`] directly (brackets use `palette.accent`,
//!   crosshair uses `palette.muted`).
//! - Safe for tiny areas: when `width` or `height` is less than 3 the frame is
//!   skipped (only the crosshair is drawn when it fits), and `inner(area)`
//!   never produces an out-of-bounds rect.
//!
//! [`Theme::palette`]: crate::Theme::palette

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::Widget,
};

use crate::Theme;

/// The bracket / frame thickness in cells. Mirrors a single-cell `Block` border.
const THICKNESS: u16 = 1;

/// Top-left corner glyph.
const CORNER_TL: &str = "┏";
/// Top-right corner glyph.
const CORNER_TR: &str = "┓";
/// Bottom-left corner glyph.
const CORNER_BL: &str = "┗";
/// Bottom-right corner glyph.
const CORNER_BR: &str = "┛";
/// Horizontal arm glyph used for the broken-bracket look.
const ARM_H: &str = "━";
/// Center crosshair glyph.
const CROSSHAIR: &str = "✛";

/// A target-lock HUD frame.
///
/// Draws four corner brackets (with short inward arms) and a center crosshair,
/// framing content. Use [`TargetLock::inner`] to get the content area, then
/// render your widget there.
///
/// ```ignore
/// use ratatui::widgets::Widget;
/// use ratatui_sci_fi::{TargetLock, Theme};
///
/// let lock = TargetLock::new().title("TARGET").theme(Theme::DeepSpace);
/// let inner = lock.inner(area);
/// lock.render(area, buf);
/// // render content inside `inner` ...
/// ```
#[derive(Debug, Default, Clone)]
pub struct TargetLock {
    /// Optional title shown near the top edge of the frame.
    pub title: Option<String>,
    /// Active theme (default [`Theme::Cyberpunk`]).
    pub theme: Theme,
}

impl TargetLock {
    /// Create a new `TargetLock` with the default theme and no title.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the title rendered near the top edge of the frame.
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the active [`Theme`].
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// The content area inside the frame.
    ///
    /// Mirrors [`ratatui::widgets::Block::inner`]: shrinks `area` by the bracket
    /// thickness ([`THICKNESS`]) on every side. For areas too small to fit a
    /// border (width or height `< 2 * THICKNESS + 1`), the returned rect is
    /// clamped to a valid (possibly zero-size) rect inside `area` — it never
    /// overflows or goes out of bounds.
    pub fn inner(&self, area: Rect) -> Rect {
        // Two thicknesses (left+right / top+bottom) must leave at least one cell.
        let horizontal = THICKNESS.saturating_mul(2);
        let vertical = THICKNESS.saturating_mul(2);
        let width = area.width.saturating_sub(horizontal);
        let height = area.height.saturating_sub(vertical);
        Rect::new(
            area.x.saturating_add(THICKNESS).min(area.right().saturating_sub(1).max(area.x)),
            area.y.saturating_add(THICKNESS).min(area.bottom().saturating_sub(1).max(area.y)),
            width,
            height,
        )
    }

    /// Paint a single cell at `(x, y)` with `symbol` + `style`, doing nothing if
    /// the cell falls outside `area`.
    fn paint(buf: &mut Buffer, area: Rect, x: u16, y: u16, symbol: &str, style: Style) {
        if area.contains((x, y).into()) {
            buf[(x, y)].set_symbol(symbol).set_style(style);
        }
    }
}

impl Widget for TargetLock {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let palette = self.theme.palette();
        let bracket_style = Style::new().fg(palette.accent.color());
        let crosshair_style = Style::new().fg(palette.muted.color());

        // Degenerate area: nothing meaningful to draw.
        if area.width == 0 || area.height == 0 {
            return;
        }

        // Single-cell area: just the crosshair if it lands on the only cell.
        if area.width == 1 && area.height == 1 {
            Self::paint(buf, area, area.x, area.y, CROSSHAIR, crosshair_style);
            return;
        }

        // Corners. (Guaranteed in-bounds: width/height >= 2 → corners distinct.)
        let left = area.left();
        let right = area.right().saturating_sub(1);
        let top = area.top();
        let bottom = area.bottom().saturating_sub(1);

        Self::paint(buf, area, left, top, CORNER_TL, bracket_style);
        Self::paint(buf, area, right, top, CORNER_TR, bracket_style);
        Self::paint(buf, area, left, bottom, CORNER_BL, bracket_style);
        Self::paint(buf, area, right, bottom, CORNER_BR, bracket_style);

        // Broken-bracket arms: a short run inward from each corner along the
        // top and bottom edges (1–2 cells). Only when there is room between the
        // corners.
        let inner_span = right.saturating_sub(left).saturating_sub(1);
        if inner_span > 0 {
            let arm = u16::min(2, inner_span);
            for dx in 1..=arm {
                Self::paint(buf, area, left.saturating_add(dx), top, ARM_H, bracket_style);
                Self::paint(buf, area, right.saturating_sub(dx), top, ARM_H, bracket_style);
                Self::paint(buf, area, left.saturating_add(dx), bottom, ARM_H, bracket_style);
                Self::paint(buf, area, right.saturating_sub(dx), bottom, ARM_H, bracket_style);
            }
        }

        // Center crosshair (muted, so overlaid content stays readable).
        let cx = area.x + area.width / 2;
        let cy = area.y + area.height / 2;
        Self::paint(buf, area, cx, cy, CROSSHAIR, crosshair_style);

        // Optional title near the top edge, centered over the inner span.
        if let Some(title) = self.title.as_deref()
            && !title.is_empty()
        {
            let cy_top = top;
            let title_w = title.chars().count() as u16;
            // Center between the arms, clamped inside [left+1, right].
            let avail = right.saturating_sub(left).saturating_sub(1);
            if title_w <= avail {
                let start = left.saturating_add(1) + (avail.saturating_sub(title_w)) / 2;
                buf.set_string(start, cy_top, title, bracket_style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    fn render(area: Rect) -> (TargetLock, Buffer) {
        let widget = TargetLock::new().title("TGT");
        let mut buf = Buffer::empty(area);
        widget.clone().render(area, &mut buf);
        (widget, buf)
    }

    #[test]
    fn corners_are_drawn() {
        // 10 wide x 6 tall, origin (0,0).
        let area = Rect::new(0, 0, 10, 6);
        let (_, buf) = render(area);
        assert_eq!(buf[(0, 0)].symbol(), CORNER_TL, "top-left corner");
        assert_eq!(buf[(9, 0)].symbol(), CORNER_TR, "top-right corner");
        assert_eq!(buf[(0, 5)].symbol(), CORNER_BL, "bottom-left corner");
        assert_eq!(buf[(9, 5)].symbol(), CORNER_BR, "bottom-right corner");
    }

    #[test]
    fn center_crosshair_is_drawn() {
        let area = Rect::new(0, 0, 10, 6);
        let (_, buf) = render(area);
        // width/2 = 5, height/2 = 3 → (5,3)
        assert_eq!(buf[(5, 3)].symbol(), CROSSHAIR, "center crosshair");
    }

    #[test]
    fn arms_leave_middle_open() {
        // 10 wide: left arm occupies cols 1-2, right arm cols 7-8; middle (col 4-5)
        // stays open (the gap-toothed look). With a title of width 3, col 4-5 may
        // hold title chars, so pick a wider area to isolate the arm assertion.
        let area = Rect::new(0, 0, 20, 6);
        let widget = TargetLock::new(); // no title
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        // Top edge: arm cells are cols 1 and 2.
        assert_eq!(buf[(1, 0)].symbol(), ARM_H);
        assert_eq!(buf[(2, 0)].symbol(), ARM_H);
        // Middle of the top edge is open (space / default).
        assert_eq!(buf[(9, 0)].symbol(), " ");
        // Bottom edge mirrors.
        assert_eq!(buf[(1, 5)].symbol(), ARM_H);
        assert_eq!(buf[(17, 5)].symbol(), ARM_H);
    }

    #[test]
    fn title_is_rendered_near_top() {
        let area = Rect::new(0, 0, 12, 4);
        let widget = TargetLock::new().title("TGT");
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        // Title should appear somewhere on row 0 between the arms.
        let row: String = (0..area.width)
            .map(|x| buf[(x, 0)].symbol().to_string())
            .collect();
        assert!(row.contains('T'), "title char on top row: {row:?}");
        assert!(row.contains('G'), "title char on top row: {row:?}");
    }

    fn within(outer: Rect, inner: Rect) -> bool {
        inner.x >= outer.x
            && inner.y >= outer.y
            && inner.right() <= outer.right()
            && inner.bottom() <= outer.bottom()
    }

    #[test]
    fn inner_is_smaller_than_area() {
        let lock = TargetLock::new();
        let area = Rect::new(0, 0, 10, 6);
        let inner = lock.inner(area);
        assert!(inner.width < area.width, "inner width {} < {}", inner.width, area.width);
        assert!(inner.height < area.height, "inner height {} < {}", inner.height, area.height);
        // inner must be fully contained in area.
        assert!(within(area, inner), "inner {:?} must be within area {:?}", inner, area);
    }

    #[test]
    fn inner_does_not_overflow_on_tiny_area() {
        let lock = TargetLock::new();
        let area = Rect::new(5, 5, 1, 1);
        let inner = lock.inner(area);
        // width/height saturate to 0, position stays in-bounds.
        assert_eq!(inner.width, 0);
        assert_eq!(inner.height, 0);
        assert!(within(area, inner), "inner {:?} within {:?}", inner, area);
    }

    #[test]
    fn tiny_area_renders_without_panic() {
        // 2x2: corners only, no center (center == a corner cell), no panic.
        let area = Rect::new(0, 0, 2, 2);
        let _ = render(area);
        // 1x1: crosshair on the only cell.
        let area = Rect::new(3, 3, 1, 1);
        let (_, buf) = render(area);
        assert_eq!(buf[(3, 3)].symbol(), CROSSHAIR);
    }

    #[test]
    fn default_theme_is_cyberpunk() {
        let lock = TargetLock::new();
        assert_eq!(lock.theme, Theme::Cyberpunk);
    }

    #[test]
    fn theme_builder_overrides_default() {
        let lock = TargetLock::new().theme(Theme::Weyland);
        assert_eq!(lock.theme, Theme::Weyland);
    }
}
