//! **VerticalSlider** — a vertical range control.
//!
//! The vertical sibling of [`crate::Slider`]: a column track with a handle
//! that marks the current `0.0..=1.0` value. `Up`/`Down` step it.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]: `step` is configuration; `value` is state.
//! - Drawn in a single column (track `│`, filled `║`, handle `◉`) centered in
//!   the area. Filled cells sit below the handle; the handle rises with the
//!   value. Colors off the [`Palette`](crate::Palette): filled/handle `accent`,
//!   track `muted`.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{Theme, VerticalSlider, VerticalSliderState};
//!
//! let slider = VerticalSlider::new().step(0.1).theme(Theme::DeepSpace);
//! let mut state = VerticalSliderState::new();
//! ```

use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::StatefulWidget};
use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::Theme;

/// Default step size.
const DEFAULT_STEP: f32 = 0.1;

/// Visual form of a [`VerticalSlider`]'s track.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VerticalSliderShape {
    /// Track `│`, filled `║`, handle `◉`.
    #[default]
    Bar,
    /// Track `░`, filled `█`, handle `◆`.
    Cell,
}

impl VerticalSliderShape {
    const fn track(self) -> char {
        match self {
            Self::Bar => '│',
            Self::Cell => '░',
        }
    }
    const fn filled(self) -> char {
        match self {
            Self::Bar => '║',
            Self::Cell => '█',
        }
    }
    const fn handle(self) -> char {
        match self {
            Self::Bar => '◉',
            Self::Cell => '◆',
        }
    }
}

/// A sci-fi vertical slider.
#[derive(Debug, Clone)]
pub struct VerticalSlider {
    /// Step per Up/Down nudge.
    pub step: f32,
    /// Track form.
    pub shape: VerticalSliderShape,
    /// Theme.
    pub theme: Theme,
}

impl Default for VerticalSlider {
    fn default() -> Self {
        Self {
            step: DEFAULT_STEP,
            shape: VerticalSliderShape::default(),
            theme: Theme::Cyberpunk,
        }
    }
}

impl VerticalSlider {
    /// Create a vertical slider, default step/theme.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the per-nudge step.
    #[must_use]
    pub fn step(mut self, step: f32) -> Self {
        self.step = step;
        self
    }

    /// Set the track form.
    #[must_use]
    pub fn shape(mut self, shape: VerticalSliderShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Apply a key: `Up` increases by `step`, `Down` decreases, `Home`/`End`
    /// jump to 1.0/0.0. Clamped to `0.0..=1.0`.
    pub fn handle_key(&self, state: &mut VerticalSliderState, key: KeyEvent) {
        match key.code {
            // Up = increase (handle rises).
            KeyCode::Up | KeyCode::Right => state.value = (state.value + self.step).min(1.0),
            KeyCode::Down | KeyCode::Left => state.value = (state.value - self.step).max(0.0),
            KeyCode::Home => state.value = 1.0,
            KeyCode::End => state.value = 0.0,
            _ => {}
        }
    }
}

/// Mutable state for [`VerticalSlider`].
#[derive(Debug, Clone, Default)]
pub struct VerticalSliderState {
    /// Current position, `0.0..=1.0`.
    pub value: f32,
}

impl VerticalSliderState {
    /// Create at the minimum (`value = 0.0`).
    pub fn new() -> Self {
        Self::default()
    }
}

impl StatefulWidget for VerticalSlider {
    type State = VerticalSliderState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }
        let value = state.value.clamp(0.0, 1.0);
        let p = self.theme.palette();
        let bar = Style::new().fg(p.accent.color());
        let empty = Style::new().fg(p.muted.color());

        let col = area.x + area.width / 2;
        let h = area.height;
        if h == 0 {
            return;
        }
        // Handle row: value 1 → top (row 0), value 0 → bottom (row h-1).
        let handle_pos = ((1.0 - value) * (h - 1) as f32).round() as u16;

        for row in 0..h {
            let (glyph, style) = if row == handle_pos {
                (self.shape.handle(), bar)
            } else if row > handle_pos {
                // Below the handle → filled.
                (self.shape.filled(), bar)
            } else {
                (self.shape.track(), empty)
            };
            buf[(col, area.y + row)].set_char(glyph).set_style(style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 5;
    const H: u16 = 8;

    fn render(value: f32, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let mut state = VerticalSliderState { value };
        StatefulWidget::render(
            VerticalSlider::new().theme(theme),
            Rect::new(0, 0, W, H),
            &mut buf,
            &mut state,
        );
        buf
    }

    const fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, ratatui::crossterm::event::KeyModifiers::NONE)
    }

    #[test]
    fn value_one_handle_at_top() {
        let buf = render(1.0, Theme::Cyberpunk);
        let col = W / 2;
        assert_eq!(buf[(col, 0)].symbol(), "◉", "value 1 → handle at top row");
    }

    #[test]
    fn value_zero_handle_at_bottom() {
        let buf = render(0.0, Theme::Cyberpunk);
        let col = W / 2;
        assert_eq!(buf[(col, H - 1)].symbol(), "◉", "value 0 → handle at bottom row");
    }

    #[test]
    fn filled_below_handle() {
        // value 0.5 → handle mid; cells below are filled (║).
        let buf = render(0.5, Theme::Cyberpunk);
        let col = W / 2;
        // Bottom row is filled (║) since it's below the mid handle.
        assert_eq!(buf[(col, H - 1)].symbol(), "║", "filled below the handle");
    }

    #[test]
    fn handle_up_increases() {
        let s = VerticalSlider::new();
        let mut st = VerticalSliderState::new();
        s.handle_key(&mut st, key(KeyCode::Up));
        assert!((st.value - 0.1).abs() < 1e-6, "Up increases by step");
    }

    #[test]
    fn handle_clamps() {
        let s = VerticalSlider::new();
        let mut st = VerticalSliderState { value: 1.0 };
        s.handle_key(&mut st, key(KeyCode::Up));
        assert_eq!(st.value, 1.0, "clamps at 1.0");
    }

    #[test]
    fn home_end() {
        let s = VerticalSlider::new();
        let mut st = VerticalSliderState { value: 0.5 };
        s.handle_key(&mut st, key(KeyCode::Home));
        assert_eq!(st.value, 1.0);
        s.handle_key(&mut st, key(KeyCode::End));
        assert_eq!(st.value, 0.0);
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let mut state = VerticalSliderState::new();
        StatefulWidget::render(VerticalSlider::new(), Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
