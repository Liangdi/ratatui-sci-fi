//! **Compass** — heading indicator (PRD §3 罗盘/方位).
//!
//! A compass rose with a pointer that eases toward a target heading in degrees
//! (`0..360`, where `0° = N`, `90° = E`, `180° = S`, `270° = W`). The pointer
//! **eases** along the *shorter* angular path each tick — it never spins the
//! long way around when crossing the 0/360 wraparound — so the widget is
//! stateful: [`CompassState::tick`] is what advances the motion.
//!
//! Contrast with [`SciFiRadar`] (a rotating *sweep scan* searching for blips)
//! versus **Compass** (a *heading pointer* indicating where you're facing).
//!
//! ## Spec
//! - Draw a circular rose on a ratatui [`Canvas`] using [`Marker::Braille`]
//!   (so curves look crisp), centered in a square sub-area, plus degree tick
//!   marks every ~30° and the four cardinal letters (N/E/S/W) overlaid as
//!   buffer-cell writes around the rim.
//! - The current `heading` is shown by one of [`CompassShape`] — a two-pointer
//!   needle (default), a single arrow, or a chevron near the rim.
//! - `0°` points North (up); heading degrees map to the canvas angle via
//!   `(90° - heading)` in radians so the geometry orients correctly (canvas y
//!   is up).
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]; the eased `heading`, its `target`, and the
//!   tick clock live in [`CompassState`].
//! - [`Marker::Braille`] canvas with `x_bounds`/`y_bounds = [-1.0, 1.0]`; the
//!   unit disk is centered at the canvas origin. The Braille cell aspect makes
//!   the circle look slightly elliptical — that's expected and accepted (the
//!   same trade-off [`SciFiRadar`] and [`RadialGauge`] make).
//! - Shortest-path easing: [`ease_angle`] takes the signed angular delta
//!   normalized to `[-180, 180]`, so crossing `0/360` moves the short way.
//! - All colors resolve through the [`Stylesheet`](crate::Theme::stylesheet)
//!   cascade (`Compass`, `Compass.needle`, `Compass.mark`, `Compass.cardinal`)
//!   using a single [`ComputeScratch`] per render, falling back to
//!   [`Theme::palette`] values.
//!
//! [`SciFiRadar`]: crate::SciFiRadar
//! [`RadialGauge`]: crate::RadialGauge
//!
//! # Example
//!
//! ```no_run
//! use ratatui_sci_fi::{Compass, CompassShape, CompassState, Theme};
//!
//! let mut state = CompassState::default();
//! let compass = Compass::new()
//!     .shape(CompassShape::Needle)
//!     .theme(Theme::Cyberpunk);
//! // in your event loop each frame: state.tick();
//! // to point it at a heading: state.set_heading(45.0);
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

/// Easing factor applied each tick: `heading += delta * EASE` where `delta` is
/// the shortest signed angular step toward `target`.
///
/// `0.18` gives a smooth, slightly snappy sci-fi sweep that settles in roughly
/// ~20 ticks. Deterministic — no RNG.
pub const EASE: f64 = 0.18;

/// Radius of the outer rose circle.
const ROSE_R: f64 = 0.95;

/// Inner radius of the degree tick marks (they run from `TICK_R_IN` to `TICK_R_OUT`).
const TICK_R_IN: f64 = 0.85;
/// Outer radius of the degree tick marks.
const TICK_R_OUT: f64 = 0.95;

/// Spacing between degree tick marks, in degrees.
const TICK_STEP: i32 = 30;

/// How the heading is indicated (config — convention #5).
///
/// This enum selects what gets drawn on the [`Canvas`] as the pointer. Because
/// it is canvas geometry (not glyphs), convention #5's Unicode width-1 rule is
/// about glyph cells and doesn't constrain these variants — but the principle
/// (config lives on the widget, default must look great) still holds.
///
/// Colors stay on the CSS cascade; a shape variant affects pointer geometry only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CompassShape {
    /// A two-pointer needle — a line from `polar(angle, -0.7)` through the
    /// center to `polar(angle, 0.85)`. The forward half (toward the heading)
    /// is drawn in the needle color; the aft half is dimmer. The default.
    #[default]
    Needle,
    /// A single arrow — a line from the center to `polar(angle, 0.85)` plus
    /// two short lines forming an arrowhead at the tip.
    Arrow,
    /// A chevron (`>` rotated to the heading) drawn near the rim.
    Chevron,
}

/// A compass rose heading indicator that eases toward a target heading.
///
/// Built with [`Compass::new`]; theme defaults to [`Theme::Cyberpunk`], shape
/// defaults to [`CompassShape::Needle`]. The eased heading, its target, and the
/// tick clock live in the companion [`CompassState`], mutated by the app's
/// event loop each tick.
#[derive(Debug, Clone)]
pub struct Compass {
    /// How the heading is indicated. Default [`CompassShape::Needle`].
    pub shape: CompassShape,
    /// Active theme; drives all colors via its [`Palette`](crate::Palette) /
    /// [`Stylesheet`](crate::Theme::stylesheet). Default [`Theme::Cyberpunk`].
    pub theme: Theme,
    /// Optional short caption drawn below the rose (e.g. `"HEADING"`).
    pub label: Option<String>,
}

impl Default for Compass {
    fn default() -> Self {
        Self { shape: CompassShape::default(), theme: Theme::Cyberpunk, label: None }
    }
}

impl Compass {
    /// Create a compass with default config (Needle shape, Cyberpunk theme).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the pointer shape (see [`CompassShape`]). Builder.
    #[must_use]
    pub fn shape(mut self, shape: CompassShape) -> Self {
        self.shape = shape;
        self
    }

    /// Replace the theme (builder). Default is [`Theme::Cyberpunk`].
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Attach a short caption drawn below the rose (e.g. `"HEADING"`). Builder.
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// Mutable state for [`Compass`].
///
/// Holds the eased `heading` (degrees `0..360`), its `target`, and a tick
/// clock. The app's event loop calls [`tick`](Self::tick) once per frame.
///
/// # Two drive modes
///
/// - **Auto (default).** [`tick`](Self::tick) both eases `heading` toward
///   `target` *and* gently wanders `target` along a slow sine of the tick
///   clock, so the compass self-animates in a demo without any external input.
///   The wander uses distinct sine ranges so it sweeps around the full dial.
/// - **Driven.** Call [`set_heading`](Self::set_heading) to point the compass
///   at a specific heading; `tick` then only eases `heading` toward `target`.
///
/// Easing is deterministic (no RNG) and always takes the *shorter* angular
/// path (see [`ease_angle`]).
#[derive(Debug, Clone)]
pub struct CompassState {
    /// Current displayed heading, eased toward `target` each tick (`0..360`).
    heading: f64,
    /// Heading the compass eases toward (`0..360`).
    target: f64,
    /// Monotonic tick counter (drives the auto-wander sine + tests).
    tick_count: u64,
}

impl Default for CompassState {
    fn default() -> Self {
        Self { heading: 0.0, target: 90.0, tick_count: 0 }
    }
}

impl CompassState {
    /// Create a fresh state at the default position (`heading = 0.0`,
    /// `target = 90.0`).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance the simulation by one tick.
    ///
    /// 1. Bump the tick clock (wrapping).
    /// 2. Wander `target` along a slow deterministic sine so the compass
    ///    self-animates in demo mode: `target = 180.0 + 170.0 * sin(tick*0.013)`
    ///    (sweeps roughly `10°..350°`).
    /// 3. Ease `heading` toward `target` along the *shorter* angular path by
    ///    [`EASE`], keeping `heading` in `[0, 360)`.
    pub fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);

        // Slow, deterministic wander sweeping a wide arc (~10°..350°).
        self.target = normalize_angle(180.0 + 170.0 * ((self.tick_count as f64) * 0.013).sin());

        // Ease toward target along the shorter angular path.
        self.heading = ease_angle(self.heading, self.target, EASE);
    }

    /// Point the compass at `deg`: sets BOTH `heading` and `target` to the
    /// given heading, normalized into `[0, 360)`.
    pub fn set_heading(&mut self, deg: f64) {
        let n = normalize_angle(deg);
        self.heading = n;
        self.target = n;
    }

    /// Current eased heading (`0..360`).
    pub fn heading(&self) -> f64 {
        self.heading
    }

    /// Current target heading (`0..360`).
    pub fn target(&self) -> f64 {
        self.target
    }

    /// Current tick clock value.
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }
}

/// Wrap an angle (in degrees) into `[0, 360)`.
fn normalize_angle(deg: f64) -> f64 {
    let mut v = deg % 360.0;
    if v < 0.0 {
        v += 360.0;
    }
    v
}

/// Ease `current` toward `target` along the *shorter* angular path.
///
/// Computes the signed delta `(target - current)` normalized to `[-180, 180]`,
/// then returns `current + delta * ease` re-wrapped into `[0, 360)`. This means
/// easing from `350°` toward `10°` moves up through `0`, not the long way down
/// through `180`.
fn ease_angle(current: f64, target: f64, ease: f64) -> f64 {
    let mut delta = (target - current) % 360.0;
    if delta < -180.0 {
        delta += 360.0;
    } else if delta > 180.0 {
        delta -= 360.0;
    }
    normalize_angle(current + delta * ease)
}

/// Polar-to-cartesian helper: `(r*cos(angle), r*sin(angle))`.
fn polar(angle: f64, r: f64) -> (f64, f64) {
    (r * angle.cos(), r * angle.sin())
}

/// Map a heading in degrees to the canvas angle in radians.
///
/// `0°` (North) must point up. On the canvas y is up and x is right, so a
/// heading of `0°` → canvas angle `90°` (up), `90°` → `0°` (right), etc.
/// Returns `canvas_angle = (90° - heading)` in radians.
fn heading_to_canvas(heading_deg: f64) -> f64 {
    (90.0 - normalize_angle(heading_deg)).to_radians()
}

impl StatefulWidget for Compass {
    type State = CompassState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // 1. Guard zero-size areas.
        if area.width == 0 || area.height == 0 {
            return;
        }

        // 2. Resolve colors from the cascade with one shared scratch.
        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();

        let rose_color = sheet
            .compute_with(&NodeRef::new("Compass"), None, &mut scratch)
            .to_style()
            .fg
            .unwrap_or_else(|| self.theme.palette().accent.color());
        let needle_color = sheet
            .compute_with(&NodeRef::new("Compass").classes(&["needle"]), None, &mut scratch)
            .to_style()
            .fg
            .unwrap_or_else(|| self.theme.palette().alert.color());
        let mark_color = sheet
            .compute_with(&NodeRef::new("Compass").classes(&["mark"]), None, &mut scratch)
            .to_style()
            .fg
            .unwrap_or_else(|| self.theme.palette().muted.color());
        let cardinal_color = sheet
            .compute_with(&NodeRef::new("Compass").classes(&["cardinal"]), None, &mut scratch)
            .to_style()
            .fg
            .unwrap_or_else(|| self.theme.palette().fg.color());
        let bg = sheet
            .compute_with(&NodeRef::new("Compass"), None, &mut scratch)
            .to_style()
            .bg
            .unwrap_or_else(|| self.theme.palette().bg.color());

        // 3. Square sub-area (mirror radial_gauge / scifi_radar).
        let side = area.width.min(area.height);
        let canvas_area = Rect::new(area.x, area.y, side, side);

        let canvas_angle = heading_to_canvas(state.heading());

        // 4. Paint the rose + pointer.
        Canvas::default()
            .marker(Marker::Braille)
            .background_color(bg)
            .x_bounds([-1.0, 1.0])
            .y_bounds([-1.0, 1.0])
            .paint(|ctx| {
                // Outer rose circle.
                ctx.draw(&Circle { x: 0.0, y: 0.0, radius: ROSE_R, color: rose_color });

                // Degree tick marks every TICK_STEP degrees.
                for deg in (0..360).step_by(TICK_STEP as usize) {
                    let a = heading_to_canvas(deg as f64);
                    let (x1, y1) = polar(a, TICK_R_IN);
                    let (x2, y2) = polar(a, TICK_R_OUT);
                    ctx.draw(&Line { x1, y1, x2, y2, color: mark_color });
                }

                // Pointer by shape, rotated to the heading's canvas angle.
                match self.shape {
                    CompassShape::Needle => {
                        // Forward half (toward heading), center → tip.
                        let (fx, fy) = polar(canvas_angle, 0.85);
                        ctx.draw(&Line {
                            x1: 0.0,
                            y1: 0.0,
                            x2: fx,
                            y2: fy,
                            color: needle_color,
                        });
                        // Aft half (dimmer), center → tail. The mark color is
                        // muted relative to the needle color.
                        let (ax, ay) = polar(canvas_angle, -0.7);
                        ctx.draw(&Line {
                            x1: 0.0,
                            y1: 0.0,
                            x2: ax,
                            y2: ay,
                            color: mark_color,
                        });
                        // Center hub dot.
                        ctx.draw(&Points { coords: &[(0.0, 0.0)], color: rose_color });
                    }
                    CompassShape::Arrow => {
                        // Shaft: center → tip.
                        let (tx, ty) = polar(canvas_angle, 0.85);
                        ctx.draw(&Line {
                            x1: 0.0,
                            y1: 0.0,
                            x2: tx,
                            y2: ty,
                            color: needle_color,
                        });
                        // Arrowhead: two short lines back from the tip at ±150°
                        // from the heading angle.
                        let head_len = 0.22;
                        let left = canvas_angle + (150.0_f64).to_radians();
                        let right = canvas_angle - (150.0_f64).to_radians();
                        let (lx, ly) = polar(left, head_len);
                        let (rx, ry) = polar(right, head_len);
                        ctx.draw(&Line {
                            x1: tx,
                            y1: ty,
                            x2: tx + lx,
                            y2: ty + ly,
                            color: needle_color,
                        });
                        ctx.draw(&Line {
                            x1: tx,
                            y1: ty,
                            x2: tx + rx,
                            y2: ty + ry,
                            color: needle_color,
                        });
                    }
                    CompassShape::Chevron => {
                        // A chevron (>) near the rim, pointing along the heading.
                        // Tip at r ≈ 0.78; two legs back toward center.
                        let tip_r = 0.78;
                        let leg_r = 0.28;
                        let (tx, ty) = polar(canvas_angle, tip_r);
                        let left = canvas_angle + (135.0_f64).to_radians();
                        let right = canvas_angle - (135.0_f64).to_radians();
                        let (lx, ly) = polar(left, leg_r);
                        let (rx, ry) = polar(right, leg_r);
                        ctx.draw(&Line {
                            x1: tx + lx,
                            y1: ty + ly,
                            x2: tx,
                            y2: ty,
                            color: needle_color,
                        });
                        ctx.draw(&Line {
                            x1: tx + rx,
                            y1: ty + ry,
                            x2: tx,
                            y2: ty,
                            color: needle_color,
                        });
                    }
                }
            })
            .render(canvas_area, buf);

        // 5. Overlay the four cardinal letters (N/E/S/W) into the buffer.
        //    The square canvas_area maps canvas x∈[-1,1] →
        //    [canvas_area.x, canvas_area.x + side]; y∈[-1,1] →
        //    [canvas_area.y + side, canvas_area.y] (canvas y up, buffer y down).
        let half = side as f64 / 2.0;
        let cx = canvas_area.x as f64 + half;
        let cy = canvas_area.y as f64 + half;
        // Place letters just inside the rim (r < 1 so they stay within the
        // square canvas_area; the rose is at r ≈ 0.95).
        let letter_r = 0.72;
        let cardinals = [
            ('N', 0.0),
            ('E', 90.0),
            ('S', 180.0),
            ('W', 270.0),
        ];
        for (ch, deg) in cardinals {
            let a = heading_to_canvas(deg);
            let (px, py) = polar(a, letter_r);
            // canvas (px, py) → buffer cell. Clamp into the area.
            let bx = (cx + px * half).round() as i64;
            let by = (cy - py * half).round() as i64;
            if bx >= canvas_area.x as i64
                && bx < (canvas_area.x + canvas_area.width) as i64
                && by >= canvas_area.y as i64
                && by < (canvas_area.y + canvas_area.height) as i64
            {
                let cell = &mut buf[(bx as u16, by as u16)];
                cell.set_symbol(ch.to_string().as_str()).set_fg(cardinal_color).set_bg(bg);
            }
        }

        // 6. Optional label, drawn into a thin row below the rose (mirror
        //    radial_gauge.rs label block).
        if let Some(label) = &self.label {
            let label_y = area.y + side;
            if label_y < area.y + area.height {
                let label_len = label.chars().count() as u16;
                let label_x = area.x + (side.saturating_sub(label_len)) / 2;
                let right = area.x + area.width;
                for (x, ch) in (label_x..).zip(label.chars()) {
                    if x >= right {
                        break;
                    }
                    buf[(x, label_y)]
                        .set_symbol(ch.to_string().as_str())
                        .set_fg(needle_color)
                        .set_bg(bg);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    /// Render the compass into a fresh buffer with the given state + widget.
    fn render(state: &mut CompassState, widget: Compass, width: u16, height: u16) -> Buffer {
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
        let mut state = CompassState::default();
        for _ in 0..10 {
            state.tick();
        }
        let buf = render(&mut state, Compass::new(), 20, 10);
        assert!(non_blank(&buf) > 0, "compass should draw something after ticks");
    }

    #[test]
    fn zero_area_does_not_panic() {
        let mut state = CompassState::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let widget = Compass::new();
        StatefulWidget::render(widget, Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        // No panic == pass.
    }

    #[test]
    fn tick_advances_clock_and_eases_heading() {
        // A tick must advance the clock, keep heading in range, and (because
        // auto-wander sweeps a wide arc) eventually move heading off its seed.
        let mut state = CompassState::new();
        state.set_heading(0.0);
        state.tick();
        assert_eq!(state.tick_count(), 1, "tick should advance the clock");
        assert!(
            (0.0..360.0).contains(&state.heading()),
            "heading must stay in [0,360): {}",
            state.heading()
        );
        // Over several ticks the wandered target sweeps a wide arc, so the
        // eased heading should leave its 0° seed.
        let mut moved = false;
        for _ in 0..20 {
            state.tick();
            if (state.heading() - 0.0).abs() > 0.5 {
                moved = true;
                break;
            }
        }
        assert!(moved, "heading should ease toward the wandering target over ticks");
    }

    #[test]
    fn set_heading_normalizes() {
        let mut state = CompassState::new();
        state.set_heading(450.0);
        assert!(
            (state.heading() - 90.0).abs() < 1e-9,
            "450° should normalize to 90°, got {}",
            state.heading()
        );
        assert!(
            (state.target() - 90.0).abs() < 1e-9,
            "target should also be 90°, got {}",
            state.target()
        );

        state.set_heading(-90.0);
        assert!(
            (state.heading() - 270.0).abs() < 1e-9,
            "-90° should normalize to 270°, got {}",
            state.heading()
        );

        state.set_heading(720.0);
        assert!(
            (state.heading() - 0.0).abs() < 1e-9,
            "720° should normalize to 0°, got {}",
            state.heading()
        );
    }

    #[test]
    fn ease_angle_takes_shortest_path() {
        // Easing from 350° toward 10° should move UP through 0, not down
        // through 180. After one tick the eased value should be closer to the
        // target via the short arc.
        let eased = ease_angle(350.0, 10.0, EASE);
        // Short-arc step: delta = +20°, eased = 350 + 20*0.18 = 353.6.
        assert!(
            (eased - 353.6).abs() < 1e-9,
            "easing 350→10 should move up through 0 (toward 353.6), got {}",
            eased
        );
        // It must NOT have moved the long way (down toward 180).
        assert!(
            eased > 350.0 || eased < 10.0,
            "eased value should be on the short arc near 350/0/10, got {}",
            eased
        );

        // Reverse direction: 10° → 350° should move DOWN through 0.
        let eased2 = ease_angle(10.0, 350.0, EASE);
        // Short-arc delta = -20°, eased = 10 - 20*0.18 = 6.4.
        assert!(
            (eased2 - 6.4).abs() < 1e-9,
            "easing 10→350 should move down through 0 (toward 6.4), got {}",
            eased2
        );

        // A 180° apart case stays neutral-ish either way.
        let _ = ease_angle(0.0, 180.0, EASE);
    }

    #[test]
    fn shape_variants_render_without_panicking() {
        for shape in [CompassShape::Needle, CompassShape::Arrow, CompassShape::Chevron] {
            let mut state = CompassState::default();
            state.set_heading(45.0);
            let buf = render(&mut state, Compass::new().shape(shape), 20, 20);
            assert!(non_blank(&buf) > 0, "{:?} shape should render non-blank", shape);
        }
    }

    #[test]
    fn builder_setters_work() {
        let w = Compass::new()
            .shape(CompassShape::Arrow)
            .theme(Theme::Weyland)
            .label("HEADING");
        assert_eq!(w.shape, CompassShape::Arrow);
        assert_eq!(w.theme, Theme::Weyland);
        assert_eq!(w.label.as_deref(), Some("HEADING"));
    }

    #[test]
    fn default_is_cyberpunk() {
        let w = Compass::default();
        assert_eq!(w.theme, Theme::Cyberpunk);
    }

    #[test]
    fn default_shape_is_needle() {
        let w = Compass::default();
        assert_eq!(w.shape, CompassShape::Needle);
    }

    #[test]
    fn default_state_is_heading_zero_target_ninety() {
        let s = CompassState::default();
        assert!((s.heading() - 0.0).abs() < 1e-9);
        assert!((s.target() - 90.0).abs() < 1e-9);
        assert_eq!(s.tick_count(), 0);
    }

    #[test]
    fn heading_to_canvas_orients_correctly() {
        // 0° (N) → canvas angle 90° (up): cos=0, sin=1.
        let a = heading_to_canvas(0.0);
        assert!((a - std::f64::consts::FRAC_PI_2).abs() < 1e-9, "N should map to 90°");
        // 90° (E) → canvas angle 0° (right): cos=1, sin=0.
        let a = heading_to_canvas(90.0);
        assert!(a.abs() < 1e-9, "E should map to 0°");
        // 180° (S) → canvas angle -90° (down).
        let a = heading_to_canvas(180.0);
        assert!((a + std::f64::consts::FRAC_PI_2).abs() < 1e-9, "S should map to -90°");
        // 270° (W) → canvas angle -180° (left).
        let a = heading_to_canvas(270.0);
        assert!((a + std::f64::consts::PI).abs() < 1e-9, "W should map to 180°/-180°");
    }

    #[test]
    fn normalize_angle_handles_wraparound() {
        assert!((normalize_angle(0.0) - 0.0).abs() < 1e-9);
        assert!((normalize_angle(360.0) - 0.0).abs() < 1e-9);
        assert!((normalize_angle(450.0) - 90.0).abs() < 1e-9);
        assert!((normalize_angle(-90.0) - 270.0).abs() < 1e-9);
        assert!((normalize_angle(-360.0) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn label_renders_below_rose() {
        let mut state = CompassState::default();
        state.set_heading(45.0);
        // Use a taller area so the label row (y == side) fits below the rose.
        let buf = render(&mut state, Compass::new().label("HDG"), 10, 14);
        // side = min(10,14) = 10; label_y = 10 (row 10), within 14. Find "H".
        let mut found_h = false;
        for y in 0..14 {
            for x in 0..10 {
                if buf[(x, y)].symbol() == "H" {
                    found_h = true;
                }
            }
        }
        assert!(found_h, "label 'HDG' should render; got no 'H' cell");
        assert!(non_blank(&buf) > 0);
    }

    #[test]
    fn render_across_many_ticks_does_not_panic() {
        // Smoke test: render repeatedly across ticks, ensuring stability.
        let mut state = CompassState::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 24, 12));
        for _ in 0..200 {
            state.tick();
            let widget = Compass::new().theme(Theme::Fallout);
            StatefulWidget::render(widget, Rect::new(0, 0, 24, 12), &mut buf, &mut state);
        }
        assert!(non_blank(&buf) > 0);
        // Heading must stay in range across many ticks.
        assert!(
            (0.0..360.0).contains(&state.heading()),
            "heading drifted out of range: {}",
            state.heading()
        );
    }

    #[test]
    fn cardinal_letters_overlay_into_buffer() {
        // A large enough area should contain at least one of N/E/S/W.
        let mut state = CompassState::default();
        state.set_heading(45.0);
        let buf = render(&mut state, Compass::new(), 24, 24);
        let symbols: Vec<&str> = buf.content.iter().map(|c| c.symbol()).collect();
        let has_cardinal = symbols.iter().any(|s| matches!(*s, "N" | "E" | "S" | "W"));
        assert!(has_cardinal, "at least one cardinal letter should be drawn");
    }
}
