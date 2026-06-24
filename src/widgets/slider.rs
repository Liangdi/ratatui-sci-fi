//! **Slider** — sci-fi horizontal range control.
//!
//! A single-row slider over a normalized `0.0..=1.0` range, keyboard-nudged by
//! a configurable step. It is the interactive counterpart of
//! [`crate::EnergyGauge`]: where the gauge passively displays a ratio, the
//! slider lets the user *set* one — Left/Right step the value, Home/End jump to
//! the bounds, and a handle glyph marks the current position on the track.
//!
//! ## Spec
//!
//! ```text
//!   ════◉──────────  42%
//!   ▰▰▰▰◆▱▱▱▱▱▱▱▱  42%
//!   ████►░░░░░░░░  42%
//! ```
//! Filled cells left of the handle, the handle itself, then empty track cells;
//! a right-aligned percentage labels the value.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]: `step` is immutable configuration on the
//!   widget struct (convention #3); only `value` is mutable state, nudged by
//!   [`Slider::handle_key`] or set directly by the app.
//! - Key handling lives on the **widget** because it needs `step`:
//!   `slider.handle_key(&mut state, key)`.
//! - Styling reuses the [`crate::EnergyGauge`] `Gauge.*` cascade nodes — filled
//!   cells + handle take the level color (`Gauge.ok`/`.warn`/`.alert` by the
//!   same ≥0.6/≥0.3 thresholds), empty track takes `Gauge.empty`, and the
//!   percentage takes `Gauge.label`. All `var(--…)`-driven off the palette.
//! - All glyphs are width-1.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{Slider, SliderState, Theme};
//! use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
//!
//! let slider = Slider::new().step(0.05).theme(Theme::DeepSpace);
//! let mut state = SliderState::new();
//! slider.handle_key(&mut state, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
//! ```

use ratatui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Default step size (normalized `0.0..=1.0` units) for keyboard nudging.
pub const DEFAULT_STEP: f32 = 0.1;
/// Columns reserved at the right edge for the `NNN%` label.
const PCT_COLS: u16 = 4;

/// Visual form of a [`Slider`]'s track.
///
/// Selects the `(track, filled, handle)` glyph triple; colors stay on the CSS
/// cascade (reusing `Gauge.*`), untouched by this enum. The
/// [`SliderShape::Bar`] default renders the `═`/`─` track with an `◉` handle.
///
/// Every glyph is Unicode width-1 (see convention #5 at the crate root).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SliderShape {
    /// Track `─`, filled `═`, handle `◉`.
    #[default]
    Bar,
    /// Track `▱`, filled `▰`, handle `◆`.
    Cell,
    /// Track `░`, filled `█`, handle `►`.
    Block,
}

impl SliderShape {
    /// The empty-track glyph (right of the handle).
    #[must_use]
    pub const fn track(self) -> char {
        match self {
            Self::Bar => '─',
            Self::Cell => '▱',
            Self::Block => '░',
        }
    }

    /// The filled-track glyph (left of the handle).
    #[must_use]
    pub const fn filled(self) -> char {
        match self {
            Self::Bar => '═',
            Self::Cell => '▰',
            Self::Block => '█',
        }
    }

    /// The handle glyph (the current position).
    #[must_use]
    pub const fn handle(self) -> char {
        match self {
            Self::Bar => '◉',
            Self::Cell => '◆',
            Self::Block => '►',
        }
    }
}

/// A sci-fi horizontal slider.
///
/// Build with [`Slider::new`], then set the step with [`Slider::step`] and the
/// theme with [`Slider::theme`]. The current value lives in [`SliderState`].
#[derive(Debug, Clone)]
pub struct Slider {
    /// Step size per Left/Right nudge, in normalized `0.0..=1.0` units.
    /// Defaults to [`DEFAULT_STEP`] (0.1).
    pub step: f32,
    /// Track-glyph form. Defaults to [`SliderShape::Bar`].
    pub shape: SliderShape,
    /// Theme whose [`Stylesheet`](ratatui_style::Stylesheet) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Default for Slider {
    fn default() -> Self {
        Self {
            step: DEFAULT_STEP,
            shape: SliderShape::default(),
            theme: Theme::Cyberpunk,
        }
    }
}

impl Slider {
    /// Create a slider with the default step (0.1) and default theme.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the per-nudge step size (normalized `0.0..=1.0`).
    #[must_use]
    pub fn step(mut self, step: f32) -> Self {
        self.step = step;
        self
    }

    /// Set the track-glyph form (see [`SliderShape`]).
    #[must_use]
    pub fn shape(mut self, shape: SliderShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the slider.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Apply a key event to `state`: `Left`/`Right` nudge by `step` (clamped to
    /// `0.0..=1.0`), `Home`/`End` jump to the bounds. Other keys are ignored.
    pub fn handle_key(&self, state: &mut SliderState, key: KeyEvent) {
        match key.code {
            KeyCode::Left => state.value = (state.value - self.step).max(0.0),
            KeyCode::Right => state.value = (state.value + self.step).min(1.0),
            KeyCode::Home => state.value = 0.0,
            KeyCode::End => state.value = 1.0,
            _ => {}
        }
    }
}

/// Mutable state for [`Slider`].
///
/// `value` is the current position in `0.0..=1.0`. It is clamped on render, so
/// an out-of-range value set directly by the app never breaks the layout.
#[derive(Debug, Clone, Default)]
pub struct SliderState {
    /// Current position, `0.0..=1.0` (clamped on render).
    pub value: f32,
}

impl SliderState {
    /// Create a state at the minimum (`value = 0.0`).
    pub fn new() -> Self {
        Self::default()
    }
}

impl StatefulWidget for Slider {
    type State = SliderState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }

        // Defensive clamp — the app may set `value` out of range directly.
        let value = state.value.clamp(0.0, 1.0);

        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();

        // Level by the same thresholds as EnergyGauge (gauge.rs).
        let level = if value >= 0.6 {
            "ok"
        } else if value >= 0.3 {
            "warn"
        } else {
            "alert"
        };
        let bar_style = sheet
            .compute_with(&NodeRef::new("Gauge").classes(&[level]), None, &mut scratch)
            .to_style();
        let empty_style = sheet
            .compute_with(&NodeRef::new("Gauge").classes(&["empty"]), None, &mut scratch)
            .to_style();
        let label_style = sheet
            .compute_with(&NodeRef::new("Gauge").classes(&["label"]), None, &mut scratch)
            .to_style();

        let y = area.y + area.height / 2;
        let right = area.x + area.width;
        let track_w = area.width.saturating_sub(PCT_COLS);
        // Not enough room for even one track cell + the label — draw nothing.
        if track_w == 0 {
            return;
        }

        // Handle position: round the normalized value to a track cell, clamped
        // to the last cell so a full slider keeps its handle in-bounds.
        let handle_pos = ((value * track_w as f32).round() as u16).min(track_w - 1);

        // Filled cells + handle (bar color) left of/at the handle, empty track
        // (empty color) to its right.
        for i in 0..track_w {
            let x = area.x + i;
            if x >= right {
                break;
            }
            let (glyph, style) = if i == handle_pos {
                (self.shape.handle(), bar_style)
            } else if i < handle_pos {
                (self.shape.filled(), bar_style)
            } else {
                (self.shape.track(), empty_style)
            };
            buf[(x, y)].set_char(glyph).set_style(style);
        }

        // Right-aligned percentage, e.g. " 42%".
        let pct = (value * 100.0).round() as u32;
        let pct_text = format!("{pct:>3}%");
        let pct_len = pct_text.chars().count() as u16;
        if pct_len <= area.width {
            let start = right.saturating_sub(pct_len);
            for (i, ch) in pct_text.chars().enumerate() {
                let px = start + i as u16;
                if px >= right {
                    break;
                }
                buf[(px, y)].set_char(ch).set_style(label_style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    const W: u16 = 16;
    const H: u16 = 3;

    /// Zero-modifier `KeyEvent` helper for tests.
    const fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn render(value: f32, theme: Theme) -> Buffer {
        render_shape(value, theme, SliderShape::Bar)
    }

    fn render_shape(value: f32, theme: Theme, shape: SliderShape) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let mut state = SliderState { value };
        StatefulWidget::render(
            Slider::new().shape(shape).theme(theme),
            Rect::new(0, 0, W, H),
            &mut buf,
            &mut state,
        );
        buf
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..W).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()
    }

    #[test]
    fn handle_key_right_advances_by_step() {
        let slider = Slider::new();
        let mut s = SliderState::new();
        slider.handle_key(&mut s, key(KeyCode::Right));
        assert!((s.value - 0.1).abs() < 1e-6, "one Right nudge ≈ step");
    }

    #[test]
    fn handle_key_right_clamps_at_one() {
        let slider = Slider::new();
        let mut s = SliderState::new();
        for _ in 0..20 {
            slider.handle_key(&mut s, key(KeyCode::Right));
        }
        assert_eq!(s.value, 1.0, "repeated Right clamps at 1.0");
    }

    #[test]
    fn handle_key_left_clamps_at_zero() {
        let slider = Slider::new();
        let mut s = SliderState { value: 0.0 };
        slider.handle_key(&mut s, key(KeyCode::Left));
        assert_eq!(s.value, 0.0, "Left from min stays at 0.0");
    }

    #[test]
    fn handle_key_home_end() {
        let slider = Slider::new();
        let mut s = SliderState { value: 0.5 };
        slider.handle_key(&mut s, key(KeyCode::Home));
        assert_eq!(s.value, 0.0);
        slider.handle_key(&mut s, key(KeyCode::End));
        assert_eq!(s.value, 1.0);
    }

    #[test]
    fn value_out_of_range_clamped_on_render() {
        // value=5.0 must clamp to 1.0 on render without panicking — every track
        // cell left of the handle is filled, and the handle sits at the end.
        let buf = render(5.0, Theme::Cyberpunk);
        let y = H / 2;
        // track_w = 16 - 4 = 12; handle at the last track cell (index 11).
        assert!(
            buf[(11, y)].symbol().starts_with(SliderShape::Bar.handle()),
            "clamped-to-full slider has its handle at the last track cell"
        );
    }

    #[test]
    fn handle_position_tracks_value() {
        // value 0.5, track_w 12 → handle at index 6.
        let buf = render(0.5, Theme::Cyberpunk);
        let y = H / 2;
        assert!(
            buf[(6, y)].symbol().starts_with(SliderShape::Bar.handle()),
            "handle sits at the value-proportional column"
        );
    }

    #[test]
    fn filled_cell_uses_level_color() {
        // value 0.5 → warn (≥0.3, <0.6); a filled cell left of the handle is warn.
        let warn = Theme::Cyberpunk.palette().warn.color();
        let buf = render(0.5, Theme::Cyberpunk);
        let y = H / 2;
        assert_eq!(buf[(0, y)].fg, warn, "filled cell should be --warn");
    }

    #[test]
    fn handle_cell_uses_level_color() {
        // value 0.8 → ok (≥0.6); the handle cell is ok.
        let ok = Theme::Cyberpunk.palette().ok.color();
        let buf = render(0.8, Theme::Cyberpunk);
        let y = H / 2;
        let hp = ((0.8_f32 * 12.0_f32).round() as u16).min(11);
        assert_eq!(buf[(hp, y)].fg, ok, "handle should be --ok");
    }

    #[test]
    fn track_cell_uses_muted_color() {
        let muted = Theme::Cyberpunk.palette().muted.color();
        let buf = render(0.5, Theme::Cyberpunk);
        let y = H / 2;
        // The cell just right of the handle (index 7) is empty track → muted.
        assert_eq!(buf[(7, y)].fg, muted, "empty track should be --muted");
    }

    #[test]
    fn percentage_renders_right_aligned() {
        let buf = render(0.42, Theme::Cyberpunk);
        let y = H / 2;
        // The last cell of the row is '%'.
        assert_eq!(buf[(W - 1, y)].symbol(), "%", "rightmost cell is '%'");
        assert!(row_text(&buf, y).contains("42"), "value 42 labels the slider");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let mut state = SliderState { value: 0.5 };
        StatefulWidget::render(Slider::new(), Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }

    #[test]
    fn non_default_shape_changes_filled_glyph() {
        // Cell shape: filled cells are '▰', not the Bar '═'.
        let buf = render_shape(0.5, Theme::Cyberpunk, SliderShape::Cell);
        let y = H / 2;
        assert!(
            buf[(0, y)].symbol().starts_with(SliderShape::Cell.filled()),
            "Cell filled cell should be '▰'"
        );
        assert!(
            !buf[(0, y)].symbol().starts_with(SliderShape::Bar.filled()),
            "must not use the Bar filled glyph"
        );
    }
}
