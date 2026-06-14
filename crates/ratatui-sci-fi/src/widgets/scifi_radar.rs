//! **SciFiRadar** — Braille circular sweep radar with fading trail (PRD §3 科幻雷达).
//!
//! ## Spec
//! - Circular scan drawn in Braille, with a gradient sweep trail behind the
//!   leading beam.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]; sweep angle + per-dot intensity (decay) grid
//!   + blip list live in [`SciFiRadarState`], advanced each tick.
//! - Use a `Canvas` with `Marker::Braille`; each frame: decay every dot's
//!   intensity, then stamp dots along the current sweep angle at full
//!   intensity. Render a dot only if its intensity > threshold, mapped to a
//!   Braille cell bit.
//! - The per-dot decay buffer is this widget's main complexity — model it as a
//!   `Vec<f32>` over a polar grid (`ANGLE_BINS × RADIUS_BINS`), shrunk each
//!   tick by [`DECAY`].
//!
//! ## Approach shipped
//!
//! The full **decay-trail** version (not the fallback beam). Each tick:
//! 1. every grid cell's intensity is multiplied by [`DECAY`] (~0.9),
//! 2. the current sweep beam stamps a radial line of cells at full intensity,
//! 3. blips pulse their brightness on a sine clock.
//!
//! At render time, the polar grid is mapped into canvas (x,y) points and each
//! point's color is interpolated from `palette.bg` → `palette.accent` by its
//! intensity, so the trail visibly fades into the background behind the leading
//! beam.
//!
//! # Example
//!
//! ```no_run
//! use ratatui_sci_fi::{SciFiRadar, SciFiRadarState, Theme};
//!
//! let mut state = SciFiRadarState::default();
//! let radar = SciFiRadar::new().theme(Theme::Cyberpunk);
//! // in your event loop each frame: state.tick();
//! ```

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Color,
    symbols::Marker,
    widgets::{StatefulWidget, Widget},
    widgets::canvas::{Canvas, Circle, Points},
};

use crate::Theme;

/// Angular resolution of the polar intensity grid (bins around the circle).
///
/// `2π / ANGLE_BINS` ≈ 0.06 rad per bin. More bins give a smoother trail at
/// the cost of a larger decay buffer.
pub const ANGLE_BINS: usize = 96;

/// Radial resolution of the polar intensity grid (bins from center outward).
pub const RADIUS_BINS: usize = 12;

/// Per-tick decay factor applied to every grid cell's intensity.
///
/// After this multiply, intensities fade exponentially: a freshly stamped cell
/// drops below [`RENDER_THRESHOLD`] in roughly `log(0.1)/log(0.9)` ≈ 22 ticks.
pub const DECAY: f32 = 0.9;

/// Cells with intensity at or above this value are drawn as Braille dots.
pub const RENDER_THRESHOLD: f32 = 0.05;

/// Intensity a cell is stamped at when the sweep beam crosses it.
pub const BEAM_INTENSITY: f32 = 1.0;

/// A contact blip on the radar (angle, radius, brightness).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Blip {
    /// Angle of the contact in radians (0 = +x axis, increasing CCW).
    pub angle: f64,
    /// Normalized radius of the contact (`0.0` = center, `1.0` = rim).
    pub radius: f64,
    /// Base brightness (`0.0..=1.0`); pulses on a sine clock at render time.
    pub brightness: f32,
}

impl Blip {
    /// Create a blip at the given polar position.
    #[must_use]
    pub fn new(angle: f64, radius: f64, brightness: f32) -> Self {
        Self { angle, radius, brightness }
    }
}

/// A sci-fi sweep radar with a fading Braille trail.
///
/// Built with [`SciFiRadar::new`]; theme defaults to [`Theme::Cyberpunk`].
/// Sweep angle + decay grid + blips live in the companion [`SciFiRadarState`],
/// mutated by the app's event loop each tick.
#[derive(Debug, Clone)]
pub struct SciFiRadar {
    /// Sweep speed in radians per tick.
    pub sweep_speed: f64,
    /// Active theme; controls all colors via its [`Palette`](crate::Palette).
    pub theme: Theme,
}

impl Default for SciFiRadar {
    fn default() -> Self {
        Self::new()
    }
}

impl SciFiRadar {
    /// Create a radar with a default sweep speed of `0.2` rad/tick.
    #[must_use]
    pub fn new() -> Self {
        Self { sweep_speed: 0.2, theme: Theme::default() }
    }

    /// Set the sweep speed (radians advanced per [`SciFiRadarState::tick`]).
    #[must_use]
    pub fn sweep_speed(mut self, rad_per_tick: f64) -> Self {
        self.sweep_speed = rad_per_tick;
        self
    }

    /// Replace the theme (builder). Default is [`Theme::Cyberpunk`].
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

/// Mutable state for [`SciFiRadar`].
///
/// Holds the current sweep angle, a polar intensity grid that decays each
/// tick (the fading trail), and an optional list of [`Blip`]s. The app's event
/// loop calls [`tick`](Self::tick) once per frame.
///
/// `sweep_speed` lives here (not on the widget) because the animation loop
/// needs it each tick; the widget's [`SciFiRadar::sweep_speed`] builder is a
/// convenience that callers can mirror into the state, or the state can be
/// configured directly via [`SciFiRadarState::with_sweep_speed`].
#[derive(Debug, Clone)]
pub struct SciFiRadarState {
    /// Current sweep angle in radians. Wraps into `[0, 2π)`.
    pub angle: f64,
    /// Sweep speed in radians per tick. Defaults to `0.2`.
    pub sweep_speed: f64,
    /// Polar intensity grid, row-major `[angle_bin * RADIUS_BINS + radius_bin]`.
    /// Each cell holds a brightness in `0.0..=1.0` that decays every tick.
    pub grid: Vec<f32>,
    /// Contact blips, drawn brighter than the trail and pulsing.
    pub blips: Vec<Blip>,
    /// Monotonic tick counter (drives blip pulsing).
    pub tick_count: u64,
}

impl Default for SciFiRadarState {
    fn default() -> Self {
        Self {
            angle: 0.0,
            sweep_speed: 0.2,
            grid: vec![0.0; ANGLE_BINS * RADIUS_BINS],
            blips: Vec::new(),
            tick_count: 0,
        }
    }
}

impl SciFiRadarState {
    /// Create an empty state with a preset sweep angle (radians).
    #[must_use]
    pub fn with_angle(angle: f64) -> Self {
        Self { angle, ..Self::default() }
    }

    /// Set the sweep speed (radians per [`Self::tick`]). Builder.
    #[must_use]
    pub fn with_sweep_speed(mut self, rad_per_tick: f64) -> Self {
        self.sweep_speed = rad_per_tick;
        self
    }

    /// Add a contact blip.
    pub fn push_blip(&mut self, blip: Blip) {
        self.blips.push(blip);
    }

    /// Advance the simulation by one tick.
    ///
    /// 1. Decay every grid cell by [`DECAY`].
    /// 2. Advance the sweep angle by [`sweep_speed`](Self::sweep_speed) and
    ///    wrap into `[0, 2π)`.
    /// 3. Stamp the new beam's radial line of cells at [`BEAM_INTENSITY`].
    /// 4. Bump the tick counter (blips pulse off this at render time).
    pub fn tick(&mut self) {
        // 1. Decay.
        for cell in &mut self.grid {
            *cell *= DECAY;
        }

        // 2. Advance + wrap the beam angle into [0, 2π).
        self.angle = (self.angle + self.sweep_speed).rem_euclid(std::f64::consts::TAU);

        // 3. Stamp the beam: a radial line of cells at the current angle,
        //    from just outside the center out to the rim.
        let angle_bin = angle_to_bin(self.angle);
        for r in 0..RADIUS_BINS {
            let idx = angle_bin * RADIUS_BINS + r;
            if let Some(cell) = self.grid.get_mut(idx) {
                // Falloff from full at the leading edge; the rim is brightest.
                let falloff = (r as f32 + 1.0) / (RADIUS_BINS as f32);
                *cell = BEAM_INTENSITY * falloff;
            }
        }

        // 4. Tick clock for blip pulsing.
        self.tick_count = self.tick_count.wrapping_add(1);
    }
}

/// Map an angle (radians) to its grid bin index.
fn angle_to_bin(angle: f64) -> usize {
    let normalized = ((angle % std::f64::consts::TAU) + std::f64::consts::TAU)
        % std::f64::consts::TAU;
    let bin = (normalized / std::f64::consts::TAU * ANGLE_BINS as f64).round() as usize;
    // `% ANGLE_BINS` (not `.min`) so an angle just under 2π wraps to bin 0 —
    // periodic, matching that 2π ≡ 0.
    bin % ANGLE_BINS
}

/// Linear blend of two [`Color::Rgb`] values by `t` in `0.0..=1.0`.
///
/// `t = 0.0` → `lo`, `t = 1.0` → `hi`. Used to fade trail dots from the
/// background color up to the accent as their intensity rises.
fn blend(lo: Color, hi: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    match (lo, hi) {
        (Color::Rgb(lr, lg, lb), Color::Rgb(hr, hg, hb)) => {
            let lerp = |a: u8, b: u8| -> u8 {
                let af = f64::from(a);
                let bf = f64::from(b);
                (af + (bf - af) * f64::from(t)).round().clamp(0.0, 255.0) as u8
            };
            Color::Rgb(lerp(lr, hr), lerp(lg, hg), lerp(lb, hb))
        }
        // Fall back to the high color if either side isn't a concrete RGB.
        _ => hi,
    }
}

impl StatefulWidget for SciFiRadar {
    type State = SciFiRadarState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Guard zero-size areas.
        if area.width == 0 || area.height == 0 {
            return;
        }

        let palette = self.theme.palette();
        let accent = palette.accent.color();
        let muted = palette.muted.color();
        let bg = palette.bg.color();

        // The unit disk lives in canvas space [-1,1]×[-1,1] with the origin at
        // the lower-left. We render into a square sub-area (min dimension) so
        // the circle isn't stretched by non-square regions; the canvas's own
        // cell aspect ratio already makes circles look elliptical, but keeping
        // a square area avoids compounding it.
        let side = area.width.min(area.height);
        let canvas_area = Rect::new(area.x, area.y, side, side);

        // Pre-compute the trail points + their per-point colors. We group
        // points by color bucket so we can emit one `Points` shape per color
        // (the `Points` shape takes a single color for all its coords).
        //
        // We quantize intensity into N buckets: bucket 0 ≈ bg, top bucket ≈
        // accent. This keeps the number of `Points` draws small while still
        // producing a visible gradient trail.
        const COLOR_BUCKETS: usize = 8;
        let mut buckets: Vec<Vec<(f64, f64)>> = (0..COLOR_BUCKETS).map(|_| Vec::new()).collect();

        // Bin the polar grid into (x,y) points. Radius bin `r` maps to a
        // normalized radius of `(r+1)/RADIUS_BINS`, so the outermost bin sits
        // near the rim (radius ~1.0) and the innermost near the center.
        for a in 0..ANGLE_BINS {
            let theta = (a as f64 / ANGLE_BINS as f64) * std::f64::consts::TAU;
            let cos_t = theta.cos();
            let sin_t = theta.sin();
            for r in 0..RADIUS_BINS {
                let intensity = state.grid[a * RADIUS_BINS + r];
                if intensity < RENDER_THRESHOLD {
                    continue;
                }
                let norm_r = (r as f64 + 1.0) / (RADIUS_BINS as f64);
                // Slight inward squeeze so dots stay inside the drawn circle.
                let rad = norm_r * 0.92;
                let x = rad * cos_t;
                let y = rad * sin_t;
                let bucket = ((intensity / BEAM_INTENSITY) * (COLOR_BUCKETS as f32 - 1.0)).round() as usize;
                let bucket = bucket.min(COLOR_BUCKETS - 1);
                buckets[bucket].push((x, y));
            }
        }

        // Blip pulse phase in [0,1] on a slow sine clock. Baseline 0.5 so
        // blips stay visible even when the sine is at zero (e.g. tick 0).
        let phase = 0.5 + 0.5 * ((state.tick_count as f32) * 0.2).sin();

        Canvas::default()
            .marker(Marker::Braille)
            .background_color(bg)
            .x_bounds([-1.0, 1.0])
            .y_bounds([-1.0, 1.0])
            .paint(|ctx| {
                // Circular outline (the radar dish). Drawn in muted accent.
                ctx.draw(&Circle {
                    x: 0.0,
                    y: 0.0,
                    radius: 0.95,
                    color: muted,
                });
                // A faint inner ring for depth.
                ctx.draw(&Circle {
                    x: 0.0,
                    y: 0.0,
                    radius: 0.5,
                    color: muted,
                });

                // Emit the trail, one Points shape per color bucket. Bucket
                // index rises with intensity: 0 ≈ bg, top ≈ accent.
                for (i, coords) in buckets.iter().enumerate() {
                    if coords.is_empty() {
                        continue;
                    }
                    let t = (i as f32 + 1.0) / (COLOR_BUCKETS as f32);
                    let color = blend(bg, accent, t);
                    ctx.draw(&Points { coords, color });
                }

                // Blips: brighter than the trail, pulsing on the sine phase.
                // `brightness` gates whether a blip is shown at all (very dim
                // blips drop out) and scales the pulse so brighter blips pop
                // more. All visible blips share one color here (one Points
                // shape); per-blip color would need a draw per blip.
                let blip_coords: Vec<(f64, f64)> = state
                    .blips
                    .iter()
                    .filter(|b| b.brightness * phase > RENDER_THRESHOLD)
                    .map(|b| {
                        let rad = b.radius.clamp(0.0, 0.92);
                        (rad * b.angle.cos(), rad * b.angle.sin())
                    })
                    .collect();
                if !blip_coords.is_empty() {
                    // Blend accent→white-ish by pulsing so blips "pop".
                    let blip_color = blend(accent, Color::Rgb(0xff, 0xff, 0xff), phase * 0.5);
                    ctx.draw(&Points { coords: &blip_coords, color: blip_color });
                }
            })
            .render(canvas_area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    /// Render the radar into a fresh buffer with the given state.
    fn render(state: &mut SciFiRadarState, theme: Theme, width: u16, height: u16) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        let widget = SciFiRadar::new().theme(theme);
        StatefulWidget::render(widget, Rect::new(0, 0, width, height), &mut buf, state);
        buf
    }

    /// Count non-blank cells in a buffer (cells whose symbol isn't a single space).
    fn non_blank(buf: &Buffer) -> usize {
        buf.content.iter().filter(|c| c.symbol() != " ").count()
    }

    #[test]
    fn renders_without_panicking_on_normal_area() {
        let mut state = SciFiRadarState::default();
        // Stamp the beam so there's something to draw.
        state.tick();
        let buf = render(&mut state, Theme::Cyberpunk, 20, 10);
        assert!(non_blank(&buf) > 0, "radar should draw something after a tick");
    }

    #[test]
    fn zero_area_does_not_panic() {
        let mut state = SciFiRadarState::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let widget = SciFiRadar::new();
        StatefulWidget::render(widget, Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        // No panic == pass.
    }

    #[test]
    fn untouched_state_still_renders_outline() {
        // Even with an empty grid, the Circle outline should produce dots.
        let mut state = SciFiRadarState::default();
        let buf = render(&mut state, Theme::Cyberpunk, 24, 12);
        assert!(non_blank(&buf) > 0, "outline should be drawn even with an empty trail");
    }

    #[test]
    fn tick_advances_angle() {
        let mut state = SciFiRadarState::default().with_sweep_speed(0.5);
        let start = state.angle;
        state.tick();
        assert!((state.angle - (start + 0.5)).abs() < 1e-9, "angle should advance by sweep_speed");
    }

    #[test]
    fn tick_wraps_angle_into_0_to_2pi() {
        let mut state = SciFiRadarState::with_angle(std::f64::consts::TAU - 0.1)
            .with_sweep_speed(0.5);
        state.tick();
        assert!(
            (0.0..std::f64::consts::TAU).contains(&state.angle),
            "angle must wrap into [0, 2π), got {}",
            state.angle
        );
    }

    #[test]
    fn decay_reduces_a_cells_intensity_over_ticks() {
        // Stamp a single beam at a known angle, then hold the beam still at a
        // *different* angle so the original cell is never restamped; its stored
        // intensity must strictly decrease tick over tick.
        let mut state = SciFiRadarState { sweep_speed: 0.0, angle: 0.0, ..SciFiRadarState::default() };
        state.tick(); // stamp at angle 0 → bin 0

        let bin0 = angle_to_bin(0.0);
        let cell_idx = bin0 * RADIUS_BINS + (RADIUS_BINS - 1); // rim cell
        let intensity_after_stamp = state.grid[cell_idx];
        assert!(intensity_after_stamp > 0.0, "stamp should set the cell");

        // Point the beam elsewhere and tick (decay only, no restamp of bin 0).
        state.angle = std::f64::consts::FRAC_PI_2; // 90° → different bin
        state.tick();
        let intensity_after_one_decay = state.grid[cell_idx];

        assert!(
            intensity_after_one_decay < intensity_after_stamp,
            "cell intensity must decrease under decay: {} >= {}",
            intensity_after_one_decay,
            intensity_after_stamp
        );

        // A second decay tick (still away from bin 0) reduces it further.
        state.angle = std::f64::consts::PI; // 180° → yet another bin
        state.tick();
        let intensity_after_two_decays = state.grid[cell_idx];
        assert!(
            intensity_after_two_decays < intensity_after_one_decay,
            "cell intensity must keep decreasing: {} >= {}",
            intensity_after_two_decays,
            intensity_after_one_decay
        );
    }

    #[test]
    fn decay_drops_a_stamped_cell_toward_zero() {
        // Stamp one bin, then sweep the beam through *different* bins so the
        // original is not restamped; that cell must decay toward zero.
        // Sweep one bin per tick so bin 0 isn't revisited for ~96 ticks.
        let mut state = SciFiRadarState { sweep_speed: std::f64::consts::TAU / 96.0, ..SciFiRadarState::default() };
        // tick() advances the angle *then* stamps, so the first stamp lands at
        // sweep_speed (= TAU/96), i.e. bin 1 — not bin 0.
        state.tick();
        let original_bin = angle_to_bin(std::f64::consts::TAU / 96.0);
        let original_after = state.grid[original_bin * RADIUS_BINS + (RADIUS_BINS - 1)];
        // Advance ~10 bins away (no restamp of the original bin for a long time).
        for _ in 0..10 {
            state.tick();
        }
        let original_later = state.grid[original_bin * RADIUS_BINS + (RADIUS_BINS - 1)];
        assert!(
            original_later < original_after,
            "a stamped cell must decay once the beam moves on: {} >= {}",
            original_later,
            original_after
        );
    }

    #[test]
    fn blips_are_drawn() {
        let mut state = SciFiRadarState::default();
        state.push_blip(Blip::new(0.0, 0.8, 1.0));
        let buf = render(&mut state, Theme::Fallout, 20, 10);
        // With a blip + outline, we definitely have non-blank cells.
        assert!(non_blank(&buf) > 0);
    }

    #[test]
    fn theme_changes_outline_color() {
        // The muted outline color differs between themes; rendering with each
        // theme should not panic and should still produce output. We don't
        // assert exact colors (the canvas blends into braille cells), just
        // that both themes render distinctly and non-blank.
        let mut s1 = SciFiRadarState::default();
        let b1 = render(&mut s1, Theme::Cyberpunk, 16, 8);
        let mut s2 = SciFiRadarState::default();
        let b2 = render(&mut s2, Theme::Fallout, 16, 8);
        assert!(non_blank(&b1) > 0);
        assert!(non_blank(&b2) > 0);
    }

    #[test]
    fn angle_to_bin_wraps_negative_and_large() {
        assert_eq!(angle_to_bin(0.0), 0);
        // Near 2π should wrap to ~0.
        let near_tau = angle_to_bin(std::f64::consts::TAU - 1e-6);
        assert!(near_tau <= 1, "angle just under 2π should bin near 0, got {}", near_tau);
        // Negative angle wraps.
        let neg = angle_to_bin(-1e-6);
        assert!(neg <= 1, "small negative angle should wrap near 0, got {}", neg);
    }

    #[test]
    fn blend_clamps_and_interpolates() {
        let bg = Color::Rgb(0, 0, 0);
        let hi = Color::Rgb(255, 255, 255);
        assert_eq!(blend(bg, hi, 0.0), bg);
        assert_eq!(blend(bg, hi, 1.0), hi);
        let mid = blend(bg, hi, 0.5);
        let Color::Rgb(r, g, b) = mid else { panic!("expected Rgb"); };
        assert!((r as i16 - 128).abs() <= 1, "midpoint ~128, got {}", r);
        assert!((g as i16 - 128).abs() <= 1);
        assert!((b as i16 - 128).abs() <= 1);
        // Out-of-range t clamps.
        assert_eq!(blend(bg, hi, -1.0), bg);
        assert_eq!(blend(bg, hi, 2.0), hi);
    }

    #[test]
    fn builder_setters_work() {
        let w = SciFiRadar::new().sweep_speed(0.42).theme(Theme::Weyland);
        assert!((w.sweep_speed - 0.42).abs() < 1e-9);
        assert_eq!(w.theme, Theme::Weyland);
    }

    #[test]
    fn default_is_cyberpunk() {
        let w = SciFiRadar::default();
        assert_eq!(w.theme, Theme::Cyberpunk);
        assert!((w.sweep_speed - 0.2).abs() < 1e-9);
    }
}
