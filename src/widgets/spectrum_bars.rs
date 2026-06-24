//! **SpectrumBars** — animated vertical bar chart (PRD §3 频谱/能量分布).
//!
//! A sci-fi power-distribution spectrum analyzer / equalizer: `N` vertical bars
//! rise from the bottom of the area, each bar's height set by its current value
//! (`0.0..=1.0`). Each bar is colored by its value's level — nominal → warn →
//! alert — so a spiking bar reads as a warning at a glance. Cells above a bar's
//! top render dim, forming the "empty" part of the column.
//!
//! ## Spec
//! - `bars` vertical columns laid out left-to-right from `area.x`; bar `i`'s left
//!   edge is `area.x + i * (bar_width + gap)`. Layout stops once a bar would
//!   exceed `area.x + area.width` (narrow areas simply draw fewer bars).
//! - Each bar's filled height = `round(value * area.height)`, drawn from the
//!   bottom up. The topmost filled cell uses a fractional block glyph (for the
//!   smooth `Bar` shape) so bar tops look anti-aliased.
//! - Color shifts with level: `value ≥ 0.6` → ok (`Spectrum.ok`), `0.3..0.6` →
//!   warn (`Spectrum.warn`), `< 0.3` → alert (`Spectrum.alert`) — the same
//!   thresholds as [`EnergyGauge`](crate::EnergyGauge). Cells above a bar's top
//!   use `Spectrum.empty`.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]; per-bar rolling sample buffers live in
//!   [`SpectrumBarsState`], advanced each tick (or fed live via `push`).
//! - Drawn cell-by-cell directly into the [`Buffer`], with every color routed
//!   through the theme's [`Stylesheet`](crate::Theme::stylesheet) cascade: the
//!   `Spectrum` / `Spectrum.ok`|`Spectrum.warn`|`Spectrum.alert` /
//!   `Spectrum.empty` rules drive the colors. Because every rule is
//!   `var(--…)`-backed off the same palette, the rendered colors are
//!   byte-identical to reading the palette directly.
//! - Two data-feed modes:
//!     1. **External**: the app calls [`SpectrumBarsState::push`] to feed live
//!        samples per bar.
//!     2. **Self-generated (demo mode)**: the app calls
//!        [`SpectrumBarsState::tick`] each frame and the state advances its own
//!        deterministic per-bar oscillator (distinct base frequency + harmonics +
//!        slow drift), giving an equalizer-like profile. The demo uses this mode.
//!
//! # Example
//!
//! ```no_run
//! use ratatui_sci_fi::{SpectrumBars, SpectrumBarsState, Theme};
//!
//! let mut state = SpectrumBarsState::new(12, 64);
//! let bars = SpectrumBars::new().bars(12).theme(Theme::Cyberpunk);
//! // In the event loop: state.tick(); each frame, then render the widget.
//! ```

use crate::Theme;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::StatefulWidget,
};
#[cfg(test)]
use ratatui::style::Color;
use ratatui_style::{ComputeScratch, NodeRef};

/// Smooth-top fractional block ramp, low → high. Index `0` is the baseline
/// (`▁`); the last entry is the full block (`█`). Width-1 glyphs only.
const BLOCK_RAMP: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

/// Visual form of a [`SpectrumBars`] column.
///
/// Selects the glyph set used for filled body / top / empty cells; colors stay
/// on the CSS cascade (`Spectrum` / `Spectrum.ok`|`Spectrum.warn`|
/// `Spectrum.alert` / `Spectrum.empty`), untouched by this enum. The
/// [`SpectrumShape::Bar`] default gives the smooth-top equalizer look.
///
/// Every glyph is Unicode width-1 (see convention #5 at the crate root),
/// keeping the per-cell column math valid.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SpectrumShape {
    /// Smooth-top: body cells `█`, topmost filled cell a fractional block
    /// (`▁▂▃▄▅▆▇█`) chosen by the remainder, empty cells a space. This is the
    /// default equalizer look.
    #[default]
    Bar,
    /// Solid: body and top both `█`, empty a space.
    Block,
    /// `▰` filled, `▱` empty — mirrors [`EnergyGauge`](crate::EnergyGauge)'s Cell
    /// look.
    Cell,
    /// `#` filled, `.` empty — plain ASCII.
    Ascii,
}

impl SpectrumShape {
    /// The glyph for a fully-filled body cell.
    #[must_use]
    pub const fn body(self) -> &'static str {
        match self {
            Self::Bar | Self::Block => "█",
            Self::Cell => "▰",
            Self::Ascii => "#",
        }
    }

    /// The glyph for the topmost filled cell of a column.
    ///
    /// `frac` is the fractional part of `value * height` in `0.0..=1.0`; the
    /// smooth [`SpectrumShape::Bar`] shape maps it onto the 8-step block ramp,
    /// every other shape just returns its full body glyph.
    #[must_use]
    pub fn top(self, frac: f64) -> &'static str {
        match self {
            Self::Bar => {
                let clamped = frac.clamp(0.0, 1.0);
                let idx = (clamped * 7.0).round() as usize;
                BLOCK_RAMP[idx.min(7)]
            }
            Self::Block | Self::Cell | Self::Ascii => self.body(),
        }
    }

    /// The glyph for an empty cell above a bar's top.
    #[must_use]
    pub const fn empty(self) -> &'static str {
        match self {
            Self::Bar | Self::Block => " ",
            Self::Cell => "▱",
            Self::Ascii => ".",
        }
    }
}

/// An animated vertical bar spectrum analyzer / equalizer.
///
/// Immutable config lives here (`bars`, `bar_width`, `gap`, `shape`, `theme`);
/// everything that changes per frame lives in [`SpectrumBarsState`].
#[derive(Debug, Clone)]
pub struct SpectrumBars {
    /// Number of vertical bars (default `12`, clamped ≥1).
    pub bars: usize,
    /// Cells per bar (default `2`, clamped ≥1).
    pub bar_width: u16,
    /// Empty cells between bars (default `1`).
    pub gap: u16,
    /// Glyph-set form. Defaults to [`SpectrumShape::Bar`].
    pub shape: SpectrumShape,
    /// Theme whose palette drives the colors via CSS cascade. Default
    /// [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Default for SpectrumBars {
    fn default() -> Self {
        Self {
            bars: 12,
            bar_width: 2,
            gap: 1,
            shape: SpectrumShape::default(),
            theme: Theme::Cyberpunk,
        }
    }
}

impl SpectrumBars {
    /// Build a spectrum analyzer with default config (12 bars).
    pub fn new() -> Self {
        Self::default()
    }

    /// Convenience constructor with a given bar count (mirrors
    /// [`BiometricChart::new`](crate::BiometricChart::new)).
    pub fn new_with(bars: usize) -> Self {
        Self::default().bars(bars)
    }

    /// Set the number of bars (clamped to at least 1). Builder.
    #[must_use]
    pub fn bars(mut self, n: usize) -> Self {
        self.bars = n.max(1);
        self
    }

    /// Set the width of each bar in cells (clamped to at least 1). Builder.
    #[must_use]
    pub fn bar_width(mut self, w: u16) -> Self {
        self.bar_width = w.max(1);
        self
    }

    /// Set the empty gap between bars, in cells. Builder.
    #[must_use]
    pub fn gap(mut self, g: u16) -> Self {
        self.gap = g;
        self
    }

    /// Set the glyph-set form (see [`SpectrumShape`]). Builder.
    #[must_use]
    pub fn shape(mut self, s: SpectrumShape) -> Self {
        self.shape = s;
        self
    }

    /// Set the theme whose palette drives colors. Builder.
    #[must_use]
    pub fn theme(mut self, t: Theme) -> Self {
        self.theme = t;
        self
    }
}

/// Mutable state for [`SpectrumBars`].
///
/// Holds a rolling sample buffer per bar (length capped at `window`) plus a tick
/// counter that drives the self-generated oscillator. The app advances it every
/// frame via [`Self::tick`] (demo mode) or feeds live samples via [`Self::push`]
/// (external mode).
#[derive(Debug, Clone)]
pub struct SpectrumBarsState {
    /// Per-bar rolling sample buffers; each `Vec<f64>` holds the most recent
    /// up-to-`window` samples, oldest first.
    buffers: Vec<Vec<f64>>,
    /// Configured window length (max samples kept per bar).
    window: usize,
    /// Animation clock, advanced each tick; used by the self-generated
    /// oscillator when no external data is pushed.
    tick: u64,
}

impl Default for SpectrumBarsState {
    fn default() -> Self {
        Self::new(12, 64)
    }
}

impl SpectrumBarsState {
    /// Build state sized for `bars` bars and a `window`-sample rolling buffer
    /// (clamped to at least 1 bar and a window of at least 2). Buffers start
    /// seeded with a mid-range baseline so the first frame isn't empty.
    pub fn new(bars: usize, window: usize) -> Self {
        let bars = bars.max(1);
        let window = window.max(2);
        // Seed each buffer with one mid-range baseline sample so the first
        // render has at least one value per bar to draw.
        let baseline = 0.5;
        let buffers = (0..bars).map(|_| vec![baseline]).collect();
        Self { buffers, window, tick: 0 }
    }

    /// Feed a live sample for `bar_index` (external-feed mode).
    ///
    /// The value is clamped to `0.0..=1.0` and appended to that bar's rolling
    /// buffer; the oldest sample is dropped once the buffer exceeds `window`.
    /// Out-of-range indices are ignored.
    pub fn push(&mut self, bar_index: usize, value: f64) {
        let Some(buf) = self.buffers.get_mut(bar_index) else {
            return;
        };
        let clamped = value.clamp(0.0, 1.0);
        crate::widgets::util::capped_push(buf, clamped, self.window);
    }

    /// Advance the spectrum by one tick (demo / self-generated mode).
    ///
    /// Computes the next oscillating sample for every bar — a sum of sines with
    /// a distinct base frequency per bar plus a couple of harmonics and a slow
    /// drift, giving an equalizer-like profile where neighboring bars differ
    /// but the whole field moves together — and pushes it into each rolling
    /// buffer. The app should call this once per frame.
    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        let t = self.tick as f64;
        for (i, buf) in self.buffers.iter_mut().enumerate() {
            let value = Self::oscillator(i, t);
            crate::widgets::util::capped_push(buf, value, self.window);
        }
    }

    /// Deterministic per-bar oscillator producing a value in `0.0..=1.0`.
    ///
    /// Each bar gets a distinct base frequency; a couple of harmonics plus a
    /// slow drift create the lively spectrum-analyzer-like shape.
    fn oscillator(bar: usize, t: f64) -> f64 {
        // Distinct base angular frequency per bar (radians/sample).
        let base = 0.30 + 0.21 * (bar as f64 + 1.0);
        // Phase offset so neighboring bars don't align.
        let phase = (bar as f64) * 0.7;
        // Two harmonics + a slow drift, normalized to roughly [-1, 1].
        let raw = (base * t + phase).sin() * 0.55
            + (base * 2.1 * t + phase).sin() * 0.30
            + (0.04 * t + phase * 1.3).sin() * 0.15;
        // Map [-1, 1] → [0, 1] and clamp for safety.
        let mapped = 0.5 + raw * 0.5;
        mapped.clamp(0.0, 1.0)
    }

    /// Current tick counter (mainly useful for tests / diagnostics).
    pub fn tick_count(&self) -> u64 {
        self.tick
    }

    /// The latest sample for `bar_index` (`0.0` if the buffer is empty or the
    /// index is out of range).
    pub fn value(&self, bar_index: usize) -> f64 {
        self.buffers
            .get(bar_index)
            .and_then(|b| b.last().copied())
            .unwrap_or(0.0)
    }
}

/// Map a bar's current value to its level's CSS class name (same thresholds as
/// [`EnergyGauge`](crate::EnergyGauge): ≥0.6 ok, ≥0.3 warn, else alert).
fn level_class(value: f64) -> &'static str {
    if value >= 0.6 {
        "ok"
    } else if value >= 0.3 {
        "warn"
    } else {
        "alert"
    }
}

impl StatefulWidget for SpectrumBars {
    type State = SpectrumBarsState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Guard zero-size areas — nothing to draw.
        if area.width == 0 || area.height == 0 {
            return;
        }

        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let base_style = sheet
            .compute_with(&NodeRef::new("Spectrum"), None, &mut scratch)
            .to_style();
        let sky_bg = base_style.bg;
        let empty_style = sheet
            .compute_with(&NodeRef::new("Spectrum").classes(&["empty"]), None, &mut scratch)
            .to_style();

        let height = area.height as f64;
        let stride = self.bar_width.saturating_add(self.gap);
        let right = area.x + area.width;
        let bottom = area.y + area.height;

        // Pre-resolve the level styles once per level (avoids recomputing the
        // cascade for every cell).
        let ok_style = sheet
            .compute_with(&NodeRef::new("Spectrum").classes(&["ok"]), None, &mut scratch)
            .to_style();
        let warn_style = sheet
            .compute_with(&NodeRef::new("Spectrum").classes(&["warn"]), None, &mut scratch)
            .to_style();
        let alert_style = sheet
            .compute_with(&NodeRef::new("Spectrum").classes(&["alert"]), None, &mut scratch)
            .to_style();

        for i in 0..self.bars {
            let bar_left = area.x.saturating_add((i as u16).saturating_mul(stride));
            if bar_left >= right {
                break;
            }
            // Bar's horizontal span, clamped to the area's right edge.
            let bar_right = bar_left.saturating_add(self.bar_width).min(right);
            if bar_right <= bar_left {
                break;
            }

            let value = state.value(i);
            let level_style = match level_class(value) {
                "ok" => ok_style,
                "warn" => warn_style,
                _ => alert_style,
            };

            // Filled cell count from the bottom up.
            let scaled = value * height;
            let mut filled = scaled.round() as u16;
            if filled > area.height {
                filled = area.height;
            }
            // Fractional remainder drives the smooth-top glyph.
            let frac = scaled - scaled.floor();

            for col in bar_left..bar_right {
                // y goes from the bottom row upward.
                for row in 0..area.height {
                    let y = bottom.saturating_sub(1).saturating_sub(row);
                    if row < filled {
                        // Filled portion of the column.
                        let glyph = if row == filled - 1 {
                            self.shape.top(frac)
                        } else {
                            self.shape.body()
                        };
                        buf[(col, y)].set_symbol(glyph).set_style(level_style);
                    } else {
                        // Empty portion above the bar's top.
                        let glyph = self.shape.empty();
                        // For Bar/Block the empty glyph is a space — paint it with
                        // the sky background so the column reads as blank sky.
                        // For Cell/Ascii use the dedicated empty style.
                        let style = if glyph == " " {
                            match sky_bg {
                                Some(bg) => Style::default().bg(bg),
                                None => base_style,
                            }
                        } else {
                            empty_style
                        };
                        buf[(col, y)].set_symbol(glyph).set_style(style);
                    }
                }
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

    /// Resolve the level color a given value should render with, via the
    /// `Spectrum.<level>` cascade node.
    fn level_color(theme: Theme, value: f64) -> Color {
        let sheet = theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let cls = level_class(value);
        sheet
            .compute_with(&NodeRef::new("Spectrum").classes(&[cls]), None, &mut scratch)
            .to_style()
            .fg
            .unwrap()
    }

    fn cell_symbol(buf: &Buffer, x: u16, y: u16) -> &str {
        buf[(x, y)].symbol()
    }

    /// Render the widget (driving the oscillator for `ticks` frames) into a
    /// fresh buffer and return it.
    fn render(
        bars: usize,
        window: usize,
        theme: Theme,
        ticks: u64,
        width: u16,
        height: u16,
    ) -> (Buffer, SpectrumBarsState) {
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        let widget = SpectrumBars::new().bars(bars).theme(theme);
        let mut state = SpectrumBarsState::new(bars, window);
        for _ in 0..ticks {
            state.tick();
        }
        StatefulWidget::render(widget, Rect::new(0, 0, width, height), &mut buf, &mut state);
        (buf, state)
    }

    /// True if the buffer has at least one non-space, non-blank cell.
    fn buffer_has_content(buf: &Buffer) -> bool {
        buf.content.iter().any(|c| c.symbol() != " ")
    }

    #[test]
    fn renders_without_panicking_after_ticks() {
        let (buf, _) = render(12, 64, Theme::Cyberpunk, 40, W, H);
        assert!(buffer_has_content(&buf), "expected non-blank cells after rendering");
    }

    #[test]
    fn tick_advances_clock_and_grows_buffers() {
        let mut state = SpectrumBarsState::new(4, 16);
        let before = state.tick_count();
        // After construction each buffer holds one baseline sample.
        assert_eq!(state.buffers.len(), 4);
        assert_eq!(state.buffers[0].len(), 1);
        state.tick();
        assert_eq!(state.tick_count(), before + 1);
        // Each bar's buffer grew by exactly one sample.
        for b in &state.buffers {
            assert_eq!(b.len(), 2);
        }
    }

    #[test]
    fn push_clamps_and_caps_window() {
        let mut state = SpectrumBarsState::new(2, 4);
        // Over-range values clamp into [0, 1].
        state.push(0, 999.0);
        assert_eq!(state.buffers[0].last().copied(), Some(1.0));
        state.push(0, -50.0);
        assert_eq!(state.buffers[0].last().copied(), Some(0.0));
        // Overflow beyond the window is trimmed.
        state.push(0, 0.2);
        state.push(0, 0.3);
        state.push(0, 0.4);
        assert!(state.buffers[0].len() <= 4, "rolling buffer must respect the window cap");
        // Out-of-range bar index is ignored (no panic).
        state.push(99, 0.5);
        assert_eq!(state.buffers.len(), 2);
    }

    #[test]
    fn oscillator_stays_in_range() {
        for bar in 0..16_u32 {
            for t in 0..1000_u32 {
                let v = SpectrumBarsState::oscillator(bar as usize, t as f64);
                assert!(
                    (0.0..=1.0).contains(&v),
                    "oscillator out of range: bar={bar} t={t} v={v}"
                );
            }
        }
    }

    #[test]
    fn zero_area_does_not_panic() {
        let widget = SpectrumBars::new().bars(8).theme(Theme::DeepSpace);
        let mut state = SpectrumBarsState::new(8, 32);
        let mut buf = Buffer::empty(Rect::ZERO);
        // Must be a no-op, not a panic.
        StatefulWidget::render(widget, Rect::ZERO, &mut buf, &mut state);
    }

    #[test]
    fn full_bar_fills_column() {
        // value 1.0 → every cell in the column is filled.
        let mut buf = Buffer::empty(Rect::new(0, 0, 6, H));
        let widget = SpectrumBars::new().bars(1).bar_width(2).gap(0).theme(Theme::Cyberpunk);
        let mut state = SpectrumBarsState::new(1, 8);
        state.push(0, 1.0);
        StatefulWidget::render(widget, Rect::new(0, 0, 6, H), &mut buf, &mut state);
        for y in 0..H {
            assert_ne!(cell_symbol(&buf, 0, y), " ", "column should be full at y={y}");
        }
    }

    #[test]
    fn zero_bar_empty_column() {
        // value 0.0 → no filled cells (all spaces for the default Bar shape).
        let mut buf = Buffer::empty(Rect::new(0, 0, 6, H));
        let widget = SpectrumBars::new().bars(1).bar_width(2).gap(0).theme(Theme::Cyberpunk);
        let mut state = SpectrumBarsState::new(1, 8);
        state.push(0, 0.0);
        StatefulWidget::render(widget, Rect::new(0, 0, 6, H), &mut buf, &mut state);
        for y in 0..H {
            assert_eq!(cell_symbol(&buf, 0, y), " ", "column should be empty at y={y}");
        }
    }

    #[test]
    fn level_color_matches_thresholds() {
        // ≥0.6 → ok, ≥0.3 → warn, <0.3 → alert, asserting against the palette
        // tokens the cascade rules resolve to.
        let palette = Theme::Cyberpunk.palette();
        assert_eq!(level_color(Theme::Cyberpunk, 0.8), palette.ok.color());
        assert_eq!(level_color(Theme::Cyberpunk, 0.6), palette.ok.color());
        assert_eq!(level_color(Theme::Cyberpunk, 0.45), palette.warn.color());
        assert_eq!(level_color(Theme::Cyberpunk, 0.3), palette.warn.color());
        assert_eq!(level_color(Theme::Cyberpunk, 0.1), palette.alert.color());
    }

    #[test]
    fn filled_cell_uses_level_color() {
        // A full bar at value 0.8 (ok level) should paint its bottom cell with
        // the ok color.
        let mut buf = Buffer::empty(Rect::new(0, 0, 6, H));
        let widget = SpectrumBars::new().bars(1).bar_width(1).gap(0).theme(Theme::Cyberpunk);
        let mut state = SpectrumBarsState::new(1, 8);
        state.push(0, 0.8);
        StatefulWidget::render(widget, Rect::new(0, 0, 6, H), &mut buf, &mut state);
        let bottom_y = H - 1;
        assert_eq!(
            buf[(0, bottom_y)].fg,
            level_color(Theme::Cyberpunk, 0.8),
            "bottom filled cell should be ok-colored"
        );
    }

    #[test]
    fn shape_variant_cell_uses_cell_glyphs() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 6, H));
        let widget = SpectrumBars::new()
            .bars(1)
            .bar_width(1)
            .gap(0)
            .shape(SpectrumShape::Cell)
            .theme(Theme::Cyberpunk);
        let mut state = SpectrumBarsState::new(1, 8);
        // Half-full: bottom half `▰`, top half `▱`.
        state.push(0, 0.5);
        StatefulWidget::render(widget, Rect::new(0, 0, 6, H), &mut buf, &mut state);
        // Bottom cell should be the Cell filled glyph.
        assert_eq!(cell_symbol(&buf, 0, H - 1), "▰");
        // Topmost cell (empty region) should be the Cell empty glyph.
        assert_eq!(cell_symbol(&buf, 0, 0), "▱");
    }

    #[test]
    fn shape_variant_ascii_uses_ascii_glyphs() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 6, H));
        let widget = SpectrumBars::new()
            .bars(1)
            .bar_width(1)
            .gap(0)
            .shape(SpectrumShape::Ascii)
            .theme(Theme::Cyberpunk);
        let mut state = SpectrumBarsState::new(1, 8);
        // Half-full: bottom half `#`, top half `.`.
        state.push(0, 0.5);
        StatefulWidget::render(widget, Rect::new(0, 0, 6, H), &mut buf, &mut state);
        assert_eq!(cell_symbol(&buf, 0, H - 1), "#");
        assert_eq!(cell_symbol(&buf, 0, 0), ".");
    }

    #[test]
    fn shape_variant_block_uses_block_glyphs() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 6, H));
        let widget = SpectrumBars::new()
            .bars(1)
            .bar_width(1)
            .gap(0)
            .shape(SpectrumShape::Block)
            .theme(Theme::Cyberpunk);
        let mut state = SpectrumBarsState::new(1, 8);
        state.push(0, 1.0);
        StatefulWidget::render(widget, Rect::new(0, 0, 6, H), &mut buf, &mut state);
        // Block shape: top glyph is also a full block.
        assert_eq!(cell_symbol(&buf, 0, H - 1), "█");
    }

    #[test]
    fn builder_setters_work() {
        let w = SpectrumBars::new()
            .bars(7)
            .bar_width(3)
            .gap(2)
            .shape(SpectrumShape::Ascii)
            .theme(Theme::Weyland);
        assert_eq!(w.bars, 7);
        assert_eq!(w.bar_width, 3);
        assert_eq!(w.gap, 2);
        assert_eq!(w.shape, SpectrumShape::Ascii);
        assert_eq!(w.theme, Theme::Weyland);
    }

    #[test]
    fn default_is_cyberpunk() {
        let w = SpectrumBars::default();
        assert_eq!(w.bars, 12);
        assert_eq!(w.bar_width, 2);
        assert_eq!(w.gap, 1);
        assert_eq!(w.shape, SpectrumShape::Bar);
        assert_eq!(w.theme, Theme::Cyberpunk);
    }

    #[test]
    fn new_with_sets_bar_count() {
        assert_eq!(SpectrumBars::new_with(5).bars, 5);
        // Clamped to ≥1.
        assert_eq!(SpectrumBars::new_with(0).bars, 1);
    }

    #[test]
    fn value_returns_latest_or_zero() {
        let mut state = SpectrumBarsState::new(2, 8);
        state.push(0, 0.42);
        assert_eq!(state.value(0), 0.42);
        // Out-of-range bar → 0.0, no panic.
        assert_eq!(state.value(99), 0.0);
    }

    #[test]
    fn render_across_many_ticks_does_not_panic() {
        // Smoke test: render repeatedly across ticks, ensuring stability.
        let mut state = SpectrumBarsState::new(8, 40);
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        for _ in 0..200 {
            state.tick();
            let widget = SpectrumBars::new().bars(8).theme(Theme::Fallout);
            StatefulWidget::render(widget, Rect::new(0, 0, W, H), &mut buf, &mut state);
        }
        assert!(buffer_has_content(&buf));
    }
}
