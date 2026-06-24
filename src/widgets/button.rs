//! **Button** — sci-fi action button (PRD §3 基础组件).
//!
//! A single-row sci-fi button that reacts to focus.
//!
//! ## Spec
//! - **Unfocused**: light bracketed frame, e.g. `[ 确认 ]`. The label is
//!   rendered in the theme's muted/accent foreground over the app
//!   background, giving a quiet "idle" look.
//! - **Focused**: the bracket glyphs switch to dynamic energy arrows
//!   `▶ 确认 ◀`, the whole cell gets the theme's accent background, and the
//!   label is drawn bright (inverted via `Modifier::REVERSED` for emphasis).
//!
//! ## Implementation notes
//! - Stateless [`Widget`]: `focused` is per-frame configuration set by the
//!   app's event loop, not animation state.
//! - Styling goes through the theme's [`Stylesheet`](ratatui_style::Stylesheet)
//!   cascade: the button queries the `Button` node (plus a `.focus` class when
//!   focused) through the cascade, so colors come from the `Button` / `Button.focus`
//!   CSS rules rather than `palette()` directly. Because those rules are
//!   `var(--…)`-driven off the same palette, the resolved colors are identical
//!   to reading `palette()` — and `Button.focus` additionally applies
//!   `font-weight: bold`.
//! - The label is horizontally centered in `area`; rendering targets the
//!   vertical middle row. All glyphs are width-1.

use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};
use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Visual form of a [`Button`].
///
/// Two kinds, both purely glyph/layout — colors stay on the CSS cascade
/// (`Button` / `Button.focus`), untouched by this enum:
/// - **Inline** shapes flank the label with marker glyphs on a single row
///   (`[ label ]`, `« label »`, …).
/// - **Boxed** shapes ([`Pill`](ButtonShape::Pill)/[`Framed`](ButtonShape::Framed))
///   draw a multi-row border around the label when the area is at least 3 rows
///   tall and 2 columns wide; otherwise they degrade to the inline Bracket look.
///
/// The [`ButtonShape::Bracket`] default reproduces the original
/// `[ label ]` / `▶ label ◀` look byte-for-byte, so existing tests pass
/// unchanged.
///
/// Every glyph is Unicode width-1 (see convention #5 at the crate root),
/// keeping the button's `chars().count() == display_width` centering math
/// valid.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ButtonShape {
    /// `[ label ]` idle, `▶ label ◀` focused — the original look.
    #[default]
    Bracket,
    /// `< label >` idle, `► label ◄` focused.
    Angle,
    /// `« label »` idle, `▶ label ◀` focused.
    Chevron,
    /// `| label |` idle, `▐ label ▌` focused.
    Pipe,
    /// `> label <` idle, `▸ label ◂` focused.
    Arrow,
    /// Rounded-capsule border (`╭─╮` / `│ label │` / `╰─╯`); needs ≥3 rows.
    Pill,
    /// Square-frame border (`┌─┐` / `│ label │` / `└─┘`); needs ≥3 rows.
    Framed,
}

/// Border glyphs for a boxed [`ButtonShape`] (`Pill` / `Framed`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoxGlyphs {
    /// Top-left, top-right, bottom-left, bottom-right corners.
    pub tl: char,
    pub tr: char,
    pub bl: char,
    pub br: char,
    /// Left/right vertical edge of the label row.
    pub side: char,
    /// Top/bottom horizontal fill between the corners.
    pub horizontal: char,
}

impl ButtonShape {
    /// Whether this shape draws a multi-row border box (vs. inline markers).
    #[must_use]
    pub const fn is_boxed(self) -> bool {
        matches!(self, Self::Pill | Self::Framed)
    }

    /// The border glyph set for a boxed shape, or `None` for inline shapes.
    #[must_use]
    pub const fn box_glyphs(self) -> Option<BoxGlyphs> {
        match self {
            Self::Pill => Some(BoxGlyphs {
                tl: '╭',
                tr: '╮',
                bl: '╰',
                br: '╯',
                side: '│',
                horizontal: '─',
            }),
            Self::Framed => Some(BoxGlyphs {
                tl: '┌',
                tr: '┐',
                bl: '└',
                br: '┘',
                side: '│',
                horizontal: '─',
            }),
            _ => None,
        }
    }

    /// The `(left, right)` marker pair for an inline render. Boxed shapes
    /// return the Bracket pair — used only when a box doesn't fit and the
    /// render degrades to the inline look.
    #[must_use]
    pub const fn markers(self, focused: bool) -> (char, char) {
        match (self, focused) {
            (Self::Bracket, false) => ('[', ']'),
            (Self::Bracket, true) => ('▶', '◀'),
            (Self::Angle, false) => ('<', '>'),
            (Self::Angle, true) => ('►', '◄'),
            (Self::Chevron, false) => ('«', '»'),
            (Self::Chevron, true) => ('▶', '◀'),
            (Self::Pipe, false) => ('|', '|'),
            (Self::Pipe, true) => ('▐', '▌'),
            (Self::Arrow, false) => ('>', '<'),
            (Self::Arrow, true) => ('▸', '◂'),
            // Boxed shapes have no inline markers; their cramped-area fallback
            // reuses the Bracket pair.
            (Self::Pill | Self::Framed, false) => ('[', ']'),
            (Self::Pill | Self::Framed, true) => ('▶', '◀'),
        }
    }
}

/// A sci-fi action button.
///
/// Build it with [`Button::new`] and toggle the focused look with
/// [`Button::focused`]. The [`Widget`] implementation renders either the
/// idle `[ label ]` style or the focused `▶ label ◀` style depending on the
/// `focused` field.
#[derive(Debug, Clone)]
pub struct Button {
    /// Visible label text.
    pub label: String,
    /// Whether this button currently has focus (drives the focused style).
    pub focused: bool,
    /// Marker-glyph form (idle/focused end glyphs). Defaults to
    /// [`ButtonShape::Bracket`], the original bracket/arrow look.
    pub shape: ButtonShape,
    /// Theme whose [`Stylesheet`](ratatui_style::Stylesheet) drives the button's
    /// colors. Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Default for Button {
    fn default() -> Self {
        Self {
            label: String::new(),
            focused: false,
            shape: ButtonShape::default(),
            theme: Theme::Cyberpunk,
        }
    }
}

impl Button {
    /// Create a new button with the given label, unfocused, default theme.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            focused: false,
            shape: ButtonShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set whether the button renders in its focused style.
    #[must_use]
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Set the marker-glyph form (see [`ButtonShape`]).
    #[must_use]
    pub fn shape(mut self, shape: ButtonShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the button.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

impl Widget for Button {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Nothing to draw outside a non-empty area.
        if area.is_empty() {
            return;
        }

        // Style comes from the theme's stylesheet cascade. The `Button` node
        // resolves to { accent fg, bg background }; `Button.focus` overrides
        // with { bg fg, accent background, bold }. Both are `var(--…)`-driven
        // off the same palette, so the resolved colors match reading
        // `palette()` directly.
        let sheet = self.theme.stylesheet();
        // One reused `ComputeScratch` for both branches (crate convention #2):
        // `compute()` would allocate a fresh scratch per call, but `compute_with`
        // borrows ours — and render runs every frame.
        let mut scratch = ComputeScratch::new();
        let computed = if self.focused {
            sheet
                .compute_with(&NodeRef::new("Button").classes(&["focus"]), None, &mut scratch)
                .to_style()
        } else {
            sheet.compute_with(&NodeRef::new("Button"), None, &mut scratch).to_style()
        };

        // Whole-button background + base style from the cascade. Focused text
        // is REVERSED so the bright accent label lands inverted on the accent
        // fill — the classic "highlighted" console look.
        let area_style = computed;
        let text_style = if self.focused {
            computed.reversed()
        } else {
            computed
        };

        // Paint the button's full area background first, so empty cells pick up
        // the highlight when focused.
        buf.set_style(area, area_style);

        // Boxed shapes (Pill / Framed) draw a multi-row border when the area
        // has room (≥3 rows, ≥2 cols). Inline shapes — and boxed shapes that
        // don't fit — fall back to the single-row marker render.
        if self.shape.is_boxed() && area.height >= 3 && area.width >= 2 {
            // `is_boxed()` true ⟹ `box_glyphs()` is `Some`.
            let glyphs = self.shape.box_glyphs().expect("boxed shape has glyphs");
            self.render_boxed(area, buf, text_style, glyphs);
        } else {
            self.render_inline(area, buf, text_style);
        }
    }
}

impl Button {
    /// Single-row render: `{left} {label} {right}` centered on the middle row.
    /// Used by every inline shape, and as the fallback when a boxed shape's
    /// area is too short or narrow for its border.
    fn render_inline(self, area: Rect, buf: &mut Buffer, text_style: Style) {
        let (left, right) = self.shape.markers(self.focused);
        // `glyph label glyph` with a space either side so the label breathes
        // inside the markers. All glyphs are width-1.
        let content = format!("{left} {label} {right}", label = self.label);

        let row = area.y + area.height / 2;
        let content_width = content.chars().count() as u16;
        let content_width = content_width.min(area.width);
        let x = area.x + area.width.saturating_sub(content_width) / 2;
        buf.set_string(x, row, &content, text_style);
    }

    /// Multi-row render: a border box with the label centered on the middle
    /// row. Caller guarantees `area.height >= 3 && area.width >= 2`.
    fn render_boxed(self, area: Rect, buf: &mut Buffer, text_style: Style, glyphs: BoxGlyphs) {
        let row_top = area.y;
        let row_bot = area.y + area.height - 1;
        let row_mid = area.y + area.height / 2;

        // Top + bottom edges: `corner (horizontal fill) corner`.
        let fill: String =
            std::iter::repeat_n(glyphs.horizontal, area.width.saturating_sub(2) as usize)
                .collect();
        let top = format!("{}{}{}", glyphs.tl, fill, glyphs.tr);
        let bot = format!("{}{}{}", glyphs.bl, fill, glyphs.br);
        buf.set_string(area.x, row_top, &top, text_style);
        buf.set_string(area.x, row_bot, &bot, text_style);

        // Interior rows: vertical sides span every row between the corners; the
        // middle row additionally carries the centered (possibly clipped) label.
        let inner_w = area.width.saturating_sub(2);
        let label: String = self.label.chars().take(inner_w as usize).collect();
        let label_w = label.chars().count() as u16;
        let pad = inner_w.saturating_sub(label_w);
        let left_pad = " ".repeat((pad / 2) as usize);
        let right_pad = " ".repeat((pad - pad / 2) as usize);
        let mid_row = format!("{}{left_pad}{label}{right_pad}{}", glyphs.side, glyphs.side);
        let blank = " ".repeat(inner_w as usize);
        let side_row = format!("{}{blank}{}", glyphs.side, glyphs.side);
        for r in (row_top + 1)..row_bot {
            if r == row_mid {
                buf.set_string(area.x, r, &mid_row, text_style);
            } else {
                buf.set_string(area.x, r, &side_row, text_style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{buffer::Buffer, layout::Rect, style::Color, widgets::Widget};

    use super::Button;
    use crate::Theme;

    /// Width/height large enough to hold `[ 确认 ]` / `▶ 确认 ◀` centered.
    const W: u16 = 16;
    const H: u16 = 3;

    fn render(button: Button) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        button.render(Rect::new(0, 0, W, H), &mut buf);
        buf
    }

    /// Collect the symbols from the middle row into a single string.
    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..W)
            .map(|x| buf[(x, y)].symbol().to_string())
            .collect::<String>()
    }

    #[test]
    fn unfocused_uses_square_brackets() {
        let buf = render(Button::new("确认"));
        let middle = H / 2;
        let text = row_text(&buf, middle);

        assert!(
            text.contains('['),
            "unfocused button should contain '[': {text:?}"
        );
        assert!(
            text.contains(']'),
            "unfocused button should contain ']': {text:?}"
        );
        assert!(
            !text.contains('▶'),
            "unfocused button must not show the focused arrow: {text:?}"
        );
        // Wide (CJK) glyphs occupy 2 cells; `row_text` joins per-cell, so the
        // continuation cell splits a glyph pair. Assert per-char instead.
        assert!(
            text.contains('确') && text.contains('认'),
            "label should render: {text:?}"
        );
    }

    #[test]
    fn focused_uses_energy_arrows() {
        let buf = render(Button::new("确认").focused(true));
        let middle = H / 2;
        let text = row_text(&buf, middle);

        assert!(
            text.contains('▶') && text.contains('◀'),
            "focused button should contain '▶' and '◀': {text:?}"
        );
        assert!(
            !text.contains('['),
            "focused button must not show the idle bracket: {text:?}"
        );
        // Wide (CJK) glyphs occupy 2 cells; `row_text` joins per-cell, so the
        // continuation cell splits a glyph pair. Assert per-char instead.
        assert!(
            text.contains('确') && text.contains('认'),
            "label should render: {text:?}"
        );
    }

    #[test]
    fn focused_paints_accent_background_across_area() {
        let theme = Theme::Cyberpunk;
        let accent = theme.palette().accent.color();
        let buf = render(Button::new("OK").theme(theme).focused(true));
        let middle = H / 2;

        // The whole middle row should carry the accent background, including
        // cells outside the rendered content (the highlight fill). `Cell`
        // exposes its colors as public `Color` fields.
        for x in 0..W {
            let cell_bg = buf[(x, middle)].bg;
            assert_eq!(
                cell_bg, accent,
                "cell ({x}, {middle}) should have accent bg, got {cell_bg:?}"
            );
        }
    }

    #[test]
    fn focused_applies_cascade_bold_and_reversed() {
        use ratatui::style::Modifier;
        let buf = render(Button::new("OK").focused(true));
        let middle = H / 2;

        // The whole focused area carries BOLD — `area_style` comes straight
        // from the `Button.focus { font-weight: bold }` cascade rule.
        assert!(
            buf[(0, middle)].modifier.contains(Modifier::BOLD),
            "focused area should be bold via the cascade, got {:?}",
            buf[(0, middle)].modifier
        );

        // The label glyph additionally carries REVERSED (inverted-label
        // emphasis). Find the 'O' cell on the middle row and check it.
        let label_x = (0..W)
            .find(|&x| buf[(x, middle)].symbol() == "O")
            .expect("'O' should be rendered on the focused middle row");
        assert!(
            buf[(label_x, middle)].modifier.contains(Modifier::REVERSED),
            "focused label should be reversed, got {:?}",
            buf[(label_x, middle)].modifier
        );

        // An unfocused button is neither bold nor reversed.
        let idle = render(Button::new("OK"));
        assert!(
            !idle[(0, middle)].modifier.contains(Modifier::BOLD),
            "unfocused button must not be bold"
        );
    }

    #[test]
    fn unfocused_keeps_app_background() {
        let theme = Theme::Fallout;
        let bg = theme.palette().bg.color();
        let buf = render(Button::new("OK").theme(theme));
        let middle = H / 2;

        let cell_bg = buf[(0, middle)].bg;
        assert_eq!(cell_bg, bg, "idle background should be the theme bg");
    }

    #[test]
    fn empty_area_is_a_noop() {
        // A zero-size area must not panic.
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        Button::new("X")
            .focused(true)
            .render(Rect::new(0, 0, 0, 0), &mut buf);
        // Just ensure the buffer is still empty / untouched.
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }

    #[test]
    fn ascii_label_renders_centered() {
        let buf = render(Button::new("OK").focused(true));
        let middle = H / 2;
        let text = row_text(&buf, middle);

        // `▶ OK ◀` should be present and not flush against the left edge.
        assert!(
            text.contains("▶ OK ◀"),
            "centered focused content missing: {text:?}"
        );
        assert!(
            text.starts_with(' '),
            "content should be centered (leading spaces): {text:?}"
        );
    }

    // Touch `Color` so the import stays used even if assertions evolve.
    #[test]
    fn palette_exposes_rgb_color() {
        let _c: Color = Theme::Cyberpunk.palette().accent.color();
    }

    #[test]
    fn non_default_shape_changes_idle_glyphs() {
        use super::ButtonShape;

        // Default Bracket idle renders '[' … ']'.
        let idle_default = render(Button::new("OK"));
        let text = row_text(&idle_default, H / 2);
        assert!(text.contains('[') && text.contains(']'));

        // The Angle shape swaps the idle markers to '<' / '>'.
        let idle_angle = render(Button::new("OK").shape(ButtonShape::Angle));
        let text_angle = row_text(&idle_angle, H / 2);
        assert!(
            text_angle.contains('<') && text_angle.contains('>'),
            "Angle idle should use '<'/'>': {text_angle:?}"
        );
        assert!(
            !text_angle.contains('['),
            "Angle idle must not use the Bracket '[': {text_angle:?}"
        );
    }

    #[test]
    fn pill_shape_renders_rounded_border() {
        use super::ButtonShape;
        // H=3 → the box fits: rounded corners top-left / bottom-right, label
        // centered on the middle row.
        let buf = render(Button::new("OK").focused(true).shape(ButtonShape::Pill));
        assert_eq!(buf[(0, 0)].symbol(), "╭", "Pill top-left corner");
        assert_eq!(buf[(15, 2)].symbol(), "╯", "Pill bottom-right corner");
        let mid = row_text(&buf, H / 2);
        assert!(mid.contains('O') && mid.contains('K'), "label on middle row: {mid:?}");
    }

    #[test]
    fn framed_shape_renders_square_border() {
        use super::ButtonShape;
        let buf = render(Button::new("OK").focused(true).shape(ButtonShape::Framed));
        assert_eq!(buf[(0, 0)].symbol(), "┌", "Framed top-left corner");
        assert_eq!(buf[(15, 2)].symbol(), "┘", "Framed bottom-right corner");
    }

    #[test]
    fn pill_shape_degrades_when_too_short() {
        use super::ButtonShape;
        // A 1-row-tall area can't hold the 3-row box, so the Pill degrades to
        // the inline Bracket look — no rounded corner appears, but the label
        // still renders.
        let mut buf = Buffer::empty(Rect::new(0, 0, W, 1));
        Button::new("OK")
            .focused(false)
            .shape(ButtonShape::Pill)
            .render(Rect::new(0, 0, W, 1), &mut buf);
        let row = row_text(&buf, 0);
        assert!(!row.contains('╭'), "degraded render must not draw the box: {row:?}");
        assert!(row.contains('O') && row.contains('K'), "label still renders: {row:?}");
    }

    #[test]
    fn pill_focused_paints_accent_background() {
        use super::ButtonShape;
        let theme = Theme::Cyberpunk;
        let accent = theme.palette().accent.color();
        let buf = render(
            Button::new("OK")
                .theme(theme)
                .focused(true)
                .shape(ButtonShape::Pill),
        );
        // Every cell of the W×H box area carries the accent background.
        for y in 0..H {
            for x in 0..W {
                assert_eq!(buf[(x, y)].bg, accent, "cell ({x},{y}) bg");
            }
        }
    }

    #[test]
    fn pill_tall_box_has_sides_on_every_interior_row() {
        use super::ButtonShape;
        // A 16×4 Pill: vertical sides must appear on BOTH interior rows (not
        // just the label row) so the box reads as a solid rectangle.
        let mut buf = Buffer::empty(Rect::new(0, 0, 16, 4));
        Button::new("OK")
            .focused(false)
            .shape(ButtonShape::Pill)
            .render(Rect::new(0, 0, 16, 4), &mut buf);
        // Rows 1 and 2 are interior (row 0 = top, row 3 = bottom); each needs
        // the `│` side glyph at both edges.
        assert_eq!(buf[(0, 1)].symbol(), "│", "interior row 1 left side");
        assert_eq!(buf[(15, 1)].symbol(), "│", "interior row 1 right side");
        assert_eq!(buf[(0, 2)].symbol(), "│", "interior row 2 left side");
        assert_eq!(buf[(15, 2)].symbol(), "│", "interior row 2 right side");
        assert_eq!(buf[(0, 0)].symbol(), "╭", "top-left corner");
        assert_eq!(buf[(15, 3)].symbol(), "╯", "bottom-right corner");
    }
}
