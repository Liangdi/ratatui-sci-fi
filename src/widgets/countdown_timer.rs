//! **CountdownTimer** — urgent MM:SS countdown.
//!
//! A self-destruct / jump-drive countdown that shows remaining time as
//! `MM:SS`, colors it by urgency, and — in its default form — blinks when the
//! count drops to 10 seconds. The app decrements `remaining` (once per second
//! of wall time); the widget drives only the blink clock.
//!
//! ## Spec
//! - `65` → `01:05` (normal, `--fg`)
//! - `20` → `00:20` (warn, `--warn`)
//! - `05` → `00:05` (alert, `--alert`, **blinks** on the default shape)
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]: no mutable configuration on the widget; the
//!   remaining count + blink clock live in [`CountdownTimerState`].
//! - The app advances [`CountdownTimerState::tick`] every frame (drives the
//!   blink) and decrements `remaining` every second of real time (e.g. every
//!   Nth frame in its own loop).
//! - Urgency thresholds: `≤ 10s` → [`Level::Alert`] (+ blink), `≤ 30s` →
//!   [`Level::Warn`], else [`Level::Normal`].
//! - Styling reuses the [`crate::Value`] node + the level's class. All glyphs
//!   are width-1 (ASCII digits + `:`).
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{CountdownTimer, CountdownTimerState, Theme};
//!
//! let mut state = CountdownTimerState::new(30);
//! let widget = CountdownTimer::new().theme(Theme::Bloodmoon);
//! // each frame: state.tick(); and each second: state.remaining = state.remaining.saturating_sub(1);
//! ```

use ratatui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};
use ratatui_style::{ComputeScratch, NodeRef};

use crate::{Level, Theme};
use crate::widgets::list::DEFAULT_CURSOR_PERIOD;

/// `--alert` threshold: at or below this many seconds the timer is urgent.
pub const ALERT_SECS: u64 = 10;
/// `--warn` threshold: at or below this many seconds the timer is cautionary.
pub const WARN_SECS: u64 = 30;

/// Blink behavior for a [`CountdownTimer`].
///
/// Only affects the urgent (`≤ 10s`) phase; colors stay on the CSS cascade.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CountdownTimerShape {
    /// Blink the readout while urgent (`≤ 10s`) — the default.
    #[default]
    Blink,
    /// Never blink (steady readout).
    Steady,
}

/// A sci-fi countdown timer.
///
/// Build with [`CountdownTimer::new`], then set the theme with
/// [`CountdownTimer::theme`]. The remaining count lives in
/// [`CountdownTimerState`].
#[derive(Debug, Clone)]
pub struct CountdownTimer {
    /// Blink behavior while urgent. Defaults to [`CountdownTimerShape::Blink`].
    pub shape: CountdownTimerShape,
    /// Theme whose [`Stylesheet`](ratatui_style::Stylesheet) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Default for CountdownTimer {
    fn default() -> Self {
        Self {
            shape: CountdownTimerShape::default(),
            theme: Theme::Cyberpunk,
        }
    }
}

impl CountdownTimer {
    /// Create a countdown timer, default shape and theme.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the blink behavior (see [`CountdownTimerShape`]).
    #[must_use]
    pub fn shape(mut self, shape: CountdownTimerShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the timer.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// The [`Level`] a given remaining-second count maps to.
    fn level_for(remaining: u64) -> Level {
        if remaining <= ALERT_SECS {
            Level::Alert
        } else if remaining <= WARN_SECS {
            Level::Warn
        } else {
            Level::Normal
        }
    }
}

/// Mutable state for [`CountdownTimer`].
///
/// `remaining` is the seconds left (the app decrements it each second of real
/// time); `blink_tick` is the animation clock (the app — or [`Self::tick`] —
/// advances it each frame).
#[derive(Debug, Clone)]
pub struct CountdownTimerState {
    /// Seconds remaining. The app decrements this each second.
    pub remaining: u64,
    /// Animation clock, advanced once per frame (drives the urgent blink).
    pub blink_tick: u64,
}

impl CountdownTimerState {
    /// Create a state with `remaining` seconds, blink clock at 0.
    pub fn new(remaining: u64) -> Self {
        Self { remaining, blink_tick: 0 }
    }

    /// Advance the blink clock one tick (call every frame).
    pub fn tick(&mut self) {
        self.blink_tick = self.blink_tick.wrapping_add(1);
    }

    /// Whether the countdown has reached zero.
    pub fn done(&self) -> bool {
        self.remaining == 0
    }

    /// Whether the blinking readout is currently visible (shared cadence).
    fn blink_visible(&self) -> bool {
        (self.blink_tick / DEFAULT_CURSOR_PERIOD.max(1)).is_multiple_of(2)
    }
}

impl Default for CountdownTimerState {
    fn default() -> Self {
        Self::new(0)
    }
}

impl StatefulWidget for CountdownTimer {
    type State = CountdownTimerState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }

        let remaining = state.remaining;
        let text = format!("{:02}:{:02}", remaining / 60, remaining % 60);
        let level = Self::level_for(remaining);

        // In Blink shape, the urgent (≤10s, non-zero) readout flashes off on
        // the dark half of the blink cycle — same-width spaces keep it aligned.
        let urgent = remaining > 0 && remaining <= ALERT_SECS;
        let blink_off =
            urgent && matches!(self.shape, CountdownTimerShape::Blink) && !state.blink_visible();
        let display: String = if blink_off { " ".repeat(text.chars().count()) } else { text };

        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let style = sheet
            .compute_with(&NodeRef::new("Value").classes(level.as_classes()), None, &mut scratch)
            .to_style();

        let row = area.y + area.height / 2;
        let w = display.chars().count() as u16;
        let x = area.x + area.width.saturating_sub(w) / 2;

        buf.set_style(area, style);
        buf.set_string(x, row, &display, style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 16;
    const H: u16 = 3;

    fn render(remaining: u64, blink_tick: u64, theme: Theme, shape: CountdownTimerShape) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let mut state = CountdownTimerState { remaining, blink_tick };
        StatefulWidget::render(
            CountdownTimer::new().shape(shape).theme(theme),
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
    fn renders_mmss() {
        let buf = render(65, 0, Theme::Cyberpunk, CountdownTimerShape::Blink);
        assert!(row_text(&buf, H / 2).contains("01:05"), "65s → 01:05");
    }

    #[test]
    fn normal_uses_fg() {
        let fg = Theme::Cyberpunk.palette().fg.color();
        let buf = render(65, 0, Theme::Cyberpunk, CountdownTimerShape::Blink);
        let y = H / 2;
        let x = (0..W).find(|&x| buf[(x, y)].symbol() == "1").expect("'1' present");
        assert_eq!(buf[(x, y)].fg, fg, ">30s should be --fg (normal)");
    }

    #[test]
    fn warn_uses_warn_color() {
        let warn = Theme::Cyberpunk.palette().warn.color();
        let buf = render(20, 0, Theme::Cyberpunk, CountdownTimerShape::Blink);
        let y = H / 2;
        let x = (0..W).find(|&x| buf[(x, y)].symbol() == "2").expect("'2' present");
        assert_eq!(buf[(x, y)].fg, warn, "≤30s should be --warn");
    }

    #[test]
    fn urgent_uses_alert_color() {
        let alert = Theme::Cyberpunk.palette().alert.color();
        let buf = render(5, 0, Theme::Cyberpunk, CountdownTimerShape::Blink);
        let y = H / 2;
        // blink_tick 0 → visible; the '5' digit carries the alert color.
        let x = (0..W).find(|&x| buf[(x, y)].symbol() == "5").expect("'5' present");
        assert_eq!(buf[(x, y)].fg, alert, "≤10s should be --alert");
    }

    #[test]
    fn blink_shape_hides_readout_on_dark_half() {
        // urgent (5s), Blink shape, blink on the dark half → readout blanked.
        let visible = render(5, 0, Theme::Cyberpunk, CountdownTimerShape::Blink);
        let hidden = render(5, DEFAULT_CURSOR_PERIOD, Theme::Cyberpunk, CountdownTimerShape::Blink);
        assert!(row_text(&visible, H / 2).contains('5'), "visible at tick 0");
        assert!(
            !row_text(&hidden, H / 2).contains('5'),
            "digits hidden on the dark half of the blink"
        );
    }

    #[test]
    fn steady_shape_never_blinks() {
        // Steady shape shows the digits even on the dark half.
        let buf = render(5, DEFAULT_CURSOR_PERIOD, Theme::Cyberpunk, CountdownTimerShape::Steady);
        assert!(row_text(&buf, H / 2).contains('5'), "Steady never blanks");
    }

    #[test]
    fn tick_advances_blink_clock() {
        let mut s = CountdownTimerState::new(10);
        s.tick();
        assert_eq!(s.blink_tick, 1);
    }

    #[test]
    fn done_at_zero() {
        let s = CountdownTimerState::new(0);
        assert!(s.done());
        let s = CountdownTimerState::new(5);
        assert!(!s.done());
    }

    #[test]
    fn zero_renders_zero_zero() {
        let buf = render(0, 0, Theme::Cyberpunk, CountdownTimerShape::Blink);
        assert!(row_text(&buf, H / 2).contains("00:00"), "0s → 00:00");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let mut state = CountdownTimerState::new(10);
        StatefulWidget::render(
            CountdownTimer::new(),
            Rect::new(0, 0, 0, 0),
            &mut buf,
            &mut state,
        );
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
