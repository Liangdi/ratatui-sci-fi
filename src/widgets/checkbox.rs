//! **Checkbox** — sci-fi boolean check box.
//!
//! The stateless boolean sibling of [`Toggle`]: where [`Toggle`] renders an
//! energized orb with an ENGAGED/STANDBY suffix, [`Checkbox`] renders a plain
//! `[✓]` / `[ ]` mark — the quieter, form-control look for "toggle a setting"
//! rather than "arm a system". Like [`Toggle`], `checked` is per-frame
//! configuration set by the app (convention #3), so there is no
//! `CheckboxState`.
//!
//! ## Spec
//! - **Checked**: `[✓] LABEL` — the check glyph, ok color, bold.
//! - **Unchecked**: `[ ] LABEL` — a blank mark, muted.
//!
//! ## Implementation notes
//! - Stateless [`Widget`]: `checked` is per-frame configuration, like
//!   [`Toggle::on`] — not animation state, so there is no `CheckboxState`.
//! - Styling reuses the [`Toggle`] cascade node on purpose: a checkbox is the
//!   same ok/muted boolean vocabulary, so it shares `Toggle.on` (ok + bold) /
//!   `Toggle.off` (muted) rather than introducing a parallel CSS node. The
//!   colors are `var(--…)`-driven off the active palette.
//! - The label is horizontally centered in `area`; rendering targets the
//!   vertical middle row. All glyphs are width-1.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{Checkbox, Theme};
//!
//! let c = Checkbox::new("SHIELDS").checked(true).theme(Theme::DeepSpace);
//! ```
//!
//! [`Toggle`]: crate::Toggle

use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};
use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Glyph drawn inside the box when checked, for the [`CheckboxShape::Check`]
/// default.
pub const MARK_CHECK: char = '✓';
/// Glyph drawn inside the box when unchecked, for the [`CheckboxShape::Cross`]
/// variant.
pub const MARK_CROSS: char = '✕';
/// Glyph drawn inside the box when unchecked, for the [`CheckboxShape::Cross`]
/// variant.
pub const MARK_DOT: char = '·';

/// Visual form of a [`Checkbox`]'s check mark.
///
/// Selects the checked/unchecked glyph pair; colors stay on the CSS cascade
/// (reusing `Toggle.on` / `Toggle.off`), untouched by this enum. The brackets
/// and label are also unchanged across variants — only the mark glyph varies.
/// The [`CheckboxShape::Check`] default renders the original `✓` / blank look.
///
/// Every mark glyph is Unicode width-1 (see convention #5 at the crate root),
/// keeping the checkbox's `chars().count() == display_width` centering math
/// valid.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CheckboxShape {
    /// `✓` when checked, blank when unchecked — the original look.
    #[default]
    Check,
    /// `✕` when checked, `·` when unchecked.
    Cross,
    /// `■` when checked, `□` when unchecked.
    Block,
}

impl CheckboxShape {
    /// The mark glyph for the given checked/unchecked state.
    #[must_use]
    pub const fn mark(self, checked: bool) -> char {
        match (self, checked) {
            (Self::Check, true) => MARK_CHECK,
            (Self::Check, false) => ' ',
            (Self::Cross, true) => MARK_CROSS,
            (Self::Cross, false) => MARK_DOT,
            (Self::Block, true) => '■',
            (Self::Block, false) => '□',
        }
    }
}

/// A sci-fi check box.
///
/// Build with [`Checkbox::new`] (label), then set the checked look with
/// [`Checkbox::checked`] and the theme with [`Checkbox::theme`].
#[derive(Debug, Clone)]
pub struct Checkbox {
    /// Visible label text.
    pub label: String,
    /// Whether the checkbox is checked.
    pub checked: bool,
    /// Mark-glyph form (checked/unchecked indicator). Defaults to
    /// [`CheckboxShape::Check`], the original `✓` / blank look.
    pub shape: CheckboxShape,
    /// Theme whose [`Stylesheet`](ratatui_style::Stylesheet) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Default for Checkbox {
    fn default() -> Self {
        Self {
            label: String::new(),
            checked: false,
            shape: CheckboxShape::default(),
            theme: Theme::Cyberpunk,
        }
    }
}

impl Checkbox {
    /// Create a checkbox with the given label, unchecked, default theme.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            checked: false,
            shape: CheckboxShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set whether the checkbox renders in its checked style.
    #[must_use]
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Set the mark-glyph form (see [`CheckboxShape`]).
    #[must_use]
    pub fn shape(mut self, shape: CheckboxShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the checkbox.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Cascade class slice for the current state — `["on"]` or `["off"]`,
    /// matching [`crate::Toggle`]'s vocabulary (checked → on, unchecked → off).
    fn state_classes(&self) -> &'static [&'static str] {
        if self.checked {
            &["on"]
        } else {
            &["off"]
        }
    }
}

impl Widget for Checkbox {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        // Style reuses the `Toggle.on` / `Toggle.off` cascade rules — a checkbox
        // is the same ok/muted boolean vocabulary as a toggle.
        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let style = sheet
            .compute_with(&NodeRef::new("Toggle").classes(self.state_classes()), None, &mut scratch)
            .to_style();

        let mark = self.shape.mark(self.checked);

        // Compose `[mark] label`.
        let content = format!("[{mark}] {label}", label = self.label);

        let row = area.y + area.height / 2;

        // Center `content` in `area`. Every glyph is width-1, so char count ==
        // display width.
        let content_width = content.chars().count() as u16;
        let available = area.width;
        let content_width = content_width.min(available);
        let x = area.x + available.saturating_sub(content_width) / 2;

        // Paint the full area background first so empty cells pick up the style,
        // then draw the centered content.
        buf.set_style(area, style);
        buf.set_string(x, row, &content, style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, style::Modifier};

    const W: u16 = 24;
    const H: u16 = 3;

    fn render(checkbox: Checkbox) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        checkbox.render(Rect::new(0, 0, W, H), &mut buf);
        buf
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..W).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()
    }

    #[test]
    fn checked_shows_check_mark_and_label() {
        let buf = render(Checkbox::new("SHIELDS").checked(true));
        let middle = H / 2;
        let text = row_text(&buf, middle);
        assert!(
            text.contains(MARK_CHECK),
            "checked box should show the check mark: {text:?}"
        );
        assert!(text.contains("SHIELDS"), "label should render: {text:?}");
    }

    #[test]
    fn unchecked_shows_blank_mark() {
        let buf = render(Checkbox::new("SHIELDS").checked(false));
        let middle = H / 2;
        let text = row_text(&buf, middle);
        assert!(
            !text.contains(MARK_CHECK),
            "unchecked box must not show the check mark: {text:?}"
        );
        assert!(
            text.contains('[') && text.contains(']'),
            "brackets should render: {text:?}"
        );
    }

    #[test]
    fn checked_uses_ok_color_and_bold() {
        let ok = Theme::Cyberpunk.palette().ok.color();
        let buf = render(Checkbox::new("X").checked(true).theme(Theme::Cyberpunk));
        let middle = H / 2;
        let mark_x = (0..W)
            .find(|&x| buf[(x, middle)].symbol().starts_with(MARK_CHECK))
            .expect("check mark present");
        assert_eq!(buf[(mark_x, middle)].fg, ok, "checked mark should be --ok");
        assert!(
            buf[(mark_x, middle)].modifier.contains(Modifier::BOLD),
            "checked content should be bold via the cascade"
        );
    }

    #[test]
    fn unchecked_uses_muted_color() {
        let muted = Theme::Cyberpunk.palette().muted.color();
        let buf = render(Checkbox::new("X").checked(false).theme(Theme::Cyberpunk));
        let middle = H / 2;
        // The leading '[' cell carries the off (muted) style.
        let bracket_x = (0..W).find(|&x| buf[(x, middle)].symbol() == "[").expect("'[' present");
        assert_eq!(buf[(bracket_x, middle)].fg, muted, "unchecked should be --muted");
    }

    #[test]
    fn label_present_in_both_states() {
        for checked in [true, false] {
            let buf = render(Checkbox::new("WARP").checked(checked));
            let text = row_text(&buf, H / 2);
            assert!(
                text.contains("WARP"),
                "label should render in state checked={checked}: {text:?}"
            );
        }
    }

    #[test]
    fn content_is_centered() {
        let buf = render(Checkbox::new("WARP").checked(true));
        let text = row_text(&buf, H / 2);
        // Centered content is padded on the left.
        assert!(text.starts_with(' '), "content should be centered: {text:?}");
    }

    #[test]
    fn empty_area_is_a_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        Checkbox::new("X").checked(true).render(Rect::new(0, 0, 0, 0), &mut buf);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }

    #[test]
    fn non_default_shape_changes_checked_mark() {
        // The Cross shape swaps the checked mark from '✓' to '✕'.
        let buf = render(Checkbox::new("X").checked(true).shape(CheckboxShape::Cross));
        let text = row_text(&buf, H / 2);
        assert!(
            text.contains(MARK_CROSS),
            "Cross checked should use the '✕' mark: {text:?}"
        );
        assert!(
            !text.contains(MARK_CHECK),
            "Cross checked must not use the default '✓' mark: {text:?}"
        );
    }

    #[test]
    fn block_shape_shows_filled_square_when_checked() {
        let buf = render(Checkbox::new("X").checked(true).shape(CheckboxShape::Block));
        let text = row_text(&buf, H / 2);
        assert!(text.contains('■'), "Block checked should use '■': {text:?}");
    }

    #[test]
    fn block_shape_shows_hollow_square_when_unchecked() {
        let buf = render(Checkbox::new("X").checked(false).shape(CheckboxShape::Block));
        let text = row_text(&buf, H / 2);
        assert!(text.contains('□'), "Block unchecked should use '□': {text:?}");
    }
}
