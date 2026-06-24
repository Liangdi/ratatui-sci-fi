//! **NumberStepper** — sci-fi incremental integer control.
//!
//! A single-row `◂ N ▸` control for stepping an integer within a `[min, max]`
//! range. Left/Down and Right/Up nudge by `step`; Home/End jump to the bounds.
//! Unlike [`crate::Slider`] (whose value carries progress/energy semantics and
//! colors by threshold), a stepper's value is a neutral count — so it renders
//! in the plain foreground with muted markers, not level-colored.
//!
//! ## Spec
//!
//! ```text
//!   ◂ 42 ▸
//! ```
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]: `min`/`max`/`step` are immutable
//!   configuration on the widget struct (convention #3); only `value` is
//!   mutable state, nudged by [`NumberStepper::handle_key`] or set directly by
//!   the app.
//! - Key handling lives on the **widget** because it needs the bounds/step:
//!   `stepper.handle_key(&mut state, key)`.
//! - Styling reuses the [`crate::Value`] (fg) and [`crate::Divider`]-style
//!   `Label` (muted) cascade nodes — `var(--…)`-driven off the palette. The
//!   value text takes `Value`, the `◂`/`▸` markers take `Label`.
//! - All glyphs are width-1.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{NumberStepper, NumberStepperState, Theme};
//! use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
//!
//! let stepper = NumberStepper::new().min(0).max(100).step(5).theme(Theme::DeepSpace);
//! let mut state = NumberStepperState::new();
//! stepper.handle_key(&mut state, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
//! ```

use ratatui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Default lower bound.
pub const DEFAULT_MIN: i64 = 0;
/// Default upper bound.
pub const DEFAULT_MAX: i64 = 100;
/// Default step size.
pub const DEFAULT_STEP: i64 = 1;

/// Visual form of a [`NumberStepper`]'s decrement/increment markers.
///
/// Selects the `(dec, inc)` glyph pair; colors stay on the CSS cascade
/// (reusing `Label`), untouched by this enum. The
/// [`NumberStepperShape::Chevron`] default renders the original `◂`/`▸` look.
///
/// Every marker glyph is Unicode width-1 (see convention #5 at the crate root).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NumberStepperShape {
    /// `◂` decrement, `▸` increment.
    #[default]
    Chevron,
    /// `◀` decrement, `▶` increment.
    Arrow,
    /// `|` decrement, `|` increment.
    Pipe,
}

impl NumberStepperShape {
    /// The decrement (left) marker glyph.
    #[must_use]
    pub const fn dec(self) -> char {
        match self {
            Self::Chevron => '◂',
            Self::Arrow => '◀',
            Self::Pipe => '|',
        }
    }

    /// The increment (right) marker glyph.
    #[must_use]
    pub const fn inc(self) -> char {
        match self {
            Self::Chevron => '▸',
            Self::Arrow => '▶',
            Self::Pipe => '|',
        }
    }
}

/// A sci-fi number stepper.
///
/// Build with [`NumberStepper::new`], then configure the bounds/step and the
/// theme. The current value lives in [`NumberStepperState`].
#[derive(Debug, Clone)]
pub struct NumberStepper {
    /// Inclusive lower bound. Defaults to [`DEFAULT_MIN`] (0).
    pub min: i64,
    /// Inclusive upper bound. Defaults to [`DEFAULT_MAX`] (100).
    pub max: i64,
    /// Step per nudge. Defaults to [`DEFAULT_STEP`] (1).
    pub step: i64,
    /// Marker-glyph form. Defaults to [`NumberStepperShape::Chevron`].
    pub shape: NumberStepperShape,
    /// Theme whose [`Stylesheet`](ratatui_style::Stylesheet) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Default for NumberStepper {
    fn default() -> Self {
        Self {
            min: DEFAULT_MIN,
            max: DEFAULT_MAX,
            step: DEFAULT_STEP,
            shape: NumberStepperShape::default(),
            theme: Theme::Cyberpunk,
        }
    }
}

impl NumberStepper {
    /// Create a stepper over `0..=100`, step 1, default theme.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the inclusive lower bound.
    #[must_use]
    pub fn min(mut self, min: i64) -> Self {
        self.min = min;
        self
    }

    /// Set the inclusive upper bound.
    #[must_use]
    pub fn max(mut self, max: i64) -> Self {
        self.max = max;
        self
    }

    /// Set the per-nudge step size.
    #[must_use]
    pub fn step(mut self, step: i64) -> Self {
        self.step = step;
        self
    }

    /// Set the marker-glyph form (see [`NumberStepperShape`]).
    #[must_use]
    pub fn shape(mut self, shape: NumberStepperShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the stepper.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Apply a key event to `state`: `Left`/`Down` decrement by `step`,
    /// `Right`/`Up` increment, `Home`/`End` jump to `min`/`max`. The value is
    /// clamped to `[min, max]`. Other keys are ignored.
    pub fn handle_key(&self, state: &mut NumberStepperState, key: KeyEvent) {
        match key.code {
            KeyCode::Left | KeyCode::Down => state.value = (state.value - self.step).max(self.min),
            KeyCode::Right | KeyCode::Up => state.value = (state.value + self.step).min(self.max),
            KeyCode::Home => state.value = self.min,
            KeyCode::End => state.value = self.max,
            _ => {}
        }
    }
}

/// Mutable state for [`NumberStepper`].
///
/// `value` is the current integer. It is clamped to `[min, max]` on render, so
/// an out-of-range value set directly by the app never breaks the layout.
#[derive(Debug, Clone, Default)]
pub struct NumberStepperState {
    /// Current value (clamped to the stepper's `[min, max]` on render).
    pub value: i64,
}

impl NumberStepperState {
    /// Create a state at `value = 0`.
    pub fn new() -> Self {
        Self::default()
    }
}

impl StatefulWidget for NumberStepper {
    type State = NumberStepperState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }

        // Defensive clamp — the app may set `value` out of range directly.
        let value = state.value.clamp(self.min, self.max);

        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let value_style = sheet.compute_with(&NodeRef::new("Value"), None, &mut scratch).to_style();
        let marker_style = sheet.compute_with(&NodeRef::new("Label"), None, &mut scratch).to_style();

        let row = area.y + area.height / 2;

        // `dec value inc`, centered. Draw the whole thing in the marker style,
        // then overwrite the value digits with the value style.
        let value_str = value.to_string();
        let content = format!("{} {value_str} {}", self.shape.dec(), self.shape.inc());
        let content_w = content.chars().count() as u16;
        let start = area.x + area.width.saturating_sub(content_w) / 2;

        buf.set_string(start, row, &content, marker_style);

        // The value begins after `{dec} ` = 2 width-1 cells.
        let value_x = start + 2;
        buf.set_string(value_x, row, &value_str, value_style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    const W: u16 = 16;
    const H: u16 = 3;

    const fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn render(value: i64, theme: Theme) -> Buffer {
        render_with(value, theme, NumberStepperShape::Chevron, 0, 100, 1)
    }

    fn render_with(
        value: i64,
        theme: Theme,
        shape: NumberStepperShape,
        min: i64,
        max: i64,
        step: i64,
    ) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let mut state = NumberStepperState { value };
        StatefulWidget::render(
            NumberStepper::new().min(min).max(max).step(step).shape(shape).theme(theme),
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
    fn handle_key_right_increases_by_step() {
        let stepper = NumberStepper::new().step(5);
        let mut s = NumberStepperState::new();
        stepper.handle_key(&mut s, key(KeyCode::Right));
        assert_eq!(s.value, 5, "one Right nudge = step");
    }

    #[test]
    fn handle_key_left_decreases_by_step() {
        let stepper = NumberStepper::new().step(5);
        let mut s = NumberStepperState { value: 20 };
        stepper.handle_key(&mut s, key(KeyCode::Left));
        assert_eq!(s.value, 15);
    }

    #[test]
    fn handle_key_right_clamps_at_max() {
        let stepper = NumberStepper::new().max(10).step(3);
        let mut s = NumberStepperState { value: 9 };
        stepper.handle_key(&mut s, key(KeyCode::Right));
        assert_eq!(s.value, 10, "clamps at max");
    }

    #[test]
    fn handle_key_left_clamps_at_min() {
        let stepper = NumberStepper::new().min(0).step(3);
        let mut s = NumberStepperState { value: 1 };
        stepper.handle_key(&mut s, key(KeyCode::Left));
        assert_eq!(s.value, 0, "clamps at min");
    }

    #[test]
    fn handle_key_home_end() {
        let stepper = NumberStepper::new().min(5).max(50);
        let mut s = NumberStepperState { value: 20 };
        stepper.handle_key(&mut s, key(KeyCode::Home));
        assert_eq!(s.value, 5);
        stepper.handle_key(&mut s, key(KeyCode::End));
        assert_eq!(s.value, 50);
    }

    #[test]
    fn value_out_of_range_clamped_on_render() {
        // value=999, max=100 → clamps to 100 on render, shows "100".
        let buf = render_with(999, Theme::Cyberpunk, NumberStepperShape::Chevron, 0, 100, 1);
        let text = row_text(&buf, H / 2);
        assert!(text.contains("100"), "out-of-range clamps to max on render: {text:?}");
        assert!(!text.contains("999"));
    }

    #[test]
    fn renders_value_and_markers() {
        let buf = render(42, Theme::Cyberpunk);
        let text = row_text(&buf, H / 2);
        assert!(text.contains('◂'), "decrement marker present: {text:?}");
        assert!(text.contains('▸'), "increment marker present: {text:?}");
        assert!(text.contains("42"), "value present: {text:?}");
    }

    #[test]
    fn content_is_centered() {
        let buf = render(42, Theme::Cyberpunk);
        let text = row_text(&buf, H / 2);
        assert!(text.starts_with(' '), "content should be centered: {text:?}");
    }

    #[test]
    fn value_uses_fg_marker_uses_muted() {
        let fg = Theme::Cyberpunk.palette().fg.color();
        let muted = Theme::Cyberpunk.palette().muted.color();
        let buf = render(42, Theme::Cyberpunk);
        let y = H / 2;
        // Find the '◂' marker cell → muted; a digit cell → fg.
        let dec_x = (0..W).find(|&x| buf[(x, y)].symbol() == "◂").expect("dec marker");
        let digit_x = (0..W).find(|&x| buf[(x, y)].symbol() == "4").expect("'4' digit");
        assert_eq!(buf[(dec_x, y)].fg, muted, "marker should be --muted");
        assert_eq!(buf[(digit_x, y)].fg, fg, "value should be --fg");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let mut state = NumberStepperState { value: 5 };
        StatefulWidget::render(NumberStepper::new(), Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }

    #[test]
    fn non_default_shape_changes_markers() {
        let buf = render_with(42, Theme::Cyberpunk, NumberStepperShape::Arrow, 0, 100, 1);
        let text = row_text(&buf, H / 2);
        assert!(text.contains('◀') && text.contains('▶'), "Arrow markers: {text:?}");
        assert!(!text.contains('◂'), "must not use the Chevron glyph: {text:?}");
    }

    #[test]
    fn negative_value_renders() {
        // min < 0 lets the value go negative; the '-' must render too.
        let buf = render_with(-5, Theme::Cyberpunk, NumberStepperShape::Chevron, -10, 10, 1);
        let text = row_text(&buf, H / 2);
        assert!(text.contains("-5"), "negative value renders: {text:?}");
    }
}
