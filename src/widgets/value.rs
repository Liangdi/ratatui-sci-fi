//! **Value** — labeled telemetry readout (PRD §3 基础组件, the readout primitive).
//!
//! A single-row readout: a quiet muted `LABEL` followed by a bright value, with
//! the value's color tied to a [`Level`] so a glance conveys status.
//!
//! ## Spec
//! - Optional left `label` (e.g. `HULL`), drawn muted so it recedes.
//! - A `value` string, drawn in the level's color: neutral foreground at
//!   [`Level::Normal`], escalating to ok / warn / alert.
//!
//! ## Implementation notes
//! - Stateless [`Widget`]; label/value/level are all per-frame configuration.
//! - Styling goes through the theme's
//!   [`Stylesheet`](ratatui_style::Stylesheet) cascade: the `Label` rule drives
//!   the label (muted), and the `Value` rule — plus a `.ok`/`.warn`/`.alert`
//!   class from the [`Level`] — drives the value. Both are `var(--…)`-driven off
//!   the same palette, so the resolved colors are byte-identical to reading
//!   `palette()` directly.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{Level, Value, Theme};
//!
//! let v = Value::new("78%").label("HULL").state(Level::Warn).theme(Theme::Weyland);
//! ```

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::Widget,
};
use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;
use crate::widgets::level::Level;

/// A labeled, status-colored telemetry readout.
///
/// Build with [`Value::new`] (the value), then optionally add a label
/// ([`Value::label`]), a status [`Level`] ([`Value::state`]), and a theme
/// ([`Value::theme`]).
#[derive(Debug, Clone)]
pub struct Value {
    /// Optional left-aligned label, e.g. `"HULL"`.
    pub label: Option<String>,
    /// The readout text, e.g. `"78%"`.
    pub value: String,
    /// Status level driving the value color. Default [`Level::Normal`].
    pub level: Level,
    /// Theme whose [`Stylesheet`](ratatui_style::Stylesheet) drives the colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Default for Value {
    fn default() -> Self {
        Self { label: None, value: String::new(), level: Level::Normal, theme: Theme::Cyberpunk }
    }
}

impl Value {
    /// Create a readout for `value`, neutral level, default theme.
    pub fn new(value: impl Into<String>) -> Self {
        Self::default().value(value)
    }

    /// Attach a left-aligned muted label.
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the readout value text.
    #[must_use]
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self
    }

    /// Set the status [`Level`] driving the value's color.
    #[must_use]
    pub fn state(mut self, level: Level) -> Self {
        self.level = level;
        self
    }

    /// Set the theme whose cascade drives colors.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

impl Widget for Value {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let label_style =
            sheet.compute_with(&NodeRef::new("Label"), None, &mut scratch).to_style();
        let value_style = sheet
            .compute_with(&NodeRef::new("Value").classes(self.level.as_classes()), None, &mut scratch)
            .to_style();

        // Render on the vertical middle row.
        let y = area.y + area.height / 2;
        let right = area.x + area.width;
        let mut x = area.x;

        // Left label + a one-cell gap, if both fit.
        if let Some(label) = &self.label {
            for ch in label.chars() {
                if x >= right {
                    break;
                }
                buf[(x, y)].set_char(ch).set_style(label_style);
                x += 1;
            }
            // One-cell gap after the label so label and value don't run together.
            if x < right {
                buf[(x, y)].set_style(label_style);
                x += 1;
            }
        }

        // Value, left-aligned after the label, clipped at the right edge.
        for ch in self.value.chars() {
            if x >= right {
                break;
            }
            buf[(x, y)].set_char(ch).set_style(value_style);
            x += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    const W: u16 = 16;
    const H: u16 = 1;

    fn render(v: Value) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        v.render(Rect::new(0, 0, W, H), &mut buf);
        buf
    }

    #[test]
    fn label_is_muted_value_is_fg_at_normal() {
        let p = Theme::Cyberpunk.palette();
        let buf = render(Value::new("78").label("HULL").theme(Theme::Cyberpunk));
        // Label "HULL" at x=0..3 in muted.
        assert_eq!(buf[(0, 0)].symbol(), "H");
        assert_eq!(buf[(0, 0)].fg, p.muted.color());
        // Gap at x=4, then value "78" at x=5.
        assert_eq!(buf[(5, 0)].symbol(), "7");
        assert_eq!(buf[(5, 0)].fg, p.fg.color(), "Normal value should be fg");
    }

    #[test]
    fn alert_level_colors_value_alert() {
        let alert = Theme::Cyberpunk.palette().alert.color();
        let buf = render(Value::new("FIRE").label("CORE").state(Level::Alert).theme(Theme::Cyberpunk));
        // Value "FIRE" starts after "CORE " (5 cells).
        assert_eq!(buf[(5, 0)].symbol(), "F");
        assert_eq!(buf[(5, 0)].fg, alert, "Alert value should be --alert");
    }

    #[test]
    fn warn_and_ok_levels_resolve_their_tokens() {
        let p = Theme::Cyberpunk.palette();
        let warn = render(Value::new("V").state(Level::Warn).theme(Theme::Cyberpunk));
        assert_eq!(warn[(0, 0)].fg, p.warn.color());
        let ok = render(Value::new("V").state(Level::Ok).theme(Theme::Cyberpunk));
        assert_eq!(ok[(0, 0)].fg, p.ok.color());
    }

    #[test]
    fn no_label_renders_value_at_left() {
        let buf = render(Value::new("SOLO").theme(Theme::Cyberpunk));
        assert_eq!(buf[(0, 0)].symbol(), "S", "value should start at x=0 with no label");
        assert_eq!(buf[(1, 0)].symbol(), "O");
    }

    #[test]
    fn value_clips_at_right_edge_no_overflow() {
        // Width 4, value "ABCDEF" — only ABCD should land; no panic, no overflow.
        let mut buf = Buffer::empty(Rect::new(0, 0, 4, 1));
        Value::new("ABCDEF").render(Rect::new(0, 0, 4, 1), &mut buf);
        assert_eq!(buf[(0, 0)].symbol(), "A");
        assert_eq!(buf[(3, 0)].symbol(), "D");
    }

    #[test]
    fn theme_builder_is_applied() {
        let v = Value::new("X").theme(Theme::Fallout);
        assert_eq!(v.theme, Theme::Fallout);
    }

    #[test]
    fn empty_area_is_a_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        Value::new("X").label("Y").render(Rect::new(0, 0, 0, 0), &mut buf);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
