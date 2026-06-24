//! **ScatterPlot** — Cartesian X/Y point cloud (PRD §3 散点图).
//!
//! A square plot of normalized points (`[0.0,1.0]²`) stamped onto a ratatui
//! [`Canvas`] using [`Marker::Braille`]. Each point orbits a deterministic
//! seed center on a per-index sine, so the cloud self-animates in a demo
//! without any RNG. Points far from the center (or with large `x + y`) read
//! "hot" in the alert color; the rest take the accent.
//!
//! ## Spec
//! - Draw a square sub-area (the smaller of `area.width` / `area.height`) and
//!   render a [`Canvas`] with `Marker::Braille`, `x_bounds = [0.0, 1.0]`,
//!   `y_bounds = [0.0, 1.0]`.
//! - A faint bounding-box frame (four [`Line`]s at `x`/`y ∈ {0.0, 1.0}`) plus
//!   mid-lines at `x = 0.5` and `y = 0.5` give orientation. The frame uses the
//!   `Scatter` base foreground color (fallback `palette.muted`); the canvas
//!   background uses `Scatter` base background (fallback `palette.bg`).
//! - Each point is classified **hot** if its distance from the center
//!   `(0.5, 0.5)` is `≥ 0.35` — a simple, deterministic rule that lights up
//!   the outer ring of the cloud. Hot points take `Scatter.hot` (fallback
//!   `palette.alert`); the rest take `Scatter.point` (fallback `palette.accent`).
//! - Points are grouped by color into two [`Points`] draws (hot vs normal) for
//!   the [`ScatterShape::Dot`] shape; [`ScatterShape::Cross`] and
//!   [`ScatterShape::Ring`] draw per-point [`Line`]s / a [`Circle`].
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]; the point buffer and tick clock live in
//!   [`ScatterPlotState`], mutated by the app's event loop each tick.
//! - All colors resolve through the [`Stylesheet`](crate::Theme::stylesheet)
//!   cascade (`Scatter`, `Scatter.point`, `Scatter.hot`) using a single
//!   [`ComputeScratch`] per render, falling back to [`Theme::palette`] values.
//! - The demo [`ScatterPlotState::tick`] is deterministic (no RNG): each point
//!   orbits its seed center on a per-index sine of small radius.
//!
//! # Example
//!
//! ```no_run
//! use ratatui_sci_fi::{ScatterPlot, ScatterPlotState, Theme};
//!
//! let mut state = ScatterPlotState::default();
//! let plot = ScatterPlot::new().theme(Theme::Cyberpunk);
//! // in your event loop each frame: state.tick();
//! // to feed a live point: state.push(0.3, 0.7);
//! ```

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    symbols::Marker,
    widgets::{StatefulWidget, Widget},
    widgets::canvas::{Canvas, Circle, Line, Points},
};

use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Half-width of a [`ScatterShape::Cross`] arm, in normalized `[0,1]` coords.
///
/// `~0.06` keeps the `+` small but visible at Braille resolution.
pub const CROSS_HALF: f64 = 0.06;

/// Radius of a [`ScatterShape::Ring`] circle, in normalized `[0,1]` coords.
pub const RING_RADIUS: f64 = 0.05;

/// Orbit radius used by the demo [`ScatterPlotState::tick`].
pub const ORBIT_RADIUS: f64 = 0.08;

/// Distance-from-center threshold above which a point reads "hot".
///
/// The center is `(0.5, 0.5)`; a point at distance `≥ 0.35` from it lights up
/// in the alert color. `0.35` puts roughly the outer ring of the unit square
/// into the hot band (a corner sits at `~0.707`).
pub const HOT_DIST: f64 = 0.35;

/// How each point is drawn on the [`Canvas`] (config — convention #5).
///
/// This enum selects the canvas geometry used per point. Because it is canvas
/// geometry (not glyphs), convention #5's Unicode width-1 rule is about glyph
/// cells and doesn't constrain these variants — but the principle (config
/// lives on the widget, default must look great) still holds.
///
/// Colors stay on the CSS cascade; a shape variant affects geometry only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ScatterShape {
    /// Each point a single [`Points`] dot. The default.
    #[default]
    Dot,
    /// Each point a small `+` made of two short [`Line`]s (`~0.06` half-width).
    Cross,
    /// Each point a tiny [`Circle`] of radius `~0.05`.
    Ring,
}

/// A Cartesian X/Y point cloud.
///
/// Built with [`ScatterPlot::new`]; theme defaults to [`Theme::Cyberpunk`],
/// shape defaults to [`ScatterShape::Dot`]. The point buffer and tick clock
/// live in the companion [`ScatterPlotState`], mutated by the app's event
/// loop each tick.
#[derive(Debug, Clone)]
pub struct ScatterPlot {
    /// Maximum number of points kept in [`ScatterPlotState`] (clamped ≥1).
    /// Default `16`.
    pub capacity: usize,
    /// How each point is drawn. Default [`ScatterShape::Dot`].
    pub shape: ScatterShape,
    /// Active theme; drives all colors via its [`Palette`](crate::Palette) /
    /// [`Stylesheet`](crate::Theme::stylesheet). Default [`Theme::Cyberpunk`].
    pub theme: Theme,
    /// Optional short caption drawn below the plot (e.g. `"TELEMETRY"`).
    pub label: Option<String>,
}

impl Default for ScatterPlot {
    fn default() -> Self {
        Self { capacity: 16, shape: ScatterShape::default(), theme: Theme::Cyberpunk, label: None }
    }
}

impl ScatterPlot {
    /// Create a plot with default config (Dot shape, capacity 16, Cyberpunk).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the point capacity (clamped to at least 1). Builder.
    #[must_use]
    pub fn capacity(mut self, n: usize) -> Self {
        self.capacity = n.max(1);
        self
    }

    /// Set the point shape (see [`ScatterShape`]). Builder.
    #[must_use]
    pub fn shape(mut self, shape: ScatterShape) -> Self {
        self.shape = shape;
        self
    }

    /// Replace the theme (builder). Default is [`Theme::Cyberpunk`].
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Attach a short caption drawn below the plot (e.g. `"TELEMETRY"`). Builder.
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// Mutable state for [`ScatterPlot`].
///
/// Holds the point buffer (`(x, y)` pairs normalized to `[0.0,1.0]²`) and a
/// tick clock. The app's event loop calls [`tick`](Self::tick) once per frame,
/// or feeds live points via [`push`](Self::push).
///
/// The demo [`tick`](Self::tick) is deterministic (no RNG): each point orbits
/// its seed center on a per-index sine of [`ORBIT_RADIUS`].
#[derive(Debug, Clone)]
pub struct ScatterPlotState {
    /// Seed centers, one per point. Each tick orbits around its seed.
    seeds: Vec<(f64, f64)>,
    /// Current point positions, normalized to `[0.0,1.0]²`.
    points: Vec<(f64, f64)>,
    /// Capacity mirroring [`ScatterPlot::capacity`]; `push` trims to this.
    capacity: usize,
    /// Monotonic tick counter (drives the orbit sine + tests).
    tick_count: u64,
}

impl Default for ScatterPlotState {
    fn default() -> Self {
        Self::new(16)
    }
}

impl ScatterPlotState {
    /// Create a fresh state sized for `capacity` points.
    ///
    /// Points are seeded on a deterministic pseudo-grid plus a sine jitter of
    /// the index (no RNG): `x = 0.1 + 0.8 * frac(i)` offset by
    /// `0.02 * sin(i)`, similarly for `y` with a cosine, so the initial cloud
    /// is spread but non-degenerate.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        let mut seeds = Vec::with_capacity(capacity);
        let mut points = Vec::with_capacity(capacity);
        for i in 0..capacity {
            let f = if capacity > 1 {
                i as f64 / (capacity - 1) as f64
            } else {
                0.5
            };
            // Spread across [0.1, 0.9] with a tiny sine/cos jitter per index.
            let x = clamp_inner(0.1 + 0.8 * f + 0.02 * (i as f64).sin());
            let y = clamp_inner(0.1 + 0.8 * f + 0.02 * (i as f64 + 1.7).cos());
            seeds.push((x, y));
            points.push((x, y));
        }
        Self { seeds, points, capacity, tick_count: 0 }
    }

    /// Advance the simulation by one tick.
    ///
    /// Bumps the tick clock, then orbits each point around its seed center on
    /// a per-index sine of radius [`ORBIT_RADIUS`]. Coordinates are clamped
    /// back into `[0.02, 0.98]` so points never kiss the frame edge.
    pub fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
        let t = self.tick_count as f64;
        for (i, (pt, seed)) in self.points.iter_mut().zip(self.seeds.iter()).enumerate() {
            // Per-index angular frequency + phase so points don't all move in
            // lockstep.
            let omega = 0.20 + 0.07 * (i as f64);
            let phase = (i as f64) * 0.9;
            let angle = omega * t + phase;
            let dx = ORBIT_RADIUS * angle.cos();
            let dy = ORBIT_RADIUS * angle.sin();
            pt.0 = clamp_inner(seed.0 + dx);
            pt.1 = clamp_inner(seed.1 + dy);
        }
    }

    /// Append a clamped point and trim to capacity (oldest dropped).
    ///
    /// Coordinates are clamped into `[0.0, 1.0]`. When the buffer exceeds
    /// `capacity`, the oldest point (and its matching seed) is dropped.
    pub fn push(&mut self, x: f64, y: f64) {
        let x = clamp_unit(x);
        let y = clamp_unit(y);
        self.points.push((x, y));
        // The pushed point orbits around itself as its own seed.
        self.seeds.push((x, y));
        while self.points.len() > self.capacity {
            self.points.remove(0);
            self.seeds.remove(0);
        }
    }

    /// Overwrite point `i`'s coordinates (clamped). Out-of-range indices are
    /// ignored. The point's seed is updated to match so its orbit recenters.
    pub fn set_point(&mut self, i: usize, x: f64, y: f64) {
        let x = clamp_unit(x);
        let y = clamp_unit(y);
        if let Some(pt) = self.points.get_mut(i) {
            *pt = (x, y);
            if let Some(seed) = self.seeds.get_mut(i) {
                *seed = (x, y);
            }
        }
    }

    /// Drop every point (the cloud is empty until the next `push`/`tick`).
    pub fn clear(&mut self) {
        self.points.clear();
        self.seeds.clear();
    }

    /// Current point count.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Whether the point buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// The point at index `i`, if any.
    pub fn point(&self, i: usize) -> Option<(f64, f64)> {
        self.points.get(i).copied()
    }

    /// Current tick clock value.
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }
}

/// Clamp `v` into `0.0..=1.0` (the unit plot bounds).
fn clamp_unit(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

/// Clamp `v` into `[0.02, 0.98]` so orbiting points never touch the frame.
fn clamp_inner(v: f64) -> f64 {
    v.clamp(0.02, 0.98)
}

/// Euclidean distance between two points.
fn dist(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let dx = ax - bx;
    let dy = ay - by;
    (dx * dx + dy * dy).sqrt()
}

/// True if a point should read "hot" (distance from center ≥ [`HOT_DIST`]).
fn is_hot(x: f64, y: f64) -> bool {
    dist(x, y, 0.5, 0.5) >= HOT_DIST
}

impl StatefulWidget for ScatterPlot {
    type State = ScatterPlotState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // 1. Guard zero-size areas.
        if area.width == 0 || area.height == 0 {
            return;
        }

        // 2. Resolve colors from the cascade with one shared scratch.
        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();

        let grid_color = sheet
            .compute_with(&NodeRef::new("Scatter"), None, &mut scratch)
            .to_style()
            .fg
            .unwrap_or_else(|| self.theme.palette().muted.color());
        let bg = sheet
            .compute_with(&NodeRef::new("Scatter"), None, &mut scratch)
            .to_style()
            .bg
            .unwrap_or_else(|| self.theme.palette().bg.color());
        let point_color = sheet
            .compute_with(&NodeRef::new("Scatter").classes(&["point"]), None, &mut scratch)
            .to_style()
            .fg
            .unwrap_or_else(|| self.theme.palette().accent.color());
        let hot_color = sheet
            .compute_with(&NodeRef::new("Scatter").classes(&["hot"]), None, &mut scratch)
            .to_style()
            .fg
            .unwrap_or_else(|| self.theme.palette().alert.color());

        // 3. Square sub-area (mirror radial_gauge).
        let side = area.width.min(area.height);
        let canvas_area = Rect::new(area.x, area.y, side, side);

        // 4. Partition points into hot / normal buckets for the Dot shape so we
        //    can emit one Points draw per color.
        let mut normal: Vec<(f64, f64)> = Vec::new();
        let mut hot: Vec<(f64, f64)> = Vec::new();
        for &(x, y) in &state.points {
            if is_hot(x, y) {
                hot.push((x, y));
            } else {
                normal.push((x, y));
            }
        }

        // 5. Paint the plot.
        Canvas::default()
            .marker(Marker::Braille)
            .background_color(bg)
            .x_bounds([0.0, 1.0])
            .y_bounds([0.0, 1.0])
            .paint(|ctx| {
                // Bounding box frame: four Lines at x/y ∈ {0.0, 1.0}.
                ctx.draw(&Line { x1: 0.0, y1: 0.0, x2: 1.0, y2: 0.0, color: grid_color });
                ctx.draw(&Line { x1: 0.0, y1: 1.0, x2: 1.0, y2: 1.0, color: grid_color });
                ctx.draw(&Line { x1: 0.0, y1: 0.0, x2: 0.0, y2: 1.0, color: grid_color });
                ctx.draw(&Line { x1: 1.0, y1: 0.0, x2: 1.0, y2: 1.0, color: grid_color });
                // Mid-lines for orientation.
                ctx.draw(&Line { x1: 0.5, y1: 0.0, x2: 0.5, y2: 1.0, color: grid_color });
                ctx.draw(&Line { x1: 0.0, y1: 0.5, x2: 1.0, y2: 0.5, color: grid_color });

                match self.shape {
                    ScatterShape::Dot => {
                        // Grouped Points draws — one per color bucket.
                        if !normal.is_empty() {
                            ctx.draw(&Points { coords: &normal, color: point_color });
                        }
                        if !hot.is_empty() {
                            ctx.draw(&Points { coords: &hot, color: hot_color });
                        }
                    }
                    ScatterShape::Cross => {
                        // Per-point `+` of two short Lines.
                        for &(x, y) in &state.points {
                            let color = if is_hot(x, y) { hot_color } else { point_color };
                            ctx.draw(&Line {
                                x1: x - CROSS_HALF,
                                y1: y,
                                x2: x + CROSS_HALF,
                                y2: y,
                                color,
                            });
                            ctx.draw(&Line {
                                x1: x,
                                y1: y - CROSS_HALF,
                                x2: x,
                                y2: y + CROSS_HALF,
                                color,
                            });
                        }
                    }
                    ScatterShape::Ring => {
                        // Per-point tiny Circle.
                        for &(x, y) in &state.points {
                            let color = if is_hot(x, y) { hot_color } else { point_color };
                            ctx.draw(&Circle { x, y, radius: RING_RADIUS, color });
                        }
                    }
                }
            })
            .render(canvas_area, buf);

        // 6. Optional label, drawn into the row just below the plot (mirror
        //    radial_gauge.rs's label block).
        if let Some(label) = &self.label
            && area.height > 0
        {
            let label_y = area.y + side;
            if label_y < area.y + area.height {
                crate::widgets::util::draw_centered_label(
                    buf,
                    area.x,
                    label_y,
                    side,
                    area.x + area.width,
                    label,
                    point_color,
                    bg,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    /// Render the plot into a fresh buffer with the given state + widget.
    fn render(state: &mut ScatterPlotState, widget: ScatterPlot, width: u16, height: u16) -> Buffer {
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
        let mut state = ScatterPlotState::default();
        for _ in 0..10 {
            state.tick();
        }
        let buf = render(&mut state, ScatterPlot::new(), 20, 10);
        assert!(non_blank(&buf) > 0, "plot should draw something after ticks");
    }

    #[test]
    fn zero_area_does_not_panic() {
        let mut state = ScatterPlotState::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let widget = ScatterPlot::new();
        StatefulWidget::render(widget, Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        // No panic == pass.
    }

    #[test]
    fn tick_advances_clock() {
        let mut state = ScatterPlotState::default();
        let before = state.tick_count();
        state.tick();
        assert_eq!(state.tick_count(), before + 1);
    }

    #[test]
    fn tick_moves_points() {
        // At least one point coord should change over a few ticks.
        let mut state = ScatterPlotState::new(4);
        let before: Vec<(f64, f64)> = state.points.clone();
        for _ in 0..5 {
            state.tick();
        }
        let after: Vec<(f64, f64)> = state.points.clone();
        let moved = before.iter().zip(after.iter()).any(|(a, b)| (a.0 - b.0).abs() > 1e-9 || (a.1 - b.1).abs() > 1e-9);
        assert!(moved, "at least one point should move over a few ticks");
    }

    #[test]
    fn push_clamps_and_trims_capacity() {
        // `new(2)` pre-seeds 2 points; clear so we test push in isolation.
        let mut state = ScatterPlotState::new(2);
        state.clear();
        // Out-of-range coords clamp into [0, 1].
        state.push(999.0, -50.0);
        assert_eq!(state.point(0), Some((1.0, 0.0)));
        // Pushing beyond capacity drops the oldest.
        state.push(0.25, 0.25);
        state.push(0.5, 0.5);
        // capacity is 2, so only the last two remain.
        assert_eq!(state.len(), 2);
        // The first pushed (1.0, 0.0) should have been dropped.
        assert_eq!(state.point(0), Some((0.25, 0.25)));
        assert_eq!(state.point(1), Some((0.5, 0.5)));
    }

    #[test]
    fn set_point_and_clear_work() {
        let mut state = ScatterPlotState::new(3);
        // set_point clamps and updates the stored point.
        state.set_point(1, 5.0, -1.0);
        assert_eq!(state.point(1), Some((1.0, 0.0)));
        // Out-of-range index is a no-op (no panic).
        state.set_point(99, 0.5, 0.5);
        // clear empties the buffer.
        state.clear();
        assert!(state.is_empty());
        assert_eq!(state.len(), 0);
    }

    #[test]
    fn shape_variants_render_without_panicking() {
        for shape in [ScatterShape::Dot, ScatterShape::Cross, ScatterShape::Ring] {
            let mut state = ScatterPlotState::default();
            state.tick();
            let buf = render(&mut state, ScatterPlot::new().shape(shape), 24, 12);
            assert!(non_blank(&buf) > 0, "{:?} shape should render non-blank", shape);
        }
    }

    #[test]
    fn builder_setters_work() {
        let w = ScatterPlot::new()
            .capacity(8)
            .shape(ScatterShape::Ring)
            .theme(Theme::Weyland)
            .label("TELEMETRY");
        assert_eq!(w.capacity, 8);
        assert_eq!(w.shape, ScatterShape::Ring);
        assert_eq!(w.theme, Theme::Weyland);
        assert_eq!(w.label.as_deref(), Some("TELEMETRY"));
    }

    #[test]
    fn capacity_clamps_to_one() {
        let w = ScatterPlot::new().capacity(0);
        assert_eq!(w.capacity, 1);
    }

    #[test]
    fn default_is_cyberpunk() {
        let w = ScatterPlot::default();
        assert_eq!(w.theme, Theme::Cyberpunk);
        assert_eq!(w.capacity, 16);
    }

    #[test]
    fn default_shape_is_dot() {
        let w = ScatterPlot::default();
        assert_eq!(w.shape, ScatterShape::Dot);
    }

    #[test]
    fn label_renders_below_plot() {
        let mut state = ScatterPlotState::default();
        state.tick();
        // Taller-than-square area so the label row (y == side) fits inside.
        let buf = render(
            &mut state,
            ScatterPlot::new().label("PWR"),
            10,
            14,
        );
        // side = min(10, 14) = 10; label_y = 10 (row 10), within height 14.
        let mut found_p = false;
        for y in 0..14 {
            for x in 0..10 {
                if buf[(x, y)].symbol() == "P" {
                    found_p = true;
                }
            }
        }
        assert!(found_p, "label 'PWR' should render; got no 'P' cell");
        assert!(non_blank(&buf) > 0);
    }

    #[test]
    fn is_hot_threshold_is_distance_from_center() {
        // Center point is not hot.
        assert!(!is_hot(0.5, 0.5));
        // A corner (distance ~0.707 from center) is hot.
        assert!(is_hot(0.0, 0.0));
        assert!(is_hot(1.0, 1.0));
        // Just inside the threshold is not hot.
        assert!(!is_hot(0.5 + 0.30, 0.5));
        // Just at/over the threshold is hot.
        assert!(is_hot(0.5 + HOT_DIST, 0.5));
    }

    #[test]
    fn clamp_helpers_keep_ranges() {
        assert_eq!(clamp_unit(-1.0), 0.0);
        assert_eq!(clamp_unit(2.0), 1.0);
        assert_eq!(clamp_unit(0.5), 0.5);
        assert_eq!(clamp_inner(-1.0), 0.02);
        assert_eq!(clamp_inner(2.0), 0.98);
    }
}
