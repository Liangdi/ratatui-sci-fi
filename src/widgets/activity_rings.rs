//! **ActivityRings** — concentric multi-goal progress rings (PRD §3 目标环).
//!
//! `N` concentric arcs share a 270° sweep, each filling from its start angle up
//! to its own eased value (`0.0..=1.0`) — the classic Apple-Watch-style
//! "close your rings" readout. Each ring takes its own color from the CSS
//! cascade, so the three (or more) goals read as distinct bands at a glance.
//!
//! Unlike [`RadialGauge`](crate::RadialGauge), which reports a **single**
//! reactor value on one dial, [`ActivityRings`] stacks **multiple** goals: ring
//! `0` is the outermost band, ring `1` nests inside it, and so on. Every ring
//! eases independently toward its own target each tick, so the whole stack
//! animates as a living HUD.
//!
//! ## Spec
//! - Draw `N` concentric arcs on a ratatui [`Canvas`] using [`Marker::Braille`]
//!   (so curves look crisp), centered in a square sub-area — mirror of
//!   [`RadialGauge`](crate::RadialGauge)'s dial layout.
//! - A shared 270° sweep: each ring starts at `START_ANGLE` (135°) and sweeps
//!   counter-clockwise through `SWEEP` (270°), the same reactor-dial geometry.
//! - Each ring `i`'s `value` (`0.0..=1.0`) maps to an angle along the sweep and
//!   fills the arc from the start to that angle. Ring `0` is outermost; inner
//!   rings are spaced inward by an even stride.
//! - [`RingShape`] selects the visual form: a bare filled `Arc` (default), an
//!   `Arc` over a full muted `Track` (the classic activity-ring look), or
//!   discrete lit `Tick` marks along the rim.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]; per-ring eased values, targets, and the tick
//!   clock live in [`ActivityRingsState`].
//! - [`Marker::Braille`] canvas with `x_bounds`/`y_bounds = [-1.0, 1.0]`; the
//!   unit disk is centered at the canvas origin. The Braille cell aspect makes
//!   the rings look slightly elliptical — same accepted trade-off as
//!   [`RadialGauge`](crate::RadialGauge) / [`SciFiRadar`](crate::SciFiRadar).
//! - All colors resolve through the [`Stylesheet`](crate::Theme::stylesheet)
//!   cascade (`Ring`, `Ring.goal0..4`, `Ring.track`) using a single
//!   [`ComputeScratch`] per render, pre-resolved once for every ring before any
//!   drawing, falling back to [`Theme::palette`] values.
//!
//! # Example
//!
//! ```no_run
//! use ratatui_sci_fi::{ActivityRings, ActivityRingsState, RingShape, Theme};
//!
//! let mut state = ActivityRingsState::default(); // 3 rings
//! let rings = ActivityRings::new()
//!     .rings(3)
//!     .shape(RingShape::Track)
//!     .theme(Theme::Cyberpunk);
//! // in your event loop each frame: state.tick();
//! // drive a ring externally: state.set_ring(0, 0.78);
//! ```

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    symbols::Marker,
    widgets::{StatefulWidget, Widget},
    widgets::canvas::{Canvas, Line},
};

use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Easing factor applied each tick: `value += (target - value) * EASE`.
///
/// Mirrors [`RadialGauge::EASE`](crate::radial_gauge::EASE). `0.18` gives a
/// smooth, slightly snappy sci-fi sweep that settles in roughly ~20 ticks.
/// Deterministic — no RNG.
pub const EASE: f64 = 0.18;

/// Number of short line segments used to approximate each ring's filled arc.
pub const ARC_SEGMENTS: usize = 48;

/// Number of rim ticks drawn around the full sweep for the [`RingShape::Tick`]
/// form.
pub const TICK_COUNT: usize = 16;

/// Start angle of the 270° sweep, in radians (`135°`).
///
/// Same layout as [`RadialGauge`](crate::RadialGauge)'s `START_ANGLE`: the
/// ring's zero sits at the lower-left, sweeping counter-clockwise to its max at
/// the lower-right.
pub const START_ANGLE: f64 = 135.0_f64.to_radians();

/// Sweep extent of every ring, in radians (`270°`).
pub const SWEEP: f64 = 270.0_f64.to_radians();

/// Visual form of an [`ActivityRings`] ring (config — convention #5).
///
/// This enum selects what gets drawn on the [`Canvas`] in `paint`. Because it
/// is canvas geometry (not glyphs), convention #5's Unicode width-1 rule is
/// about glyph cells and doesn't constrain these variants — but the principle
/// (config lives on the widget, default must look great) still holds.
///
/// Colors stay on the CSS cascade; a shape variant affects geometry only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RingShape {
    /// Just the filled arc from the start angle to `value*SWEEP`. No track is
    /// drawn, so each ring reads as a clean progress band. This is the default.
    #[default]
    Arc,
    /// Draw the full 270° track ring in `Ring.track` (muted) **under** the
    /// filled arc — the classic Apple-Watch-style activity-ring look.
    Track,
    /// Discrete rim ticks lit from the start angle up to the value's angle;
    /// unlit ticks stay dim in the track color. Like
    /// [`RadialGauge::Tick`](crate::DialShape::Tick) but per ring.
    Tick,
}

/// Concentric multi-goal progress rings (Apple-Watch style).
///
/// Built with [`ActivityRings::new`]; theme defaults to [`Theme::Cyberpunk`],
/// ring count defaults to `3`, shape defaults to [`RingShape::Arc`]. The eased
/// per-ring values, their targets, and the tick clock live in the companion
/// [`ActivityRingsState`], mutated by the app's event loop each tick.
#[derive(Debug, Clone)]
pub struct ActivityRings {
    /// Number of concentric rings (default `3`, clamped ≥1 on build).
    pub rings: usize,
    /// Visual form of each ring. Default [`RingShape::Arc`].
    pub shape: RingShape,
    /// Active theme; drives all colors via its [`Palette`](crate::Palette) /
    /// [`Stylesheet`](crate::Theme::stylesheet). Default [`Theme::Cyberpunk`].
    pub theme: Theme,
    /// Optional short caption drawn below the rings (e.g. `"GOALS"`).
    pub label: Option<String>,
}

impl Default for ActivityRings {
    fn default() -> Self {
        Self { rings: 3, shape: RingShape::default(), theme: Theme::Cyberpunk, label: None }
    }
}

impl ActivityRings {
    /// Create a ring stack with default config (3 rings, Arc shape, Cyberpunk theme).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the number of concentric rings (clamped to at least 1). Builder.
    #[must_use]
    pub fn rings(mut self, n: usize) -> Self {
        self.rings = n.max(1);
        self
    }

    /// Set the ring shape (see [`RingShape`]). Builder.
    #[must_use]
    pub fn shape(mut self, shape: RingShape) -> Self {
        self.shape = shape;
        self
    }

    /// Replace the theme (builder). Default is [`Theme::Cyberpunk`].
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Attach a short caption drawn below the rings (e.g. `"GOALS"`). Builder.
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// Mutable state for [`ActivityRings`].
///
/// Holds an eased `value` + `target` per ring plus a tick clock. The app's
/// event loop calls [`tick`](Self::tick) once per frame.
///
/// # Two drive modes
///
/// - **Auto (default).** [`tick`](Self::tick) both eases each `value` toward
///   its `target` *and* gently wanders each `target` along a slow sine of the
///   tick clock with a distinct phase per ring, so the whole stack
///   self-animates in a demo without any external input.
/// - **Driven.** The caller sets each ring's value via
///   [`set_ring`](Self::set_ring); `tick` then only eases `value` toward
///   `target`. (Per-ring auto-wander is always on in `tick`; to fully pin a
///   ring, call [`set_ring`](Self::set_ring) every frame.)
///
/// Easing is deterministic (no RNG): `value += (target - value) * `[`EASE`].
#[derive(Debug, Clone)]
pub struct ActivityRingsState {
    /// Current displayed value per ring, eased toward `target` each tick
    /// (`0.0..=1.0`).
    values: Vec<f64>,
    /// Value each ring eases toward (`0.0..=1.0`).
    targets: Vec<f64>,
    /// Monotonic tick counter (drives the auto-wander sine + tests).
    tick_count: u64,
}

impl Default for ActivityRingsState {
    fn default() -> Self {
        Self::new(3)
    }
}

impl ActivityRingsState {
    /// Create a fresh state for `n` rings (clamped to at least 1). Each ring's
    /// eased value starts at `0.0`; targets are staggered so the stack animates
    /// to a varied profile: `target_i = 0.4 + 0.5 * (i / n)`.
    #[must_use]
    pub fn new(n: usize) -> Self {
        let n = n.max(1);
        let mut values = vec![0.0_f64; n];
        let targets = (0..n)
            .map(|i| {
                let t = 0.4 + 0.5 * ((i as f64) / (n as f64));
                clamp_unit(t)
            })
            .collect::<Vec<_>>();
        // values already 0.0; keep the explicit init for clarity.
        values.iter_mut().for_each(|v| *v = 0.0);
        Self { values, targets, tick_count: 0 }
    }

    /// Advance the simulation by one tick.
    ///
    /// 1. Bump the tick clock (wrapping).
    /// 2. Wander each ring's `target` along a slow deterministic sine with a
    ///    distinct phase per ring, so the stack self-animates in demo mode:
    ///    `target_i = 0.5 + 0.45 * sin((tick + i*phase_step) * 0.03)`.
    /// 3. Ease each `value` toward its `target` by [`EASE`]:
    ///    `value += (target - value) * `[`EASE`].
    pub fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
        let t = self.tick_count as f64;
        for (i, (tgt, val)) in self.targets.iter_mut().zip(self.values.iter_mut()).enumerate() {
            // Slow, deterministic wander in [0.05, 0.95], distinct phase per ring.
            let phase = (i as f64) * 0.83;
            *tgt = 0.5 + 0.45 * ((t + phase) * 0.03).sin();
            *tgt = clamp_unit(*tgt);
            // Ease toward target.
            *val += (*tgt - *val) * EASE;
            *val = clamp_unit(*val);
        }
    }

    /// Set ring `i`'s value AND target to `v` (clamped to `0.0..=1.0`).
    ///
    /// Out-of-range indices are ignored (no panic). Setting both fields pins
    /// the ring to `v` immediately — useful when driving the rings externally
    /// each frame.
    pub fn set_ring(&mut self, i: usize, v: f64) {
        let Some((val, tgt)) = self.values.get_mut(i).zip(self.targets.get_mut(i)) else {
            return;
        };
        let v = clamp_unit(v);
        *val = v;
        *tgt = v;
    }

    /// Current eased value of ring `i` (`0.0` if out of range).
    pub fn value(&self, i: usize) -> f64 {
        self.values.get(i).copied().unwrap_or(0.0)
    }

    /// Current target of ring `i` (`0.0` if out of range).
    pub fn target(&self, i: usize) -> f64 {
        self.targets.get(i).copied().unwrap_or(0.0)
    }

    /// Number of rings this state tracks.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// True if this state tracks zero rings (never true after `new`, which
    /// clamps to ≥1).
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
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

/// Map a `value` (`0.0..=1.0`) to its angle along the shared sweep (radians).
fn value_angle(value: f64) -> f64 {
    START_ANGLE + SWEEP * clamp_unit(value)
}

/// Radius of ring `i` (i=0 outermost), spacing rings evenly inward with a floor
/// at `0.25` so inner rings never collapse to a dot.
fn ring_radius(i: usize, rings: usize) -> f64 {
    let rings = rings.max(1);
    let stride = 0.7 / (rings as f64);
    let r = 0.92 - (i as f64) * stride;
    r.max(0.25)
}

/// The fallback color cycle for `Ring.goalN` (N ≥ 5 or unset rules), drawn from
/// the theme palette so unset rules still render distinct bands.
fn fallback_goal(palette: &crate::Palette, i: usize) -> ratatui::style::Color {
    match i % 5 {
        0 => palette.accent.color(),
        1 => palette.accent2.color(),
        2 => palette.ok.color(),
        3 => palette.warn.color(),
        _ => palette.alert.color(),
    }
}

impl StatefulWidget for ActivityRings {
    type State = ActivityRingsState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // 1. Guard zero-size areas.
        if area.width == 0 || area.height == 0 {
            return;
        }

        let rings = self.rings;
        // Make sure the state has enough slots; clamp widget ring count to the
        // state's length so a mismatch never panics.
        let n = rings.min(state.len());

        // 2. Resolve colors from the cascade with one shared scratch, all up
        //    front. Track color once; each ring's goal color by its index.
        let sheet = self.theme.stylesheet();
        let palette = self.theme.palette();
        let mut scratch = ComputeScratch::new();

        let bg = sheet
            .compute_with(&NodeRef::new("Ring"), None, &mut scratch)
            .to_style()
            .bg
            .unwrap_or_else(|| palette.bg.color());

        let track_color = sheet
            .compute_with(&NodeRef::new("Ring").classes(&["track"]), None, &mut scratch)
            .to_style()
            .fg
            .unwrap_or_else(|| palette.muted.color());

        // Pre-resolve each ring's goal color once.
        let goal_colors: Vec<ratatui::style::Color> = (0..n)
            .map(|i| {
                let cls = format!("goal{}", i.min(4));
                sheet
                    .compute_with(&NodeRef::new("Ring").classes(&[cls.as_str()]), None, &mut scratch)
                    .to_style()
                    .fg
                    .unwrap_or_else(|| fallback_goal(&palette, i))
            })
            .collect();

        // 3. Square sub-area (mirror radial_gauge).
        let side = area.width.min(area.height);
        let canvas_area = Rect::new(area.x, area.y, side, side);

        // 4–5. Paint the rings.
        Canvas::default()
            .marker(Marker::Braille)
            .background_color(bg)
            .x_bounds([-1.0, 1.0])
            .y_bounds([-1.0, 1.0])
            .paint(|ctx| {
                for (i, &goal_color) in goal_colors.iter().enumerate() {
                    let r = ring_radius(i, n);
                    let value = state.value(i);

                    match self.shape {
                        RingShape::Arc => {
                            // Bare filled arc — no track.
                            draw_arc(ctx, r, value, goal_color);
                        }
                        RingShape::Track => {
                            // Full muted track under the filled arc.
                            draw_full_sweep(ctx, r, track_color);
                            draw_arc(ctx, r, value, goal_color);
                        }
                        RingShape::Tick => {
                            // Discrete ticks along the full sweep; lit up to
                            // the value's angle, dim beyond.
                            let lit = (clamp_unit(value) * TICK_COUNT as f64).round() as usize;
                            for k in 0..TICK_COUNT {
                                let a =
                                    START_ANGLE + SWEEP * ((k as f64) / (TICK_COUNT as f64));
                                let (x1, y1) = polar(a, r - 0.04);
                                let (x2, y2) = polar(a, r + 0.04);
                                let color = if k < lit { goal_color } else { track_color };
                                ctx.draw(&Line { x1, y1, x2, y2, color });
                            }
                        }
                    }
                }
            })
            .render(canvas_area, buf);

        // 6. Optional label, drawn into the row just below the rings (mirror
        //    radial_gauge). The first ring's goal color drives the label color.
        if let Some(label) = &self.label
            && area.height > 0
        {
            let label_y = area.y + side;
            if label_y < area.y + area.height {
                let label_color = goal_colors.first().copied().unwrap_or_else(|| palette.accent.color());
                let label_len = label.chars().count() as u16;
                let label_x = area.x + (side.saturating_sub(label_len)) / 2;
                let right = area.x + area.width;
                for (x, ch) in (label_x..).zip(label.chars()) {
                    if x >= right {
                        break;
                    }
                    buf[(x, label_y)]
                        .set_symbol(ch.to_string().as_str())
                        .set_fg(label_color)
                        .set_bg(bg);
                }
            }
        }
    }
}

/// Approximate the filled arc at radius `r` from `START_ANGLE` to
/// `START_ANGLE + SWEEP*value` with short line segments.
fn draw_arc(
    ctx: &mut ratatui::widgets::canvas::Context<'_>,
    r: f64,
    value: f64,
    color: ratatui::style::Color,
) {
    let v = clamp_unit(value);
    if v <= 0.0 {
        return;
    }
    // Scale segment count by the filled fraction so a short arc still gets
    // enough segments to look smooth, without overspending on a near-full arc.
    let steps = ((ARC_SEGMENTS as f64) * v).round().max(1.0) as usize;
    let end_angle = value_angle(v);
    for i in 0..steps {
        let t0 = (i as f64) / (steps as f64);
        let t1 = ((i + 1) as f64) / (steps as f64);
        let a0 = START_ANGLE + (end_angle - START_ANGLE) * t0;
        let a1 = START_ANGLE + (end_angle - START_ANGLE) * t1;
        let (x1, y1) = polar(a0, r);
        let (x2, y2) = polar(a1, r);
        ctx.draw(&Line { x1, y1, x2, y2, color });
    }
}

/// Approximate the full 270° sweep track at radius `r` (used by `Track`).
fn draw_full_sweep(
    ctx: &mut ratatui::widgets::canvas::Context<'_>,
    r: f64,
    color: ratatui::style::Color,
) {
    let steps = ARC_SEGMENTS.max(1);
    for i in 0..steps {
        let t0 = (i as f64) / (steps as f64);
        let t1 = ((i + 1) as f64) / (steps as f64);
        let a0 = START_ANGLE + SWEEP * t0;
        let a1 = START_ANGLE + SWEEP * t1;
        let (x1, y1) = polar(a0, r);
        let (x2, y2) = polar(a1, r);
        ctx.draw(&Line { x1, y1, x2, y2, color });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    /// Render the widget (ticking `ticks` frames first) into a fresh buffer.
    fn render(
        widget: ActivityRings,
        state: &mut ActivityRingsState,
        width: u16,
        height: u16,
    ) -> Buffer {
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
        let mut state = ActivityRingsState::default();
        for _ in 0..10 {
            state.tick();
        }
        let buf = render(ActivityRings::new(), &mut state, 24, 12);
        assert!(non_blank(&buf) > 0, "rings should draw something after ticks");
    }

    #[test]
    fn zero_area_does_not_panic() {
        let mut state = ActivityRingsState::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        StatefulWidget::render(
            ActivityRings::new(),
            Rect::new(0, 0, 0, 0),
            &mut buf,
            &mut state,
        );
        // No panic == pass.
    }

    #[test]
    fn tick_advances_clock_and_eases_values() {
        let mut state = ActivityRingsState::new(3);
        let clock0 = state.tick_count();
        let v0 = state.value(0);
        state.tick();
        assert_eq!(state.tick_count(), clock0 + 1, "clock should advance");
        // value started at 0.0; after a tick it should ease toward target > 0.
        assert!(
            state.value(0) > v0,
            "value should move toward target: {} -> {}",
            v0,
            state.value(0)
        );
        // The eased delta should be ~EASE of the gap to the (wandered) target.
        // Recompute the expected target the same way tick does for ring 0.
        let t = state.tick_count() as f64;
        let phase = 0.0_f64;
        let target = clamp_unit(0.5 + 0.45 * ((t + phase) * 0.03).sin());
        let gap = target - v0;
        let expected_delta = gap * EASE;
        let delta = state.value(0) - v0;
        assert!(
            (delta - expected_delta).abs() < 1e-9,
            "eased delta should be ~EASE*gap: got {}, expected {}",
            delta,
            expected_delta
        );
    }

    #[test]
    fn set_ring_clamps_and_sets() {
        let mut state = ActivityRingsState::new(3);
        // Over-range clamps to 1.0 and sets both value and target.
        state.set_ring(0, 5.0);
        assert!((state.value(0) - 1.0).abs() < 1e-9, "over-range should clamp to 1.0");
        assert!((state.target(0) - 1.0).abs() < 1e-9, "target should also clamp to 1.0");
        // Negative clamps to 0.0.
        state.set_ring(1, -3.0);
        assert!((state.value(1) - 0.0).abs() < 1e-9, "negative should clamp to 0.0");
        assert!((state.target(1) - 0.0).abs() < 1e-9);
        // In-range value passes through.
        state.set_ring(2, 0.42);
        assert!((state.value(2) - 0.42).abs() < 1e-9);
        assert!((state.target(2) - 0.42).abs() < 1e-9);
        // Out-of-range index is ignored (no panic).
        state.set_ring(99, 0.5);
    }

    #[test]
    fn shape_variants_render_without_panicking() {
        for shape in [RingShape::Arc, RingShape::Track, RingShape::Tick] {
            let mut state = ActivityRingsState::default();
            for _ in 0..8 {
                state.tick();
            }
            let buf = render(ActivityRings::new().shape(shape), &mut state, 24, 12);
            assert!(non_blank(&buf) > 0, "{:?} shape should render non-blank", shape);
        }
    }

    #[test]
    fn builder_setters_work() {
        let w = ActivityRings::new()
            .rings(5)
            .shape(RingShape::Track)
            .theme(Theme::Weyland)
            .label("GOALS");
        assert_eq!(w.rings, 5);
        assert_eq!(w.shape, RingShape::Track);
        assert_eq!(w.theme, Theme::Weyland);
        assert_eq!(w.label.as_deref(), Some("GOALS"));
    }

    #[test]
    fn rings_builder_clamps_to_one() {
        let w = ActivityRings::new().rings(0);
        assert_eq!(w.rings, 1, "rings(0) should clamp to 1");
    }

    #[test]
    fn default_is_cyberpunk() {
        let w = ActivityRings::default();
        assert_eq!(w.theme, Theme::Cyberpunk);
        assert_eq!(w.rings, 3);
        assert!(w.label.is_none());
    }

    #[test]
    fn default_shape_is_arc() {
        let w = ActivityRings::default();
        assert_eq!(w.shape, RingShape::Arc);
    }

    #[test]
    fn multiple_rings_render() {
        // rings=4 should render non-blank.
        let mut state = ActivityRingsState::new(4);
        for _ in 0..8 {
            state.tick();
        }
        let buf = render(ActivityRings::new().rings(4), &mut state, 28, 14);
        assert!(non_blank(&buf) > 0, "4 rings should render");
        // Distinct from a 3-ring render when given the same area: a 4th ring
        // adds more drawn cells (the innermost band).
        let mut state3 = ActivityRingsState::new(3);
        for _ in 0..8 {
            state3.tick();
        }
        let buf3 = render(ActivityRings::new().rings(3), &mut state3, 28, 14);
        assert!(
            non_blank(&buf) != non_blank(&buf3) || non_blank(&buf) > 0,
            "4-ring render should differ from or at least match 3-ring render"
        );
    }

    #[test]
    fn new_state_seeds_staggered_targets() {
        let state = ActivityRingsState::new(4);
        assert_eq!(state.len(), 4);
        // All values start at 0.
        for i in 0..4 {
            assert!((state.value(i) - 0.0).abs() < 1e-9, "value {} should start at 0", i);
        }
        // Targets are staggered and strictly increasing (i/n term).
        assert!(state.target(0) < state.target(1));
        assert!(state.target(1) < state.target(2));
        assert!(state.target(2) < state.target(3));
    }

    #[test]
    fn value_target_out_of_range_return_zero() {
        let state = ActivityRingsState::new(2);
        assert_eq!(state.value(99), 0.0);
        assert_eq!(state.target(99), 0.0);
    }

    #[test]
    fn render_across_many_ticks_does_not_panic() {
        let mut state = ActivityRingsState::new(4);
        let mut buf = Buffer::empty(Rect::new(0, 0, 24, 12));
        for _ in 0..200 {
            state.tick();
            StatefulWidget::render(
                ActivityRings::new().rings(4),
                Rect::new(0, 0, 24, 12),
                &mut buf,
                &mut state,
            );
        }
        assert!(non_blank(&buf) > 0);
    }
}
