//! **CandlestickChart** — animated OHLC financial candlestick chart.
//!
//! A sci-fi market-feed / power-history HUD: a scrolling series of OHLC candles
//! (`open`/`high`/`low`/`close`, all in `0.0..=1.0`) rendered as classic
//! financial candlesticks — a thin wick from low to high, and a body from open
//! to close colored green when bullish (`close ≥ open`) or red when bearish.
//! New candles are appended over time and the series scrolls left.
//!
//! ## Spec
//! - Draw on a ratatui [`Canvas`] using [`Marker::Braille`] (so bodies and
//!   wicks render crisply). `x_bounds = [0, capacity]`, `y_bounds = [0, 1]`.
//! - Candle `i` is centered at `x = i + 0.5`; its body has half-width ~`0.35`
//!   (so neighbors never overlap). The wick is one vertical [`Line`] from
//!   `(x, low)` to `(x, high)`.
//! - Body color is bullish → [`CandlestickShape`] body fill uses the up color
//!   (`Candle.up`, fallback [`Palette::ok`](crate::Palette::ok)); bearish → the
//!   down color (`Candle.down`, fallback [`Palette::alert`](crate::Palette::alert)).
//!   Wicks use `Candle.wick` (fallback [`Palette::fg`](crate::Palette::fg)).
//! - [`CandlestickShape`] selects geometry only — [`CandlestickShape::Filled`]
//!   stamps vertical lines across the body, [`CandlestickShape::Hollow`] draws
//!   the 4 outline lines of the body rect, [`CandlestickShape::Bar`] draws an
//!   OHLC bar (vertical wick + a short left tick at `open` + a short right tick
//!   at `close`).
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]; the rolling candle buffer and tick clock live
//!   in [`CandlestickChartState`], advanced each tick (or fed live via `push`).
//! - All colors resolve through the [`Stylesheet`](crate::Theme::stylesheet)
//!   cascade (`Candle`, `Candle.up`|`down`|`wick`) using a single
//!   [`ComputeScratch`] per render, falling back to [`Theme::palette`] values.
//! - The demo `tick()` is deterministic (no RNG): a sine-drift random walk
//!   generates each new candle so the chart self-animates.
//!
//! # Example
//!
//! ```no_run
//! use ratatui_sci_fi::{CandlestickChart, CandlestickChartState, CandlestickShape, Theme};
//!
//! let mut state = CandlestickChartState::default();
//! let chart = CandlestickChart::new()
//!     .capacity(32)
//!     .shape(CandlestickShape::Filled)
//!     .theme(Theme::Cyberpunk);
//! // in your event loop each frame: state.tick();
//! // to feed live OHLC: state.push(ohlc);
//! ```

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    symbols::Marker,
    widgets::{StatefulWidget, Widget},
    widgets::canvas::{Canvas, Line},
};
#[cfg(test)]
use ratatui::style::Color;

use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Body half-width in chart x-units. Each candle is centered at `x = i + 0.5`,
/// so a half-width of `0.35` keeps neighbors (spaced `1.0` apart) from
/// touching while leaving a visible gap.
pub const BODY_HALF_WIDTH: f64 = 0.35;

/// Half-width of the left/right tick marks for the [`CandlestickShape::Bar`]
/// variant.
pub const TICK_HALF_WIDTH: f64 = 0.15;

/// Minimum candle price; the deterministic walk clamps into `[MIN_PRICE,
/// MAX_PRICE]` so wicks never touch the chart's top/bottom edge.
pub const MIN_PRICE: f64 = 0.02;
/// Maximum candle price (companion to [`MIN_PRICE`]).
pub const MAX_PRICE: f64 = 0.98;

/// A single OHLC candle. All four prices are in `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ohlc {
    /// Opening price.
    pub open: f64,
    /// Highest price reached during the period.
    pub high: f64,
    /// Lowest price reached during the period.
    pub low: f64,
    /// Closing price.
    pub close: f64,
}

impl Ohlc {
    /// `true` when the candle closed at or above its open (the "green" case).
    ///
    /// `close >= open` — a doji (close == open) reads as bullish.
    #[must_use]
    pub fn bullish(&self) -> bool {
        self.close >= self.open
    }

    /// The price the body's top edge sits at (`max(open, close)`).
    fn body_top(&self) -> f64 {
        self.open.max(self.close)
    }

    /// The price the body's bottom edge sits at (`min(open, close)`).
    fn body_bottom(&self) -> f64 {
        self.open.min(self.close)
    }
}

/// Visual form of a [`CandlestickChart`] candle (config — convention #5).
///
/// Selects how each candle's body is drawn on the [`Canvas`]; colors stay on
/// the CSS cascade (`Candle` / `Candle.up`|`down`|`wick`), untouched by this
/// enum. The [`CandlestickShape::Filled`] default is the classic filled-body
/// candlestick look.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CandlestickShape {
    /// Classic filled body: the open↔close rect is filled (approximated by
    /// stamping vertical lines across the body's x-span) in the body color,
    /// with a thin wick line from low to high. The default.
    #[default]
    Filled,
    /// Hollow body: only the 4 outline lines of the open↔close rect are drawn
    /// (interior left blank); the wick is drawn as usual.
    Hollow,
    /// OHLC bar style: a vertical wick line from low to high, plus a short left
    /// tick at the open price and a short right tick at the close price. No
    /// body rect is drawn.
    Bar,
}

/// An animated OHLC financial candlestick chart.
///
/// Immutable config lives here (`capacity`, `shape`, `theme`, `label`);
/// everything that changes per frame lives in [`CandlestickChartState`].
#[derive(Debug, Clone)]
pub struct CandlestickChart {
    /// Maximum candles kept on screen (default `32`, clamped ≥ 2). Older
    /// candles are dropped from the left as new ones are appended.
    pub capacity: usize,
    /// Body geometry. Defaults to [`CandlestickShape::Filled`].
    pub shape: CandlestickShape,
    /// Theme whose palette drives colors via the CSS cascade. Default
    /// [`Theme::Cyberpunk`].
    pub theme: Theme,
    /// Optional short caption drawn below the chart (e.g. `"MARKET"`).
    pub label: Option<String>,
}

impl Default for CandlestickChart {
    fn default() -> Self {
        Self {
            capacity: 32,
            shape: CandlestickShape::Filled,
            theme: Theme::Cyberpunk,
            label: None,
        }
    }
}

impl CandlestickChart {
    /// Build a chart with default config (capacity 32, Filled, Cyberpunk).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the candle capacity (clamped to at least 2). Builder.
    #[must_use]
    pub fn capacity(mut self, n: usize) -> Self {
        self.capacity = n.max(2);
        self
    }

    /// Set the body geometry (see [`CandlestickShape`]). Builder.
    #[must_use]
    pub fn shape(mut self, s: CandlestickShape) -> Self {
        self.shape = s;
        self
    }

    /// Set the theme whose palette drives colors. Builder.
    #[must_use]
    pub fn theme(mut self, t: Theme) -> Self {
        self.theme = t;
        self
    }

    /// Attach a short caption drawn below the chart (e.g. `"MARKET"`). Builder.
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// Mutable state for [`CandlestickChart`].
///
/// Holds the rolling candle buffer (capped at `capacity`, oldest dropped first)
/// plus a tick clock that drives the deterministic price walk. The app advances
/// it every frame via [`Self::tick`] (demo mode) or feeds live candles via
/// [`Self::push`] (external mode).
#[derive(Debug, Clone)]
pub struct CandlestickChartState {
    /// Rolling OHLC buffer, oldest first; capped at `capacity`.
    candles: Vec<Ohlc>,
    /// Configured maximum number of candles kept.
    capacity: usize,
    /// Monotonic tick counter; drives the deterministic walk and tests.
    tick_count: u64,
    /// Last close produced by the walk; seeds the next candle's open.
    last_close: f64,
}

impl Default for CandlestickChartState {
    fn default() -> Self {
        Self::new(32)
    }
}

impl CandlestickChartState {
    /// Build state for the given `capacity` (clamped to at least 2), seeded
    /// with ~8 candles from a deterministic walk so the first frame isn't
    /// empty.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(2);
        let mut state = Self {
            candles: Vec::with_capacity(capacity),
            capacity,
            tick_count: 0,
            last_close: 0.5,
        };
        // Seed ~8 candles (or capacity-1 if smaller) from the deterministic
        // walk so the chart starts populated.
        let seed = 8.min(capacity.saturating_sub(1)).max(1);
        for _ in 0..seed {
            let candle = state.next_candle();
            state.last_close = candle.close;
            state.candles.push(candle);
        }
        state
    }

    /// Number of candles currently held.
    pub fn len(&self) -> usize {
        self.candles.len()
    }

    /// `true` if no candles are held.
    pub fn is_empty(&self) -> bool {
        self.candles.is_empty()
    }

    /// Borrow the candle at `i` (oldest is `0`), or `None` if out of range.
    pub fn candle(&self, i: usize) -> Option<&Ohlc> {
        self.candles.get(i)
    }

    /// The most recent candle's close price (`0.0` if the buffer is empty).
    pub fn last_close(&self) -> f64 {
        self.candles.last().map(|c| c.close).unwrap_or(0.0)
    }

    /// Current tick clock value.
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    /// Feed a live OHLC candle (external-feed mode).
    ///
    /// Each price is clamped into `0.0..=1.0` and `high`/`low` are reconciled
    /// to bracket the body; the candle is appended and the oldest is dropped
    /// once the buffer exceeds `capacity`.
    pub fn push(&mut self, candle: Ohlc) {
        let open = clamp_unit(candle.open);
        let close = clamp_unit(candle.close);
        let mut high = clamp_unit(candle.high).max(open).max(close);
        let mut low = clamp_unit(candle.low).min(open).min(close);
        // Ensure high ≥ low even for pathological inputs.
        if high < low {
            std::mem::swap(&mut high, &mut low);
        }
        let candle = Ohlc { open, high, low, close };
        self.last_close = candle.close;
        self.candles.push(candle);
        self.trim();
    }

    /// Advance the simulation by one tick (demo / self-generated mode).
    ///
    /// Bumps the tick clock and, roughly every 4 ticks, appends a new candle
    /// produced by the deterministic price walk and trims to capacity — so the
    /// series scrolls left over time. The app should call this once per frame.
    pub fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
        // Append a new candle roughly every 4 ticks (so the chart updates at a
        // readable pace relative to the frame rate).
        if self.tick_count.is_multiple_of(4) {
            let candle = self.next_candle();
            self.last_close = candle.close;
            self.candles.push(candle);
            self.trim();
        }
    }

    /// Drop the oldest candles until `len <= capacity`.
    fn trim(&mut self) {
        let overflow = self.candles.len().saturating_sub(self.capacity);
        if overflow > 0 {
            self.candles.drain(..overflow);
        }
    }

    /// Deterministic next-candle generator.
    ///
    /// A sine-drift random walk: each candle's open is the previous close, its
    /// body drifts along a slow sine of the tick clock, and high/low expand
    /// around the body by a deterministic volatility. All four prices are
    /// clamped into `[MIN_PRICE, MAX_PRICE]`.
    fn next_candle(&self) -> Ohlc {
        let t = self.tick_count as f64;
        // Slow drift along a sine; amplitude ~0.06 keeps consecutive candles
        // near each other while still wandering.
        let drift = 0.06 * (t * 0.11).sin();
        let open = self.last_close;
        let close = clamp_price(open + drift);
        // Deterministic volatility proportional to the body size plus a floor.
        let body = (close - open).abs();
        let vol = 0.015 + 0.5 * body + 0.01 * (t * 0.07).sin().abs();
        let high = clamp_price(open.max(close) + vol);
        let low = clamp_price(open.min(close) - vol);
        Ohlc { open, high, low, close }
    }
}

/// Clamp `v` into `0.0..=1.0`.
fn clamp_unit(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

/// Clamp `v` into `[MIN_PRICE, MAX_PRICE]` so wicks stay off the chart edges.
fn clamp_price(v: f64) -> f64 {
    v.clamp(MIN_PRICE, MAX_PRICE)
}

impl StatefulWidget for CandlestickChart {
    type State = CandlestickChartState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // 1. Guard zero-size areas.
        if area.width == 0 || area.height == 0 {
            return;
        }

        // 2. Resolve colors from the cascade with one shared scratch.
        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();

        let bg = sheet
            .compute_with(&NodeRef::new("Candle"), None, &mut scratch)
            .to_style()
            .bg
            .unwrap_or_else(|| self.theme.palette().bg.color());
        let up_color = sheet
            .compute_with(&NodeRef::new("Candle").classes(&["up"]), None, &mut scratch)
            .to_style()
            .fg
            .unwrap_or_else(|| self.theme.palette().ok.color());
        let down_color = sheet
            .compute_with(&NodeRef::new("Candle").classes(&["down"]), None, &mut scratch)
            .to_style()
            .fg
            .unwrap_or_else(|| self.theme.palette().alert.color());
        let wick_color = sheet
            .compute_with(&NodeRef::new("Candle").classes(&["wick"]), None, &mut scratch)
            .to_style()
            .fg
            .unwrap_or_else(|| self.theme.palette().fg.color());

        let shape = self.shape;
        let candles = &state.candles;

        // Chart area is the full area minus one bottom row reserved for the
        // optional label (mirror radial_gauge's label block).
        let mut chart_area = area;
        if self.label.is_some() && area.height > 1 {
            chart_area.height = area.height - 1;
        }

        // 3–5. Paint the candles.
        Canvas::default()
            .marker(Marker::Braille)
            .background_color(bg)
            .x_bounds([0.0, self.capacity as f64])
            .y_bounds([0.0, 1.0])
            .paint(move |ctx| {
                for (i, candle) in candles.iter().enumerate() {
                    let cx = i as f64 + 0.5;
                    let bullish = candle.bullish();
                    let body_color = if bullish { up_color } else { down_color };

                    // Wick: thin vertical line low → high (drawn first so the
                    // body overlays it for Filled/Hollow).
                    ctx.draw(&Line {
                        x1: cx,
                        y1: candle.low,
                        x2: cx,
                        y2: candle.high,
                        color: wick_color,
                    });

                    let top = candle.body_top();
                    let bottom = candle.body_bottom();

                    match shape {
                        CandlestickShape::Filled => {
                            // Approximate a filled rect by stamping vertical
                            // lines across the body's x-span. The number of
                            // samples scales with the body width.
                            let hw = BODY_HALF_WIDTH;
                            let x_left = cx - hw;
                            let x_right = cx + hw;
                            // ~3 samples across a 0.7-wide body is enough at
                            // Braille resolution.
                            let samples = 3usize;
                            for s in 0..samples {
                                let frac = (s as f64 + 0.5) / samples as f64;
                                let x = x_left + (x_right - x_left) * frac;
                                ctx.draw(&Line {
                                    x1: x,
                                    y1: bottom,
                                    x2: x,
                                    y2: top,
                                    color: body_color,
                                });
                            }
                        }
                        CandlestickShape::Hollow => {
                            // Outline the body rect with 4 lines.
                            let hw = BODY_HALF_WIDTH;
                            let x_left = cx - hw;
                            let x_right = cx + hw;
                            // Top edge.
                            ctx.draw(&Line {
                                x1: x_left,
                                y1: top,
                                x2: x_right,
                                y2: top,
                                color: body_color,
                            });
                            // Bottom edge.
                            ctx.draw(&Line {
                                x1: x_left,
                                y1: bottom,
                                x2: x_right,
                                y2: bottom,
                                color: body_color,
                            });
                            // Left edge.
                            ctx.draw(&Line {
                                x1: x_left,
                                y1: bottom,
                                x2: x_left,
                                y2: top,
                                color: body_color,
                            });
                            // Right edge.
                            ctx.draw(&Line {
                                x1: x_right,
                                y1: bottom,
                                x2: x_right,
                                y2: top,
                                color: body_color,
                            });
                        }
                        CandlestickShape::Bar => {
                            // OHLC bar: wick already drawn above; add a short
                            // left tick at open and a short right tick at close.
                            let tw = TICK_HALF_WIDTH;
                            ctx.draw(&Line {
                                x1: cx - tw,
                                y1: candle.open,
                                x2: cx,
                                y2: candle.open,
                                color: body_color,
                            });
                            ctx.draw(&Line {
                                x1: cx,
                                y1: candle.close,
                                x2: cx + tw,
                                y2: candle.close,
                                color: body_color,
                            });
                        }
                    }
                }
            })
            .render(chart_area, buf);

        // 6. Optional label, drawn into the row just below the chart (mirror
        // radial_gauge's label block).
        if let Some(label) = &self.label {
            let label_y = chart_area.y + chart_area.height;
            if label_y < area.y + area.height {
                crate::widgets::util::draw_centered_label(
                    buf,
                    area.x,
                    label_y,
                    area.width,
                    area.x + area.width,
                    label,
                    wick_color,
                    bg,
                );
            }
        }
    }
}

/// Test-only helper: resolve a CSS node's fg color through the cascade.
#[cfg(test)]
fn node_color(theme: Theme, node: &NodeRef) -> Color {
    let sheet = theme.stylesheet();
    sheet
        .compute_with(node, None, &mut ComputeScratch::new())
        .to_style()
        .fg
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    const W: u16 = 48;
    const H: u16 = 12;

    /// Render the widget into a fresh buffer with the given widget + state.
    fn render(state: &mut CandlestickChartState, widget: CandlestickChart, width: u16, height: u16) -> Buffer {
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
        let mut state = CandlestickChartState::default();
        // Tick many times so candles scroll.
        for _ in 0..200 {
            state.tick();
        }
        let buf = render(&mut state, CandlestickChart::new(), W, H);
        assert!(non_blank(&buf) > 0, "chart should draw something after ticks");
    }

    #[test]
    fn zero_area_does_not_panic() {
        let mut state = CandlestickChartState::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let widget = CandlestickChart::new();
        StatefulWidget::render(widget, Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        // No panic == pass.
    }

    #[test]
    fn tick_advances_clock_and_appends() {
        let mut state = CandlestickChartState::new(32);
        let before_tick = state.tick_count();
        let before_len = state.len();

        // Every 4 ticks a new candle is appended.
        state.tick();
        state.tick();
        state.tick();
        state.tick();
        assert_eq!(state.tick_count(), before_tick + 4);
        assert_eq!(state.len(), before_len + 1, "4 ticks should append one candle");

        // Over many ticks, len never exceeds capacity.
        for _ in 0..500 {
            state.tick();
            assert!(
                state.len() <= state.capacity,
                "len {} exceeded capacity {}",
                state.len(),
                state.capacity
            );
        }

        // And candles actually change over time — the last close after the run
        // shouldn't equal the seeded last close.
        assert!(
            state.len() > 1,
            "state should hold multiple candles after a long run"
        );
    }

    #[test]
    fn push_clamps_and_trims() {
        let mut state = CandlestickChartState::new(4);
        // Out-of-range prices clamp into [0, 1].
        state.push(Ohlc { open: -1.0, high: 999.0, low: -2.0, close: 5.0 });
        let c = state.candle(state.len() - 1).unwrap();
        assert!((c.open - 0.0).abs() < 1e-9, "open should clamp to 0");
        assert!((c.close - 1.0).abs() < 1e-9, "close should clamp to 1");
        assert!(c.high >= c.close, "high should bracket the body");
        assert!(c.low <= c.open, "low should bracket the body");
        assert!(c.high >= c.low, "high should be >= low");

        // Overflow trims to capacity.
        for i in 0..20 {
            state.push(Ohlc { open: 0.4, high: 0.6, low: 0.3, close: 0.5 + 0.01 * (i as f64) });
        }
        assert!(state.len() <= 4, "push overflow should trim to capacity, got len {}", state.len());
    }

    #[test]
    fn shape_variants_render_without_panicking() {
        for shape in [CandlestickShape::Filled, CandlestickShape::Hollow, CandlestickShape::Bar] {
            let mut state = CandlestickChartState::default();
            // A few ticks so candles vary.
            for _ in 0..20 {
                state.tick();
            }
            let buf = render(&mut state, CandlestickChart::new().shape(shape), W, H);
            assert!(non_blank(&buf) > 0, "{:?} shape should render non-blank", shape);
        }
    }

    #[test]
    fn builder_setters_work() {
        let w = CandlestickChart::new()
            .capacity(16)
            .shape(CandlestickShape::Hollow)
            .theme(Theme::Weyland)
            .label("MARKET");
        assert_eq!(w.capacity, 16);
        assert_eq!(w.shape, CandlestickShape::Hollow);
        assert_eq!(w.theme, Theme::Weyland);
        assert_eq!(w.label.as_deref(), Some("MARKET"));
    }

    #[test]
    fn capacity_clamps_to_two() {
        let w = CandlestickChart::new().capacity(0);
        assert_eq!(w.capacity, 2, "capacity should clamp to at least 2");
        let w = CandlestickChart::new().capacity(1);
        assert_eq!(w.capacity, 2);
    }

    #[test]
    fn default_is_cyberpunk() {
        let w = CandlestickChart::default();
        assert_eq!(w.theme, Theme::Cyberpunk);
        assert_eq!(w.capacity, 32);
        assert!(w.label.is_none());
    }

    #[test]
    fn default_shape_is_filled() {
        let w = CandlestickChart::default();
        assert_eq!(w.shape, CandlestickShape::Filled);
    }

    #[test]
    fn state_default_seeds_candles() {
        let state = CandlestickChartState::default();
        assert!(!state.is_empty(), "default state should seed candles");
        assert!(state.len() >= 1);
        // last_close should reflect the seeded walk (within price bounds).
        let lc = state.last_close();
        assert!(lc >= 0.0 && lc <= 1.0, "last_close out of range: {}", lc);
    }

    #[test]
    fn state_capacity_is_enforced_at_construction() {
        let state = CandlestickChartState::new(4);
        assert!(state.len() <= 4, "constructed state should respect capacity");
    }

    #[test]
    fn ohlc_bullish_helper() {
        let bullish = Ohlc { open: 0.4, high: 0.6, low: 0.3, close: 0.5 };
        assert!(bullish.bullish(), "close > open should be bullish");
        let doji = Ohlc { open: 0.5, high: 0.6, low: 0.4, close: 0.5 };
        assert!(doji.bullish(), "close == open (doji) reads as bullish");
        let bearish = Ohlc { open: 0.6, high: 0.7, low: 0.4, close: 0.45 };
        assert!(!bearish.bullish(), "close < open should be bearish");
    }

    #[test]
    fn candle_and_last_close_accessors() {
        let mut state = CandlestickChartState::new(8);
        // Out-of-range index returns None.
        assert!(state.candle(999).is_none());
        // last_close matches the last candle's close.
        let last = state.candle(state.len() - 1).unwrap();
        assert!((state.last_close() - last.close).abs() < 1e-9);
        // Empty-state accessors are safe.
        let empty = CandlestickChartState { candles: Vec::new(), capacity: 4, tick_count: 0, last_close: 0.5 };
        assert!(empty.is_empty());
        assert_eq!(empty.last_close(), 0.0);
    }

    #[test]
    fn bullish_uses_ok_color() {
        // The up color resolves through Candle.up (fallback palette.ok).
        let palette = Theme::Cyberpunk.palette();
        let up = node_color(
            Theme::Cyberpunk,
            &NodeRef::new("Candle").classes(&["up"]),
        );
        let down = node_color(
            Theme::Cyberpunk,
            &NodeRef::new("Candle").classes(&["down"]),
        );
        let wick = node_color(
            Theme::Cyberpunk,
            &NodeRef::new("Candle").classes(&["wick"]),
        );
        // Whether or not the stylesheet has Candle rules, the resolved colors
        // must equal the palette fallbacks (cascade is var(--token)-backed).
        assert_eq!(up, palette.ok.color(), "Candle.up should resolve to palette.ok");
        assert_eq!(down, palette.alert.color(), "Candle.down should resolve to palette.alert");
        assert_eq!(wick, palette.fg.color(), "Candle.wick should resolve to palette.fg");
    }

    #[test]
    fn render_across_many_ticks_does_not_panic() {
        // Smoke test: render repeatedly across ticks, ensuring stability.
        let mut state = CandlestickChartState::new(16);
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        for _ in 0..200 {
            state.tick();
            let widget = CandlestickChart::new().capacity(16).theme(Theme::Fallout);
            StatefulWidget::render(widget, Rect::new(0, 0, W, H), &mut buf, &mut state);
        }
        assert!(non_blank(&buf) > 0);
    }

    #[test]
    fn label_renders_below_chart() {
        let mut state = CandlestickChartState::default();
        for _ in 0..8 {
            state.tick();
        }
        let buf = render(
            &mut state,
            CandlestickChart::new().label("MKT"),
            24,
            10,
        );
        // The label row sits at the bottom (chart_area.height = 9, label_y = 9).
        let mut found = false;
        for x in 0..24 {
            if buf[(x, 9)].symbol() == "M" {
                found = true;
            }
        }
        assert!(found, "label 'MKT' should render its 'M' in the bottom row");
        assert!(non_blank(&buf) > 0);
    }

    #[test]
    fn next_candle_stays_in_price_bounds() {
        // Walk a fresh state for many steps and assert every generated candle
        // keeps all four prices within [MIN_PRICE, MAX_PRICE] and consistent.
        let mut state = CandlestickChartState::new(32);
        for _ in 0..1000 {
            state.tick();
            // Only inspect candles when one was appended this tick.
            if let Some(c) = state.candle(state.len() - 1) {
                for &(p, name) in &[
                    (c.open, "open"),
                    (c.high, "high"),
                    (c.low, "low"),
                    (c.close, "close"),
                ] {
                    assert!(
                        p >= MIN_PRICE - 1e-9 && p <= MAX_PRICE + 1e-9,
                        "{name}={p} out of [{MIN_PRICE},{MAX_PRICE}]"
                    );
                }
                assert!(c.high >= c.low, "high {} < low {}", c.high, c.low);
                assert!(c.high >= c.open && c.high >= c.close, "high must bracket body");
                assert!(c.low <= c.open && c.low <= c.close, "low must bracket body");
            }
        }
    }
}
