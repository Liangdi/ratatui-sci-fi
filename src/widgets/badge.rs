//! **Badge** — a small status label.
//!
//! A compact `[ ONLINE ]` / `3` chip colored by level — the persistent status
//! tag (vs. [`crate::StatusLED`]'s dot, or [`crate::Value`]'s reading). Two
//! forms: [`Filled`](BadgeShape::Filled) paints the level color as the
//! background; [`Outlined`](BadgeShape::Outlined) brackets the text in it.
//!
//! ## Implementation notes
//! - Stateless [`Widget`]: `text`/`level` are configuration.
//! - Colors off the [`Palette`](crate::Palette): the level's color (filled as
//!   bg with `--bg` text for contrast, or as fg when outlined).
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{Badge, BadgeShape, Level, Theme};
//!
//! let b = Badge::new("ONLINE").level(Level::Ok).shape(BadgeShape::Filled).theme(Theme::Fallout);
//! ```

use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};

use crate::{Level, Theme};

/// Visual form of a [`Badge`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BadgeShape {
    /// Level color as the background, `--bg` text — the default.
    #[default]
    Filled,
    /// `[ text ]` in the level color, no background.
    Outlined,
}

/// A sci-fi status badge.
///
/// Build with [`Badge::new`] (the text), then set the level and shape.
#[derive(Debug, Clone)]
pub struct Badge {
    /// Badge text.
    pub text: String,
    /// Status level, which picks the color.
    pub level: Level,
    /// Filled vs outlined. Defaults to [`BadgeShape::Filled`].
    pub shape: BadgeShape,
    /// Theme whose [`Palette`](crate::Palette) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Badge {
    /// Create a badge with `text`, normal level, default theme.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            level: Level::default(),
            shape: BadgeShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the status level (drives the color).
    #[must_use]
    pub fn level(mut self, level: Level) -> Self {
        self.level = level;
        self
    }

    /// Set the form (see [`BadgeShape`]).
    #[must_use]
    pub fn shape(mut self, shape: BadgeShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the badge.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// The palette color for this badge's level.
    fn level_color(&self) -> ratatui::style::Color {
        let p = self.theme.palette();
        match self.level {
            Level::Ok => p.ok.color(),
            Level::Warn => p.warn.color(),
            Level::Alert => p.alert.color(),
            Level::Normal => p.fg.color(),
        }
    }
}

impl Widget for Badge {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let p = self.theme.palette();
        let level_color = self.level_color();
        let row = area.y + area.height / 2;

        match self.shape {
            BadgeShape::Filled => {
                // " text " painted across the badge with the level as bg.
                let style = Style::new().fg(p.bg.color()).bg(level_color);
                let content: Vec<char> = format!(" {} ", self.text).chars().collect();
                for (i, &ch) in content.iter().enumerate() {
                    let col = area.x + i as u16;
                    if col >= area.right() {
                        break;
                    }
                    buf[(col, row)].set_char(ch).set_style(style);
                }
                // Fill the rest of the badge row with the level bg.
                for col in (area.x + content.len() as u16)..area.right() {
                    buf[(col, row)].set_style(style);
                }
            }
            BadgeShape::Outlined => {
                let style = Style::new().fg(level_color);
                let content = format!("[ {} ]", self.text);
                let cw = content.chars().count() as u16;
                let x = area.x + area.width.saturating_sub(cw) / 2;
                for (i, ch) in content.chars().enumerate() {
                    let col = x + i as u16;
                    if col >= area.right() {
                        break;
                    }
                    buf[(col, row)].set_char(ch).set_style(style);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 16;
    const H: u16 = 3;

    fn render(text: &str, level: Level, shape: BadgeShape, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        Badge::new(text)
            .level(level)
            .shape(shape)
            .theme(theme)
            .render(Rect::new(0, 0, W, H), &mut buf);
        buf
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..W).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()
    }

    #[test]
    fn filled_renders_text_with_level_bg() {
        let ok = Theme::Cyberpunk.palette().ok.color();
        let bg = Theme::Cyberpunk.palette().bg.color();
        let buf = render("ON", Level::Ok, BadgeShape::Filled, Theme::Cyberpunk);
        let row = H / 2;
        // 'O' cell: fg=bg (for contrast), bg=ok (level).
        let ox = (0..W).find(|&x| buf[(x, row)].symbol() == "O").expect("'O'");
        assert_eq!(buf[(ox, row)].bg, ok, "filled bg is the level color");
        assert_eq!(buf[(ox, row)].fg, bg, "filled fg is --bg for contrast");
    }

    #[test]
    fn outlined_brackets_text_in_level_color() {
        let ok = Theme::Cyberpunk.palette().ok.color();
        let buf = render("ON", Level::Ok, BadgeShape::Outlined, Theme::Cyberpunk);
        let text = row_text(&buf, H / 2);
        assert!(text.contains("[ ON ]"), "outlined brackets: {text:?}");
        let ox = (0..W).find(|&x| buf[(x, H / 2)].symbol() == "O").expect("'O'");
        assert_eq!(buf[(ox, H / 2)].fg, ok, "outlined text is the level color");
    }

    #[test]
    fn warn_level_uses_warn_color() {
        let warn = Theme::Cyberpunk.palette().warn.color();
        let buf = render("X", Level::Warn, BadgeShape::Outlined, Theme::Cyberpunk);
        let xx = (0..W).find(|&x| buf[(x, H / 2)].symbol() == "X").expect("'X'");
        assert_eq!(buf[(xx, H / 2)].fg, warn, "Warn → --warn");
    }

    #[test]
    fn alert_level_uses_alert_color() {
        let alert = Theme::Cyberpunk.palette().alert.color();
        let buf = render("X", Level::Alert, BadgeShape::Outlined, Theme::Cyberpunk);
        let xx = (0..W).find(|&x| buf[(x, H / 2)].symbol() == "X").expect("'X'");
        assert_eq!(buf[(xx, H / 2)].fg, alert, "Alert → --alert");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        Badge::new("X").level(Level::Ok).render(Rect::new(0, 0, 0, 0), &mut buf);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
