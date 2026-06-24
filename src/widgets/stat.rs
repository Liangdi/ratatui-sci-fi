//! **Stat** — a big-number statistic card.
//!
//! A single headline metric: a large value, a caption, and an optional trend
//! arrow — the "KPI tile" / dashboard readout. Where [`crate::Value`] is a
//! compact label+reading, [`Stat`] makes the number the hero.
//!
//! ## Spec
//! - [`Card`](StatShape::Card) (default): big value centered, caption + trend
//!   beneath.
//! - [`Inline`](StatShape::Inline): `value caption ↑` on one row.
//! - Trend: [`Up`](Trend::Up) → `↑` `--ok`, [`Down`](Trend::Down) → `↓`
//!   `--alert`, [`Flat`](Trend::Flat) → `→` `--muted`.
//!
//! ## Implementation notes
//! - Stateless [`Widget`]: value/label/trend are configuration.
//! - Colors come off the [`Palette`](crate::Palette): value `accent` (bold),
//!   caption `muted`, trend by direction.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{Stat, StatShape, Theme, Trend};
//!
//! let s = Stat::new("1.2k", "SIGNUPS").trend(Trend::Up).theme(Theme::Cyberpunk);
//! ```

use ratatui::{buffer::Buffer, layout::Rect, style::{Modifier, Style}, widgets::Widget};

use crate::Theme;

/// Direction of a [`Stat`]'s trend arrow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trend {
    /// Up — `↑`, colored `--ok`.
    Up,
    /// Down — `↓`, colored `--alert`.
    Down,
    /// Flat — `→`, colored `--muted`.
    Flat,
}

impl Trend {
    /// The arrow glyph.
    #[must_use]
    pub const fn glyph(self) -> char {
        match self {
            Self::Up => '↑',
            Self::Down => '↓',
            Self::Flat => '→',
        }
    }
}

/// Visual form of a [`Stat`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StatShape {
    /// Big value centered, caption + trend beneath — the default.
    #[default]
    Card,
    /// `value caption ↑` on a single row.
    Inline,
}

/// A sci-fi statistic card.
///
/// Build with [`Stat::new`] (value, caption), then add a [`Stat::trend`] and
/// set the theme.
#[derive(Debug, Clone)]
pub struct Stat {
    /// The headline value (a string, so units/formatting are the app's choice).
    pub value: String,
    /// Caption beneath (or beside) the value.
    pub label: String,
    /// Optional trend arrow.
    pub trend: Option<Trend>,
    /// Layout form. Defaults to [`StatShape::Card`].
    pub shape: StatShape,
    /// Theme whose [`Palette`](crate::Palette) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Stat {
    /// Create a stat with `value` and `label`, no trend, default theme.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            trend: None,
            shape: StatShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the trend arrow.
    #[must_use]
    pub fn trend(mut self, trend: Trend) -> Self {
        self.trend = Some(trend);
        self
    }

    /// Set the layout form (see [`StatShape`]).
    #[must_use]
    pub fn shape(mut self, shape: StatShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the stat.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Draw a string centered on `row`, clipped to `area`.
    fn draw_centered(buf: &mut Buffer, area: Rect, row: u16, text: &str, style: Style) {
        if row >= area.bottom() {
            return;
        }
        let w = text.chars().count() as u16;
        let x = area.x + area.width.saturating_sub(w.min(area.width)) / 2;
        for (i, ch) in text.chars().enumerate() {
            let cx = x + i as u16;
            if cx >= area.right() {
                break;
            }
            buf[(cx, row)].set_char(ch).set_style(style);
        }
    }
}

impl Widget for Stat {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        let p = self.theme.palette();
        let value_style = Style::new().fg(p.accent.color()).add_modifier(Modifier::BOLD);
        let label_style = Style::new().fg(p.muted.color());
        let trend_style = Style::new().fg(match self.trend {
            Some(Trend::Up) => p.ok.color(),
            Some(Trend::Down) => p.alert.color(),
            _ => p.muted.color(),
        });
        let arrow = self.trend.map(|t| t.glyph());

        match self.shape {
            StatShape::Card => {
                let mid = area.y + area.height / 2;
                Self::draw_centered(buf, area, mid, &self.value, value_style);
                // Caption + arrow beneath the value (or above if mid is the bottom).
                let cap_row = if mid + 1 < area.bottom() { mid + 1 } else { mid.saturating_sub(1) };
                let mut caption = self.label.clone();
                if let Some(a) = arrow {
                    caption.push(' ');
                    caption.push(a);
                }
                // Draw caption muted, then overwrite the arrow cell with trend color.
                Self::draw_centered(buf, area, cap_row, &caption, label_style);
                if let Some(a) = arrow {
                    let cap_w = caption.chars().count() as u16;
                    let cap_x = area.x + area.width.saturating_sub(cap_w.min(area.width)) / 2;
                    let arrow_x = cap_x + (cap_w.saturating_sub(1));
                    if arrow_x < area.right() {
                        buf[(arrow_x, cap_row)].set_char(a).set_style(trend_style);
                    }
                }
            }
            StatShape::Inline => {
                let mid = area.y + area.height / 2;
                let mut line = format!("{} {}", self.value, self.label);
                if let Some(a) = arrow {
                    line.push(' ');
                    line.push(a);
                }
                Self::draw_centered(buf, area, mid, &line, value_style);
                // Recolor the caption + arrow (non-leading part) for legibility.
                let line_w = line.chars().count() as u16;
                let start = area.x + area.width.saturating_sub(line_w.min(area.width)) / 2;
                let value_w = self.value.chars().count() as u16;
                // Caption cells (after "value ").
                let cap_start = start + value_w + 1; // +1 for the space
                for (i, ch) in self.label.chars().enumerate() {
                    let px = cap_start + i as u16;
                    if px >= area.right() {
                        break;
                    }
                    buf[(px, mid)].set_char(ch).set_style(label_style);
                }
                if let Some(a) = arrow {
                    let ax = cap_start + self.label.chars().count() as u16 + 1;
                    if ax < area.right() {
                        buf[(ax, mid)].set_char(a).set_style(trend_style);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 20;
    const H: u16 = 3;

    fn render(value: &str, label: &str, trend: Option<Trend>, shape: StatShape, theme: Theme) -> Buffer {
        let mut s = Stat::new(value, label).shape(shape).theme(theme);
        if let Some(t) = trend {
            s = s.trend(t);
        }
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        s.render(Rect::new(0, 0, W, H), &mut buf);
        buf
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..W).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()
    }

    #[test]
    fn card_renders_value_and_label() {
        let buf = render("1.2k", "SIGNUPS", None, StatShape::Card, Theme::Cyberpunk);
        let mid = H / 2;
        assert!(row_text(&buf, mid).contains("1.2k"), "value on mid row");
        assert!(row_text(&buf, mid + 1).contains("SIGNUPS"), "caption beneath");
    }

    #[test]
    fn trend_up_draws_up_arrow() {
        let buf = render("10", "X", Some(Trend::Up), StatShape::Card, Theme::Cyberpunk);
        let cap_row = H / 2 + 1;
        assert!(row_text(&buf, cap_row).contains('↑'), "Up trend arrow");
    }

    #[test]
    fn trend_up_uses_ok_color() {
        let ok = Theme::Cyberpunk.palette().ok.color();
        let buf = render("10", "X", Some(Trend::Up), StatShape::Card, Theme::Cyberpunk);
        let cap_row = H / 2 + 1;
        let ax = (0..W).find(|&x| buf[(x, cap_row)].symbol() == "↑").expect("arrow present");
        assert_eq!(buf[(ax, cap_row)].fg, ok, "Up arrow is --ok");
    }

    #[test]
    fn trend_down_uses_alert_color() {
        let alert = Theme::Cyberpunk.palette().alert.color();
        let buf = render("10", "X", Some(Trend::Down), StatShape::Card, Theme::Cyberpunk);
        let cap_row = H / 2 + 1;
        let ax = (0..W).find(|&x| buf[(x, cap_row)].symbol() == "↓").expect("arrow present");
        assert_eq!(buf[(ax, cap_row)].fg, alert, "Down arrow is --alert");
    }

    #[test]
    fn no_trend_no_arrow() {
        let buf = render("10", "X", None, StatShape::Card, Theme::Cyberpunk);
        let cap_row = H / 2 + 1;
        assert!(!row_text(&buf, cap_row).contains('↑'));
        assert!(!row_text(&buf, cap_row).contains('↓'));
    }

    #[test]
    fn value_uses_accent_bold() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let buf = render("42", "X", None, StatShape::Card, Theme::Cyberpunk);
        let mid = H / 2;
        let vx = (0..W).find(|&x| buf[(x, mid)].symbol() == "4").expect("'4' present");
        assert_eq!(buf[(vx, mid)].fg, accent, "value is --accent");
        assert!(
            buf[(vx, mid)].modifier.contains(Modifier::BOLD),
            "value is bold"
        );
    }

    #[test]
    fn inline_renders_on_one_row() {
        let buf = render("42", "PTS", Some(Trend::Up), StatShape::Inline, Theme::Cyberpunk);
        let mid = H / 2;
        let text = row_text(&buf, mid);
        assert!(text.contains("42") && text.contains("PTS") && text.contains('↑'), "inline row: {text:?}");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        Stat::new("1", "X").render(Rect::new(0, 0, 0, 0), &mut buf);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
