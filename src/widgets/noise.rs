//! **Noise** — a static / snow overlay (lost-signal look).
//!
//! A full-area overlay that sprinkles random block-noise glyphs (`░▒▓█`) over
//! the screen — the "no signal" / interference look. Like
//! [`crate::ScanlineOverlay`] it is an ambient layer rendered last; cells it
//! touches are overwritten, untouched cells keep their content, so `intensity`
//! controls how much of the UI the static obscures.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]: the snow clock lives in [`NoiseState`].
//! - Each cell's noise is derived from a hash of `(x, y, tick)`; [`NoiseShape`]
//!   decides whether `tick` participates ([`Snow`](NoiseShape::Snow) → animated
//!   TV snow) or is pinned to 0 ([`Static`](NoiseShape::Static) → stable
//!   interference). Colors come off the [`Palette`](crate::Palette): most noise
//!   is `fg`, a sparse fraction is `accent` (brighter flecks).
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{Noise, NoiseShape, NoiseState, Theme};
//!
//! let mut state = NoiseState::new();
//! // render last, over the root area:
//! // f.render_stateful_widget(Noise::new().intensity(0.4).theme(theme), f.area(), &mut state);
//! ```

use ratatui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};

use crate::Theme;

/// Default noise intensity (fraction of cells filled).
const DEFAULT_INTENSITY: f32 = 0.5;
/// Knuth's multiplicative hash constant.
const HASH_A: u32 = 2654435761;

/// Noise glyph ramp, lightest → heaviest.
const GLYPHS: [char; 4] = ['░', '▒', '▓', '█'];

/// Visual form of [`Noise`].
///
/// [`Snow`](NoiseShape::Snow) animates every frame (TV static); [`Static`]
/// (NoiseShape::Static) is frozen. Colors stay on the palette either way.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NoiseShape {
    /// Animated snow — the noise re-rolls every tick. The default.
    #[default]
    Snow,
    /// Frozen static — stable across ticks.
    Static,
}

/// A sci-fi noise / snow overlay.
///
/// Build with [`Noise::new`], then set [`Noise::intensity`] and [`Noise::theme`].
#[derive(Debug, Clone)]
pub struct Noise {
    /// Fraction of cells filled with noise, `0.0..=1.0`.
    /// Defaults to [`DEFAULT_INTENSITY`].
    pub intensity: f32,
    /// Snow vs static. Defaults to [`NoiseShape::Snow`].
    pub shape: NoiseShape,
    /// Theme whose [`Palette`](crate::Palette) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Noise {
    /// Create a snow overlay, default intensity and theme.
    pub fn new() -> Self {
        Self {
            intensity: DEFAULT_INTENSITY,
            shape: NoiseShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the noise intensity (`0.0..=1.0`).
    #[must_use]
    pub fn intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity;
        self
    }

    /// Set the snow vs static form (see [`NoiseShape`]).
    #[must_use]
    pub fn shape(mut self, shape: NoiseShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for the noise colors.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Hash a cell + tick into a deterministic u32.
    fn hash(x: u32, y: u32, t: u32) -> u32 {
        let mut h = x.wrapping_mul(HASH_A).wrapping_add(y);
        h = h.wrapping_mul(HASH_A).wrapping_add(t);
        h ^ 0x9E37_79B9
    }
}

impl Default for Noise {
    fn default() -> Self {
        Self::new()
    }
}

/// Mutable state for [`Noise`].
///
/// `tick` drives [`Snow`](NoiseShape::Snow); the app advances it each frame.
#[derive(Debug, Default, Clone)]
pub struct NoiseState {
    /// Animation clock, advanced once per frame.
    pub tick: u64,
}

impl NoiseState {
    /// Create a state at tick 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance the clock one tick.
    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }
}

impl StatefulWidget for Noise {
    type State = NoiseState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }
        let intensity = self.intensity.clamp(0.0, 1.0);
        if intensity == 0.0 {
            return;
        }
        let t = if matches!(self.shape, NoiseShape::Snow) { state.tick as u32 } else { 0 };
        let fg = self.theme.palette().fg.color();
        let accent = self.theme.palette().accent.color();

        for row in 0..area.height {
            for col in 0..area.width {
                let h = Self::hash(col as u32, row as u32, t);
                let density = (h % 100) as f32 / 100.0;
                if density < intensity {
                    let glyph = GLYPHS[(h % GLYPHS.len() as u32) as usize];
                    // A sparse fraction of flecks is accent (brighter); rest fg.
                    let color = if h % 8 == 0 { accent } else { fg };
                    let cell = &mut buf[(area.x + col, area.y + row)];
                    cell.set_char(glyph).set_fg(color);
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
    const H: u16 = 10;

    fn render(intensity: f32, shape: NoiseShape, tick: u64, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let mut state = NoiseState { tick };
        StatefulWidget::render(
            Noise::new().intensity(intensity).shape(shape).theme(theme),
            Rect::new(0, 0, W, H),
            &mut buf,
            &mut state,
        );
        buf
    }

    fn noise_count(buf: &Buffer) -> usize {
        let mut n = 0;
        for x in 0..W {
            for y in 0..H {
                let s = buf[(x, y)].symbol();
                if GLYPHS.iter().any(|g| s.starts_with(*g)) {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn intensity_one_fills_everything() {
        let buf = render(1.0, NoiseShape::Static, 0, Theme::Cyberpunk);
        assert_eq!(noise_count(&buf), (W as usize) * (H as usize), "intensity 1 fills all");
    }

    #[test]
    fn intensity_zero_fills_nothing() {
        let buf = render(0.0, NoiseShape::Snow, 5, Theme::Cyberpunk);
        assert_eq!(noise_count(&buf), 0, "intensity 0 fills nothing");
    }

    #[test]
    fn higher_intensity_means_more_noise() {
        let low = render(0.1, NoiseShape::Static, 0, Theme::Cyberpunk);
        let high = render(0.9, NoiseShape::Static, 0, Theme::Cyberpunk);
        assert!(noise_count(&low) < noise_count(&high), "intensity scales noise count");
    }

    #[test]
    fn snow_changes_with_tick() {
        // Static at the same tick is stable; Snow at different ticks differs.
        let a = render(0.5, NoiseShape::Snow, 0, Theme::Cyberpunk);
        let b = render(0.5, NoiseShape::Snow, 7, Theme::Cyberpunk);
        let differs = (0..W).any(|x| (0..H).any(|y| a[(x, y)].symbol() != b[(x, y)].symbol()));
        assert!(differs, "Snow re-rolls across ticks");
    }

    #[test]
    fn static_is_stable_across_ticks() {
        let a = render(0.6, NoiseShape::Static, 0, Theme::Cyberpunk);
        let b = render(0.6, NoiseShape::Static, 99, Theme::Cyberpunk);
        for x in 0..W {
            for y in 0..H {
                assert_eq!(a[(x, y)].symbol(), b[(x, y)].symbol(), "Static stable at ({x},{y})");
            }
        }
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let mut state = NoiseState::new();
        StatefulWidget::render(Noise::new(), Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }

    #[test]
    fn tick_advances_clock() {
        let mut s = NoiseState::new();
        s.tick();
        assert_eq!(s.tick, 1);
    }
}
