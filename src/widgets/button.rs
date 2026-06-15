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
//!   focused) via `compute`, so colors come from the `Button` / `Button.focus`
//!   CSS rules rather than `palette()` directly. Because those rules are
//!   `var(--…)`-driven off the same palette, the resolved colors are identical
//!   to reading `palette()` — and `Button.focus` additionally applies
//!   `font-weight: bold`.
//! - The label is horizontally centered in `area`; rendering targets the
//!   vertical middle row. All glyphs are width-1.

use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};
use ratatui_style::NodeRef;

use crate::Theme;

/// Visual form of a [`Button`]'s flanking markers.
///
/// Selects the end glyphs that frame the label; colors stay on the CSS
/// cascade (`Button` / `Button.focus`), untouched by this enum. The
/// [`ButtonShape::Bracket`] default reproduces the original `[ label ]` /
/// `▶ label ◀` look byte-for-byte, so existing tests pass unchanged.
///
/// Every marker glyph is Unicode width-1 (see convention #5 at the crate
/// root), keeping the button's `chars().count() == display_width` centering
/// math valid.
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
}

impl ButtonShape {
    /// The `(left, right)` marker pair for the given focus state.
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
        let computed = if self.focused {
            sheet
                .compute(&NodeRef::new("Button").classes(&["focus"]), None)
                .to_style()
        } else {
            sheet.compute(&NodeRef::new("Button"), None).to_style()
        };

        // Pick the marker glyphs from the configured shape + focus state.
        let (left, right) = self.shape.markers(self.focused);

        // Compose the inner content: `glyph label glyph`.
        // Use a single leading/trailing space so the label breathes inside the
        // brackets, matching the spec examples (`[ 确认 ]` / `▶ 确认 ◀`).
        let content = format!("{left} {label} {right}", label = self.label);

        // We render on the vertical middle row of the area.
        let row = area.y + area.height / 2;

        // Horizontally center `content` within `area`. Every glyph we emit is
        // ASCII or a width-1 char, so char count == display width here.
        let content_width = content.chars().count() as u16;
        let available = area.width;
        let content_width = content_width.min(available);
        let x = area.x + available.saturating_sub(content_width) / 2;

        // Whole-button background + base style from the cascade. Focused text is
        // REVERSED so the bright accent label lands inverted on the accent fill
        // — the classic "highlighted" console look.
        let area_style = computed;
        let text_style = if self.focused {
            computed.reversed()
        } else {
            computed
        };

        // Paint the button's full area background first, so empty cells pick up
        // the highlight when focused.
        buf.set_style(area, area_style);

        // Draw the centered content with the text style.
        buf.set_string(x, row, content, text_style);
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
}
