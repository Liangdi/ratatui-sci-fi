//! **AlertPopup** — double-line red alert popup with brief flash (PRD §3 弹窗).
//!
//! ## Spec
//! - Warning popups use a double-line, alert-red border.
//! - Briefly flashes when first shown: while [`AlertPopupState::flash_remaining`]
//!   is greater than zero the border alternates intensity (fg/bg inverted by
//!   parity) so the popup visibly blinks each tick; once the flash expires it
//!   settles into a steady alert-red double border.
//! - Optional centered `title` (top border) and centered `message` (interior).
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]; the flash is driven by a countdown in
//!   [`AlertPopupState`] — arm it on show with [`AlertPopupState::flash`] and
//!   step it each tick with [`AlertPopupState::tick`].
//! - The `area` passed to `render` is the popup's own rect — the caller is
//!   responsible for centering it over its parent (e.g. via `Rect`/`Layout`).
//! - Styling uses `theme.palette()` directly: `palette.alert` for the border,
//!   `palette.panel` / `palette.fg` for the interior.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::Style,
    text::Line,
    widgets::{Block, BorderType, Borders, Paragraph, StatefulWidget, Widget},
};

use crate::Theme;

/// A centered alert popup with a double-line alert-red border and a brief
/// flash-on-show blink cycle.
///
/// Construct with [`AlertPopup::new`], optionally set a title with
/// [`AlertPopup::title`] or a theme with [`AlertPopup::theme`], then render
/// it as a [`StatefulWidget`] passing an [`AlertPopupState`] armed with
/// [`AlertPopupState::flash`].
///
/// ```ignore
/// use ratatui_sci_fi::widgets::{AlertPopup, AlertPopupState};
/// use ratatui_sci_fi::Theme;
///
/// let mut state = AlertPopupState::default();
/// state.flash(8); // blink for 8 ticks
/// let popup = AlertPopup::new("HULL BREACH")
///     .title(" ⚠ ALERT ")
///     .theme(Theme::DeepSpace);
/// // popup.render(area, buf, &mut state);
/// ```
#[derive(Debug, Default, Clone)]
pub struct AlertPopup {
    /// Optional centered title rendered on the top border.
    pub title: Option<String>,
    /// Centered message rendered inside the popup.
    pub message: String,
    /// Active theme (defaults to [`Theme::Cyberpunk`]). Drives the palette
    /// used for the border, background, and text.
    pub theme: Theme,
}

impl AlertPopup {
    /// Create a new alert popup showing `message`.
    pub fn new(message: impl Into<String>) -> Self {
        Self { title: None, message: message.into(), theme: Theme::default() }
    }

    /// Set the optional top-border title.
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the active theme.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

/// Mutable state for [`AlertPopup`].
///
/// Holds only the flash countdown; all other configuration lives on the
/// widget. Arm the flash on show with [`flash`](Self::flash) and step it each
/// tick with [`tick`](Self::tick).
#[derive(Debug, Default, Clone)]
pub struct AlertPopupState {
    /// Ticks of flash remaining after the popup is shown. While > 0 the
    /// border blinks; when it reaches 0 the popup is in its steady state.
    pub flash_remaining: u32,
}

impl AlertPopupState {
    /// Arm the flash for `ticks` ticks.
    pub fn flash(&mut self, ticks: u32) {
        self.flash_remaining = ticks;
    }

    /// Advance one tick; returns whether still flashing.
    pub fn tick(&mut self) -> bool {
        if self.flash_remaining > 0 {
            self.flash_remaining -= 1;
        }
        self.flash_remaining > 0
    }
}

impl StatefulWidget for AlertPopup {
    type State = AlertPopupState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let palette = self.theme.palette();
        let alert = palette.alert.color();
        let panel = palette.panel.color();
        let fg = palette.fg.color();

        // While flashing, alternate the border between a steady alert-red
        // (on panel bg) and an inverted, high-intensity blink (alert-red bg,
        // bright white-ish fg) based on the parity of the remaining ticks.
        // This makes the popup visibly blink each tick until it expires.
        let flashing = state.flash_remaining > 0;
        let blink_phase = state.flash_remaining % 2 == 1;

        let (border_style, content_style) = if !flashing {
            // Steady state: red border on panel background.
            (Style::new().fg(alert).bg(panel), Style::new().fg(fg).bg(panel))
        } else if blink_phase {
            // Inverted "on" phase: bright border, alert background.
            (Style::new().fg(fg).bg(alert), Style::new().fg(alert).bg(panel))
        } else {
            // Dimmed "off" phase: muted-ish border (panel bg, muted border).
            (Style::new().fg(palette.muted.color()).bg(panel), Style::new().fg(fg).bg(panel))
        };

        // Fill the area with the panel background first so the interior is
        // solid (not whatever was underneath), then draw the border on top.
        Block::new().style(Style::new().bg(panel)).render(area, buf);

        let mut block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(border_style)
            .style(Style::new().bg(panel));
        if let Some(title) = self.title {
            block = block.title(Line::from(title).alignment(Alignment::Center));
        }
        let inner = block.inner(area);
        block.render(area, buf);

        // Render the centered message inside the bordered area.
        Paragraph::new(Line::from(self.message))
            .alignment(Alignment::Center)
            .style(content_style)
            .render(inner, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    const W: u16 = 24;
    const H: u16 = 7;

    fn corner(buf: &Buffer) -> &str {
        buf[(0, 0)].symbol()
    }

    fn top_edge(buf: &Buffer) -> &str {
        // (1,0) is on the top edge, between the two corners.
        buf[(1, 0)].symbol()
    }

    #[test]
    fn renders_double_line_border() {
        let mut state = AlertPopupState::default();
        // Steady state: no flash.
        let area = Rect::new(0, 0, W, H);
        let mut buf = Buffer::empty(area);
        AlertPopup::new("HELLO").render(area, &mut buf, &mut state);

        // Double-line corners / edges should be present.
        assert_eq!(corner(&buf), "╔", "top-left corner must be the double-line glyph");
        assert_eq!(top_edge(&buf), "═", "top edge must be the double-line glyph");
        // Bottom-right corner.
        assert_eq!(buf[(W - 1, H - 1)].symbol(), "╝");
    }

    #[test]
    fn flash_changes_border_style_vs_steady() {
        let area = Rect::new(0, 0, W, H);

        // Steady (no flash): border fg is the alert color.
        let mut steady_state = AlertPopupState::default();
        let mut steady_buf = Buffer::empty(area);
        AlertPopup::new("ALERT").render(area, &mut steady_buf, &mut steady_state);
        let steady_fg = steady_buf[(0, 0)].fg;

        // Flashing, blink "on" phase (flash_remaining odd): border style
        // differs from steady (inverted fg/bg).
        let mut flash_state = AlertPopupState::default();
        flash_state.flash(3); // odd -> blink "on" inverted phase
        let mut flash_buf = Buffer::empty(area);
        AlertPopup::new("ALERT").render(area, &mut flash_buf, &mut flash_state);
        let flash_fg = flash_buf[(0, 0)].fg;

        assert_ne!(
            steady_fg, flash_fg,
            "flashing border fg must differ from steady border fg"
        );
    }

    #[test]
    fn flash_parity_flips_between_phases() {
        let area = Rect::new(0, 0, W, H);

        let mk = |remaining: u32| {
            let mut s = AlertPopupState { flash_remaining: remaining };
            let mut b = Buffer::empty(area);
            AlertPopup::new("X").render(area, &mut b, &mut s);
            b[(0, 0)].fg
        };

        // Odd remaining -> inverted "on" phase; even -> dimmed "off" phase.
        // Their border fgs must differ from each other.
        assert_ne!(mk(3), mk(2), "odd vs even flash parity must differ");
        // And both must differ from the steady state.
        let steady = {
            let mut s = AlertPopupState::default();
            let mut b = Buffer::empty(area);
            AlertPopup::new("X").render(area, &mut b, &mut s);
            b[(0, 0)].fg
        };
        assert_ne!(mk(3), steady);
        assert_ne!(mk(2), steady);
    }

    #[test]
    fn tick_counts_down() {
        let mut s = AlertPopupState::default();
        s.flash(2);
        assert!(s.tick(), "still flashing after first tick (1 left)");
        assert!(!s.tick(), "flash expired after second tick");
        // Ticking past zero stays expired and never underflows.
        assert!(!s.tick());
        assert_eq!(s.flash_remaining, 0);
    }

    #[test]
    fn theme_builder_overrides_default() {
        let p = AlertPopup::new("X").theme(Theme::DeepSpace);
        assert_eq!(p.theme, Theme::DeepSpace);
        // Default is Cyberpunk.
        assert_eq!(AlertPopup::new("X").theme, Theme::Cyberpunk);
    }
}
