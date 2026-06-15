//! **AreaChart** — filled area under a single trend curve (PRD §3 趋势图).
//!
//! A sci-fi telemetry / sensor trend plot: a rolling window of `0.0..=1.0`
//! samples drawn as a filled area beneath a trend line, like a starship
//! hull-integrity or power-output trace. The stateful [`AreaChartState`] holds a
//! rolling sample buffer that can be driven externally via
//! [`push`](AreaChartState::push) or self-animated via
//! [`tick`](AreaChartState::tick).
//!
//! This is the filled-area counterpart to [`Sparkline`](crate::Sparkline) /
//! [`BiometricChart`](crate::BiometricChart).
//!
//! ## Spec
//! - Draw the trend on a ratatui [`Canvas`] using [`Marker::Braille`], spanning
//!   the full area. `x_bounds = [0.0, window]`, `y_bounds = [0.0, 1.0]`.
//! - The curve is a polyline through the rolling samples `(i, sample[i])`. Under
//!   [`AreaShape::Solid`] (default) and [`AreaShape::Fill`] the area below the
//!   curve is filled; under [`AreaShape::Solid`] and [`AreaShape::Line`] the
//!   top-edge line is drawn on top of (or instead of) the fill.
//! - All colors resolve through the [`Stylesheet`](crate::Theme::stylesheet)
//!   cascade (`Area` / `Area.fill` / `Area.line` / `Area.grid`) using a single
//!   [`ComputeScratch`] per render, falling back to [`Theme::palette`] values.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]; the rolling sample window + tick clock live in
//!   [`AreaChartState`], advanced each tick (or fed live via `push`).
//! - [`Marker::Braille`] canvas. ratatui's Canvas has no fill primitive, so the
//!   area is rendered as a dense cloud of [`Points`] — for each column across
//!   `[0, samples.len()]`, the curve `y` is interpolated and every point
//!   `(x, y')` for `y'` from `0` up to that `y` is emitted, giving a solid
//!   braille fill. The top-edge line is approximated with short [`Line`]
//!   segments between consecutive samples, mirroring [`RadialGauge`]'s arc
//!   approach.
//! - Shape enum is CONFIG ([`AreaShape`]); colors stay on CSS.
//!
//! [`RadialGauge`]: crate::RadialGauge
//! [`Canvas`]: ratatui::widgets::canvas::Canvas
//!
//! # Example
//!
//! ```no_run
//! use ratatui_sci_fi::{AreaChart, AreaChartState, AreaShape, Theme};
//!
//! let mut state = AreaChartState::default();
//! let chart = AreaChart::new()
//!     .shape(AreaShape::Solid)
//!     .theme(Theme::Cyberpunk)
//!     .label("HULL INTEGRITY");
//! // in your event loop each frame: state.tick();
//! ```

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    symbols::Marker,
    widgets::{StatefulWidget, Widget},
    widgets::canvas::{Canvas, Line, Points},
};

use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Step size (in canvas x units) between sampled columns when stamping the fill
/// cloud. Smaller = denser fill, more [`Points`] emitted.
const FILL_X_STEP: f64 = 0.5;
/// Step size (in canvas y units) between stacked fill points at a given column.
const FILL_Y_STEP: f64 = 0.5;

/// How the area chart is rendered (config — convention #5).
///
/// This enum selects what gets drawn on the [`Canvas`]: the filled area below
/// the curve, the top-edge line, or both. Colors stay on the CSS cascade; a
/// shape variant affects geometry only.
///
/// [`Canvas`]: ratatui::widgets::canvas::Canvas
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AreaShape {
    /// Filled area below the curve **plus** the top-edge line drawn on top.
    /// The default.
    #[default]
    Solid,
    /// Filled area below the curve only (no top-edge line).
    Fill,
    /// Top-edge line only (no fill) — essentially a sparkline.
    Line,
}

/// A filled area chart tracing a single trend curve over a rolling window.
///
/// Immutable config lives here (`window`, `shape`, `theme`, `label`); everything
/// that changes per frame lives in [`AreaChartState`]. Defaults: `window = 64`,
/// `shape = `[`AreaShape::Solid`], `theme = `[`Theme::Cyberpunk`], no label.
#[derive(Debug, Clone)]
pub struct AreaChart {
    /// Rolling window length in samples (default `64`, clamped ≥ 2).
    pub window: usize,
    /// Rendered form. Defaults to [`AreaShape::Solid`].
    pub shape: AreaShape,
    /// Theme whose palette drives the colors via CSS cascade. Default
    /// [`Theme::Cyberpunk`].
    pub theme: Theme,
    /// Optional short caption drawn centered below the chart (e.g. `"TELEMETRY"`).
    pub label: Option<String>,
}

impl Default for AreaChart {
    fn default() -> Self {
        Self { window: 64, shape: AreaShape::default(), theme: Theme::Cyberpunk, label: None }
    }
}

impl AreaChart {
    /// Build an area chart with default config (Solid shape, Cyberpunk theme).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the rolling window length (clamped to at least 2). Builder.
    #[must_use]
    pub fn window(mut self, n: usize) -> Self {
        self.window = n.max(2);
        self
    }

    /// Set the rendered form (see [`AreaShape`]). Builder.
    #[must_use]
    pub fn shape(mut self, s: AreaShape) -> Self {
        self.shape = s;
        self
    }

    /// Set the theme whose palette drives colors. Builder.
    #[must_use]
    pub fn theme(mut self, t: Theme) -> Self {
        self.theme = t;
        self
    }

    /// Attach a short caption drawn centered below the chart. Builder.
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// Mutable state for [`AreaChart`].
///
/// Holds a rolling sample buffer (`0.0..=1.0`, capped at `window`) plus a tick
/// counter that drives the self-generated oscillator. The app advances it every
/// frame via [`Self::tick`] (demo mode) or feeds live samples via [`Self::push`]
/// (external mode).
#[derive(Debug, Clone)]
pub struct AreaChartState {
    /// Rolling sample window, oldest first; each value in `0.0..=1.0`.
    samples: Vec<f64>,
    /// Configured window length (max samples kept).
    window: usize,
    /// Animation clock, advanced each tick; drives the self-generated
    /// oscillator when no external data is pushed.
    tick_count: u64,
}

impl Default for AreaChartState {
    fn default() -> Self {
        Self::new(64)
    }
}

impl AreaChartState {
    /// Build state with a `window`-sample rolling buffer (clamped to at least
    /// 2). The buffer starts seeded with a single `0.5` baseline so the first
    /// render has at least one value to draw.
    #[must_use]
    pub fn new(window: usize) -> Self {
        let window = window.max(2);
        Self { samples: vec![0.5], window, tick_count: 0 }
    }

    /// Advance the trend by one tick (demo / self-generated mode).
    ///
    /// Computes the next deterministic oscillator sample (a single base
    /// frequency plus a couple of harmonics and a slow drift) and pushes it
    /// into the rolling buffer. The app should call this once per frame.
    pub fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
        let value = Self::oscillator(self.tick_count as f64);
        self.samples.push(value);
        let overflow = self.samples.len().saturating_sub(self.window);
        if overflow > 0 {
            self.samples.drain(..overflow);
        }
    }

    /// Feed a live sample (external-feed mode).
    ///
    /// The value is clamped to `0.0..=1.0` and appended to the rolling buffer;
    /// the oldest sample is dropped once the buffer exceeds `window`.
    pub fn push(&mut self, value: f64) {
        let clamped = value.clamp(0.0, 1.0);
        self.samples.push(clamped);
        let overflow = self.samples.len().saturating_sub(self.window);
        if overflow > 0 {
            self.samples.drain(..overflow);
        }
    }

    /// Deterministic oscillator producing a value in `0.0..=1.0` for the given
    /// tick time `t`.
    ///
    /// A single base frequency plus two harmonics and a slow drift, normalized
    /// to roughly `[-1, 1]` then mapped to `[0, 1]`. Deterministic — no RNG.
    fn oscillator(t: f64) -> f64 {
        let base = 0.22;
        let phase = 0.6;
        let raw = (base * t + phase).sin() * 0.55
            + (base * 2.1 * t + phase).sin() * 0.30
            + (0.04 * t + phase * 1.3).sin() * 0.15;
        let mapped = 0.5 + raw * 0.5;
        mapped.clamp(0.0, 1.0)
    }

    /// Latest sample (`0.0` if the buffer is empty).
    pub fn value(&self) -> f64 {
        self.samples.last().copied().unwrap_or(0.0)
    }

    /// Number of samples currently held in the rolling buffer.
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Current tick counter (mainly useful for tests / diagnostics).
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }
}

/// Linearly interpolate the curve's `y` at a fractional `x` within
/// `[0, n - 1]` (where `n = samples.len()`), clamping `x` to the sample span.
/// Returns `0.0` if there are no samples.
fn curve_y_at(samples: &[f64], x: f64) -> f64 {
    let n = samples.len();
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return samples[0];
    }
    let span = (n - 1) as f64;
    let xc = x.clamp(0.0, span);
    let i = xc.floor() as usize;
    let frac = xc - i as f64;
    let y0 = samples[i];
    // Guard the upper index (x clamped to span keeps i ≤ n-2 here, but be safe).
    let y1 = if i + 1 < n { samples[i + 1] } else { samples[i] };
    y0 + (y1 - y0) * frac
}

impl StatefulWidget for AreaChart {
    type State = AreaChartState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // 1. Guard zero-size areas.
        if area.width == 0 || area.height == 0 {
            return;
        }

        // 2. Resolve colors from the cascade with one shared scratch.
        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();

        let fill_color = sheet
            .compute_with(&NodeRef::new("Area").classes(&["fill"]), None, &mut scratch)
            .to_style()
            .fg
            .unwrap_or_else(|| self.theme.palette().accent.color());
        let line_color = sheet
            .compute_with(&NodeRef::new("Area").classes(&["line"]), None, &mut scratch)
            .to_style()
            .fg
            .unwrap_or_else(|| self.theme.palette().accent2.color());
        let bg = sheet
            .compute_with(&NodeRef::new("Area"), None, &mut scratch)
            .to_style()
            .bg
            .unwrap_or_else(|| self.theme.palette().bg.color());

        let samples = &state.samples;
        let window = self.window;
        let n = samples.len();

        // 3. Paint the chart on a Braille canvas. x spans [0, window]; the
        //    samples occupy the leftmost n columns.
        Canvas::default()
            .marker(Marker::Braille)
            .background_color(bg)
            .x_bounds([0.0, window as f64])
            .y_bounds([0.0, 1.0])
            .paint(move |ctx| {
                if n == 0 {
                    return;
                }

                // Fill / Solid: stamp the area below the curve as a dense cloud
                // of points. For each column across [0, n-1] (stepped by
                // FILL_X_STEP), interpolate the curve y and emit every point
                // (x, y') for y' from 0 up to that y (stepped by FILL_Y_STEP).
                if matches!(self.shape, AreaShape::Solid | AreaShape::Fill) {
                    let mut area_pts: Vec<(f64, f64)> = Vec::new();
                    let x_max = if n >= 2 { (n - 1) as f64 } else { 0.0 };
                    let mut x = 0.0_f64;
                    while x <= x_max {
                        let y_top = curve_y_at(samples, x);
                        let mut y = 0.0_f64;
                        while y <= y_top {
                            area_pts.push((x, y));
                            y += FILL_Y_STEP;
                        }
                        // Always include the top point itself for a crisp edge.
                        area_pts.push((x, y_top));
                        x += FILL_X_STEP;
                    }
                    // Ensure the final column is covered exactly.
                    if x_max > 0.0 {
                        let y_top = curve_y_at(samples, x_max);
                        let mut y = 0.0_f64;
                        while y <= y_top {
                            area_pts.push((x_max, y));
                            y += FILL_Y_STEP;
                        }
                        area_pts.push((x_max, y_top));
                    }
                    ctx.draw(&Points { coords: &area_pts, color: fill_color });
                }

                // Solid / Line: draw the top-edge line as short segments
                // between consecutive samples.
                if matches!(self.shape, AreaShape::Solid | AreaShape::Line) {
                    for i in 1..n {
                        let x1 = (i - 1) as f64;
                        let y1 = samples[i - 1];
                        let x2 = i as f64;
                        let y2 = samples[i];
                        ctx.draw(&Line { x1, y1, x2, y2, color: line_color });
                    }
                    // For a single sample, draw a single dot so Line isn't empty.
                    if n == 1 {
                        ctx.draw(&Points { coords: &[(0.0, samples[0])], color: line_color });
                    }
                }
            })
            .render(area, buf);

        // 4. Optional label, drawn into the row just below the chart.
        if let Some(label) = &self.label
            && area.height > 0
        {
            // Place the label on the bottom row of the area, centered.
            let label_y = area.y + area.height - 1;
            let label_len = label.chars().count() as u16;
            let label_x = area.x + area.width.saturating_sub(label_len) / 2;
            let right = area.x + area.width;
            for (x, ch) in (label_x..).zip(label.chars()) {
                if x >= right {
                    break;
                }
                buf[(x, label_y)].set_symbol(ch.to_string().as_str()).set_fg(line_color).set_bg(bg);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    const W: u16 = 48;
    const H: u16 = 10;

    /// Render the widget (driving the oscillator for `ticks` frames) into a
    /// fresh buffer and return it along with the final state.
    fn render(
        window: usize,
        theme: Theme,
        shape: AreaShape,
        ticks: u64,
        width: u16,
        height: u16,
    ) -> (Buffer, AreaChartState) {
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        let widget = AreaChart::new().window(window).shape(shape).theme(theme);
        let mut state = AreaChartState::new(window);
        for _ in 0..ticks {
            state.tick();
        }
        StatefulWidget::render(widget, Rect::new(0, 0, width, height), &mut buf, &mut state);
        (buf, state)
    }

    /// Count non-blank cells in a buffer (cells whose symbol isn't a single space).
    fn non_blank(buf: &Buffer) -> usize {
        buf.content.iter().filter(|c| c.symbol() != " ").count()
    }

    #[test]
    fn renders_without_panicking_after_ticks() {
        let (buf, _) = render(64, Theme::Cyberpunk, AreaShape::Solid, 40, W, H);
        assert!(non_blank(&buf) > 0, "chart should draw something after ticks");
    }

    #[test]
    fn zero_area_does_not_panic() {
        let widget = AreaChart::new().window(32);
        let mut state = AreaChartState::new(32);
        let mut buf = Buffer::empty(Rect::ZERO);
        // Must be a no-op, not a panic.
        StatefulWidget::render(widget, Rect::ZERO, &mut buf, &mut state);
    }

    #[test]
    fn tick_advances_clock_and_grows_samples() {
        let mut state = AreaChartState::new(16);
        let before = state.tick_count();
        assert_eq!(state.sample_count(), 1, "starts with one baseline sample");
        state.tick();
        assert_eq!(state.tick_count(), before + 1);
        assert_eq!(state.sample_count(), 2);
        // Tick past the window cap; samples must stay ≤ window.
        for _ in 0..50 {
            state.tick();
        }
        assert!(state.sample_count() <= 16, "samples must respect the window cap");
    }

    #[test]
    fn push_clamps_and_caps_window() {
        let mut state = AreaChartState::new(4);
        // Over-range values clamp into [0, 1].
        state.push(999.0);
        assert!((state.value() - 1.0).abs() < 1e-9, "over-range should clamp to 1.0");
        state.push(-50.0);
        assert!((state.value() - 0.0).abs() < 1e-9, "negative should clamp to 0.0");
        // Overflow beyond the window is trimmed.
        state.push(0.2);
        state.push(0.3);
        state.push(0.4);
        assert!(state.sample_count() <= 4, "rolling buffer must respect the window cap");
    }

    #[test]
    fn oscillator_stays_in_range() {
        for t in 0..1000_u32 {
            let v = AreaChartState::oscillator(t as f64);
            assert!(
                (0.0..=1.0).contains(&v),
                "oscillator out of range: t={t} v={v}"
            );
        }
    }

    #[test]
    fn value_returns_latest_or_zero() {
        let mut state = AreaChartState::new(8);
        // Seeded baseline.
        assert!((state.value() - 0.5).abs() < 1e-9);
        state.push(0.42);
        assert!((state.value() - 0.42).abs() < 1e-9);
        // An empty buffer reports 0.0 (drain all samples).
        state.samples.clear();
        assert!((state.value() - 0.0).abs() < 1e-9);
    }

    #[test]
    fn shape_variants_render_without_panicking() {
        for shape in [AreaShape::Solid, AreaShape::Fill, AreaShape::Line] {
            let (buf, _) = render(64, Theme::Cyberpunk, shape, 40, W, H);
            assert!(non_blank(&buf) > 0, "{:?} shape should render non-blank", shape);
        }
    }

    #[test]
    fn builder_setters_work() {
        let w = AreaChart::new()
            .window(32)
            .shape(AreaShape::Line)
            .theme(Theme::Weyland)
            .label("TELEMETRY");
        assert_eq!(w.window, 32);
        assert_eq!(w.shape, AreaShape::Line);
        assert_eq!(w.theme, Theme::Weyland);
        assert_eq!(w.label.as_deref(), Some("TELEMETRY"));
    }

    #[test]
    fn default_is_cyberpunk() {
        let w = AreaChart::default();
        assert_eq!(w.window, 64);
        assert_eq!(w.theme, Theme::Cyberpunk);
        assert!(w.label.is_none());
    }

    #[test]
    fn default_shape_is_solid() {
        let w = AreaChart::default();
        assert_eq!(w.shape, AreaShape::Solid);
    }

    #[test]
    fn window_clamps_to_two() {
        let w = AreaChart::new().window(0);
        assert_eq!(w.window, 2, "window must clamp to ≥ 2");
        let s = AreaChartState::new(0);
        assert_eq!(s.sample_count(), 1);
    }

    #[test]
    fn curve_y_at_interpolates() {
        let samples = vec![0.0, 1.0, 0.0];
        assert!((curve_y_at(&samples, 0.0) - 0.0).abs() < 1e-9);
        assert!((curve_y_at(&samples, 1.0) - 1.0).abs() < 1e-9);
        // Midpoint between sample 0 and 1 → 0.5.
        assert!((curve_y_at(&samples, 0.5) - 0.5).abs() < 1e-9);
        // Empty / single-sample guards.
        assert!((curve_y_at(&[], 0.0) - 0.0).abs() < 1e-9);
        assert!((curve_y_at(&[0.7], 5.0) - 0.7).abs() < 1e-9);
    }

    #[test]
    fn label_renders() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 24, 6));
        let widget = AreaChart::new().window(32).label("PWR");
        let mut state = AreaChartState::new(32);
        state.tick();
        StatefulWidget::render(widget, Rect::new(0, 0, 24, 6), &mut buf, &mut state);
        // Find a 'P' cell from the label somewhere on the bottom row.
        let mut found_p = false;
        for x in 0..24 {
            if buf[(x, 5)].symbol() == "P" {
                found_p = true;
            }
        }
        assert!(found_p, "label 'PWR' should render");
    }

    #[test]
    fn render_across_many_ticks_does_not_panic() {
        // Smoke test: render repeatedly across ticks, ensuring stability.
        let mut state = AreaChartState::new(40);
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        for _ in 0..200 {
            state.tick();
            let widget = AreaChart::new().window(40).theme(Theme::Fallout);
            StatefulWidget::render(widget, Rect::new(0, 0, W, H), &mut buf, &mut state);
        }
        assert!(non_blank(&buf) > 0);
    }
}
