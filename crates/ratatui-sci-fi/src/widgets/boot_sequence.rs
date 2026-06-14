//! **BootSequence** — retro power-on self-test (PRD §3 开机闪烁组件).
//!
//! ## Spec
//! - Simulates an old computer powering on: lines of boot text appear one by
//!   one (POST / hardware checks), with occasional retro screen flicker.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]; the animation clock + flicker flag live in
//!   [`BootSequenceState`], advanced each tick by the app's event loop.
//! - **Reveal cadence:** at render time the widget computes
//!   `revealed = min(lines.len(), tick / ticks_per_line)`. So at `tick == 0`
//!   zero lines are visible; the i-th line appears once `tick >= i * ticks_per_line`.
//!   Call [`BootSequenceState::tick`] once per frame.
//! - **Flicker cadence:** flicker fires once per [`DEFAULT_FLICKER_PERIOD`]
//!   ticks, lasting exactly one tick. During a flicker tick the whole frame's
//!   foreground is switched to `palette.muted` (a dimmed look reminiscent of a
//!   CRT losing sync). The flag is also exposed as
//!   [`BootSequenceState::flicker`] for callers that want to react to it.
//! - **Line styling:** normal boot lines use `palette.fg`, "ok" / nominal
//!   messages use `palette.ok`, and any line containing the substring `ERROR`
//!   or `FAIL` (case-sensitive, ASCII only) is rendered in `palette.alert`.
//!   This is an intentionally cheap heuristic; for richer classification wrap
//!   the input strings yourself.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::StatefulWidget,
};

use crate::Theme;

/// Default reveal speed: one new boot line every [`DEFAULT_TICKS_PER_LINE`]
/// ticks (≈6 frames at 60 fps ⇒ ~0.1 s per line).
pub const DEFAULT_TICKS_PER_LINE: u64 = 6;

/// Default flicker period (in ticks). Every [`DEFAULT_FLICKER_PERIOD`] ticks
/// the frame flickers (dims) for exactly one tick. Chosen to be prime-ish so
/// the flicker doesn't lock to the reveal beat.
pub const DEFAULT_FLICKER_PERIOD: u64 = 37;

/// A retro boot / power-on self-test sequence.
///
/// Built from an iterator of boot-line strings and an optional [`Theme`]
/// (default [`Theme::Cyberpunk`]). Reveal count and flicker animation live in
/// the companion [`BootSequenceState`], advanced by the app's event loop each
/// tick.
///
/// # Example
///
/// ```no_run
/// use ratatui_sci_fi::{BootSequence, BootSequenceState, Theme};
///
/// let mut state = BootSequenceState::default();
/// let boot = BootSequence::new([
///     "BIOS v2.41 (c) 1987 Weyland-Yutani",
///     "CPU: Z80A @ 3.58MHz ........... OK",
///     "MEMORY TEST: 640K ............. OK",
///     "DISK DRIVE 0: NOT PRESENT",
///     "ERROR: keyboard controller failure",
///     "Boot failed — press any key",
/// ])
/// .theme(Theme::Weyland);
/// // in your event loop: state.tick(); then render `boot`.
/// ```
#[derive(Debug, Clone, Default)]
pub struct BootSequence {
    /// The boot lines, revealed top-to-bottom one per `ticks_per_line` ticks.
    pub lines: Vec<String>,
    /// How many ticks must elapse before the next line is revealed.
    pub ticks_per_line: u64,
    /// Active theme; controls all colors via its [`Palette`](crate::Palette).
    pub theme: Theme,
}

impl BootSequence {
    /// Build a boot sequence from any iterator of stringifiable lines.
    pub fn new(lines: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            lines: lines.into_iter().map(Into::into).collect(),
            ticks_per_line: DEFAULT_TICKS_PER_LINE,
            theme: Theme::default(),
        }
    }

    /// Set how many ticks elapse before each new line is revealed (clamped to
    /// a minimum of 1 so progress is never frozen).
    #[must_use]
    pub fn ticks_per_line(mut self, n: u64) -> Self {
        self.ticks_per_line = n.max(1);
        self
    }

    /// Replace the theme (builder). Default is [`Theme::Cyberpunk`].
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// How many boot lines are currently revealed given a tick value.
    ///
    /// Exposed so tests / callers can predict the layout without rendering.
    /// Documented reveal rule: `min(lines.len(), tick / ticks_per_line)`.
    fn revealed_count(&self, tick: u64) -> usize {
        if self.lines.is_empty() {
            return 0;
        }
        let step = self.ticks_per_line.max(1);
        usize::try_from(tick / step).unwrap_or(usize::MAX).min(self.lines.len())
    }
}

/// Mutable state for [`BootSequence`].
///
/// `tick` is the animation clock (advanced each frame); `flicker` mirrors
/// whether the current frame should be drawn in the dimmed flicker style.
/// Both are updated by [`Self::tick`].
#[derive(Debug, Default, Clone)]
pub struct BootSequenceState {
    /// Animation clock, incremented every tick. Drives reveal + flicker.
    pub tick: u64,
    /// `true` on the single tick within each `DEFAULT_FLICKER_PERIOD` cycle
    /// where the frame should be drawn dimmed. Updated by [`Self::tick`].
    pub flicker: bool,
}

impl BootSequenceState {
    /// Advance the sequence by one tick.
    ///
    /// Increments the internal clock and recomputes the flicker flag.
    /// Call this once per frame, before rendering.
    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        // Flicker fires on the very first tick of each period (i.e. when the
        // clock has just wrapped to a multiple of the period). Using a
        // wrapping counter keeps this correct across the u64 wrap, too.
        self.flicker = self.tick.rem_euclid(DEFAULT_FLICKER_PERIOD) == 0;
    }

    /// Whether a frame rendered right now (for the *current* `tick`) should be
    /// drawn in the dimmed flicker style. Exposed for callers/tests; the render
    /// path uses this directly so its definition is the single source of truth.
    pub fn flicker_active(&self) -> bool {
        self.tick.rem_euclid(DEFAULT_FLICKER_PERIOD) == 0
    }
}

impl StatefulWidget for BootSequence {
    type State = BootSequenceState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Guard zero-size areas and empty line sets — nothing to draw.
        if area.width == 0 || area.height == 0 || self.lines.is_empty() {
            return;
        }

        let palette = self.theme.palette();
        let fg = palette.fg.color();
        let ok = palette.ok.color();
        let alert = palette.alert.color();
        let muted = palette.muted.color();

        // During a flicker tick the whole frame is dimmed to `muted`.
        let flickering = state.flicker_active();

        // Number of lines revealed at this instant.
        let revealed = self.revealed_count(state.tick);
        if revealed == 0 {
            // Nothing visible yet — a flicker on an empty frame is a no-op.
            return;
        }

        // Render visible lines top-to-bottom, one buffer row each. We write
        // cell-by-cell (matching the ScanList pattern) so per-line coloring
        // and the whole-frame flicker dim are both exact, and we never write
        // out of bounds: we stop at the area's bottom and right edges.
        for (i, src) in self.lines.iter().take(revealed).enumerate() {
            let y = area.y.saturating_add(i as u16);
            if y >= area.bottom() {
                break;
            }

            let color = if flickering {
                muted
            } else {
                line_color(src, fg, ok, alert)
            };
            let style = Style::default().fg(color);

            let mut col = area.x;
            for ch in src.chars() {
                if col >= area.right() {
                    break;
                }
                let cell = &mut buf[(col, y)];
                cell.set_char(ch).set_style(style);
                col += 1;
            }
            // Pad the rest of the row with spaces so the styled background
            // fill stays continuous (matches how the rest of the crate
            // treats full-width rows).
            while col < area.right() {
                let cell = &mut buf[(col, y)];
                cell.set_symbol(" ").set_style(style);
                col += 1;
            }
        }
    }
}

/// Pick the foreground color for a single boot line.
///
/// Cheap heuristic (documented at the crate root of this module):
/// - case-sensitive ASCII substring `ERROR` or `FAIL` → `alert`
/// - lines ending in ` OK` (with optional trailing dots/whitespace) → `ok`
/// - otherwise → `fg` (the default passed in)
fn line_color(src: &str, fg: Color, ok: Color, alert: Color) -> Color {
    if src.contains("ERROR") || src.contains("FAIL") {
        alert
    } else {
        // Treat a trailing "OK" (the classic POST check result) as nominal.
        let trimmed = src.trim_end_matches(|c: char| c == '.' || c.is_whitespace());
        if trimmed.ends_with("OK") {
            ok
        } else {
            fg
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    /// Helper: render the boot widget into a fresh buffer and return it.
    fn render(lines: &[&str], tick: u64, theme: Theme, width: u16, height: u16) -> (Buffer, BootSequenceState) {
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        let widget = BootSequence::new(lines.iter().copied()).theme(theme);
        // Advance the state `tick` times so its internal clock matches.
        let mut state = BootSequenceState::default();
        for _ in 0..tick {
            state.tick();
        }
        StatefulWidget::render(widget, Rect::new(0, 0, width, height), &mut buf, &mut state);
        (buf, state)
    }

    #[test]
    fn zero_ticks_shows_no_lines() {
        // At tick 0, revealed = 0/6 = 0, so row 0 should remain the default
        // empty cell (a space). The contract says "≤ 1 line visible".
        let (buf, _) = render(&["BIOS v1", "CPU OK", "MEMORY OK"], 0, Theme::Fallout, 30, 4);
        assert_eq!(buf[(0, 0)].symbol(), " ", "no boot text should be visible at tick 0");
    }

    #[test]
    fn after_many_ticks_all_lines_visible() {
        // 3 lines × 6 ticks/line = 18 ticks to fully reveal; go well past it.
        let (buf, _) = render(&["BIOS v1", "CPU OK", "MEMORY OK"], 200, Theme::Fallout, 30, 4);
        // First chars of each of the three lines should now be in the buffer.
        assert_eq!(buf[(0, 0)].symbol(), "B", "line 0 first char");
        assert_eq!(buf[(0, 1)].symbol(), "C", "line 1 first char");
        assert_eq!(buf[(0, 2)].symbol(), "M", "line 2 first char");
        // Row 3 was never written (only 3 lines).
        assert_eq!(buf[(0, 3)].symbol(), " ", "row beyond last line stays empty");
    }

    #[test]
    fn reveal_progresses_one_line_per_period() {
        let widget = BootSequence::new(["one", "two", "three"]).ticks_per_line(4);
        assert_eq!(widget.revealed_count(0), 0);
        assert_eq!(widget.revealed_count(3), 0, "not enough ticks for line 0 yet");
        assert_eq!(widget.revealed_count(4), 1, "line 0 revealed at tick 4");
        assert_eq!(widget.revealed_count(7), 1);
        assert_eq!(widget.revealed_count(8), 2, "line 1 revealed at tick 8");
        assert_eq!(widget.revealed_count(12), 3, "all three revealed");
        assert_eq!(widget.revealed_count(10_000), 3, "clamped to len");
    }

    #[test]
    fn error_lines_use_alert_color() {
        let (buf, _) = render(&["BIOS OK", "ERROR: disk failure"], 100, Theme::Cyberpunk, 30, 2);
        let alert = Theme::Cyberpunk.palette().alert.color();
        // Line 0 ends in "OK" → ok color; line 1 contains "ERROR" → alert color.
        let ok = Theme::Cyberpunk.palette().ok.color();
        assert_eq!(buf[(0, 0)].style().fg, Some(ok), "trailing-OK line should be ok-colored");
        assert_eq!(buf[(0, 1)].style().fg, Some(alert), "ERROR line should be alert-colored");
    }

    #[test]
    fn fail_lines_use_alert_color() {
        let (buf, _) = render(&["BOOT FAIL"], 100, Theme::Cyberpunk, 30, 1);
        let alert = Theme::Cyberpunk.palette().alert.color();
        assert_eq!(buf[(0, 0)].style().fg, Some(alert));
    }

    #[test]
    fn normal_lines_use_fg_color() {
        let (buf, _) = render(&["BIOS v2.41 (c) Weyland"], 100, Theme::Weyland, 30, 1);
        let fg = Theme::Weyland.palette().fg.color();
        assert_eq!(buf[(0, 0)].style().fg, Some(fg));
    }

    #[test]
    fn flicker_dims_the_whole_frame() {
        // Pick a tick where flicker is active: a multiple of the period.
        let flicker_tick = DEFAULT_FLICKER_PERIOD;
        // Sanity: that tick really is a flicker tick.
        assert!(
            BootSequenceState { tick: flicker_tick, flicker: false }.flicker_active(),
            "test setup: chosen tick must be a flicker tick"
        );

        let palette = Theme::DeepSpace.palette();
        let muted = palette.muted.color();
        let ok = palette.ok.color();

        // Flickering frame: even an "OK" line should be dimmed to muted.
        let (buf_on, _) = render(&["CPU OK"], flicker_tick, Theme::DeepSpace, 20, 1);
        assert_eq!(buf_on[(0, 0)].style().fg, Some(muted), "flicker tick should force muted fg");

        // Non-flickering frame right next to it: normal coloring resumes.
        let (buf_off, _) = render(&["CPU OK"], flicker_tick + 1, Theme::DeepSpace, 20, 1);
        assert_eq!(
            buf_off[(0, 0)].style().fg,
            Some(ok),
            "tick immediately after flicker should restore normal coloring"
        );
    }

    #[test]
    fn tick_advances_clock_and_sets_flicker_flag() {
        let mut s = BootSequenceState::default();
        assert_eq!(s.tick, 0);
        assert!(!s.flicker, "default state should not flicker");

        // Advance until we hit a flicker tick (multiple of the period).
        let mut saw_flicker = false;
        for i in 1..=DEFAULT_FLICKER_PERIOD {
            s.tick();
            // After `i` ticks the clock reads `i`.
            assert_eq!(s.tick, i);
            if s.flicker {
                saw_flicker = true;
                // Flicker only ever lands on multiples of the period.
                assert_eq!(i % DEFAULT_FLICKER_PERIOD, 0, "flicker should only fire on period boundaries");
            }
        }
        assert!(saw_flicker, "should have observed at least one flicker tick");
    }

    #[test]
    fn zero_size_area_is_a_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let widget = BootSequence::new(["a", "b"]).theme(Theme::Cyberpunk);
        let mut state = BootSequenceState { tick: 100, flicker: false };
        // Must not panic.
        StatefulWidget::render(widget, Rect::new(0, 0, 0, 0), &mut buf, &mut state);
    }

    #[test]
    fn empty_lines_renders_nothing() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 4));
        let widget = BootSequence::new(std::iter::empty::<&str>()).theme(Theme::Cyberpunk);
        let mut state = BootSequenceState { tick: 999, flicker: false };
        StatefulWidget::render(widget, Rect::new(0, 0, 10, 4), &mut buf, &mut state);
        assert_eq!(buf[(0, 0)].symbol(), " ");
    }

    #[test]
    fn ticks_per_line_clamped_to_one() {
        let w = BootSequence::new(["a"]).ticks_per_line(0);
        assert_eq!(w.ticks_per_line, 1, "0 ticks/line would freeze the widget; clamp to 1");
    }

    #[test]
    fn theme_builder_changes_colors() {
        // Same content, Fallout theme → its fg color, not Cyberpunk's.
        let (buf, _) = render(&["booting..."], 100, Theme::Fallout, 20, 1);
        let fallout_fg = Theme::Fallout.palette().fg.color();
        assert_eq!(buf[(0, 0)].style().fg, Some(fallout_fg));
        assert_ne!(
            fallout_fg,
            Theme::Cyberpunk.palette().fg.color(),
            "test must actually exercise a different theme"
        );
    }
}
