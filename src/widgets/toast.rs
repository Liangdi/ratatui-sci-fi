//! **Toast** — an auto-dismissing notification.
//!
//! A transient pop-up that shows a message for N ticks then hides itself —
//! the "saved" / "copied" / "error" feedback. Unlike [`crate::AlertPopup`]
//! (a blocking dialog the app toggles), a [`Toast`] is fire-and-forget: call
//! [`ToastState::show`], and [`ToastState::tick`] counts it down to hidden.
//!
//! It's an overlay — the app renders it into a small centered `Rect` (optionally
//! `Clear`-ing beneath first), only while [`ToastState::visible`].
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]: the message + level + countdown live in
//!   [`ToastState`]; the widget carries only the border form + theme.
//! - The border + message take the level's color off the [`Palette`](crate::Palette).
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{Level, Toast, ToastState, Theme};
//!
//! let mut state = ToastState::new();
//! state.show("SAVED", Level::Ok, 40);
//! // each frame: state.tick();  render only while state.visible().
//! let toast = Toast::new().theme(Theme::Cyberpunk);
//! ```

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    widgets::{Block, BorderType, Paragraph, StatefulWidget, Widget},
};

use crate::{Level, Theme};

/// Visual form of a [`Toast`]'s border.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ToastShape {
    /// A rounded border — the default.
    #[default]
    Rounded,
    /// A plain square border.
    Square,
}

impl ToastShape {
    /// The ratatui border type for this shape.
    fn border_type(self) -> BorderType {
        match self {
            Self::Rounded => BorderType::Rounded,
            Self::Square => BorderType::Plain,
        }
    }
}

/// A sci-fi toast notification.
///
/// Build with [`Toast::new`], then set the theme. The message/countdown live in
/// [`ToastState`].
#[derive(Debug, Clone)]
pub struct Toast {
    /// Border form. Defaults to [`ToastShape::Rounded`].
    pub shape: ToastShape,
    /// Theme whose [`Palette`](crate::Palette) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Default for Toast {
    fn default() -> Self {
        Self {
            shape: ToastShape::default(),
            theme: Theme::Cyberpunk,
        }
    }
}

impl Toast {
    /// Create a toast, default shape/theme.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the border form (see [`ToastShape`]).
    #[must_use]
    pub fn shape(mut self, shape: ToastShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the toast.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// The palette color for a level.
    fn level_color(level: Level, theme: &Theme) -> ratatui::style::Color {
        let p = theme.palette();
        match level {
            Level::Ok => p.ok.color(),
            Level::Warn => p.warn.color(),
            Level::Alert => p.alert.color(),
            Level::Normal => p.fg.color(),
        }
    }
}

/// Mutable state for [`Toast`].
///
/// `remaining` ticks down each [`Self::tick`]; the toast is visible while it's
/// `> 0` and `message` is non-empty.
#[derive(Debug, Clone)]
pub struct ToastState {
    /// The message text (empty when nothing is queued).
    pub message: String,
    /// Status level, which picks the color.
    pub level: Level,
    /// Ticks remaining before the toast hides.
    pub remaining: u32,
}

impl Default for ToastState {
    fn default() -> Self {
        Self::new()
    }
}

impl ToastState {
    /// Create an empty (hidden) state.
    pub fn new() -> Self {
        Self {
            message: String::new(),
            level: Level::Normal,
            remaining: 0,
        }
    }

    /// Show `message` at `level` for `ticks` frames (replaces any current toast).
    pub fn show(&mut self, message: impl Into<String>, level: Level, ticks: u32) {
        self.message = message.into();
        self.level = level;
        self.remaining = ticks;
    }

    /// Advance the countdown one tick (no-op when already hidden).
    pub fn tick(&mut self) {
        if self.remaining > 0 {
            self.remaining -= 1;
            if self.remaining == 0 {
                self.message.clear();
            }
        }
    }

    /// Whether the toast should currently render.
    pub fn visible(&self) -> bool {
        self.remaining > 0 && !self.message.is_empty()
    }
}

impl StatefulWidget for Toast {
    type State = ToastState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() || !state.visible() {
            return;
        }

        let color = Self::level_color(state.level, &self.theme);
        let border_style = Style::new().fg(color);

        let block = Block::bordered()
            .border_type(self.shape.border_type())
            .border_style(border_style);
        let inner = block.inner(area);
        block.render(area, buf);

        let text = Paragraph::new(state.message.clone())
            .alignment(Alignment::Center)
            .style(Style::new().fg(color).add_modifier(Modifier::BOLD));
        text.render(inner, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 20;
    const H: u16 = 3;

    fn render(state: &mut ToastState, shape: ToastShape, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        StatefulWidget::render(Toast::new().shape(shape).theme(theme), Rect::new(0, 0, W, H), &mut buf, state);
        buf
    }

    #[test]
    fn show_makes_visible() {
        let mut s = ToastState::new();
        assert!(!s.visible());
        s.show("HI", Level::Ok, 5);
        assert!(s.visible());
    }

    #[test]
    fn tick_counts_down_and_hides() {
        let mut s = ToastState::new();
        s.show("HI", Level::Ok, 2);
        s.tick();
        assert!(s.visible(), "still visible at 1");
        s.tick();
        assert!(!s.visible(), "hidden at 0");
        assert!(s.message.is_empty(), "message cleared when hidden");
    }

    #[test]
    fn render_when_visible_shows_message() {
        let mut s = ToastState::new();
        s.show("SAVED", Level::Ok, 10);
        let buf = render(&mut s, ToastShape::Rounded, Theme::Cyberpunk);
        let row = H / 2;
        let has_text = (0..W).any(|x| {
            let sym = buf[(x, row)].symbol();
            sym == "S" || sym == "A" || sym == "V" || sym == "E" || sym == "D"
        });
        assert!(has_text, "message rendered while visible");
    }

    #[test]
    fn noop_when_not_visible() {
        let mut s = ToastState::new();
        let buf = render(&mut s, ToastShape::Rounded, Theme::Cyberpunk);
        // Nothing drawn.
        assert_eq!(buf[(0, 0)].symbol(), " ");
    }

    #[test]
    fn alert_level_uses_alert_border() {
        let alert = Theme::Cyberpunk.palette().alert.color();
        let mut s = ToastState::new();
        s.show("ERR", Level::Alert, 10);
        let buf = render(&mut s, ToastShape::Rounded, Theme::Cyberpunk);
        // Top-left border corner carries the level color.
        assert_eq!(buf[(0, 0)].fg, alert, "border is the level color");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let mut state = ToastState::new();
        state.show("HI", Level::Ok, 5);
        StatefulWidget::render(Toast::new(), Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
