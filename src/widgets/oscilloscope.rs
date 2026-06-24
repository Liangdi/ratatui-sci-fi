//! **Oscilloscope** — a live waveform trace.
//!
//! A single-channel oscilloscope that draws a glowing waveform (sine / square
//! / saw / triangle) on a Braille canvas and scrolls it leftward as the app
//! ticks. Where [`crate::StripChart`] is a multi-trace rolling medical monitor,
//! [`Oscilloscope`] is one focused signal with a higher-resolution trace.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]: `freq`/`amplitude`/`shape` are configuration;
//!   only the scroll clock lives in [`OscilloscopeState`].
//! - Drawn on a Braille sub-pixel grid (2×4 dots per cell): each pixel column
//!   samples the waveform once, and the lit dot is packed into its cell's
//!   Braille glyph. Colors come straight off the [`Palette`](crate::Palette)
//!   (`accent` for the trace).
//! - All waveform samples stay in `−1..=1`; the trace is centered vertically.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{Oscilloscope, OscilloscopeShape, OscilloscopeState, Theme};
//!
//! let mut state = OscilloscopeState::new();
//! let scope = Oscilloscope::new(OscilloscopeShape::Sine).freq(0.12).theme(Theme::DeepSpace);
//! // each frame: state.tick(); then render with &mut state.
//! ```

use std::f32::consts::TAU;

use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::StatefulWidget};

use crate::Theme;

/// Default frequency (cycles per pixel-column).
const DEFAULT_FREQ: f32 = 0.12;
/// Default amplitude (0.0..=1.0 of half the canvas height).
const DEFAULT_AMPLITUDE: f32 = 0.8;
/// Pixel-columns the trace scrolls per tick.
const SCROLL_PER_TICK: f32 = 0.5;

/// Visual form of an [`Oscilloscope`]: the waveform shape it traces.
///
/// Colors stay on the palette (`accent`), untouched by this enum.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OscilloscopeShape {
    /// A sine wave — the default.
    #[default]
    Sine,
    /// A square wave.
    Square,
    /// A sawtooth wave.
    Saw,
    /// A triangle wave.
    Triangle,
}

impl OscilloscopeShape {
    /// Sample the waveform at `phase` (radians), returning a value in `−1..=1`.
    fn sample(self, phase: f32) -> f32 {
        match self {
            Self::Sine => phase.sin(),
            Self::Square => {
                if phase.sin() >= 0.0 {
                    1.0
                } else {
                    -1.0
                }
            }
            Self::Saw => 2.0 * (phase / TAU).fract() - 1.0,
            // |sin| folds the sine into a triangle-ish wave at double frequency.
            Self::Triangle => 2.0 * phase.sin().abs() - 1.0,
        }
    }
}

/// A sci-fi oscilloscope trace.
///
/// Build with [`Oscilloscope::new`] (the shape), then tune [`Oscilloscope::freq`]
/// and [`Oscilloscope::amplitude`].
#[derive(Debug, Clone)]
pub struct Oscilloscope {
    /// Waveform shape.
    pub shape: OscilloscopeShape,
    /// Frequency (cycles per pixel-column). Defaults to [`DEFAULT_FREQ`].
    pub freq: f32,
    /// Amplitude, `0.0..=1.0` of half the canvas height. Defaults to
    /// [`DEFAULT_AMPLITUDE`].
    pub amplitude: f32,
    /// Theme whose [`Palette`](crate::Palette) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Oscilloscope {
    /// Create an oscilloscope tracing `shape`, default freq/amplitude/theme.
    pub fn new(shape: OscilloscopeShape) -> Self {
        Self {
            shape,
            freq: DEFAULT_FREQ,
            amplitude: DEFAULT_AMPLITUDE,
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the waveform frequency (cycles per pixel-column).
    #[must_use]
    pub fn freq(mut self, freq: f32) -> Self {
        self.freq = freq;
        self
    }

    /// Set the amplitude (`0.0..=1.0`).
    #[must_use]
    pub fn amplitude(mut self, amplitude: f32) -> Self {
        self.amplitude = amplitude;
        self
    }

    /// Set the theme used for coloring the trace.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

/// Mutable state for [`Oscilloscope`].
///
/// `tick` is the scroll clock; the app advances it each frame (or calls
/// [`Self::tick`]).
#[derive(Debug, Default, Clone)]
pub struct OscilloscopeState {
    /// Scroll clock, advanced once per frame.
    pub tick: u64,
}

impl OscilloscopeState {
    /// Create a state at tick 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance the scroll clock one tick.
    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }
}

impl StatefulWidget for Oscilloscope {
    type State = OscilloscopeState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }
        // Braille sub-pixel grid: 2 columns × 4 rows per cell.
        let pw = (area.width as usize).saturating_mul(2);
        let ph = (area.height as usize).saturating_mul(4);
        if pw == 0 || ph == 0 {
            return;
        }

        let center = ph as f32 / 2.0;
        let amp = self.amplitude.clamp(0.0, 1.0);
        let scroll = state.tick as f32 * SCROLL_PER_TICK;
        let max_py = (ph - 1) as f32;

        // Sample the waveform once per pixel column → its pixel row.
        let wave_y: Vec<u16> = (0..pw)
            .map(|px| {
                let phase = (px as f32 + scroll) * self.freq;
                let v = self.shape.sample(phase);
                let py = center - v * amp * center;
                py.round().clamp(0.0, max_py) as u16
            })
            .collect();

        let style = Style::new().fg(self.theme.palette().accent.color());

        for row in 0..area.height {
            for col in 0..area.width {
                // Pack the two pixel-columns of this cell into one Braille glyph.
                // Dot bit = sub_x + sub_y * 2 (standard Braille sub-cell layout).
                let mut bits = 0u8;
                for sx in 0..2u16 {
                    let px = (col * 2 + sx) as usize;
                    if px < pw {
                        let wy = wave_y[px] as usize;
                        if wy / 4 == row as usize {
                            let sy = (wy % 4) as u16;
                            bits |= 1u8 << (sx + sy * 2);
                        }
                    }
                }
                if bits != 0 {
                    let ch = char::from_u32(0x2800 + bits as u32).unwrap_or(' ');
                    buf[(area.x + col, area.y + row)].set_char(ch).set_style(style);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 30;
    const H: u16 = 7;

    fn render(shape: OscilloscopeShape, freq: f32, amp: f32, tick: u64, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let mut state = OscilloscopeState { tick };
        StatefulWidget::render(
            Oscilloscope::new(shape).freq(freq).amplitude(amp).theme(theme),
            Rect::new(0, 0, W, H),
            &mut buf,
            &mut state,
        );
        buf
    }

    /// Count cells holding a Braille trace glyph (non-space, non-default).
    fn trace_cells(buf: &Buffer) -> usize {
        let mut n = 0;
        for x in 0..W {
            for y in 0..H {
                let s = buf[(x, y)].symbol();
                if s != " " && s.chars().next().map(|c| c >= '\u{2800}').unwrap_or(false) {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn renders_a_trace() {
        let buf = render(OscilloscopeShape::Sine, 0.12, 0.8, 0, Theme::Cyberpunk);
        assert!(trace_cells(&buf) > 0, "sine wave should light some cells");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let mut state = OscilloscopeState::new();
        StatefulWidget::render(
            Oscilloscope::new(OscilloscopeShape::Sine),
            Rect::new(0, 0, 0, 0),
            &mut buf,
            &mut state,
        );
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }

    #[test]
    fn tick_advances_clock() {
        let mut s = OscilloscopeState::new();
        s.tick();
        assert_eq!(s.tick, 1);
    }

    #[test]
    fn different_shapes_render_differently() {
        // Sine vs Square: a square wave saturates at the rails, so the trace's
        // pixel distribution differs from a sine's.
        let sine = render(OscilloscopeShape::Sine, 0.12, 0.8, 0, Theme::Cyberpunk);
        let square = render(OscilloscopeShape::Square, 0.12, 0.8, 0, Theme::Cyberpunk);
        let sine_row_of = |buf: &Buffer, x: u16| -> u16 {
            (0..H).find(|&y| buf[(x, y)].symbol() != " ").unwrap_or(H)
        };
        // At least one column places the trace on a different row between shapes.
        let differs = (0..W).any(|x| sine_row_of(&sine, x) != sine_row_of(&square, x));
        assert!(differs, "sine and square traces should differ somewhere");
    }

    #[test]
    fn amplitude_zero_is_a_flat_center_line() {
        // amplitude 0 → every sample is the center row; all lit cells share it.
        let buf = render(OscilloscopeShape::Sine, 0.12, 0.0, 0, Theme::Cyberpunk);
        let mut lit_rows = std::collections::HashSet::new();
        for x in 0..W {
            for y in 0..H {
                if buf[(x, y)].symbol() != " " {
                    lit_rows.insert(y);
                }
            }
        }
        assert_eq!(lit_rows.len(), 1, "flat trace lights exactly one row");
    }

    #[test]
    fn trace_uses_accent_color() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let buf = render(OscilloscopeShape::Sine, 0.12, 0.8, 0, Theme::Cyberpunk);
        let mut lit = None;
        'outer: for x in 0..W {
            for y in 0..H {
                if buf[(x, y)].symbol() != " " {
                    lit = Some((x, y));
                    break 'outer;
                }
            }
        }
        let lit = lit.expect("at least one lit cell");
        assert_eq!(buf[lit].fg, accent, "trace should be --accent");
    }

    #[test]
    fn tick_wraps_without_panic() {
        let mut s = OscilloscopeState { tick: u64::MAX };
        s.tick();
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        StatefulWidget::render(
            Oscilloscope::new(OscilloscopeShape::Sine),
            Rect::new(0, 0, W, H),
            &mut buf,
            &mut s,
        );
    }
}
