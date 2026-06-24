//! **RadialGauge** — circular reactor-core dial gauge (PRD §3 仪表盘).
//!
//! A single value (`0.0..=1.0`) is read out as a needle / filled arc / lit rim
//! of ticks sweeping a 270° dial, like a starship power-output HUD. The needle
//! **eases** toward a target value each tick (a smooth sci-fi sweep), so the
//! widget is stateful — [`RadialGaugeState::tick`] is what advances the motion.
//!
//! ## Spec
//! - Draw a circular dial on a ratatui [`Canvas`] using [`Marker::Braille`]
//!   (so curves look crisp), centered in a square sub-area.
//! - A 270° sweep: the gauge starts at `START_ANGLE` (135°) and sweeps
//!   counter-clockwise through `SWEEP` (270°) back around, mirroring a classic
//!   analog reactor dial.
//! - `value` (`0.0..=1.0`) maps to an angle along the sweep: the current value
//!   is indicated by [`DialShape`] — a needle, a filled arc, or lit rim ticks.
//! - Level-based coloring via the CSS cascade: `value ≥ 0.6` → ok,
//!   `0.3..0.6` → warn, `< 0.3` → alert (same thresholds as [`EnergyGauge`]).
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]; the eased `value`, its `target`, and the
//!   tick clock live in [`RadialGaugeState`].
//! - [`Marker::Braille`] canvas with `x_bounds`/`y_bounds = [-1.0, 1.0]`; the
//!   unit disk is centered at the canvas origin. The Braille cell aspect makes
//!   the circle look slightly elliptical — that's expected and accepted (the
//!   same trade-off [`SciFiRadar`] makes).
//! - All colors resolve through the [`Stylesheet`](crate::Theme::stylesheet)
//!   cascade (`Dial`, `Dial.grid`, `Dial.ok`|`warn`|`alert`) using a single
//!   [`ComputeScratch`] per render, falling back to [`Theme::palette`] values.
//!
//! [`EnergyGauge`]: crate::EnergyGauge
//! [`SciFiRadar`]: crate::SciFiRadar
//!
//! # Example
//!
//! ```no_run
//! use ratatui_sci_fi::{DialShape, RadialGauge, RadialGaugeState, Theme};
//!
//! let mut state = RadialGaugeState::default();
//! let gauge = RadialGauge::new()
//!     .shape(DialShape::Needle)
//!     .theme(Theme::Cyberpunk);
//! // in your event loop each frame: state.tick();
//! // to drive it externally: state.set_target_value(0.78);
//! ```

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    symbols::Marker,
    widgets::{StatefulWidget, Widget},
    widgets::canvas::{Canvas, Circle, Line, Points},
};
#[cfg(test)]
use ratatui::style::Color;

use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Easing factor applied each tick: `value += (target - value) * EASE`.
///
/// `0.18` gives a smooth, slightly snappy sci-fi sweep that settles in roughly
/// ~20 ticks. Deterministic — no RNG.
pub const EASE: f64 = 0.18;

/// Number of rim ticks drawn around the dial.
pub const TICK_COUNT: usize = 12;

/// Number of short line segments used to approximate the filled `Arc` shape.
pub const ARC_SEGMENTS: usize = 24;

/// Start angle of the 270° sweep, in radians.
///
/// `135°` puts the dial's zero at the lower-left, sweeping counter-clockwise
/// (up and around) to its max at the lower-right — the classic reactor layout.
pub const START_ANGLE: f64 = 135.0_f64.to_radians();

/// Sweep extent of the dial, in radians (`270°`).
pub const SWEEP: f64 = 270.0_f64.to_radians();

/// How the current value is indicated on the dial rim (config — convention #5).
///
/// This enum selects what gets drawn on the [`Canvas`] in `paint`. Because it
/// is canvas geometry (not glyphs), convention #5's Unicode width-1 rule is
/// about glyph cells and doesn't constrain these variants — but the principle
/// (config lives on the widget, default must look great) still holds.
///
/// Colors stay on the CSS cascade; a shape variant affects geometry only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DialShape {
    /// A straight needle line from the center to the value's angle, plus a
    /// small center hub dot. The default — looks like a classic reactor needle.
    #[default]
    Needle,
    /// A filled arc segment swept along the rim from the gauge's start angle
    /// to the value's angle (thicker / brighter than the rim).
    Arc,
    /// Discrete rim ticks lit from the start angle up to the value's angle;
    /// unlit ticks stay dim (muted), lit ticks take the level color.
    Tick,
}

/// A circular reactor-core dial gauge that eases toward a target value.
///
/// Built with [`RadialGauge::new`]; theme defaults to [`Theme::Cyberpunk`],
/// shape defaults to [`DialShape::Needle`]. The eased value, its target, and
/// the tick clock live in the companion [`RadialGaugeState`], mutated by the
/// app's event loop each tick.
#[derive(Debug, Clone)]
pub struct RadialGauge {
    /// How the current value is indicated on the rim. Default [`DialShape::Needle`].
    pub shape: DialShape,
    /// Active theme; drives all colors via its [`Palette`](crate::Palette) /
    /// [`Stylesheet`](crate::Theme::stylesheet). Default [`Theme::Cyberpunk`].
    pub theme: Theme,
    /// Optional short caption drawn below the dial (e.g. `"REACTOR"`).
    pub label: Option<String>,
}

impl Default for RadialGauge {
    fn default() -> Self {
        Self { shape: DialShape::default(), theme: Theme::Cyberpunk, label: None }
    }
}

impl RadialGauge {
    /// Create a gauge with default config (Needle shape, Cyberpunk theme).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the dial shape (see [`DialShape`]). Builder.
    #[must_use]
    pub fn shape(mut self, shape: DialShape) -> Self {
        self.shape = shape;
        self
    }

    /// Replace the theme (builder). Default is [`Theme::Cyberpunk`].
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Attach a short caption drawn below the dial (e.g. `"REACTOR"`). Builder.
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// Mutable state for [`RadialGauge`].
///
/// Holds the eased `value`, its `target`, and a tick clock. The app's event
/// loop calls [`tick`](Self::tick) once per frame.
///
/// # Two drive modes
///
/// - **Auto (default).** With `auto == true`, [`tick`](Self::tick) both eases
///   `value` toward `target` *and* gently wanders `target` along a slow sine
///   of the tick clock, so the gauge self-animates in a demo without any
///   external input. Disable it with [`set_auto`](Self::set_auto)`(false)`.
/// - **Driven.** With `auto == false`, `tick` only eases `value` toward
///   `target`; the caller sets `target` via [`set_target_value`](Self::set_target_value).
///
/// Easing is deterministic (no RNG): `value += (target - value) * `[`EASE`].
#[derive(Debug, Clone)]
pub struct RadialGaugeState {
    /// Current displayed value, eased toward `target` each tick (`0.0..=1.0`).
    pub value: f64,
    /// Value the gauge eases toward (`0.0..=1.0`).
    pub target: f64,
    /// Monotonic tick counter (drives the auto-wander sine + tests).
    pub tick_count: u64,
    /// Whether [`tick`](Self::tick) auto-wanders `target` along a slow sine.
    /// Default `true`. Set `false` when driving `target` externally.
    pub auto: bool,
}

impl Default for RadialGaugeState {
    fn default() -> Self {
        Self { value: 0.0, target: 0.5, tick_count: 0, auto: true }
    }
}

impl RadialGaugeState {
    /// Create a fresh state at the default position (`value = 0.0`, `target = 0.5`).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Seed both `value` and `target` to `v` (clamped to `0.0..=1.0`).
    #[must_use]
    pub fn with_value(v: f64) -> Self {
        let v = clamp_unit(v);
        Self { value: v, target: v, tick_count: 0, auto: true }
    }

    /// Builder-style: set `target` (clamped into `0.0..=1.0`).
    #[must_use]
    pub fn set_target(mut self, v: f64) -> Self {
        self.target = clamp_unit(v);
        self
    }

    /// Set `target` at runtime (clamped into `0.0..=1.0`). Named
    /// `set_target_value` (not `target`) to avoid clashing with the builder.
    pub fn set_target_value(&mut self, v: f64) {
        self.target = clamp_unit(v);
    }

    /// Set the auto-wander mode (see the type-level docs for the two modes).
    pub fn set_auto(&mut self, on: bool) {
        self.auto = on;
    }

    /// Current eased value (`0.0..=1.0`).
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Current tick clock value.
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    /// Advance the simulation by one tick.
    ///
    /// 1. Bump the tick clock (wrapping).
    /// 2. If `auto`, wander `target` along a slow deterministic sine so the
    ///    gauge self-animates in demo mode:
    ///    `target = 0.5 + 0.45 * sin(tick_count * 0.03)`.
    /// 3. Ease `value` toward `target` by [`EASE`]:
    ///    `value += (target - value) * `[`EASE`].
    pub fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);

        if self.auto {
            // Slow, deterministic wander in [0.05, 0.95].
            self.target = 0.5 + 0.45 * ((self.tick_count as f64) * 0.03).sin();
        }

        // Ease toward target. When target == value this is a no-op drift-wise.
        self.value += (self.target - self.value) * EASE;
        // Clamp defensively against float drift beyond the unit interval.
        self.value = clamp_unit(self.value);
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

/// Map a `value` (`0.0..=1.0`) to its angle along the dial sweep (radians).
fn value_angle(value: f64) -> f64 {
    START_ANGLE + SWEEP * clamp_unit(value)
}

/// Pick the level CSS class for a value, matching [`EnergyGauge`]'s thresholds.
fn level_class(value: f64) -> &'static str {
    if value >= 0.6 {
        "ok"
    } else if value >= 0.3 {
        "warn"
    } else {
        "alert"
    }
}

impl StatefulWidget for RadialGauge {
    type State = RadialGaugeState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // 1. Guard zero-size areas.
        if area.width == 0 || area.height == 0 {
            return;
        }

        // 2. Resolve colors from the cascade with one shared scratch.
        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();

        let accent = sheet
            .compute_with(&NodeRef::new("Dial"), None, &mut scratch)
            .to_style()
            .fg
            .unwrap_or_else(|| self.theme.palette().accent.color());
        let grid_color = sheet
            .compute_with(&NodeRef::new("Dial").classes(&["grid"]), None, &mut scratch)
            .to_style()
            .fg
            .unwrap_or_else(|| self.theme.palette().muted.color());
        let bg = sheet
            .compute_with(&NodeRef::new("Dial"), None, &mut scratch)
            .to_style()
            .bg
            .unwrap_or_else(|| self.theme.palette().bg.color());

        let value = state.value();
        let level_cls = level_class(value);
        let level_color = sheet
            .compute_with(&NodeRef::new("Dial").classes(&[level_cls]), None, &mut scratch)
            .to_style()
            .fg
            .unwrap_or_else(|| {
                match level_cls {
                    "ok" => self.theme.palette().ok.color(),
                    "warn" => self.theme.palette().warn.color(),
                    _ => self.theme.palette().alert.color(),
                }
            });

        // 3. Square sub-area (mirror scifi_radar).
        let side = area.width.min(area.height);
        let canvas_area = Rect::new(area.x, area.y, side, side);

        let cur_angle = value_angle(value);

        // 4–5. Paint the dial.
        Canvas::default()
            .marker(Marker::Braille)
            .background_color(bg)
            .x_bounds([-1.0, 1.0])
            .y_bounds([-1.0, 1.0])
            .paint(|ctx| {
                // Outer rim + faint inner ring for depth.
                ctx.draw(&Circle { x: 0.0, y: 0.0, radius: 0.95, color: grid_color });
                ctx.draw(&Circle { x: 0.0, y: 0.0, radius: 0.6, color: grid_color });

                // Rim ticks. For the Tick shape, ticks up to the value are lit
                // with the level color; the rest stay dim (grid). For the other
                // shapes all ticks are dim and the value is shown separately.
                let lit_count = (value.clamp(0.0, 1.0) * TICK_COUNT as f64).round() as usize;
                for k in 0..=TICK_COUNT {
                    let a = START_ANGLE + SWEEP * (k as f64 / TICK_COUNT as f64);
                    let (x1, y1) = polar(a, 0.85);
                    let (x2, y2) = polar(a, 0.95);
                    let color = match self.shape {
                        DialShape::Tick if k <= lit_count => level_color,
                        _ => grid_color,
                    };
                    ctx.draw(&Line { x1, y1, x2, y2, color });
                }

                // Value indicator by shape.
                match self.shape {
                    DialShape::Needle => {
                        // Needle from center to the value's rim point.
                        let (nx, ny) = polar(cur_angle, 0.9);
                        ctx.draw(&Line { x1: 0.0, y1: 0.0, x2: nx, y2: ny, color: level_color });
                        // Center hub dot.
                        ctx.draw(&Points { coords: &[(0.0, 0.0)], color: accent });
                    }
                    DialShape::Arc => {
                        // Approximate the swept arc with short line segments at
                        // r ≈ 0.78, from the start angle to the value's angle.
                        let r = 0.78;
                        let steps = ARC_SEGMENTS.max(1);
                        for i in 0..steps {
                            let t0 = i as f64 / steps as f64;
                            let t1 = (i + 1) as f64 / steps as f64;
                            let a0 = START_ANGLE + SWEEP * clamp_unit(value) * t0;
                            let a1 = START_ANGLE + SWEEP * clamp_unit(value) * t1;
                            let (x1, y1) = polar(a0, r);
                            let (x2, y2) = polar(a1, r);
                            ctx.draw(&Line { x1, y1, x2, y2, color: level_color });
                        }
                    }
                    DialShape::Tick => {
                        // Rim ticks already lit above; nothing extra to draw.
                    }
                }
            })
            .render(canvas_area, buf);

        // 6. Optional label, drawn into a thin bottom row so it overlays
        // cleanly below the dial. Kept simple — text only, level color.
        if let Some(label) = &self.label
            && area.height > 0
        {
            let label_y = area.y + side;
            if label_y < area.y + area.height {
                // Center the label under the dial.
                crate::widgets::util::draw_centered_label(
                    buf,
                    area.x,
                    label_y,
                    side,
                    area.x + area.width,
                    label,
                    level_color,
                    bg,
                );
            }
        }
    }
}

/// Test-only helper: resolve the level color for a value through the cascade.
#[cfg(test)]
impl RadialGauge {
    fn level_color(&self, value: f64) -> Color {
        let sheet = self.theme.stylesheet();
        let cls = level_class(value);
        sheet
            .compute_with(&NodeRef::new("Dial").classes(&[cls]), None, &mut ComputeScratch::new())
            .to_style()
            .fg
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    /// Render the gauge into a fresh buffer with the given state + widget.
    fn render(state: &mut RadialGaugeState, widget: RadialGauge, width: u16, height: u16) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        StatefulWidget::render(widget, Rect::new(0, 0, width, height), &mut buf, state);
        buf
    }

    /// Count non-blank cells in a buffer (cells whose symbol isn't a single space).
    fn non_blank(buf: &Buffer) -> usize {
        buf.content.iter().filter(|c| c.symbol() != " ").count()
    }

    #[test]
    fn renders_without_panicking_on_normal_area() {
        let mut state = RadialGaugeState::default();
        state.tick();
        let buf = render(&mut state, RadialGauge::new(), 20, 10);
        assert!(non_blank(&buf) > 0, "gauge should draw something after a tick");
    }

    #[test]
    fn zero_area_does_not_panic() {
        let mut state = RadialGaugeState::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let widget = RadialGauge::new();
        StatefulWidget::render(widget, Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        // No panic == pass.
    }

    #[test]
    fn untouched_state_still_renders_rim() {
        // Even with value == 0.0 (default), the Circle outline + ticks draw.
        let mut state = RadialGaugeState::default();
        let buf = render(&mut state, RadialGauge::new(), 24, 12);
        assert!(non_blank(&buf) > 0, "rim should be drawn even at value 0.0");
    }

    #[test]
    fn tick_eases_value_toward_target() {
        // auto = false so target stays put; value should ease strictly toward it.
        let mut state = RadialGaugeState::with_value(0.0).set_target(1.0);
        state.set_auto(false);
        let before = state.value;
        state.tick();
        let after = state.value;
        assert!(
            after > before,
            "value should move toward target: {} -> {}",
            before,
            after
        );
        // The eased delta should be ~EASE of the gap.
        let gap = 1.0 - before;
        let expected_delta = gap * EASE;
        assert!(
            (after - before - expected_delta).abs() < 1e-9,
            "eased delta should be ~EASE*gap: got delta {}, expected {}",
            after - before,
            expected_delta
        );
    }

    #[test]
    fn tick_clamps_value_in_range() {
        // Drive target to extremes; value must stay within [0.0, 1.0].
        let mut state = RadialGaugeState::with_value(0.0);
        state.set_auto(false);
        state.set_target_value(1.0);
        for _ in 0..200 {
            state.tick();
            assert!(state.value >= 0.0 && state.value <= 1.0, "value out of range: {}", state.value);
        }
        state.set_target_value(0.0);
        for _ in 0..200 {
            state.tick();
            assert!(state.value >= 0.0 && state.value <= 1.0, "value out of range: {}", state.value);
        }
    }

    #[test]
    fn set_target_clamps() {
        let s = RadialGaugeState::default().set_target(5.0);
        assert!((s.target - 1.0).abs() < 1e-9, "over-range target should clamp to 1.0");
        let s = RadialGaugeState::default().set_target(-3.0);
        assert!((s.target - 0.0).abs() < 1e-9, "negative target should clamp to 0.0");
    }

    #[test]
    fn level_color_matches_thresholds() {
        let palette = Theme::Cyberpunk.palette();

        // ok (>= 0.6)
        let g = RadialGauge::new().theme(Theme::Cyberpunk);
        assert_eq!(g.level_color(0.8), palette.ok.color(), "0.8 should be ok");
        // warn (>= 0.3, < 0.6)
        assert_eq!(g.level_color(0.45), palette.warn.color(), "0.45 should be warn");
        // alert (< 0.3)
        assert_eq!(g.level_color(0.1), palette.alert.color(), "0.1 should be alert");
    }

    #[test]
    fn theme_renders_distinctly() {
        // Both themes render non-blank; we don't assert exact colors here.
        let mut s1 = RadialGaugeState::with_value(0.7);
        s1.set_auto(false);
        let b1 = render(&mut s1, RadialGauge::new().theme(Theme::Cyberpunk), 16, 8);
        let mut s2 = RadialGaugeState::with_value(0.7);
        s2.set_auto(false);
        let b2 = render(&mut s2, RadialGauge::new().theme(Theme::Fallout), 16, 8);
        assert!(non_blank(&b1) > 0, "Cyberpunk should render");
        assert!(non_blank(&b2) > 0, "Fallout should render");
    }

    #[test]
    fn shape_variants_render_without_panicking() {
        for shape in [DialShape::Needle, DialShape::Arc, DialShape::Tick] {
            let mut state = RadialGaugeState::with_value(0.6);
            state.set_auto(false);
            let buf = render(&mut state, RadialGauge::new().shape(shape), 20, 10);
            assert!(non_blank(&buf) > 0, "{:?} shape should render non-blank", shape);
        }
    }

    #[test]
    fn builder_setters_work() {
        let w = RadialGauge::new()
            .shape(DialShape::Arc)
            .theme(Theme::Weyland)
            .label("REACTOR");
        assert_eq!(w.shape, DialShape::Arc);
        assert_eq!(w.theme, Theme::Weyland);
        assert_eq!(w.label.as_deref(), Some("REACTOR"));
    }

    #[test]
    fn default_is_cyberpunk() {
        let w = RadialGauge::default();
        assert_eq!(w.theme, Theme::Cyberpunk);
    }

    #[test]
    fn default_shape_is_needle() {
        let w = RadialGauge::default();
        assert_eq!(w.shape, DialShape::Needle);
    }

    #[test]
    fn auto_mode_wanders_target() {
        // In auto mode, tick should move target off the seeded value via sine.
        let mut state = RadialGaugeState::with_value(0.5);
        let t0 = state.target;
        state.tick();
        // After one tick the wander sine has advanced; target should differ.
        // (At tick_count==1 the sine of 0.03 is small but nonzero.)
        assert!(
            (state.target - t0).abs() > 0.0,
            "auto mode should wander target: {} -> {}",
            t0,
            state.target
        );
    }

    #[test]
    fn set_auto_disables_wander() {
        let mut state = RadialGaugeState::with_value(0.5);
        state.set_auto(false);
        state.set_target_value(0.2);
        let t_before = state.target;
        state.tick();
        assert!(
            (state.target - t_before).abs() < 1e-9,
            "with auto off, target must not wander"
        );
    }

    #[test]
    fn label_renders_below_dial() {
        let mut state = RadialGaugeState::with_value(0.7);
        state.set_auto(false);
        // First render just must not panic with a label in a square area.
        let _ = render(&mut state, RadialGauge::new().label("REACTOR"), 24, 12);
        // The label row sits at y == side (the square sub-area height). With a
        // 12×12 area, side=12, but area height is 12 so label_y==12 is out of
        // bounds — use a taller area so the label row fits.
        let mut state = RadialGaugeState::with_value(0.7);
        state.set_auto(false);
        let buf = render(
            &mut state,
            RadialGauge::new().label("PWR"),
            10,
            14,
        );
        // side = min(10,14) = 10; label_y = 10 (row 10), within 14. Find "P".
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
    fn value_angle_maps_endpoints() {
        // value 0 -> START_ANGLE, value 1 -> START_ANGLE + SWEEP.
        let a0 = value_angle(0.0);
        let a1 = value_angle(1.0);
        assert!((a0 - START_ANGLE).abs() < 1e-9, "value 0 should map to START_ANGLE");
        assert!((a1 - (START_ANGLE + SWEEP)).abs() < 1e-9, "value 1 should map to START_ANGLE + SWEEP");
    }

    #[test]
    fn polar_returns_radius_scaled_cos_sin() {
        let (x, y) = polar(0.0, 1.0);
        assert!((x - 1.0).abs() < 1e-9 && y.abs() < 1e-9);
        let (x, y) = polar(std::f64::consts::FRAC_PI_2, 2.0);
        assert!(x.abs() < 1e-9 && (y - 2.0).abs() < 1e-9);
    }
}
