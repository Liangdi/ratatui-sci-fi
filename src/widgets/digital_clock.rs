//! **DigitalClock** — seven-segment digital readout.
//!
//! A `HH:MM:SS` time display rendered as glowing seven-segment digits — the
//! reactor-core / VCR / flight-deck readout look. The app supplies the time
//! each frame (the library does no I/O); [`DigitalClockState`] only drives the
//! blinking colon.
//!
//! ## Spec
//!
//! ```text
//!    ███     ███        ███     ███        ███     ███
//!    █ █     █ █        █ █     █ █        █ █     █ █
//!    ███     ███        ███     ███        ███     ███
//!    █ █     █ █        █ █     █ █        █ █     █ █
//!    ███     ███   ▌    ███     ███   ▌    ███     ███
//! ```
//! (digits built from `█` on-segments over `░` off-segments; the colon blinks.)
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]: `hours`/`mins`/`secs` are per-frame
//!   configuration on the widget struct (convention #3); only the blink clock
//!   lives in [`DigitalClockState`], advanced each tick by the app.
//! - No `handle_key` — a clock just displays; the app feeds it the time.
//! - The seven-segment digits are pure block drawing (like a `Canvas`), so
//!   colors come straight off the [`Palette`](crate::Palette): on-segments +
//!   colon take `accent`, off-segments take `muted`. The colon blinks on the
//!   shared `DEFAULT_CURSOR_PERIOD` cadence.
//! - [`DigitalClockShape::Segment`] needs ≥20 cols × 5 rows; a smaller area
//!   degrades to [`DigitalClockShape::Plain`] ("HH:MM:SS" text) so it always
//!   shows something.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{DigitalClock, DigitalClockState, Theme};
//!
//! let mut state = DigitalClockState::new();
//! let clock = DigitalClock::new(8, 30, 5).theme(Theme::Fallout);
//! // each frame: state.tick(); render the clock with the current time.
//! ```

use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::StatefulWidget};

use crate::Theme;
use crate::widgets::list::DEFAULT_CURSOR_PERIOD;

/// Total cell footprint of the seven-segment display: 6 digits (3 cols each) +
/// 2 colons (1 col each) = 20 cols wide, 5 rows tall.
const SEG_W: u16 = 20;
const SEG_H: u16 = 5;

/// Seven-segment bitmaps for `0..=9`, indexed `[a, b, c, d, e, f, g]`.
///
/// Segment layout:
/// ```text
///  aaa
/// f   b
///  ggg
/// e   c
///  ddd
/// ```
const SEGMENTS: [[bool; 7]; 10] = [
    [true, true, true, true, true, true, false],    // 0
    [false, true, true, false, false, false, false], // 1
    [true, true, false, true, true, false, true],    // 2
    [true, true, true, true, false, false, true],    // 3
    [false, true, true, false, false, true, true],   // 4
    [true, false, true, true, false, true, true],    // 5
    [true, false, true, true, true, true, true],     // 6
    [true, true, true, false, false, false, false],  // 7
    [true, true, true, true, true, true, true],      // 8
    [true, true, true, true, false, true, true],     // 9
];

/// Map a `(row, col)` cell within one 3×5 digit to its segment index
/// (`0=a … 6=g`), or `None` for the gap columns (col 1 of rows 1 and 3).
fn cell_segment(row: u16, col: u16) -> Option<usize> {
    match (row, col) {
        (0, _) => Some(0), // a (top)
        (1, 0) => Some(5), // f (top-left)
        (1, 2) => Some(1), // b (top-right)
        (2, _) => Some(6), // g (middle)
        (3, 0) => Some(4), // e (bottom-left)
        (3, 2) => Some(2), // c (bottom-right)
        (4, _) => Some(3), // d (bottom)
        _ => None,
    }
}

/// Visual form of a [`DigitalClock`].
///
/// - [`Segment`](DigitalClockShape::Segment): the seven-segment block look.
/// - [`Plain`](DigitalClockShape::Plain): a simple `HH:MM:SS` text string.
///
/// Both take the accent color; the segment form additionally dims off-segments
/// with `muted`. The segment form degrades to plain when the area is too small.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DigitalClockShape {
    /// Seven-segment block digits (needs ≥20×5; degrades to plain if smaller).
    #[default]
    Segment,
    /// Plain `HH:MM:SS` text.
    Plain,
}

/// A sci-fi digital clock readout.
///
/// Build with [`DigitalClock::new`] (hours, minutes, seconds), then set the
/// theme with [`DigitalClock::theme`]. The colon-blink clock lives in
/// [`DigitalClockState`].
#[derive(Debug, Clone)]
pub struct DigitalClock {
    /// Hours (0..=23 expected; only the last two digits render).
    pub hours: u32,
    /// Minutes (0..=59 expected).
    pub mins: u32,
    /// Seconds (0..=59 expected).
    pub secs: u32,
    /// Display form. Defaults to [`DigitalClockShape::Segment`].
    pub shape: DigitalClockShape,
    /// Theme whose [`Palette`](crate::Palette) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl DigitalClock {
    /// Create a clock showing `hours:mins:secs`, default theme.
    pub fn new(hours: u32, mins: u32, secs: u32) -> Self {
        Self {
            hours,
            mins,
            secs,
            shape: DigitalClockShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the display form (see [`DigitalClockShape`]).
    #[must_use]
    pub fn shape(mut self, shape: DigitalClockShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the clock.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Whether the colon is currently visible (blinks on the shared cadence).
    fn colon_on(state: &DigitalClockState) -> bool {
        (state.tick / DEFAULT_CURSOR_PERIOD.max(1)).is_multiple_of(2)
    }

    /// Render the seven-segment form. Caller guarantees the area fits.
    fn render_segment(&self, area: Rect, buf: &mut Buffer, state: &DigitalClockState) {
        let p = self.theme.palette();
        let on_style = Style::new().fg(p.accent.color());
        let off_style = Style::new().fg(p.muted.color());

        let start_x = area.x + area.width.saturating_sub(SEG_W) / 2;
        let start_y = area.y + area.height.saturating_sub(SEG_H) / 2;

        // Six digit values; % 10 keeps each cell index in 0..=9 (defensive —
        // the app may pass out-of-range values).
        let digits = [
            (self.hours / 10) % 10,
            self.hours % 10,
            (self.mins / 10) % 10,
            self.mins % 10,
            (self.secs / 10) % 10,
            self.secs % 10,
        ];
        // Left edge of each digit: H1 H2 : M1 M2 : S1 S2.
        let digit_x: [u16; 6] = [0, 3, 7, 10, 14, 17];
        for (i, &d) in digits.iter().enumerate() {
            draw_digit(buf, start_x + digit_x[i], start_y, d as usize, on_style, off_style);
        }

        // Two colons between the digit pairs.
        let colon_on = Self::colon_on(state);
        draw_colon(buf, start_x + 6, start_y, colon_on, on_style, off_style);
        draw_colon(buf, start_x + 13, start_y, colon_on, on_style, off_style);
    }

    /// Render the plain text form `HH:MM:SS` (accent-colored, blinking colon).
    fn render_plain(&self, area: Rect, buf: &mut Buffer, state: &DigitalClockState) {
        let accent = self.theme.palette().accent.color();
        let sep = if Self::colon_on(state) { ':' } else { ' ' };
        let text = format!("{:02}{sep}{:02}{sep}{:02}", self.hours, self.mins, self.secs);
        let style = Style::new().fg(accent);
        let row = area.y + area.height / 2;
        let w = text.chars().count() as u16;
        let x = area.x + area.width.saturating_sub(w) / 2;
        buf.set_string(x, row, &text, style);
    }
}

/// Mutable state for [`DigitalClock`].
///
/// `tick` drives the blinking colon; the app advances it each frame (or calls
/// [`Self::tick`]).
#[derive(Debug, Default, Clone)]
pub struct DigitalClockState {
    /// Animation clock, advanced once per frame.
    pub tick: u64,
}

impl DigitalClockState {
    /// Create a state at tick 0 (colon visible).
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance the clock one tick.
    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }
}

impl StatefulWidget for DigitalClock {
    type State = DigitalClockState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }
        match self.shape {
            DigitalClockShape::Segment if area.width >= SEG_W && area.height >= SEG_H => {
                self.render_segment(area, buf, state);
            }
            _ => self.render_plain(area, buf, state),
        }
    }
}

/// Draw one seven-segment digit with its top-left corner at `(x, y)`. On-segments
/// take `on_style` (`█`), off-segments take `off_style` (`░`).
fn draw_digit(buf: &mut Buffer, x: u16, y: u16, digit: usize, on_style: Style, off_style: Style) {
    let seg = SEGMENTS[digit];
    for row in 0..SEG_H {
        for col in 0..3u16 {
            let Some(seg_idx) = cell_segment(row, col) else {
                continue;
            };
            let on = seg[seg_idx];
            let (glyph, style) = if on { ('█', on_style) } else { ('░', off_style) };
            buf[(x + col, y + row)].set_char(glyph).set_style(style);
        }
    }
}

/// Draw a blinking colon (two dots) in the column at `x`.
fn draw_colon(buf: &mut Buffer, x: u16, y: u16, on: bool, on_style: Style, off_style: Style) {
    let (glyph, style) = if on { ('█', on_style) } else { ('░', off_style) };
    buf[(x, y + 1)].set_char(glyph).set_style(style);
    buf[(x, y + 3)].set_char(glyph).set_style(style);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    /// Wide/tall enough for the 20×5 segment form: start_x = (24-20)/2 = 2,
    /// start_y = (7-5)/2 = 1.
    const W: u16 = 24;
    const H: u16 = 7;

    fn render(h: u32, m: u32, s: u32, theme: Theme, tick: u64, shape: DigitalClockShape) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let mut state = DigitalClockState { tick };
        StatefulWidget::render(
            DigitalClock::new(h, m, s).shape(shape).theme(theme),
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
    fn segment_renders_on_segment_for_digit_with_top() {
        // H1 = hours/10 = 0; digit 0 lights segment a (top row). start_x=2,
        // start_y=1, so cell (2, 1) is the 'a' segment → '█'.
        let buf = render(0, 0, 0, Theme::Cyberpunk, 0, DigitalClockShape::Segment);
        assert_eq!(buf[(2, 1)].symbol(), "█", "digit 0's top segment is on");
    }

    #[test]
    fn segment_dims_off_segment_for_digit_one() {
        // H1 = 1 (render 10:00:00); digit 1 has NO top segment → '░'.
        let buf = render(10, 0, 0, Theme::Cyberpunk, 0, DigitalClockShape::Segment);
        assert_eq!(buf[(2, 1)].symbol(), "░", "digit 1's top segment is off");
    }

    #[test]
    fn segment_on_uses_accent_off_uses_muted() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let muted = Theme::Cyberpunk.palette().muted.color();
        // render 10:00:00: H1=1 (top off → muted), H2=0 (top on → accent).
        // H2 left edge = start_x(2) + digit_x[1](3) = 5.
        let buf = render(10, 0, 0, Theme::Cyberpunk, 0, DigitalClockShape::Segment);
        assert_eq!(buf[(2, 1)].fg, muted, "off segment is --muted");
        assert_eq!(buf[(5, 1)].fg, accent, "on segment is --accent");
    }

    #[test]
    fn segment_colon_blinks() {
        // Colon 1 sits at start_x(2) + 6 = 8; its dots are at rows y+1, y+3.
        let on = render(0, 0, 0, Theme::Cyberpunk, 0, DigitalClockShape::Segment);
        let off = render(0, 0, 0, Theme::Cyberpunk, DEFAULT_CURSOR_PERIOD, DigitalClockShape::Segment);
        assert_eq!(on[(8, 2)].symbol(), "█", "colon on at tick 0");
        assert_eq!(off[(8, 2)].symbol(), "░", "colon dims at tick = period");
    }

    #[test]
    fn plain_renders_time_string() {
        let buf = render(8, 30, 5, Theme::Cyberpunk, 0, DigitalClockShape::Plain);
        let text = row_text(&buf, H / 2);
        assert!(text.contains("08:30:05"), "plain form shows the time: {text:?}");
    }

    #[test]
    fn plain_uses_accent_color() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let buf = render(8, 30, 5, Theme::Cyberpunk, 0, DigitalClockShape::Plain);
        let y = H / 2;
        let x = (0..W).find(|&x| buf[(x, y)].symbol() == "0").expect("'0' present");
        assert_eq!(buf[(x, y)].fg, accent, "plain text is --accent");
    }

    #[test]
    fn plain_colon_blinks_to_space() {
        // At tick = period the colon turns into a space.
        let on = render(8, 30, 5, Theme::Cyberpunk, 0, DigitalClockShape::Plain);
        let off = render(8, 30, 5, Theme::Cyberpunk, DEFAULT_CURSOR_PERIOD, DigitalClockShape::Plain);
        assert!(row_text(&on, H / 2).contains(':'), "colon visible at tick 0");
        assert!(
            !row_text(&off, H / 2).contains(':'),
            "colon hidden at tick = period"
        );
    }

    #[test]
    fn segment_degrades_to_plain_when_too_small() {
        // A 10×3 area can't fit the 20×5 segment form → degrades to plain text.
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 3));
        let mut state = DigitalClockState::new();
        StatefulWidget::render(
            DigitalClock::new(8, 30, 5).shape(DigitalClockShape::Segment).theme(Theme::Cyberpunk),
            Rect::new(0, 0, 10, 3),
            &mut buf,
            &mut state,
        );
        // Read only the buffer's actual width (10), not the module's W (24).
        let text: String = (0..buf.area().width)
            .map(|x| buf[(x, 1)].symbol().to_string())
            .collect();
        assert!(text.contains("08:30:05"), "degrades to plain when too small: {text:?}");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let mut state = DigitalClockState::new();
        StatefulWidget::render(
            DigitalClock::new(8, 30, 5),
            Rect::new(0, 0, 0, 0),
            &mut buf,
            &mut state,
        );
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }

    #[test]
    fn out_of_range_time_does_not_panic() {
        // 99:99:99 — % 10 keeps each digit in range; must not index past SEGMENTS.
        let buf = render(99, 99, 99, Theme::Cyberpunk, 0, DigitalClockShape::Segment);
        let _ = row_text(&buf, 1);
    }
}
