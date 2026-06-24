//! **StarMap** — a twinkling starfield.
//!
//! A scatter of stars that drift in place and twinkle — the deep-space
//! backdrop / navigation chart. Star positions are derived deterministically
//! from a hash of the star index (so they're stable across frames and need no
//! stored state), and a subset twinkle on/off with the tick clock.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]: `density`/`shape` are configuration; only the
//!   twinkle clock lives in [`StarMapState`].
//! - Star count scales with the area (`density` stars per ~100 cells). Each
//!   star's `(x, y)` comes from a Knuth multiplicative hash of its index;
//!   brightness is `hash % 3 == 0` (a third of stars are "bright"), and bright
//!   stars blink on a `(tick + i) % 10` cadence. Colors come off the
//!   [`Palette`](crate::Palette): bright = `accent`, dim = `muted`.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{StarMap, StarMapState, Theme};
//!
//! let mut state = StarMapState::new();
//! let stars = StarMap::new().density(6).theme(Theme::DeepSpace);
//! // each frame: state.tick(); then render with &mut state.
//! ```

use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::StatefulWidget};

use crate::Theme;

/// Default density: stars per ~100 cells.
const DEFAULT_DENSITY: u32 = 5;
/// Knuth's multiplicative hash constant.
const HASH_A: u32 = 2654435761;

/// Visual form of a [`StarMap`]'s bright stars.
///
/// Selects the glyph drawn for a bright (visible) star; dim stars always use
/// `·`, and colors stay on the palette. The [`StarShape::Dot`] default draws
/// `●`.
///
/// Every glyph is Unicode width-1 (see convention #5 at the crate root).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StarShape {
    /// `●` — the default.
    #[default]
    Dot,
    /// `✦`.
    Sparkle,
    /// `+`.
    Plus,
}

impl StarShape {
    /// The bright-star glyph.
    #[must_use]
    pub const fn bright(self) -> char {
        match self {
            Self::Dot => '●',
            Self::Sparkle => '✦',
            Self::Plus => '+',
        }
    }
}

/// A sci-fi starfield.
///
/// Build with [`StarMap::new`], then set [`StarMap::density`] and
/// [`StarMap::theme`].
#[derive(Debug, Clone)]
pub struct StarMap {
    /// Stars per ~100 cells. Defaults to [`DEFAULT_DENSITY`].
    pub density: u32,
    /// Bright-star glyph form. Defaults to [`StarShape::Dot`].
    pub shape: StarShape,
    /// Theme whose [`Palette`](crate::Palette) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl StarMap {
    /// Create a starfield, default density and theme.
    pub fn new() -> Self {
        Self {
            density: DEFAULT_DENSITY,
            shape: StarShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the star density (stars per ~100 cells).
    #[must_use]
    pub fn density(mut self, density: u32) -> Self {
        self.density = density;
        self
    }

    /// Set the bright-star glyph form (see [`StarShape`]).
    #[must_use]
    pub fn shape(mut self, shape: StarShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the stars.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Deterministic hash of a star index.
    fn hash(i: u32) -> u32 {
        i.wrapping_mul(HASH_A).wrapping_add(0x9E37_79B9)
    }
}

impl Default for StarMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Mutable state for [`StarMap`].
///
/// `tick` drives the twinkle; the app advances it each frame (or calls
/// [`Self::tick`]).
#[derive(Debug, Default, Clone)]
pub struct StarMapState {
    /// Twinkle clock, advanced once per frame.
    pub tick: u64,
}

impl StarMapState {
    /// Create a state at tick 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance the twinkle clock one tick.
    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }
}

impl StatefulWidget for StarMap {
    type State = StarMapState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }

        let cells = area.width as u32 * area.height as u32;
        let n = (self.density.saturating_mul(cells) / 100).max(1);

        let accent = self.theme.palette().accent.color();
        let muted = self.theme.palette().muted.color();
        let bright_glyph = self.shape.bright();

        for i in 0..n {
            let h = Self::hash(i);
            let x = (h % area.width as u32) as u16;
            let y = ((h >> 11) % area.height as u32) as u16;
            // A third of stars are "bright"; the rest are always dim dots.
            let is_bright = h % 3 == 0;
            // Bright stars blink on for the first half of each 10-tick cycle.
            let visible = ((state.tick + i as u64) % 10) < 5;
            let (glyph, color) = if is_bright && visible {
                (bright_glyph, accent)
            } else {
                ('·', muted)
            };
            buf[(area.x + x, area.y + y)].set_char(glyph).set_style(Style::new().fg(color));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 40;
    const H: u16 = 12;

    fn render(density: u32, tick: u64, theme: Theme, shape: StarShape) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let mut state = StarMapState { tick };
        StatefulWidget::render(
            StarMap::new().density(density).shape(shape).theme(theme),
            Rect::new(0, 0, W, H),
            &mut buf,
            &mut state,
        );
        buf
    }

    fn has_glyph(buf: &Buffer, ch: char) -> bool {
        (0..W).any(|x| (0..H).any(|y| buf[(x, y)].symbol().starts_with(ch)))
    }

    #[test]
    fn renders_dim_and_bright_stars() {
        // With enough stars, both the dim '·' and at least one bright dot appear
        // at tick 0 (bright stars visible on the first half of the cycle).
        let buf = render(10, 0, Theme::Cyberpunk, StarShape::Dot);
        assert!(has_glyph(&buf, '·'), "dim stars present");
        assert!(has_glyph(&buf, '●'), "bright stars present");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let mut state = StarMapState::new();
        StatefulWidget::render(
            StarMap::new(),
            Rect::new(0, 0, 0, 0),
            &mut buf,
            &mut state,
        );
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }

    #[test]
    fn tick_advances_clock() {
        let mut s = StarMapState::new();
        s.tick();
        assert_eq!(s.tick, 1);
    }

    #[test]
    fn bright_star_color_is_accent() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let buf = render(20, 0, Theme::Cyberpunk, StarShape::Dot);
        let mut lit = None;
        'outer: for x in 0..W {
            for y in 0..H {
                if buf[(x, y)].symbol().starts_with('●') {
                    lit = Some((x, y));
                    break 'outer;
                }
            }
        }
        let lit = lit.expect("at least one bright star at tick 0");
        assert_eq!(buf[lit].fg, accent, "bright star should be --accent");
    }

    #[test]
    fn sparkle_shape_uses_sparkle_glyph() {
        let buf = render(20, 0, Theme::Cyberpunk, StarShape::Sparkle);
        assert!(has_glyph(&buf, '✦'), "Sparkle shape draws '✦'");
    }

    #[test]
    fn positions_are_stable_across_renders() {
        // Same config + tick → identical buffer (deterministic hash).
        let a = render(8, 3, Theme::Cyberpunk, StarShape::Dot);
        let b = render(8, 3, Theme::Cyberpunk, StarShape::Dot);
        for x in 0..W {
            for y in 0..H {
                assert_eq!(a[(x, y)].symbol(), b[(x, y)].symbol(), "stable at ({x},{y})");
            }
        }
    }

    #[test]
    fn tick_wraps_without_panic() {
        let mut s = StarMapState { tick: u64::MAX };
        s.tick();
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        StatefulWidget::render(StarMap::new(), Rect::new(0, 0, W, H), &mut buf, &mut s);
    }
}
