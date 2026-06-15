//! **Divider** — full-width horizontal rule with optional centered label.
//!
//! A single-row divider drawn as a faint full-width `─` rule, with an optional
//! centered label punched through the middle (`──── SECTION ────`). It replaces
//! the hand-rolled `Span`/`─` filler the examples were building inline.
//!
//! ## Spec
//! - Fill the middle row with `─` in the theme's muted color.
//! - When a label is present, center it across the row, flanked by dashes on
//!   both sides.
//!
//! ## Implementation notes
//! - Stateless [`Widget`]; label + theme are per-frame config.
//! - Styling goes through the `Scanline` cascade rule (muted), matching the
//!   per-row separators drawn by [`crate::widgets::ScanList`]. The label is
//!   drawn in the theme foreground so it reads as a section header rather than
//!   fading into the rule.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::Widget,
};
use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Glyph for the rule. Shared with `ScanList`'s per-row scanline.
const RULE_GLYPH: char = '─';

/// A full-width horizontal rule, optionally with a centered label.
///
/// ```rust
/// use ratatui_sci_fi::{Divider, Theme};
///
/// let d = Divider::new().label("TELEMETRY").theme(Theme::Weyland);
/// ```
#[derive(Debug, Clone, Default)]
pub struct Divider {
    /// Optional centered label punched through the rule.
    pub label: Option<String>,
    /// Theme whose [`Stylesheet`](ratatui_style::Stylesheet) drives the rule
    /// color. Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Divider {
    /// Create an unlabeled divider, default theme.
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach a centered label punched through the rule.
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the theme whose cascade drives the rule color.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Resolve the rule style via the `Scanline` cascade rule.
    fn rule_style(&self, scratch: &mut ComputeScratch) -> Style {
        let sheet = self.theme.stylesheet();
        sheet.compute_with(&NodeRef::new("Scanline"), None, scratch).to_style()
    }
}

impl Widget for Divider {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let mut scratch = ComputeScratch::new();
        let rule_style = self.rule_style(&mut scratch);

        let y = area.y + area.height / 2;

        match &self.label {
            None => {
                // Plain full-width rule.
                for x in area.x..area.x + area.width {
                    buf[(x, y)].set_char(RULE_GLYPH).set_style(rule_style);
                }
            }
            Some(label) => {
                // Center the label across the row, filling the rest with the
                // rule glyph. Label is drawn in the theme foreground so it reads
                // as a header; the flanking dashes keep the muted rule style.
                let label_width = label.chars().count() as u16;
                let pad = area.width.saturating_sub(label_width);
                let left_pad = pad / 2;

                // Left flanking rule.
                for x in 0..left_pad {
                    let px = area.x + x;
                    buf[(px, y)].set_char(RULE_GLYPH).set_style(rule_style);
                }

                // Centered label in the theme foreground (the `Value` rule),
                // bright on the muted rule so it reads as a section header.
                let label_style = {
                    let sheet = self.theme.stylesheet();
                    sheet.compute_with(&NodeRef::new("Value"), None, &mut scratch).to_style()
                };
                let mut lx = area.x + left_pad;
                let right = area.x + area.width;
                for ch in label.chars() {
                    if lx >= right {
                        break;
                    }
                    buf[(lx, y)].set_symbol(ch.to_string().as_str()).set_style(label_style);
                    lx += 1;
                }

                // Right flanking rule fills whatever remains.
                while lx < right {
                    buf[(lx, y)].set_char(RULE_GLYPH).set_style(rule_style);
                    lx += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    const W: u16 = 16;
    const H: u16 = 3;

    fn render(divider: Divider) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        divider.render(Rect::new(0, 0, W, H), &mut buf);
        buf
    }

    #[test]
    fn fills_middle_row_with_rule_glyph() {
        let buf = render(Divider::new().theme(Theme::Cyberpunk));
        let middle = H / 2;
        for x in 0..W {
            assert_eq!(buf[(x, middle)].symbol(), "─", "cell {x} on the middle row should be the rule glyph");
        }
        // Rows above/below the middle are untouched.
        assert_eq!(buf[(0, 0)].symbol(), " ", "non-middle rows stay clear");
    }

    #[test]
    fn rule_color_is_muted() {
        let muted = Theme::Cyberpunk.palette().muted.color();
        let buf = render(Divider::new().theme(Theme::Cyberpunk));
        let middle = H / 2;
        assert_eq!(buf[(0, middle)].fg, muted, "rule should be --muted via the Scanline rule");
    }

    #[test]
    fn label_is_centered_with_flanking_dashes() {
        let buf = render(Divider::new().label("SEC").theme(Theme::Cyberpunk));
        let middle = H / 2;
        // "SEC" is 3 wide; total 16 → 13 dashes split 6 left / 7 right.
        // Left flank (x=0..5) is dashes, label at x=6..8.
        assert_eq!(buf[(0, middle)].symbol(), "─");
        assert_eq!(buf[(6, middle)].symbol(), "S");
        assert_eq!(buf[(8, middle)].symbol(), "C");
        // Right flank resumes dashes.
        assert_eq!(buf[(9, middle)].symbol(), "─");
    }

    #[test]
    fn label_renders_in_fg() {
        let fg = Theme::Cyberpunk.palette().fg.color();
        let buf = render(Divider::new().label("X").theme(Theme::Cyberpunk));
        let middle = H / 2;
        let lx = (0..W).find(|&x| buf[(x, middle)].symbol() == "X").expect("label present");
        assert_eq!(buf[(lx, middle)].fg, fg, "label should be foreground");
    }

    #[test]
    fn width_one_renders_single_rule() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        Divider::new().render(Rect::new(0, 0, 1, 1), &mut buf);
        assert_eq!(buf[(0, 0)].symbol(), "─");
    }

    #[test]
    fn empty_area_is_a_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        Divider::new().label("X").render(Rect::new(0, 0, 0, 0), &mut buf);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
