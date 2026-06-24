//! **RadialBarChart** — polar bars radiating from a center point (PRD §3 仪表盘).
//!
//! A sci-fi radial readout: `N` categories are arranged evenly around a circle,
//! one bar per category, each bar a thick radial spoke whose length is
//! proportional to its value (`0.0..=1.0`). Unlike [`SpectrumBars`]
//! (vertical cartesian bars rising from the bottom), `RadialBarChart` lays its
//! bars out around a center — like a reactor's subsystem output graph or a
//! multi-channel radar plot.
//!
//! ## Spec
//! - Draw on a ratatui [`Canvas`] using [`Marker::Braille`] (so spokes look
//!   crisp), centered in a square sub-area (mirroring [`RadialGauge`]).
//! - Inner radius `r0 = 0.25`, outer radius `r1 = 0.92` (in unit-disk coords).
//! - Bar `i` sits at angle `i * (2π / bars)` and extends from `r0` outward to
//!   `r0 + value_i * (r1 - r0)`.
//! - A faint full-circle track at `r1` plus faint spoke lines from center to
//!   `r1` at each bar's angle provide orientation.
//! - [`RBarShape`] selects bar geometry: a thick radial `Line` (default), a
//!   filled arc wedge, or a thin needle.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]; per-bar eased `value`s, their `target`s, and
//!   the tick clock live in [`RadialBarState`].
//! - [`Marker::Braille`] canvas with `x_bounds`/`y_bounds = [-1.0, 1.0]`; the
//!   unit disk is centered at the canvas origin. The Braille cell aspect makes
//!   the circle look slightly elliptical — expected and accepted (same trade-off
//!   as [`RadialGauge`] / [`SciFiRadar`]).
//! - All colors resolve through the [`Stylesheet`](crate::Theme::stylesheet)
//!   cascade (`RBar`, `RBar.bar0`..`RBar.bar4`, `RBar.track`, `RBar.grid`)
//!   using a single [`ComputeScratch`] per render, falling back to
//!   [`Theme::palette`] values. Each bar's color is pre-resolved once before
//!   drawing.
//!
//! [`SpectrumBars`]: crate::SpectrumBars
//! [`RadialGauge`]: crate::RadialGauge
//! [`SciFiRadar`]: crate::SciFiRadar
//!
//! # Example
//!
//! ```no_run
//! use ratatui_sci_fi::{RadialBarChart, RadialBarState, RBarShape, Theme};
//!
//! let mut state = RadialBarState::new(6);
//! let chart = RadialBarChart::new()
//!     .bars(6)
//!     .shape(RBarShape::Bar)
//!     .theme(Theme::Cyberpunk);
//! // in your event loop each frame: state.tick();
//! // to drive a bar externally: state.set_bar(2, 0.78);
//! ```

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    symbols::Marker,
    widgets::{StatefulWidget, Widget},
    widgets::canvas::{Canvas, Circle, Line},
};

use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Easing factor applied each tick: `value += (target - value) * EASE`.
///
/// `0.18` gives a smooth, slightly snappy sci-fi sweep that settles in roughly
/// ~20 ticks. Deterministic — no RNG.
pub const EASE: f64 = 0.18;

/// Inner radius of the bar band (where bars start), in unit-disk coords.
pub const R0: f64 = 0.25;

/// Outer radius of the bar band (full-value bar tip), in unit-disk coords.
pub const R1: f64 = 0.92;

/// Number of short line segments used to approximate an `Arc`-shape wedge.
pub const ARC_SEGMENTS: usize = 14;

/// Angular thickness of a `Bar`-shape spoke, drawn as parallel offset lines
/// (radians, symmetric about the bar's center angle).
pub const BAR_HALF_WIDTH: f64 = 0.06;

/// How a category's value is drawn as a polar bar (config — convention #5).
///
/// Selects the geometry painted on the [`Canvas`] in `render`. Because it is
/// canvas geometry (not glyphs), convention #5's Unicode width-1 rule is about
/// glyph cells and doesn't constrain these variants — but the principle (config
/// lives on the widget, default looks great) still holds.
///
/// Colors stay on the CSS cascade; a shape variant affects geometry only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RBarShape {
    /// A thick radial spoke from `r0` out to `r0 + value*(r1-r0)` at the bar's
    /// angle. Drawn as three parallel [`Line`]s slightly offset in angle
    /// (center ± [`BAR_HALF_WIDTH`]) so the bar reads as a thick band rather
    /// than a single thin line. The default.
    #[default]
    Bar,
    /// A filled arc wedge (a DonutChart-like slice) at the bar's angle, swept
    /// from `r0` out to `r0 + value*(r1-r0)` and spanning a wedge width of
    /// `0.6 * (2π / bars)`. Approximated with [`ARC_SEGMENTS`] short radial
    /// segments.
    Arc,
    /// A single thin radial line (needle) from the center out to
    /// `r0 + value*(r1-r0)`.
    Needle,
}

/// A radial bar chart: polar bars radiating from a center, one per category.
///
/// Immutable config lives here (`bars`, `shape`, `theme`, `label`); everything
/// that changes per frame lives in [`RadialBarState`].
#[derive(Debug, Clone)]
pub struct RadialBarChart {
    /// Number of bars (default `6`, clamped ≥1).
    pub bars: usize,
    /// Bar geometry form. Defaults to [`RBarShape::Bar`].
    pub shape: RBarShape,
    /// Theme whose palette drives the colors via CSS cascade. Default
    /// [`Theme::Cyberpunk`].
    pub theme: Theme,
    /// Optional short caption drawn below the chart (e.g. `"SUBSYSTEMS"`).
    pub label: Option<String>,
}

impl Default for RadialBarChart {
    fn default() -> Self {
        Self { bars: 6, shape: RBarShape::Bar, theme: Theme::Cyberpunk, label: None }
    }
}

impl RadialBarChart {
    /// Build a radial bar chart with default config (6 bars, Bar shape).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the number of bars (clamped to at least 1). Builder.
    #[must_use]
    pub fn bars(mut self, n: usize) -> Self {
        self.bars = n.max(1);
        self
    }

    /// Set the bar geometry form (see [`RBarShape`]). Builder.
    #[must_use]
    pub fn shape(mut self, s: RBarShape) -> Self {
        self.shape = s;
        self
    }

    /// Set the theme whose palette drives colors. Builder.
    #[must_use]
    pub fn theme(mut self, t: Theme) -> Self {
        self.theme = t;
        self
    }

    /// Attach a short caption drawn below the chart. Builder.
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// Mutable state for [`RadialBarChart`].
///
/// Holds per-bar eased `value`s (`0.0..=1.0`), their `target`s, and a tick
/// clock. The app's event loop calls [`tick`](Self::tick) once per frame.
///
/// # Two drive modes
///
/// - **Auto (default).** [`tick`](Self::tick) both eases each `value` toward its
///   `target` *and* gently wanders each `target` along a per-index sine (distinct
///   phase per bar), so the chart self-animates in a demo without any external
///   input.
/// - **Driven.** The caller sets per-bar values via [`set_bar`](Self::set_bar);
///   `tick` then only eases each `value` toward its `target`.
///
/// Easing is deterministic (no RNG): `value += (target - value) * `[`EASE`].
#[derive(Debug, Clone)]
pub struct RadialBarState {
    /// Per-bar current displayed values, eased toward `targets` each tick.
    values: Vec<f64>,
    /// Per-bar targets the values ease toward (`0.0..=1.0`).
    targets: Vec<f64>,
    /// Monotonic tick counter (drives the auto-wander sines + tests).
    tick_count: u64,
}

impl Default for RadialBarState {
    fn default() -> Self {
        Self::new(6)
    }
}

impl RadialBarState {
    /// Create fresh state for `n` bars (clamped to at least 1). Each bar's
    /// eased `value` starts at `0.0`; each bar's `target` is staggered as
    /// `0.3 + 0.6 * (i / n)` so the demo immediately has distinct bar heights.
    #[must_use]
    pub fn new(n: usize) -> Self {
        let n = n.max(1);
        let mut values = vec![0.0_f64; n];
        let targets: Vec<f64> = (0..n)
            .map(|i| {
                let t = 0.3 + 0.6 * (i as f64) / (n as f64);
                clamp_unit(t)
            })
            .collect();
        // Keep values sized to match (already 0.0-filled).
        values.resize(n, 0.0);
        Self { values, targets, tick_count: 0 }
    }

    /// Advance the simulation by one tick.
    ///
    /// 1. Bump the tick clock (wrapping).
    /// 2. Wander each bar's `target` along a per-index sine with a distinct
    ///    phase so neighboring bars move differently (demo auto-mode).
    /// 3. Ease each `value` toward its `target` by [`EASE`].
    pub fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
        let t = self.tick_count as f64;
        for (i, (v, target)) in self.values.iter_mut().zip(self.targets.iter_mut()).enumerate() {
            // Distinct phase per bar; slow wander in roughly [0.1, 0.9].
            let phase = (i as f64) * 0.9;
            *target = 0.5 + 0.4 * (t * 0.05 + phase).sin();
            *target = clamp_unit(*target);
            // Ease toward target.
            *v += (*target - *v) * EASE;
            *v = clamp_unit(*v);
        }
        // Parallel lengths stay in sync (n never changes after new()).
        debug_assert_eq!(self.values.len(), self.targets.len());
    }

    /// Set bar `i`'s value and target to `v` (clamped into `0.0..=1.0`).
    /// Out-of-range indices are ignored (no panic).
    pub fn set_bar(&mut self, i: usize, v: f64) {
        let clamped = clamp_unit(v);
        if let Some(val) = self.values.get_mut(i)
            && let Some(tgt) = self.targets.get_mut(i)
        {
            *val = clamped;
            *tgt = clamped;
        }
    }

    /// Current eased value for bar `i` (`0.0` if out of range).
    pub fn value(&self, i: usize) -> f64 {
        self.values.get(i).copied().unwrap_or(0.0)
    }

    /// Current target for bar `i` (`0.0` if out of range).
    pub fn target(&self, i: usize) -> f64 {
        self.targets.get(i).copied().unwrap_or(0.0)
    }

    /// Current tick clock value.
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }
}

/// Clamp `v` into `0.0..=1.0`.
fn clamp_unit(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

/// Polar-to-cartesian helper: `(r*cos(angle), r*sin(angle))`.
fn polar(angle: f64, r: f64) -> (f64, f64) {
    (r * angle.cos(), r * angle.sin())
}

/// Resolve a bar's color from the cascade, falling back to the palette. The
/// bar's color class cycles `bar0..bar4` (`i % 5`); fallback cycles the palette
/// accent / accent2 / ok / warn / alert tokens.
fn bar_color(sheet: &ratatui_style::Stylesheet, theme: Theme, i: usize, scratch: &mut ComputeScratch) -> ratatui::style::Color {
    let cls = match i % 5 {
        0 => "bar0",
        1 => "bar1",
        2 => "bar2",
        3 => "bar3",
        _ => "bar4",
    };
    let fallback = match i % 5 {
        0 => theme.palette().accent.color(),
        1 => theme.palette().accent2.color(),
        2 => theme.palette().ok.color(),
        3 => theme.palette().warn.color(),
        _ => theme.palette().alert.color(),
    };
    sheet
        .compute_with(&NodeRef::new("RBar").classes(&[cls]), None, scratch)
        .to_style()
        .fg
        .unwrap_or(fallback)
}

impl StatefulWidget for RadialBarChart {
    type State = RadialBarState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // 1. Guard zero-size areas.
        if area.width == 0 || area.height == 0 {
            return;
        }

        // 2. Resolve colors from the cascade with one shared scratch.
        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();

        let track_color = sheet
            .compute_with(&NodeRef::new("RBar").classes(&["track"]), None, &mut scratch)
            .to_style()
            .fg
            .unwrap_or_else(|| self.theme.palette().muted.color());
        let grid_color = sheet
            .compute_with(&NodeRef::new("RBar").classes(&["grid"]), None, &mut scratch)
            .to_style()
            .fg
            .unwrap_or_else(|| self.theme.palette().muted.color());
        let bg = sheet
            .compute_with(&NodeRef::new("RBar"), None, &mut scratch)
            .to_style()
            .bg
            .unwrap_or_else(|| self.theme.palette().bg.color());

        // Pre-resolve each bar's color once (up to self.bars).
        let bars = self.bars;
        let bar_colors: Vec<ratatui::style::Color> =
            (0..bars).map(|i| bar_color(sheet, self.theme, i, &mut scratch)).collect();
        // Label color is resolved up front (a Copy Color) so the paint closure
        // may move `bar_colors` without breaking the post-canvas label write.
        let label_color = bar_colors
            .first()
            .copied()
            .unwrap_or_else(|| self.theme.palette().accent.color());

        // 3. Square sub-area (mirror radial_gauge).
        let side = area.width.min(area.height);
        let canvas_area = Rect::new(area.x, area.y, side, side);

        // Angular stride between bars (radians). Guard bars >= 1 (builder clamps).
        let stride = (std::f64::consts::TAU) / (bars as f64);

        // 4–5. Paint the chart.
        Canvas::default()
            .marker(Marker::Braille)
            .background_color(bg)
            .x_bounds([-1.0, 1.0])
            .y_bounds([-1.0, 1.0])
            .paint(move |ctx| {
                // Faint full-circle track at the outer radius.
                ctx.draw(&Circle { x: 0.0, y: 0.0, radius: R1, color: track_color });

                // Faint spoke lines from center to R1 at each bar's angle, for
                // orientation. Drawn in the grid color.
                for i in 0..bars {
                    let angle = (i as f64) * stride;
                    let (ex, ey) = polar(angle, R1);
                    ctx.draw(&Line { x1: 0.0, y1: 0.0, x2: ex, y2: ey, color: grid_color });
                }

                // Bars.
                for (i, &color) in bar_colors.iter().enumerate() {
                    let angle = (i as f64) * stride;
                    let value = state.value(i);
                    let len = R0 + value * (R1 - R0);

                    match self.shape {
                        RBarShape::Bar => {
                            // Three parallel radial lines (center ± half-width)
                            // for a thick band look. Even at value 0 the tiny
                            // r0..r0 segment is a no-op draw.
                            for &da in &[-BAR_HALF_WIDTH, 0.0, BAR_HALF_WIDTH] {
                                let a = angle + da;
                                let (x1, y1) = polar(a, R0);
                                let (x2, y2) = polar(a, len);
                                ctx.draw(&Line { x1, y1, x2, y2, color });
                            }
                        }
                        RBarShape::Arc => {
                            // A filled wedge: short radial segments sweeping the
                            // bar's angular span (width 0.6 * stride) from R0 to
                            // len. Approximated with ARC_SEGMENTS slices.
                            let half_wedge = 0.3 * stride;
                            let steps = ARC_SEGMENTS.max(1);
                            for s in 0..steps {
                                let t0 = (s as f64) / (steps as f64);
                                let t1 = ((s + 1) as f64) / (steps as f64);
                                let a0 = angle - half_wedge + (2.0 * half_wedge) * t0;
                                let a1 = angle - half_wedge + (2.0 * half_wedge) * t1;
                                // Inner edge segment.
                                let (ix1, iy1) = polar(a0, R0);
                                let (ix2, iy2) = polar(a1, R0);
                                ctx.draw(&Line { x1: ix1, y1: iy1, x2: ix2, y2: iy2, color });
                                // Outer edge segment (at the bar's tip radius).
                                let (ox1, oy1) = polar(a0, len);
                                let (ox2, oy2) = polar(a1, len);
                                ctx.draw(&Line { x1: ox1, y1: oy1, x2: ox2, y2: oy2, color });
                                // Connecting side at the sweep ends.
                                let (sx1, sy1) = polar(a1, R0);
                                let (sx2, sy2) = polar(a1, len);
                                ctx.draw(&Line { x1: sx1, y1: sy1, x2: sx2, y2: sy2, color });
                            }
                        }
                        RBarShape::Needle => {
                            // Single thin radial line from center to the tip.
                            let (x1, y1) = polar(angle, 0.0);
                            let (x2, y2) = polar(angle, len);
                            ctx.draw(&Line { x1, y1, x2, y2, color });
                        }
                    }
                }
            })
            .render(canvas_area, buf);

        // 6. Optional label, drawn into a thin bottom row below the chart.
        if let Some(label) = &self.label
            && area.height > 0
        {
            let label_y = area.y + side;
            if label_y < area.y + area.height {
                // Label color: reuse bar0 (the first/primary bar color).
                crate::widgets::util::draw_centered_label(
                    buf,
                    area.x,
                    label_y,
                    side,
                    area.x + area.width,
                    label,
                    label_color,
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

    /// Render the chart into a fresh buffer with the given state + widget.
    fn render(state: &mut RadialBarState, widget: RadialBarChart, width: u16, height: u16) -> Buffer {
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
        let mut state = RadialBarState::new(6);
        for _ in 0..8 {
            state.tick();
        }
        let buf = render(&mut state, RadialBarChart::new(), 20, 10);
        assert!(non_blank(&buf) > 0, "chart should draw something after ticks");
    }

    #[test]
    fn zero_area_does_not_panic() {
        let mut state = RadialBarState::new(6);
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let widget = RadialBarChart::new();
        StatefulWidget::render(widget, Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        // No panic == pass.
    }

    #[test]
    fn tick_advances_clock_and_eases_values() {
        let mut state = RadialBarState::new(4);
        let clock_before = state.tick_count();
        let v_before = state.value(0);
        state.tick();
        assert_eq!(state.tick_count(), clock_before + 1);
        let v_after = state.value(0);
        // value starts at 0.0; target is staggered > 0, so value should rise.
        assert!(
            v_after > v_before,
            "value should ease toward target: {} -> {}",
            v_before,
            v_after
        );
        // The eased delta should be ~EASE of the gap (gap = target - 0.0).
        let target = state.target(0);
        let expected = target * EASE;
        assert!(
            (v_after - expected).abs() < 1e-9,
            "eased value should be ~EASE*target: got {}, expected {}",
            v_after,
            expected
        );
    }

    #[test]
    fn set_bar_clamps_and_sets() {
        let mut state = RadialBarState::new(3);
        // Over-range clamps to 1.0.
        state.set_bar(0, 5.0);
        assert!((state.value(0) - 1.0).abs() < 1e-9, "over-range should clamp to 1.0");
        assert!((state.target(0) - 1.0).abs() < 1e-9, "target should also clamp to 1.0");
        // Negative clamps to 0.0.
        state.set_bar(1, -3.0);
        assert!((state.value(1) - 0.0).abs() < 1e-9, "negative should clamp to 0.0");
        // Mid-range passes through.
        state.set_bar(2, 0.42);
        assert!((state.value(2) - 0.42).abs() < 1e-9, "mid-range should pass through");
        // Out-of-range index is ignored (no panic).
        state.set_bar(99, 0.5);
        assert_eq!(state.value(99), 0.0);
    }

    #[test]
    fn shape_variants_render_without_panicking() {
        for shape in [RBarShape::Bar, RBarShape::Arc, RBarShape::Needle] {
            let mut state = RadialBarState::new(6);
            for _ in 0..6 {
                state.tick();
            }
            let buf = render(&mut state, RadialBarChart::new().shape(shape), 22, 12);
            assert!(non_blank(&buf) > 0, "{:?} shape should render non-blank", shape);
        }
    }

    #[test]
    fn builder_setters_work() {
        let w = RadialBarChart::new()
            .bars(8)
            .shape(RBarShape::Arc)
            .theme(Theme::Weyland)
            .label("SUBSYSTEMS");
        assert_eq!(w.bars, 8);
        assert_eq!(w.shape, RBarShape::Arc);
        assert_eq!(w.theme, Theme::Weyland);
        assert_eq!(w.label.as_deref(), Some("SUBSYSTEMS"));
    }

    #[test]
    fn default_is_cyberpunk() {
        let w = RadialBarChart::default();
        assert_eq!(w.theme, Theme::Cyberpunk);
        assert_eq!(w.bars, 6);
    }

    #[test]
    fn default_shape_is_bar() {
        let w = RadialBarChart::default();
        assert_eq!(w.shape, RBarShape::Bar);
    }

    #[test]
    fn bars_setter_clamps_to_one() {
        let w = RadialBarChart::new().bars(0);
        assert_eq!(w.bars, 1, "bars(0) should clamp to 1");
    }

    #[test]
    fn new_seeds_staggered_targets() {
        let state = RadialBarState::new(4);
        // Targets should be distinct and staggered: 0.3 + 0.6*(i/n).
        let t0 = state.target(0);
        let t3 = state.target(3);
        assert!(t3 > t0, "later bars should have larger staggered targets");
        // value starts at 0.0.
        assert!((state.value(0) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn label_renders_below_chart() {
        let mut state = RadialBarState::new(6);
        for _ in 0..4 {
            state.tick();
        }
        // side = min(10,14) = 10; label_y = 10, within height 14.
        let buf = render(&mut state, RadialBarChart::new().label("PWR"), 10, 14);
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
}
