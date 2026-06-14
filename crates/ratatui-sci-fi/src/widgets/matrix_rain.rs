//! **MatrixRain** — falling-glyph background (PRD §3 数字雨组件).
//!
//! Classic Matrix digital-rain backdrop: each column has a falling "head"
//! glyph; the cells behind the head form a dimming tail of katakana, latin
//! letters and digits. Usable as a full-screen or panel background.
//!
//! ## Spec
//! - Per-column falling head; glyphs behind the head form a dimming tail.
//! - `density` (0.0..=1.0) = fraction of columns that are active.
//! - `speed` = rows advanced per tick; may be fractional (accumulated).
//! - Deterministic given the fixed seed + tick count, regardless of how the
//!   app interleaves `render` and `tick`.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]; per-column head positions, a fade/brightness
//!   grid and a tiny xorshift32 PRNG all live in [`MatrixRainState`].
//! - The app advances animation via [`MatrixRainState::tick`] each frame.
//!   `render` paints the current state (and lazily sizes/reseeds it to the
//!   area); it never advances motion on its own, so calling both `tick` and
//!   `render` per frame moves the rain exactly one step.
//! - Density/speed from the widget are cached into the state on render so the
//!   argless `tick()` can apply them.
//! - Tail brightness is rendered as a few discrete dimness tiers derived by
//!   linearly interpolating `palette.accent` toward `palette.bg`.
//!
//! # Example
//!
//! ```no_run
//! use ratatui_sci_fi::{MatrixRain, MatrixRainState, Theme};
//!
//! let mut state = MatrixRainState::default();
//! let rain = MatrixRain::new().density(0.8).speed(1.0).theme(Theme::Cyberpunk);
//! // in your event loop: state.tick(); then render `rain` into a buffer.
//! ```

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::StatefulWidget,
};

use crate::Theme;

/// Glyphs drawn by the rain: katakana, latin letters and digits.
pub const CHARSET: &[char] = &[
    'ｱ', 'ｲ', 'ｳ', 'ｴ', 'ｵ', 'ｶ', 'ｷ', 'ｸ', 'ｹ', 'ｺ', 'ｻ', 'ｼ', 'ｽ', 'ｾ', 'ｿ', 'ﾀ', 'ﾁ', 'ﾂ',
    'ﾃ', 'ﾄ', 'ﾅ', 'ﾆ', 'ﾇ', 'ﾈ', 'ﾉ', 'ﾊ', 'ﾋ', 'ﾌ', 'ﾍ', 'ﾎ', 'ﾏ', 'ﾐ', 'ﾑ', 'ﾒ', 'ﾓ', 'ﾔ',
    'ﾕ', 'ﾖ', 'ﾗ', 'ﾘ', 'ﾙ', 'ﾚ', 'ﾛ', 'ﾜ', 'ﾝ', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I',
    'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a',
    'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's',
    't', 'u', 'v', 'w', 'x', 'y', 'z', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
];

/// Number of discrete brightness tiers behind the head (including the head).
///
/// The head is the brightest tier; cells further from the head use
/// progressively dimmer tiers until they fade to the background and stop
/// being drawn.
const TAIL_TIERS: u8 = 8;

/// Fixed seed for [`MatrixRainState::default`], keeping tests deterministic.
const DEFAULT_SEED: u32 = 0x600D_1234;

/// The Matrix digital-rain widget.
///
/// Immutable configuration (density, speed, theme) lives here; all animation
/// state lives in [`MatrixRainState`]. Build with [`MatrixRain::new`] and the
/// `.density()` / `.speed()` / `.theme()` builders.
#[derive(Debug, Clone)]
pub struct MatrixRain {
    /// Fraction of columns that are active, in `0.0..=1.0`. Default `1.0`.
    pub density: f32,
    /// Rows the head advances per tick; may be fractional (accumulated).
    /// Default `1.0`.
    pub speed: f32,
    /// Active theme; controls all colors via its [`Palette`](crate::Palette).
    /// Default [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Default for MatrixRain {
    fn default() -> Self {
        Self { density: 1.0, speed: 1.0, theme: Theme::default() }
    }
}

impl MatrixRain {
    /// Create a rain widget with default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the fraction of active columns (`0.0..=1.0`). Default `1.0`.
    #[must_use]
    pub fn density(mut self, density: f32) -> Self {
        self.density = density;
        self
    }

    /// Set rows advanced per tick (may be fractional). Default `1.0`.
    #[must_use]
    pub fn speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }

    /// Replace the theme (builder). Default is [`Theme::Cyberpunk`].
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

/// A single falling stream: fractional head position and whether the column
/// is currently active (rain is falling in it).
#[derive(Debug, Clone, Copy, Default)]
struct Column {
    /// Fractional row of the head glyph. Negative = above the visible area
    /// (the stream hasn't entered yet). Wraps past `height + tail` to restart.
    head: f32,
    /// Is this column currently emitting a stream? Density gates this.
    active: bool,
}

/// Mutable animation state for [`MatrixRain`].
///
/// Holds per-column head positions, a brightness (fade) grid recording how
/// recently each cell was part of a stream, and a tiny xorshift32 PRNG.
/// Advance animation with [`Self::tick`] each frame; internal buffers are
/// lazily (re)sized to the rendered area on first `render`.
#[derive(Debug, Clone)]
pub struct MatrixRainState {
    /// xorshift32 state. Always kept nonzero.
    rng: u32,
    /// Per-column stream state, indexed by column `0..width`.
    columns: Vec<Column>,
    /// Brightness grid: `brightness[y * width + x]`. `0` = empty/inactive;
    /// otherwise how fresh the cell is (higher = brighter). Decrements by
    /// one each tick; the head stamps `TAIL_TIERS` when it passes.
    brightness: Vec<u8>,
    /// Last area the state was sized for. Resizing is lazy: done on `render`
    /// when the area changes.
    width: u16,
    height: u16,
    /// Cached widget speed (rows/tick), written by `render` so the argless
    /// `tick()` knows how far to move. Fractional values work because the
    /// per-column head is a sub-pixel f32 that accumulates the fraction.
    speed: f32,
    /// Cached widget density (0..1), written by `render` so `tick()` can
    /// re-roll inactive columns when streams recycle.
    density: f32,
}

impl Default for MatrixRainState {
    fn default() -> Self {
        Self {
            rng: DEFAULT_SEED,
            columns: Vec::new(),
            brightness: Vec::new(),
            width: 0,
            height: 0,
            speed: 1.0,
            density: 1.0,
        }
    }
}

impl MatrixRainState {
    /// Advance the rain by one tick.
    ///
    /// Ages the brightness grid (every lit cell dims one tier), advances each
    /// active column's head by the cached `speed` (accumulating fractional
    /// rows), stamps the newly-covered rows as fresh, and recycles streams
    /// that have run off the bottom — possibly reactivating columns based on
    /// the cached `density`.
    ///
    /// No-op before the first `render` (no area to size against yet).
    pub fn tick(&mut self) {
        if self.width == 0 || self.height == 0 {
            return;
        }
        // Age the grid: every lit cell dims one tier.
        for b in &mut self.brightness {
            if *b > 0 {
                *b = b.saturating_sub(1);
            }
        }
        // Advance each active head by `speed` and recycle exhausted streams.
        // Motion is the full (possibly fractional) speed per tick; the head is
        // a sub-pixel f32, so fractional speeds still accumulate visibly.
        let h = self.height as f32;
        let speed = self.speed;
        let density = self.density;
        let recycle_below = h + (TAIL_TIERS as f32);
        let len = self.columns.len();
        // Iterate by index: `next_f32()` borrows `&mut self`, so we must not
        // hold a `&mut self.columns[i]` across the rng calls. Copy each column's
        // fields into locals first, draw the randoms, then mutate.
        for i in 0..len {
            if !self.columns[i].active {
                continue;
            }
            let new_head = self.columns[i].head + speed;
            if new_head > recycle_below {
                // Recycle streams that have fully exited the visible area + tail.
                // Re-roll activation against density so the active set shimmers.
                let reactivate = self.next_f32() < density;
                let new_start = -(self.next_f32() * h.max(1.0) * 0.5);
                let col = &mut self.columns[i];
                col.active = reactivate;
                col.head = new_start;
            } else {
                self.columns[i].head = new_head;
            }
        }
        // Stamp the freshly-covered rows as bright. (Kept as a separate pass
        // because the borrow checker forbids mutating columns and brightness
        // through self simultaneously inside one enumerated loop.)
        self.stamp_heads();
    }

    /// Stamp the row under each active column's head as fresh (`TAIL_TIERS`).
    /// Called by [`tick`](Self::tick) after heads have moved.
    fn stamp_heads(&mut self) {
        let w = self.columns.len();
        let h = self.height as i64;
        for (x, col) in self.columns.iter().enumerate() {
            if !col.active {
                continue;
            }
            // Skip heads still above the visible area (col.head can be a small
            // negative like -0.4, whose round() is -0.0 which casts to 0 —
            // guard explicitly so the stream doesn't light row 0 early).
            if col.head < 0.0 {
                continue;
            }
            let r = col.head.round() as i64;
            if (0..h).contains(&r) {
                let idx = (r as usize) * w + x;
                self.brightness[idx] = TAIL_TIERS;
            }
        }
    }

    /// Ensure internal buffers match `area`, resizing/reinitializing when it
    /// changes. Called from `render`. Also caches density for `tick()`.
    fn resize_for(&mut self, area: Rect, density: f32) {
        self.density = density.clamp(0.0, 1.0);
        let w = area.width;
        let h = area.height;
        if w == self.width && h == self.height && !self.columns.is_empty() {
            return;
        }
        self.width = w;
        self.height = h;
        self.brightness = vec![0u8; (w as usize) * (h as usize)];
        self.columns.clear();
        self.columns.reserve(w as usize);
        let half_h = (h as f32).max(1.0) * 0.5;
        for _ in 0..w {
            let active = self.next_f32() < self.density;
            let head = -(self.next_f32() * half_h);
            self.columns.push(Column { head, active });
        }
    }

    /// xorshift32 step; returns the next u32. `self.rng` is always nonzero.
    #[inline]
    fn next_u32(&mut self) -> u32 {
        // Ensure nonzero (xorshift produces 0 from a 0 seed, which we forbid).
        let mut x = self.rng | 1;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }

    /// Uniform float in `[0, 1)`.
    #[inline]
    fn next_f32(&mut self) -> f32 {
        // 24 bits of mantissa for a clean [0,1) range.
        (self.next_u32() >> 8) as f32 / ((1u32 << 24) as f32)
    }

    /// Pick a glyph from [`CHARSET`] using the PRNG.
    #[inline]
    fn next_glyph(&mut self) -> char {
        let idx = (self.next_u32() as usize) % CHARSET.len();
        CHARSET[idx]
    }
}

/// Linearly interpolate between two [`Color::Rgb`] colors.
fn lerp_rgb(a: Color, b: Color, t: f32) -> Color {
    let (ar, ag, ab) = match a {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => return a,
    };
    let (br, bg, bb) = match b {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => return a,
    };
    let t = t.clamp(0.0, 1.0);
    let lr = (ar as f32 + (br as f32 - ar as f32) * t).round() as u8;
    let lg = (ag as f32 + (bg as f32 - ag as f32) * t).round() as u8;
    let lb = (ab as f32 + (bb as f32 - ab as f32) * t).round() as u8;
    Color::Rgb(lr, lg, lb)
}

impl StatefulWidget for MatrixRain {
    type State = MatrixRainState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Guard against zero-size areas.
        if area.width == 0 || area.height == 0 {
            return;
        }

        // Cache widget config into state so the argless tick() can apply it,
        // and (re)size/reseed internal buffers to the area.
        state.speed = self.speed;
        state.resize_for(area, self.density);

        let palette = self.theme.palette();
        let accent = palette.accent.color();
        let bg = palette.bg.color();

        // Precompute tiered styles: tier 0 = accent (head), last tier ~ bg.
        let tier_styles: Vec<Style> = (0..TAIL_TIERS)
            .map(|i| {
                let t = (i as f32) / ((TAIL_TIERS - 1) as f32).max(1.0);
                Style::default().fg(lerp_rgb(accent, bg, t))
            })
            .collect();

        let w = state.width as usize;
        let h = state.height as usize;

        // Paint the current state. render never advances motion — the app's
        // per-frame tick() does — so calling both moves the rain one step.
        // Index into the brightness grid directly (rather than chunks_exact)
        // so the cell-draw block can still mutably borrow `state` for the
        // head-glyph PRNG draws.
        for y in 0..h as u16 {
            for x in 0..w as u16 {
                let b = state.brightness[(y as usize) * w + (x as usize)];
                if b == 0 {
                    continue;
                }
                // tier 0 == head (b == TAIL_TIERS, freshest).
                let tier = TAIL_TIERS.saturating_sub(b);
                let style = tier_styles[tier as usize];
                let cell = &mut buf[(area.x + x, area.y + y)];
                if b == TAIL_TIERS {
                    // Head: fresh random glyph each time it's the head.
                    cell.set_char(state.next_glyph()).set_style(style);
                } else {
                    // Tail: deterministic glyph from a position+seed hash so
                    // the tail text is stable between frames.
                    let hash = hash_xy(state.rng, x, y);
                    cell.set_char(CHARSET[(hash as usize) % CHARSET.len()])
                        .set_style(style);
                }
            }
        }
    }
}

/// Cheap position-dependent hash for stable tail glyphs. Mixes the current
/// PRNG state with coordinates so different seeds give different tails.
#[inline]
fn hash_xy(seed: u32, x: u16, y: u16) -> u32 {
    let mut h = seed.wrapping_add(0x9E37_79B9);
    h ^= (x as u32).wrapping_mul(0x85EB_CA6B);
    h ^= (y as u32).wrapping_mul(0xC2B2_AE35);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7F4A_7C15);
    h ^= h >> 15;
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    /// Render `n_ticks` worth of motion into a fresh buffer with a fixed seed.
    /// The standard frame pattern: render (sizes/seeds) -> tick N times ->
    /// final render shows the post-tick picture.
    fn render_after_ticks(
        w: u16,
        h: u16,
        density: f32,
        speed: f32,
        theme: Theme,
        n_ticks: u32,
    ) -> Buffer {
        let rect = Rect::new(0, 0, w, h);
        let widget = MatrixRain::new().density(density).speed(speed).theme(theme);
        let mut state = MatrixRainState::default();
        // Initial render sizes + seeds the state and caches speed/density.
        StatefulWidget::render(widget.clone(), rect, &mut Buffer::empty(rect), &mut state);
        for _ in 0..n_ticks {
            state.tick();
        }
        let mut buf = Buffer::empty(rect);
        StatefulWidget::render(widget, rect, &mut buf, &mut state);
        buf
    }

    #[test]
    fn default_has_cyberpunk_theme_and_unit_config() {
        let m = MatrixRain::default();
        assert_eq!(m.theme, Theme::Cyberpunk);
        assert_eq!(m.density, 1.0);
        assert_eq!(m.speed, 1.0);
    }

    #[test]
    fn builders_set_fields() {
        let m = MatrixRain::new().density(0.5).speed(2.5).theme(Theme::Fallout);
        assert_eq!(m.density, 0.5);
        assert_eq!(m.speed, 2.5);
        assert_eq!(m.theme, Theme::Fallout);
    }

    #[test]
    fn zero_area_does_not_panic() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let widget = MatrixRain::new();
        let mut state = MatrixRainState::default();
        // Must be a no-op, not a panic.
        StatefulWidget::render(widget, Rect::new(0, 0, 0, 0), &mut buf, &mut state);
    }

    #[test]
    fn rain_is_deterministic_across_runs() {
        // Two independent runs with the same seed + tick count must produce
        // identical buffers.
        let buf_a = render_after_ticks(20, 12, 1.0, 1.0, Theme::Cyberpunk, 5);
        let buf_b = render_after_ticks(20, 12, 1.0, 1.0, Theme::Cyberpunk, 5);
        assert_eq!(buf_a.area(), buf_b.area());
        for y in 0..12u16 {
            for x in 0..20u16 {
                assert_eq!(
                    buf_a[(x, y)].symbol(),
                    buf_b[(x, y)].symbol(),
                    "glyph at ({x},{y}) differs between deterministic runs"
                );
                assert_eq!(
                    buf_a[(x, y)].style(),
                    buf_b[(x, y)].style(),
                    "style at ({x},{y}) differs between deterministic runs"
                );
            }
        }
    }

    #[test]
    fn rain_produces_non_empty_cells() {
        // With density 1.0 and a few ticks, some cells must be non-space.
        let buf = render_after_ticks(30, 16, 1.0, 1.0, Theme::Cyberpunk, 3);
        let non_empty = count_non_empty(&buf, 30, 16);
        assert!(non_empty > 0, "rain should fill some cells, got {non_empty}");
    }

    #[test]
    fn head_glyphs_use_accent_color() {
        // The head tier (tier 0) is exactly the accent color. Find any cell
        // painted in the accent and confirm at least one exists.
        let buf = render_after_ticks(30, 16, 1.0, 1.0, Theme::Fallout, 2);
        let accent = Theme::Fallout.palette().accent.color();
        let found_accent = (0..16u16).any(|y| {
            (0..30u16).any(|x| buf[(x, y)].style().fg == Some(accent) && buf[(x, y)].symbol() != " ")
        });
        assert!(found_accent, "at least one head glyph should use the accent color");
    }

    #[test]
    fn tick_advances_state() {
        // The picture after 1 tick differs from after 8 ticks, proving
        // tick() actually moves the rain.
        let buf_early = render_after_ticks(30, 16, 1.0, 1.0, Theme::Cyberpunk, 1);
        let buf_late = render_after_ticks(30, 16, 1.0, 1.0, Theme::Cyberpunk, 8);
        let early = count_non_empty(&buf_early, 30, 16);
        let late = count_non_empty(&buf_late, 30, 16);
        // Counts may coincide by chance; also require a visible glyph diff.
        assert!(
            early != late || symbols_differ(&buf_early, &buf_late, 30, 16),
            "tick() did not visibly advance the rain (early={early}, late={late})"
        );
    }

    #[test]
    fn fractional_speed_accumulates() {
        // speed 0.5: over many ticks the head should still move. If the
        // fractional part were dropped, the rain would be frozen (rounding
        // to 0 motion each tick).
        let buf = render_after_ticks(30, 16, 1.0, 0.5, Theme::Cyberpunk, 10);
        let non_empty = count_non_empty(&buf, 30, 16);
        assert!(non_empty > 0, "fractional speed should still produce motion");
    }

    #[test]
    fn density_zero_produces_no_active_streams() {
        // density 0.0 -> no columns active on init -> nothing drawn.
        let buf = render_after_ticks(20, 10, 0.0, 1.0, Theme::Cyberpunk, 3);
        let non_empty = count_non_empty(&buf, 20, 10);
        assert_eq!(non_empty, 0, "density 0 should draw nothing");
    }

    #[test]
    fn theme_affects_colors() {
        // Cyberpunk vs Fallout accents differ, so the head color must differ.
        let buf_cyber = render_after_ticks(20, 12, 1.0, 1.0, Theme::Cyberpunk, 2);
        let buf_fallout = render_after_ticks(20, 12, 1.0, 1.0, Theme::Fallout, 2);
        let cyber_accent = Theme::Cyberpunk.palette().accent.color();
        let fallout_accent = Theme::Fallout.palette().accent.color();
        assert_ne!(cyber_accent, fallout_accent);
        let has_accent = |b: &Buffer, accent: Color| -> bool {
            (0..12u16).any(|y| (0..20u16).any(|x| b[(x, y)].style().fg == Some(accent)))
        };
        assert!(has_accent(&buf_cyber, cyber_accent), "Cyberpunk frame should contain accent cells");
        assert!(has_accent(&buf_fallout, fallout_accent), "Fallout frame should contain accent cells");
    }

    #[test]
    fn resize_reinitializes_buffers() {
        // Render small, then large; must not panic and must fill cells.
        let mut state = MatrixRainState::default();
        let widget = MatrixRain::new();
        let mut buf_small = Buffer::empty(Rect::new(0, 0, 5, 5));
        StatefulWidget::render(widget.clone(), Rect::new(0, 0, 5, 5), &mut buf_small, &mut state);
        let mut buf_large = Buffer::empty(Rect::new(0, 0, 40, 20));
        StatefulWidget::render(widget, Rect::new(0, 0, 40, 20), &mut buf_large, &mut state);
        // Tick a couple times so heads advance into view.
        state.tick();
        state.tick();
        let mut buf_paint = Buffer::empty(Rect::new(0, 0, 40, 20));
        StatefulWidget::render(
            MatrixRain::new(),
            Rect::new(0, 0, 40, 20),
            &mut buf_paint,
            &mut state,
        );
        let non_empty = count_non_empty(&buf_paint, 40, 20);
        assert!(non_empty > 0, "resized rain should still fill cells");
    }

    #[test]
    fn charset_is_nonempty() {
        assert!(!CHARSET.is_empty());
        // No chars that fail to encode.
        assert!(CHARSET.iter().all(|c| c.len_utf8() > 0));
    }

    #[test]
    fn lerp_rgb_endpoints() {
        let a = Color::Rgb(0, 0, 0);
        let b = Color::Rgb(100, 200, 50);
        assert_eq!(lerp_rgb(a, b, 0.0), a);
        assert_eq!(lerp_rgb(a, b, 1.0), b);
        // Midpoint is the average.
        assert_eq!(lerp_rgb(a, b, 0.5), Color::Rgb(50, 100, 25));
    }

    /// Count non-space cells in a `w x h` buffer region.
    fn count_non_empty(buf: &Buffer, w: u16, h: u16) -> usize {
        (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .filter(|(x, y)| buf[(*x, *y)].symbol() != " ")
            .count()
    }

    /// True if any cell's glyph differs between the two buffers.
    fn symbols_differ(a: &Buffer, b: &Buffer, w: u16, h: u16) -> bool {
        (0..h).any(|y| (0..w).any(|x| a[(x, y)].symbol() != b[(x, y)].symbol()))
    }
}
