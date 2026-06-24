//! **DonutChart** — multi-slice proportional ring (PRD §3 仪表盘 / 分布).
//!
//! A sci-fi pie/donut: `N` slices of a ring whose sweep angles are proportional
//! to per-slice values (raw magnitudes `≥ 0`; proportions are derived as
//! `value_i / sum`). Each slice is colored by its index modulo 5
//! (`Donut.slice0..4` → accent / accent2 / ok / warn / alert), so a glance at the
//! ring reads the distribution. The widget is stateful —
//! [`DonutChartState::tick`] gently wanders each slice's value along a
//! per-index sine so the ring breathes in a demo.
//!
//! ## Spec
//! - Draw a circular ring on a ratatui [`Canvas`] using [`Marker::Braille`]
//!   (so curves look crisp), centered in a square sub-area.
//! - Slice `i` sweeps `proportion_i * 2π` starting where slice `i-1` ended. The
//!   first slice starts at angle `0` (rightward, +x) and slices advance
//!   counter-clockwise (toward +y) — the conventional math/CCW direction.
//! - Each slice's arc is approximated by short [`Line`] segments between
//!   successive polar samples (mirrors [`RadialGauge`]'s `Arc` segment loop).
//! - Per-slice color via the CSS cascade: `Donut.slice{i % 5}` with palette
//!   fallbacks accent / accent2 / ok / warn / alert respectively.
//! - [`DonutShape`] selects the geometry: [`DonutShape::Arc`] (default) draws a
//!   single-radius ring; [`DonutShape::Thick`] draws a filled annulus (two
//!   radii connected by radial lines); [`DonutShape::Tick`] stamps discrete rim
//!   ticks along each slice's span.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]; per-slice magnitudes and the tick clock live
//!   in [`DonutChartState`].
//! - [`Marker::Braille`] canvas with `x_bounds`/`y_bounds = [-1.0, 1.0]`; the
//!   unit disk is centered at the canvas origin. The Braille cell aspect makes
//!   the ring look slightly elliptical — that's expected and accepted (the same
//!   trade-off [`RadialGauge`] makes).
//! - All colors resolve through the [`Stylesheet`](crate::Theme::stylesheet)
//!   cascade (`Donut`, `Donut.grid`, `Donut.slice0`..`Donut.slice4`) using a
//!   single [`ComputeScratch`] per render, falling back to [`Theme::palette`]
//!   values.
//!
//! [`RadialGauge`]: crate::RadialGauge
//!
//! # Example
//!
//! ```no_run
//! use ratatui_sci_fi::{DonutChart, DonutChartState, DonutShape, Theme};
//!
//! let mut state = DonutChartState::default();
//! let donut = DonutChart::new()
//!     .slices(5)
//!     .shape(DonutShape::Arc)
//!     .theme(Theme::Cyberpunk);
//! // in your event loop each frame: state.tick();
//! // to drive it externally: state.set_slice(0, 1.4);
//! ```

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Color,
    symbols::Marker,
    widgets::{StatefulWidget, Widget},
    widgets::canvas::{Canvas, Circle, Context, Line, Points},
};

use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Number of short line segments per *full ring* used to approximate slice arcs.
/// A slice's own segment count scales with its sweep so tiny slices still draw
/// at least a couple of segments.
pub const ARC_SEGMENTS: usize = 96;

/// Total rim tick budget for the [`DonutShape::Tick`] shape; each slice stamps
/// `max(2, round(proportion * TICKS_PER_RING))` ticks along its span.
pub const TICKS_PER_RING: usize = 60;

/// Outer radius of the ring.
pub const R_OUTER: f64 = 0.92;
/// Inner radius used by the [`DonutShape::Thick`] annulus.
pub const R_INNER: f64 = 0.62;
/// Radius of the faint full-circle depth rim.
pub const R_RIM: f64 = 0.95;
/// Inner radius of a [`DonutShape::Tick`] tick.
pub const R_TICK_INNER: f64 = 0.85;
/// Outer radius of a [`DonutShape::Tick`] tick.
pub const R_TICK_OUTER: f64 = 0.95;
/// Radius used for the single-line [`DonutShape::Arc`] ring.
pub const R_ARC: f64 = 0.82;

/// Minimum slice magnitude floor so a slice never fully vanishes in demo mode.
const VALUE_FLOOR: f64 = 0.05;

/// How the proportional ring is drawn (config — convention #5).
///
/// This enum selects what gets drawn on the [`Canvas`] in `paint`. Because it
/// is canvas geometry (not glyphs), convention #5's Unicode width-1 rule is
/// about glyph cells and doesn't constrain these variants — but the principle
/// (config lives on the widget, default must look great) still holds. This is a
/// new widget, so the default variant is chosen to look great outright rather
/// than to preserve a prior look.
///
/// Colors stay on the CSS cascade; a shape variant affects geometry only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DonutShape {
    /// A single-radius ring of arc segments, one per slice. The default — a
    /// crisp, readable donut.
    #[default]
    Arc,
    /// A filled annulus: each slice's arc is drawn at two radii (outer ≈0.92,
    /// inner ≈0.62) and connected by radial closing lines, so slices read as a
    /// thick band.
    Thick,
    /// Discrete short rim ticks spaced along each slice's angular span.
    Tick,
}

/// A multi-slice proportional ring (pie/donut).
///
/// Built with [`DonutChart::new`]; theme defaults to [`Theme::Cyberpunk`],
/// shape defaults to [`DonutShape::Arc`], slice count defaults to `5`. The
/// per-slice magnitudes and the tick clock live in the companion
/// [`DonutChartState`], mutated by the app's event loop each tick.
#[derive(Debug, Clone)]
pub struct DonutChart {
    /// Number of slices (default `5`, clamped ≥1).
    pub slices: usize,
    /// Ring geometry form. Default [`DonutShape::Arc`].
    pub shape: DonutShape,
    /// Active theme; drives all colors via its [`Palette`](crate::Palette) /
    /// [`Stylesheet`](crate::Theme::stylesheet). Default [`Theme::Cyberpunk`].
    pub theme: Theme,
    /// Optional short caption drawn below the ring (e.g. `"LOAD"`).
    pub label: Option<String>,
}

impl Default for DonutChart {
    fn default() -> Self {
        Self { slices: 5, shape: DonutShape::default(), theme: Theme::Cyberpunk, label: None }
    }
}

impl DonutChart {
    /// Create a donut with default config (5 slices, Arc shape, Cyberpunk theme).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the number of slices (clamped to at least 1). Builder.
    #[must_use]
    pub fn slices(mut self, n: usize) -> Self {
        self.slices = n.max(1);
        self
    }

    /// Set the ring geometry form (see [`DonutShape`]). Builder.
    #[must_use]
    pub fn shape(mut self, shape: DonutShape) -> Self {
        self.shape = shape;
        self
    }

    /// Replace the theme (builder). Default is [`Theme::Cyberpunk`].
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Attach a short caption drawn below the ring (e.g. `"LOAD"`). Builder.
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// Mutable state for [`DonutChart`].
///
/// Holds a raw magnitude per slice (`≥ 0`) and a tick clock. Proportions are
/// derived (`value_i / sum`). The app's event loop calls
/// [`tick`](Self::tick) once per frame for demo-mode animation, or drives slices
/// externally via [`set_slice`](Self::set_slice).
///
/// The demo oscillator is deterministic (no RNG): each slice wanders along a
/// per-index sine, kept above [`VALUE_FLOOR`] so a slice never fully vanishes.
#[derive(Debug, Clone)]
pub struct DonutChartState {
    /// Raw per-slice magnitudes (`≥ 0`). Proportions are derived on demand.
    values: Vec<f64>,
    /// Monotonic tick counter (drives the demo wander sines + tests).
    tick_count: u64,
}

impl Default for DonutChartState {
    fn default() -> Self {
        Self::new(5)
    }
}

impl DonutChartState {
    /// Create a fresh state with `slices` slices (clamped to at least 1), each
    /// seeded to a distinct positive baseline (`1.0 + i`) so slices are visibly
    /// unequal and proportions sum sanely on the first frame.
    #[must_use]
    pub fn new(slices: usize) -> Self {
        let slices = slices.max(1);
        let values = (0..slices).map(|i| 1.0 + i as f64).collect();
        Self { values, tick_count: 0 }
    }

    /// Advance the demo oscillator by one tick.
    ///
    /// Bumps the tick clock (wrapping) and wanders each slice's value along a
    /// per-index sine, kept above [`VALUE_FLOOR`] so a slice never fully
    /// vanishes. Deterministic — no RNG.
    pub fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
        let t = self.tick_count as f64;
        for (i, v) in self.values.iter_mut().enumerate() {
            // Distinct base frequency + phase per slice; gentle wander.
            let base = 0.20 + 0.13 * (i as f64 + 1.0);
            let phase = (i as f64) * 0.9;
            let wave = (base * t + phase).sin() * 0.5 + (0.07 * t + phase).sin() * 0.2;
            let next = 1.0 + (i as f64) + wave * (1.0 + 0.3 * (i as f64));
            *v = next.max(VALUE_FLOOR);
        }
    }

    /// Set slice `i`'s magnitude (clamped to `≥ 0`). Out-of-range indices are
    /// ignored (no panic).
    pub fn set_slice(&mut self, i: usize, v: f64) {
        if let Some(slot) = self.values.get_mut(i) {
            *slot = v.max(0.0);
        }
    }

    /// Raw magnitude of slice `i` (`0.0` if out of range).
    pub fn value(&self, i: usize) -> f64 {
        self.values.get(i).copied().unwrap_or(0.0)
    }

    /// Proportion of slice `i` (`value_i / sum`, or `0.0` if the sum is `0`).
    pub fn proportion(&self, i: usize) -> f64 {
        let sum: f64 = self.values.iter().sum();
        if sum == 0.0 {
            0.0
        } else {
            self.value(i) / sum
        }
    }

    /// Current tick clock value.
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }
}

/// Polar-to-cartesian helper: `(r*cos(angle), r*sin(angle))`.
fn polar(angle: f64, r: f64) -> (f64, f64) {
    (r * angle.cos(), r * angle.sin())
}

/// Pick the CSS class name for slice `i` (cycles accent / accent2 / ok / warn /
/// alert). Returns a pair of `(class, fallback_index)` where `fallback_index`
/// selects the palette fallback color.
fn slice_class(i: usize) -> (&'static str, usize) {
    match i % 5 {
        0 => ("slice0", 0),
        1 => ("slice1", 1),
        2 => ("slice2", 2),
        3 => ("slice3", 3),
        _ => ("slice4", 4),
    }
}

impl StatefulWidget for DonutChart {
    type State = DonutChartState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // 1. Guard zero-size areas.
        if area.width == 0 || area.height == 0 {
            return;
        }

        // 2. Resolve colors from the cascade with one shared scratch.
        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let palette = self.theme.palette();

        let accent = sheet
            .compute_with(&NodeRef::new("Donut"), None, &mut scratch)
            .to_style()
            .fg
            .unwrap_or_else(|| palette.accent.color());
        let grid_color = sheet
            .compute_with(&NodeRef::new("Donut").classes(&["grid"]), None, &mut scratch)
            .to_style()
            .fg
            .unwrap_or_else(|| palette.muted.color());
        let bg = sheet
            .compute_with(&NodeRef::new("Donut"), None, &mut scratch)
            .to_style()
            .bg
            .unwrap_or_else(|| palette.bg.color());

        // Pre-resolve each slice color once (cycles slice0..slice4).
        let slice_colors: Vec<Color> = (0..self.slices)
            .map(|i| {
                let (cls, fb) = slice_class(i);
                let fallback = match fb {
                    0 => palette.accent.color(),
                    1 => palette.accent2.color(),
                    2 => palette.ok.color(),
                    3 => palette.warn.color(),
                    _ => palette.alert.color(),
                };
                sheet
                    .compute_with(&NodeRef::new("Donut").classes(&[cls]), None, &mut scratch)
                    .to_style()
                    .fg
                    .unwrap_or(fallback)
            })
            .collect();

        // 3. Square sub-area (mirror radial_gauge).
        let side = area.width.min(area.height);
        let canvas_area = Rect::new(area.x, area.y, side, side);

        // 4–5. Paint the ring.
        Canvas::default()
            .marker(Marker::Braille)
            .background_color(bg)
            .x_bounds([-1.0, 1.0])
            .y_bounds([-1.0, 1.0])
            .paint(|ctx| {
                // Faint full-circle depth rim.
                ctx.draw(&Circle { x: 0.0, y: 0.0, radius: R_RIM, color: grid_color });

                // Walk slices: each starts where the previous ended. First slice
                // starts at angle 0 (rightward, +x); slices advance CCW (+y).
                let mut cursor = 0.0_f64;
                for (i, &color) in slice_colors.iter().enumerate() {
                    let prop = state.proportion(i);
                    let sweep = prop * std::f64::consts::TAU;
                    if sweep <= 0.0 {
                        continue;
                    }
                    let a0 = cursor;
                    let a1 = cursor + sweep;

                    match self.shape {
                        DonutShape::Arc => {
                            draw_arc(ctx, a0, a1, R_ARC, color);
                        }
                        DonutShape::Thick => {
                            // Outer + inner arcs connected by radial closing lines.
                            draw_arc(ctx, a0, a1, R_OUTER, color);
                            draw_arc(ctx, a0, a1, R_INNER, color);
                            let (ox0, oy0) = polar(a0, R_OUTER);
                            let (ix0, iy0) = polar(a0, R_INNER);
                            ctx.draw(&Line { x1: ox0, y1: oy0, x2: ix0, y2: iy0, color });
                            let (ox1, oy1) = polar(a1, R_OUTER);
                            let (ix1, iy1) = polar(a1, R_INNER);
                            ctx.draw(&Line { x1: ox1, y1: oy1, x2: ix1, y2: iy1, color });
                        }
                        DonutShape::Tick => {
                            let ticks = ((prop * TICKS_PER_RING as f64).round() as usize).max(2);
                            for k in 0..ticks {
                                let t = if ticks > 1 {
                                    k as f64 / (ticks - 1) as f64
                                } else {
                                    0.0
                                };
                                let a = a0 + sweep * t;
                                let (x1, y1) = polar(a, R_TICK_INNER);
                                let (x2, y2) = polar(a, R_TICK_OUTER);
                                ctx.draw(&Line { x1, y1, x2, y2, color });
                            }
                        }
                    }

                    cursor = a1;
                }

                // Center hub dot.
                ctx.draw(&Points { coords: &[(0.0, 0.0)], color: accent });
            })
            .render(canvas_area, buf);

        // 6. Optional label, drawn into a thin bottom row so it overlays
        // cleanly below the ring. Mirrors radial_gauge.rs's label block.
        if let Some(label) = &self.label
            && area.height > 0
        {
            let label_y = area.y + side;
            if label_y < area.y + area.height {
                // Center the label under the ring.
                crate::widgets::util::draw_centered_label(
                    buf,
                    area.x,
                    label_y,
                    side,
                    area.x + area.width,
                    label,
                    accent,
                    bg,
                );
            }
        }
    }
}

/// Approximate an arc from `a0` to `a1` at radius `r` with short [`Line`]
/// segments (mirrors [`RadialGauge`](crate::RadialGauge)'s arc segment loop).
fn draw_arc(
    ctx: &mut Context<'_>,
    a0: f64,
    a1: f64,
    r: f64,
    color: Color,
) {
    let span = (a1 - a0).abs();
    // Segment count scales with the slice's fraction of the full ring, with a
    // small floor so tiny slices still draw at least a couple of segments.
    let steps = ((span / std::f64::consts::TAU) * ARC_SEGMENTS as f64).round() as usize;
    let steps = steps.max(2);
    for i in 0..steps {
        let t0 = i as f64 / steps as f64;
        let t1 = (i + 1) as f64 / steps as f64;
        let ang0 = a0 + span * t0;
        let ang1 = a0 + span * t1;
        let (x1, y1) = polar(ang0, r);
        let (x2, y2) = polar(ang1, r);
        ctx.draw(&Line { x1, y1, x2, y2, color });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    /// Render the donut into a fresh buffer with the given state + widget.
    fn render(state: &mut DonutChartState, widget: DonutChart, width: u16, height: u16) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        StatefulWidget::render(widget, Rect::new(0, 0, width, height), &mut buf, state);
        buf
    }

    /// Count non-blank cells in a buffer (cells whose symbol isn't a single space).
    fn non_blank(buf: &Buffer) -> usize {
        buf.content.iter().filter(|c| c.symbol() != " ").count()
    }

    #[test]
    fn renders_without_panicking_after_ticks() {
        let mut state = DonutChartState::default();
        for _ in 0..5 {
            state.tick();
        }
        let buf = render(&mut state, DonutChart::new(), 24, 12);
        assert!(non_blank(&buf) > 0, "donut should draw something after ticks");
    }

    #[test]
    fn zero_area_does_not_panic() {
        let mut state = DonutChartState::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let widget = DonutChart::new();
        StatefulWidget::render(widget, Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        // No panic == pass.
    }

    #[test]
    fn tick_advances_clock_and_changes_values() {
        let mut state = DonutChartState::default();
        let start_values: Vec<f64> = (0..state.values.len()).map(|i| state.value(i)).collect();
        assert_eq!(state.tick_count(), 0);
        for _ in 0..5 {
            state.tick();
        }
        assert_eq!(state.tick_count(), 5, "tick_count should advance");
        let mut any_changed = false;
        for i in 0..start_values.len() {
            if (state.value(i) - start_values[i]).abs() > 1e-9 {
                any_changed = true;
                break;
            }
        }
        assert!(any_changed, "at least one slice value should change over several ticks");
    }

    #[test]
    fn proportions_sum_to_one() {
        let state = DonutChartState::default();
        let sum: f64 = (0..state.values.len()).map(|i| state.proportion(i)).sum();
        assert!((sum - 1.0).abs() < 1e-9, "proportions should sum to 1.0, got {}", sum);
    }

    #[test]
    fn set_slice_clamps_nonnegative() {
        let mut state = DonutChartState::new(3);
        state.set_slice(1, -5.0);
        assert!(
            state.value(1) >= 0.0,
            "set_slice should clamp to >= 0, got {}",
            state.value(1)
        );
        assert!(
            (state.value(1) - 0.0).abs() < 1e-9,
            "negative value should clamp to 0.0"
        );
        state.set_slice(0, 2.5);
        assert!((state.value(0) - 2.5).abs() < 1e-9, "positive value should be stored as-is");
        // Proportion reflects the new value.
        let sum: f64 = state.values.iter().sum();
        let prop0 = state.value(0) / sum;
        assert!((state.proportion(0) - prop0).abs() < 1e-9, "proportion should update");
        // Out-of-range index is ignored (no panic).
        state.set_slice(99, 1.0);
    }

    #[test]
    fn shape_variants_render_without_panicking() {
        for shape in [DonutShape::Arc, DonutShape::Thick, DonutShape::Tick] {
            let mut state = DonutChartState::default();
            state.tick();
            let buf = render(&mut state, DonutChart::new().shape(shape), 24, 12);
            assert!(non_blank(&buf) > 0, "{:?} shape should render non-blank", shape);
        }
    }

    #[test]
    fn builder_setters_work() {
        let w = DonutChart::new()
            .slices(4)
            .shape(DonutShape::Thick)
            .theme(Theme::Weyland)
            .label("LOAD");
        assert_eq!(w.slices, 4);
        assert_eq!(w.shape, DonutShape::Thick);
        assert_eq!(w.theme, Theme::Weyland);
        assert_eq!(w.label.as_deref(), Some("LOAD"));
    }

    #[test]
    fn default_is_cyberpunk() {
        let w = DonutChart::default();
        assert_eq!(w.theme, Theme::Cyberpunk);
        assert_eq!(w.slices, 5);
    }

    #[test]
    fn default_shape_is_arc() {
        let w = DonutChart::default();
        assert_eq!(w.shape, DonutShape::Arc);
    }

    #[test]
    fn slices_clamps_to_at_least_one() {
        assert_eq!(DonutChart::new().slices(0).slices, 1, "slices should clamp to >= 1");
    }

    #[test]
    fn single_slice_fills_ring() {
        let mut state = DonutChartState::new(1);
        state.tick();
        // 1 slice → proportion 1.0.
        assert!((state.proportion(0) - 1.0).abs() < 1e-9, "single slice proportion should be 1.0");
        let buf = render(&mut state, DonutChart::new().slices(1), 24, 12);
        assert!(non_blank(&buf) > 0, "single slice should draw a full ring");
    }

    #[test]
    fn label_renders_below_dial() {
        let mut state = DonutChartState::default();
        state.tick();
        // Use a taller-than-square area so the label row at y == side is in
        // bounds. side = min(10, 14) = 10; label_y = 10, within height 14.
        let buf = render(&mut state, DonutChart::new().label("LOAD"), 10, 14);
        let mut found_l = false;
        for y in 0..14 {
            for x in 0..10 {
                if buf[(x, y)].symbol() == "L" {
                    found_l = true;
                }
            }
        }
        assert!(found_l, "label 'LOAD' should render; got no 'L' cell");
        assert!(non_blank(&buf) > 0);
    }

    #[test]
    fn proportion_zero_sum_is_zero() {
        let mut state = DonutChartState::new(3);
        for i in 0..3 {
            state.set_slice(i, 0.0);
        }
        assert!((state.proportion(0) - 0.0).abs() < 1e-9, "zero-sum proportions should be 0.0");
    }

    #[test]
    fn new_seeds_unequal_baselines() {
        let state = DonutChartState::new(4);
        let v0 = state.value(0);
        let v1 = state.value(1);
        let v2 = state.value(2);
        // Baselines are 1.0, 2.0, 3.0, 4.0 — all distinct.
        assert!(v0 < v1 && v1 < v2, "baselines should be distinct and increasing");
    }

    #[test]
    fn value_out_of_range_is_zero() {
        let state = DonutChartState::new(2);
        assert!((state.value(99) - 0.0).abs() < 1e-9, "out-of-range value should be 0.0");
    }

    #[test]
    fn polar_returns_radius_scaled_cos_sin() {
        let (x, y) = polar(0.0, 1.0);
        assert!((x - 1.0).abs() < 1e-9 && y.abs() < 1e-9);
        let (x, y) = polar(std::f64::consts::FRAC_PI_2, 2.0);
        assert!(x.abs() < 1e-9 && (y - 2.0).abs() < 1e-9);
    }
}
