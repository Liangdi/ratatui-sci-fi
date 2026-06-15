//! **HeatGrid** — animated 2D sensor-array heatmap (PRD §3 热力网格).
//!
//! A `rows × cols` grid of sensor cells, each holding an intensity in
//! `0.0..=1.0`. Each cell renders as a single width-1 glyph whose shade and
//! color encode its intensity: cold/dim at `0` → the theme accent at mid →
//! alert red at the hottest tier. The field self-animates via a deterministic
//! ripple/wave oscillator each tick (demo mode) and can be fed cell values
//! directly via [`HeatGridState::set`] (external mode).
//!
//! ## Spec
//! - A grid of `rows × cols` cells, each intensity `0.0..=1.0`.
//! - Per-cell glyph + color encode intensity: a shaded ramp of block glyphs
//!   (or ASCII / dots, per [`HeatShape`]) tinted from `--bg` up to `--accent`,
//!   with the hottest tier escalating to `--alert`.
//! - Demo mode evolves a flowing wave field deterministically each tick; no
//!   RNG. External mode lets the caller write cells directly.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]; the intensity grid + tick clock live in
//!   [`HeatGridState`], advanced by the app's event loop each tick.
//! - **Intensity → color blend.** Like `scifi_radar`, colors interpolate
//!   through a `blend` helper copied from there. The cold/dim endpoint is the
//!   `Heat` base node's `background` (`var(--bg)`); the mid/high endpoint is
//!   `theme.palette().accent.color()` (the documented exception for
//!   canvas/intensity-blend widgets, since the `Heat` base carries no `color`).
//!   Cells at or above the [`HOT_THRESHOLD`] escalate from accent → `--alert`
//!   (`Heat.hot`) for the top sliver, so a fully-hot cell resolves to exactly
//!   `alert`.
//! - **Deterministic wave field.** [`HeatGridState::tick`] sets each cell to
//!   `0.5 + 0.4·sin(0.6·c + 0.3·t)·cos(0.5·r − 0.2·t) + radial`, where the
//!   radial pulse is a slow cosine of the cell's distance from the grid
//!   center. Neighboring cells correlate, so the field reads as a flowing
//!   wave rather than static noise. All values are clamped to `0.0..=1.0`.
//! - **Cell-block mapping.** Each data cell occupies a block of terminal cells
//!   (`cell_w = max(1, area.width / cols)`, `cell_h = max(1, area.height /
//!   rows)`); every terminal cell in that block is written with the glyph +
//!   blended color, like `gauge.rs`'s cell-by-cell buffer writes.
//!
//! # Example
//!
//! ```no_run
//! use ratatui_sci_fi::{HeatGrid, HeatGridState, Theme};
//!
//! let mut state = HeatGridState::new(8, 16);
//! let grid = HeatGrid::new().theme(Theme::Cyberpunk);
//! // in your event loop each frame: state.tick();
//! ```

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::StatefulWidget,
};

use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Intensity at or above which a cell escalates from the accent ramp to the
/// alert tint (`Heat.hot` / `var(--alert)`). A fully-hot cell (`1.0`) resolves
/// to exactly `alert`.
pub const HOT_THRESHOLD: f64 = 0.8;

/// Visual form of a [`HeatGrid`] cell (convention #5 — shape variants).
///
/// Selects the per-cell glyph and how intensity maps onto a glyph ramp. The
/// [`HeatShape::Block`] default reproduces the canonical shaded-block heatmap
/// look. Colors stay on the CSS cascade (intensity → color is handled in
/// render, not by this enum); a shape variant affects glyphs only.
///
/// Every glyph is Unicode display-width 1 (see convention #5 at the crate
/// root), so each cell occupies exactly one terminal column.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HeatShape {
    /// Five-step shaded block ramp `[" ", "░", "▒", "▓", "█"]` indexed by
    /// intensity. `~0` → space, `~1` → full block. This is the default and
    /// reads as the canonical terminal heatmap.
    #[default]
    Block,
    /// Same shaded ramp as [`Block`](Self::Block). Kept as a distinct variant
    /// for API richness; behavior is identical to `Block`.
    Shade,
    /// Nine-step ASCII ramp `[" ", ".", ":", "-", "=", "+", "*", "#", "@"]` —
    /// the classic terminal heatmap look for ASCII-only environments.
    Ascii,
    /// Two-tone dot: `"·"` (U+00B7, width 1) dim vs `"●"` (U+25CF, width 1)
    /// bright, picked by an intensity threshold of `0.5`.
    Dot,
}

impl HeatShape {
    /// The width-1 glyph that represents the given intensity (`0.0..=1.0`)
    /// under this shape variant (convention #5: every glyph is display-width 1).
    #[must_use]
    pub fn glyph(self, intensity: f64) -> &'static str {
        let t = intensity.clamp(0.0, 1.0);
        match self {
            Self::Block | Self::Shade => {
                const RAMP: [&str; 5] = [" ", "░", "▒", "▓", "█"];
                let idx = (t * (RAMP.len() as f64 - 1.0)).round() as usize;
                RAMP[idx.min(RAMP.len() - 1)]
            }
            Self::Ascii => {
                const RAMP: [&str; 9] = [" ", ".", ":", "-", "=", "+", "*", "#", "@"];
                let idx = (t * (RAMP.len() as f64 - 1.0)).round() as usize;
                RAMP[idx.min(RAMP.len() - 1)]
            }
            Self::Dot => {
                if t < 0.5 {
                    "·"
                } else {
                    "●"
                }
            }
        }
    }
}

/// An animated 2D sensor-array heatmap.
///
/// Built with [`HeatGrid::new`]; theme defaults to [`Theme::Cyberpunk`]. The
/// mutable intensity grid + tick clock live in the companion [`HeatGridState`],
/// advanced by the app's event loop each tick.
#[derive(Debug, Clone)]
pub struct HeatGrid {
    /// Grid row count. Default `8`, clamped to `≥ 1`.
    pub rows: usize,
    /// Grid column count. Default `16`, clamped to `≥ 1`.
    pub cols: usize,
    /// Per-cell glyph form (see [`HeatShape`]). Defaults to [`HeatShape::Block`].
    pub shape: HeatShape,
    /// Theme whose palette drives all colors. Default [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Default for HeatGrid {
    fn default() -> Self {
        Self { rows: 8, cols: 16, shape: HeatShape::Block, theme: Theme::Cyberpunk }
    }
}

impl HeatGrid {
    /// Create a heatmap with default dimensions (`8 × 16`).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the grid row count (clamped to `≥ 1`).
    #[must_use]
    pub fn rows(mut self, n: usize) -> Self {
        self.rows = n.max(1);
        self
    }

    /// Set the grid column count (clamped to `≥ 1`).
    #[must_use]
    pub fn cols(mut self, n: usize) -> Self {
        self.cols = n.max(1);
        self
    }

    /// Set the per-cell glyph form (see [`HeatShape`]).
    #[must_use]
    pub fn shape(mut self, s: HeatShape) -> Self {
        self.shape = s;
        self
    }

    /// Set the theme whose palette drives colors.
    #[must_use]
    pub fn theme(mut self, t: Theme) -> Self {
        self.theme = t;
        self
    }
}

/// Mutable state for [`HeatGrid`].
///
/// Holds the row-major intensity grid (`values[r*cols + c]`, each `0.0..=1.0`),
/// the grid dimensions (so render/set can index without the widget), and a
/// monotonic tick counter that drives the demo-mode wave field. The app's
/// event loop calls [`tick`](Self::tick) once per frame for demo mode, or
/// writes cells directly via [`set`](Self::set) for external mode.
#[derive(Debug, Clone)]
pub struct HeatGridState {
    /// Row-major intensity grid `[r*cols + c]`, each value clamped to
    /// `0.0..=1.0`. Kept `pub` for direct test inspection.
    pub values: Vec<f64>,
    /// Grid column count (stored so render/set can index without the widget).
    pub cols: usize,
    /// Grid row count (stored so render/set can index without the widget).
    pub rows: usize,
    /// Monotonic tick counter (drives the demo-mode wave field). Wraps on
    /// overflow via `wrapping_add`.
    pub tick_count: u64,
}

impl HeatGridState {
    /// Create a state for a `rows × cols` grid (both clamped to `≥ 1`).
    ///
    /// Values start at a calm baseline: a soft radial gradient from the grid
    /// center, so the very first frame isn't a flat blank field.
    #[must_use]
    pub fn new(rows: usize, cols: usize) -> Self {
        let rows = rows.max(1);
        let cols = cols.max(1);
        let mut values = vec![0.0_f64; rows * cols];
        let center_r = (rows as f64 - 1.0) / 2.0;
        let center_c = (cols as f64 - 1.0) / 2.0;
        let max_dist = (center_r.hypot(center_c)).max(1.0);
        for r in 0..rows {
            for c in 0..cols {
                let dist = ((r as f64 - center_r).hypot(c as f64 - center_c)) / max_dist;
                // Brighter at the center, fading to ~0.15 at the corners.
                let v = 0.5 * (1.0 - dist) + 0.15;
                values[r * cols + c] = v.clamp(0.0, 1.0);
            }
        }
        Self { values, cols, rows, tick_count: 0 }
    }

    /// External feed: set cell `(r, c)` to `v`, clamped to `0.0..=1.0`.
    /// Out-of-range `(r, c)` is silently ignored (no panic).
    pub fn set(&mut self, r: usize, c: usize, v: f64) {
        if r < self.rows && c < self.cols {
            self.values[r * self.cols + c] = v.clamp(0.0, 1.0);
        }
    }

    /// Read cell `(r, c)`. Returns `0.0` if out of range.
    #[must_use]
    pub fn get(&self, r: usize, c: usize) -> f64 {
        if r < self.rows && c < self.cols {
            self.values[r * self.cols + c]
        } else {
            0.0
        }
    }

    /// Current tick count (monotonic, wrapping).
    #[must_use]
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    /// Demo mode: advance the clock and evolve the field with a deterministic
    /// wave/ripple oscillator.
    ///
    /// Each cell's new intensity combines a pair of traveling sine/cosine waves
    /// (so neighboring cells correlate) and a slow radial pulse from the grid
    /// center. The result is a flowing field rather than static noise. All
    /// values are clamped to `0.0..=1.0`. No RNG.
    pub fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
        let t = self.tick_count as f64;
        let center_r = (self.rows as f64 - 1.0) / 2.0;
        let center_c = (self.cols as f64 - 1.0) / 2.0;
        let max_dist = (center_r.hypot(center_c)).max(1.0);
        for r in 0..self.rows {
            for c in 0..self.cols {
                let wave = 0.4
                    * (0.6 * c as f64 + 0.3 * t).sin()
                    * (0.5 * r as f64 - 0.2 * t).cos();
                let dist = ((r as f64 - center_r).hypot(c as f64 - center_c)) / max_dist;
                // Radial pulse: a slow cosine of distance + time.
                let radial = 0.25 * (std::f64::consts::TAU * (dist - 0.08 * t)).cos();
                let v = 0.5 + wave + radial;
                self.values[r * self.cols + c] = v.clamp(0.0, 1.0);
            }
        }
    }
}

/// Linear blend of two [`Color::Rgb`] values by `t` in `0.0..=1.0`.
///
/// `t = 0.0` → `lo`, `t = 1.0` → `hi`. Adapted from `scifi_radar::blend` (kept
/// in `f64` to match the intensity math); used to ramp each cell's color from
/// the base background up to the accent (and from accent up to alert for the
/// hottest tier).
fn blend(lo: Color, hi: Color, t: f64) -> Color {
    let t = t.clamp(0.0, 1.0);
    match (lo, hi) {
        (Color::Rgb(lr, lg, lb), Color::Rgb(hr, hg, hb)) => {
            let lerp = |a: u8, b: u8| -> u8 {
                let af = f64::from(a);
                let bf = f64::from(b);
                (af + (bf - af) * t).round().clamp(0.0, 255.0) as u8
            };
            Color::Rgb(lerp(lr, hr), lerp(lg, hg), lerp(lb, hb))
        }
        // Fall back to the high color if either side isn't a concrete RGB.
        _ => hi,
    }
}

impl StatefulWidget for HeatGrid {
    type State = HeatGridState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // 1. Guard zero-size areas.
        if area.width == 0 || area.height == 0 {
            return;
        }

        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let palette = self.theme.palette();

        // 2. Resolve heat-ramp endpoint colors.
        //    Cold/dim endpoint = Heat base node's background (var(--bg)),
        //    falling back to palette.bg. Mid/high endpoint = palette.accent
        //    (the documented exception: the Heat base carries no `color`, so
        //    intensity-blend widgets read the palette accent directly, like
        //    scifi_radar). Hottest tier = Heat.hot (var(--alert)).
        let base_bg = sheet
            .compute_with(&NodeRef::new("Heat"), None, &mut scratch)
            .to_style()
            .bg
            .unwrap_or_else(|| palette.bg.color());
        let accent = palette.accent.color();
        let alert = sheet
            .compute_with(&NodeRef::new("Heat").classes(&["hot"]), None, &mut scratch)
            .to_style()
            .fg
            .unwrap_or_else(|| palette.alert.color());

        // The data grid dimensions come from the widget config, but we render
        // against the state's grid (which is what holds the values). Use the
        // widget's requested shape as the upper bound; index the state via
        // `get`, which returns 0.0 out of range — so a mismatch never panics.
        let grid_rows = self.rows.max(1);
        let grid_cols = self.cols.max(1);

        // 3. Map the rows × cols data grid onto the area. Each data cell
        //    occupies a block of terminal cells.
        let cell_w = ((area.width as usize) / grid_cols).max(1) as u16;
        let cell_h = ((area.height as usize) / grid_rows).max(1) as u16;

        let right = area.x + area.width;
        let bottom = area.y + area.height;

        for gr in 0..grid_rows {
            for gc in 0..grid_cols {
                let intensity = state.get(gr, gc).clamp(0.0, 1.0);

                // Intensity → color. Below HOT_THRESHOLD: blend bg → accent.
                // At/above HOT_THRESHOLD: blend accent → alert for the top
                // sliver, so a fully-hot (1.0) cell resolves to exactly alert.
                let color = if intensity >= HOT_THRESHOLD {
                    let hot_t = (intensity - HOT_THRESHOLD) / (1.0 - HOT_THRESHOLD);
                    blend(accent, alert, hot_t)
                } else {
                    blend(base_bg, accent, intensity / HOT_THRESHOLD)
                };

                let glyph = self.shape.glyph(intensity);
                let style = Style::default().fg(color).bg(base_bg);

                // Terminal sub-rect for this data cell.
                let x0 = area.x.saturating_add((gc as u16) * cell_w);
                let y0 = area.y.saturating_add((gr as u16) * cell_h);
                let x1 = (x0 + cell_w).min(right);
                let y1 = (y0 + cell_h).min(bottom);

                let mut y = y0;
                while y < y1 {
                    let mut x = x0;
                    while x < x1 {
                        buf[(x, y)].set_symbol(glyph).set_style(style);
                        x += 1;
                    }
                    y += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    const W: u16 = 24;
    const H: u16 = 8;

    /// Render the grid into a fresh buffer with the given state + widget.
    fn render(state: &mut HeatGridState, widget: HeatGrid, width: u16, height: u16) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        StatefulWidget::render(widget, Rect::new(0, 0, width, height), &mut buf, state);
        buf
    }

    /// Count non-blank cells (symbol != single space).
    fn non_blank(buf: &Buffer) -> usize {
        buf.content.iter().filter(|c| c.symbol() != " ").count()
    }

    #[test]
    fn renders_without_panicking_after_ticks() {
        let mut state = HeatGridState::new(8, 16);
        for _ in 0..10 {
            state.tick();
        }
        let buf = render(&mut state, HeatGrid::new(), W, H);
        assert!(non_blank(&buf) > 0, "heat grid should draw something after ticks");
    }

    #[test]
    fn set_and_get_roundtrip() {
        let mut state = HeatGridState::new(4, 4);
        state.set(1, 2, 0.7);
        assert!((state.get(1, 2) - 0.7).abs() < 1e-9, "get should return what set wrote");
        // Untouched cell reads back the baseline value (not necessarily 0).
        let baseline = state.get(3, 3);
        let _ = baseline;
    }

    #[test]
    fn set_clamps() {
        let mut state = HeatGridState::new(4, 4);
        state.set(0, 0, 5.0);
        assert!((state.get(0, 0) - 1.0).abs() < 1e-9, "5.0 clamps to 1.0");
        state.set(0, 1, -1.0);
        assert!((state.get(0, 1) - 0.0).abs() < 1e-9, "-1.0 clamps to 0.0");
    }

    #[test]
    fn set_out_of_range_is_ignored() {
        let mut state = HeatGridState::new(2, 2);
        // Out-of-range set must not panic and must not mutate anything.
        state.set(5, 5, 0.9);
        state.set(0, 5, 0.9);
        state.set(5, 0, 0.9);
        // get returns 0.0 out of range.
        assert!((state.get(5, 5) - 0.0).abs() < 1e-9);
        assert!((state.get(0, 5) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn tick_advances_clock_and_stays_in_range() {
        let mut state = HeatGridState::new(6, 10);
        assert_eq!(state.tick_count(), 0);
        for _ in 0..200 {
            state.tick();
        }
        assert_eq!(state.tick_count(), 200);
        for r in 0..6 {
            for c in 0..10 {
                let v = state.get(r, c);
                assert!(
                    (0.0..=1.0).contains(&v),
                    "cell ({r},{c}) out of range after ticks: {v}"
                );
            }
        }
    }

    #[test]
    fn hot_cell_uses_alert_tint() {
        // A fully-hot cell must resolve to exactly the alert color.
        let mut state = HeatGridState::new(4, 4);
        // Zero the field, then pin one cell to 1.0.
        for r in 0..4 {
            for c in 0..4 {
                state.set(r, c, 0.0);
            }
        }
        state.set(0, 0, 1.0);

        let widget = HeatGrid::new().rows(4).cols(4).shape(HeatShape::Block);
        let buf = render(&mut state, widget, W, H);

        let alert = Theme::Cyberpunk.palette().alert.color();
        assert_eq!(
            buf[(0, 0)].fg,
            alert,
            "a fully-hot cell must escalate to the alert tint"
        );
    }

    #[test]
    fn cold_cell_uses_dimmest_glyph() {
        // A 0.0 cell renders the dimmest Block glyph — a single space.
        let mut state = HeatGridState::new(4, 4);
        for r in 0..4 {
            for c in 0..4 {
                state.set(r, c, 0.0);
            }
        }
        let widget = HeatGrid::new().rows(4).cols(4).shape(HeatShape::Block);
        let buf = render(&mut state, widget, W, H);
        assert_eq!(buf[(0, 0)].symbol(), " ", "0.0 intensity → dimmest glyph (space)");
    }

    #[test]
    fn shape_variant_changes_glyphs_ascii() {
        // Ascii shape renders an ASCII ramp char (#/@) somewhere when hot.
        let mut state = HeatGridState::new(2, 4);
        for r in 0..2 {
            for c in 0..4 {
                state.set(r, c, 1.0);
            }
        }
        let widget = HeatGrid::new().rows(2).cols(4).shape(HeatShape::Ascii);
        let buf = render(&mut state, widget, W, H);
        let symbols: Vec<&str> = buf.content.iter().map(|c| c.symbol()).collect();
        assert!(
            symbols.contains(&"@"),
            "Ascii shape with hot cells should render '@' somewhere"
        );
    }

    #[test]
    fn shape_variant_changes_glyphs_dot() {
        // Dot shape renders '●' when hot.
        let mut state = HeatGridState::new(2, 4);
        for r in 0..2 {
            for c in 0..4 {
                state.set(r, c, 1.0);
            }
        }
        let widget = HeatGrid::new().rows(2).cols(4).shape(HeatShape::Dot);
        let buf = render(&mut state, widget, W, H);
        let symbols: Vec<&str> = buf.content.iter().map(|c| c.symbol()).collect();
        assert!(
            symbols.contains(&"●"),
            "Dot shape with hot cells should render '●'"
        );
    }

    #[test]
    fn zero_area_does_not_panic() {
        let mut state = HeatGridState::new(4, 4);
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let widget = HeatGrid::new();
        StatefulWidget::render(widget, Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        // No panic == pass.
    }

    #[test]
    fn builder_setters_work() {
        let w = HeatGrid::new()
            .rows(5)
            .cols(9)
            .shape(HeatShape::Ascii)
            .theme(Theme::Weyland);
        assert_eq!(w.rows, 5);
        assert_eq!(w.cols, 9);
        assert_eq!(w.shape, HeatShape::Ascii);
        assert_eq!(w.theme, Theme::Weyland);
    }

    #[test]
    fn builder_clamps_rows_and_cols() {
        let w = HeatGrid::new().rows(0).cols(0);
        assert_eq!(w.rows, 1, "rows clamps to >= 1");
        assert_eq!(w.cols, 1, "cols clamps to >= 1");
    }

    #[test]
    fn default_is_cyberpunk() {
        let w = HeatGrid::default();
        assert_eq!(w.theme, Theme::Cyberpunk);
        assert_eq!(w.rows, 8);
        assert_eq!(w.cols, 16);
    }

    #[test]
    fn default_shape_is_block() {
        let w = HeatGrid::default();
        assert_eq!(w.shape, HeatShape::Block);
        // And the default glyph for max intensity is the full block.
        assert_eq!(HeatShape::Block.glyph(1.0), "█");
    }

    #[test]
    fn non_square_area_renders() {
        let mut state = HeatGridState::new(8, 16);
        for _ in 0..5 {
            state.tick();
        }
        // Wide + short: 60 x 3.
        let buf = render(&mut state, HeatGrid::new(), 60, 3);
        assert!(non_blank(&buf) > 0, "wide-short area should render non-blank cells");
    }

    #[test]
    fn blend_clamps_and_interpolates() {
        let lo = Color::Rgb(0, 0, 0);
        let hi = Color::Rgb(255, 255, 255);
        assert_eq!(blend(lo, hi, 0.0), lo);
        assert_eq!(blend(lo, hi, 1.0), hi);
        let mid = blend(lo, hi, 0.5);
        let Color::Rgb(r, g, b) = mid else {
            panic!("expected Rgb");
        };
        assert!((r as i16 - 128).abs() <= 1, "midpoint ~128, got {r}");
        assert!((g as i16 - 128).abs() <= 1);
        assert!((b as i16 - 128).abs() <= 1);
        // Out-of-range t clamps.
        assert_eq!(blend(lo, hi, -1.0), lo);
        assert_eq!(blend(lo, hi, 2.0), hi);
    }
}
