//! **StatusLED** — a single status-indicator dot.
//!
//! A compact `● LABEL` readout that colors by a coarse status level — the
//! "system online / warning / critical" indicator that peppers every HUD. It
//! is the one-glyph cousin of [`crate::Value`]: where `Value` shows a reading
//! colored by level, [`StatusLED`] shows just the level itself, as a lit dot.
//!
//! ## Spec
//! - Renders `● LABEL`, where the dot + label take the level's color:
//!   - [`Level::Ok`] → `--ok` (green-ish)
//!   - [`Level::Warn`] → `--warn`
//!   - [`Level::Alert`] → `--alert`
//!   - [`Level::Normal`] → `--fg`
//!
//! ## Implementation notes
//! - Stateless [`Widget`]: `level` is per-frame configuration set by the app
//!   (convention #3, like [`crate::Toggle::on`]) — no `StatusLEDState`.
//! - Styling reuses the [`crate::Value`] cascade node plus the level's class
//!   ([`Level::as_classes`]) — the same ok/warn/alert vocabulary, driven off
//!   the palette.
//! - Centered; all glyphs are width-1.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{Level, StatusLED, Theme};
//!
//! let led = StatusLED::new("REACTOR").level(Level::Ok).theme(Theme::Fallout);
//! ```

use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};
use ratatui_style::{ComputeScratch, NodeRef};

use crate::{Level, Theme};

/// Glyph for the [`LEDShape::Dot`] default.
pub const DOT: char = '●';

/// Visual form of a [`StatusLED`]'s indicator dot.
///
/// Selects the dot glyph; the color stays on the CSS cascade (reusing `Value`
/// and the level class), untouched by this enum. The [`LEDShape::Dot`] default
/// draws the original `●`.
///
/// Every glyph is Unicode width-1 (see convention #5 at the crate root).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LEDShape {
    /// `●` — the original look.
    #[default]
    Dot,
    /// `■`.
    Square,
    /// `◆`.
    Diamond,
}

impl LEDShape {
    /// The indicator glyph for this shape.
    #[must_use]
    pub const fn glyph(self) -> char {
        match self {
            Self::Dot => DOT,
            Self::Square => '■',
            Self::Diamond => '◆',
        }
    }
}

/// A sci-fi status LED.
///
/// Build with [`StatusLED::new`] (label), then set the level with
/// [`StatusLED::level`] and the theme with [`StatusLED::theme`].
#[derive(Debug, Clone)]
pub struct StatusLED {
    /// Visible label text.
    pub label: String,
    /// Status level, which picks the color.
    pub level: Level,
    /// Dot-glyph form. Defaults to [`LEDShape::Dot`].
    pub shape: LEDShape,
    /// Theme whose [`Stylesheet`](ratatui_style::Stylesheet) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Default for StatusLED {
    fn default() -> Self {
        Self {
            label: String::new(),
            level: Level::default(),
            shape: LEDShape::default(),
            theme: Theme::Cyberpunk,
        }
    }
}

impl StatusLED {
    /// Create an LED with the given label, normal level, default theme.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            level: Level::default(),
            shape: LEDShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the status level (drives the color).
    #[must_use]
    pub fn level(mut self, level: Level) -> Self {
        self.level = level;
        self
    }

    /// Set the dot-glyph form (see [`LEDShape`]).
    #[must_use]
    pub fn shape(mut self, shape: LEDShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the LED.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

impl Widget for StatusLED {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        // Style reuses the `Value` node + the level's class (ok/warn/alert/fg).
        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let style = sheet
            .compute_with(
                &NodeRef::new("Value").classes(self.level.as_classes()),
                None,
                &mut scratch,
            )
            .to_style();

        let glyph = self.shape.glyph();
        let content = format!("{glyph} {label}", label = self.label);

        let row = area.y + area.height / 2;
        let content_width = content.chars().count() as u16;
        let available = area.width;
        let content_width = content_width.min(available);
        let x = area.x + available.saturating_sub(content_width) / 2;

        buf.set_style(area, style);
        buf.set_string(x, row, &content, style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 20;
    const H: u16 = 3;

    fn render(led: StatusLED) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        led.render(Rect::new(0, 0, W, H), &mut buf);
        buf
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..W).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()
    }

    #[test]
    fn renders_dot_and_label() {
        let buf = render(StatusLED::new("REACTOR").level(Level::Ok));
        let text = row_text(&buf, H / 2);
        assert!(text.contains(DOT), "dot present: {text:?}");
        assert!(text.contains("REACTOR"), "label present: {text:?}");
    }

    #[test]
    fn ok_level_uses_ok_color() {
        let ok = Theme::Cyberpunk.palette().ok.color();
        let buf = render(StatusLED::new("X").level(Level::Ok).theme(Theme::Cyberpunk));
        let y = H / 2;
        let dot_x = (0..W).find(|&x| buf[(x, y)].symbol().starts_with(DOT)).expect("dot present");
        assert_eq!(buf[(dot_x, y)].fg, ok, "Ok level dot should be --ok");
    }

    #[test]
    fn warn_level_uses_warn_color() {
        let warn = Theme::Cyberpunk.palette().warn.color();
        let buf = render(StatusLED::new("X").level(Level::Warn).theme(Theme::Cyberpunk));
        let y = H / 2;
        let dot_x = (0..W).find(|&x| buf[(x, y)].symbol().starts_with(DOT)).expect("dot present");
        assert_eq!(buf[(dot_x, y)].fg, warn, "Warn level dot should be --warn");
    }

    #[test]
    fn alert_level_uses_alert_color() {
        let alert = Theme::Cyberpunk.palette().alert.color();
        let buf = render(StatusLED::new("X").level(Level::Alert).theme(Theme::Cyberpunk));
        let y = H / 2;
        let dot_x = (0..W).find(|&x| buf[(x, y)].symbol().starts_with(DOT)).expect("dot present");
        assert_eq!(buf[(dot_x, y)].fg, alert, "Alert level dot should be --alert");
    }

    #[test]
    fn normal_level_uses_fg_color() {
        let fg = Theme::Cyberpunk.palette().fg.color();
        let buf = render(StatusLED::new("X").level(Level::Normal).theme(Theme::Cyberpunk));
        let y = H / 2;
        let dot_x = (0..W).find(|&x| buf[(x, y)].symbol().starts_with(DOT)).expect("dot present");
        assert_eq!(buf[(dot_x, y)].fg, fg, "Normal level dot should be --fg");
    }

    #[test]
    fn content_is_centered() {
        let buf = render(StatusLED::new("X").level(Level::Ok));
        let text = row_text(&buf, H / 2);
        assert!(text.starts_with(' '), "content should be centered: {text:?}");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        StatusLED::new("X").level(Level::Ok).render(Rect::new(0, 0, 0, 0), &mut buf);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }

    #[test]
    fn square_shape_uses_square_glyph() {
        let buf = render(StatusLED::new("X").level(Level::Ok).shape(LEDShape::Square));
        let text = row_text(&buf, H / 2);
        assert!(text.contains('■'), "Square shape uses '■': {text:?}");
        assert!(!text.contains(DOT), "must not use the Dot glyph: {text:?}");
    }
}
