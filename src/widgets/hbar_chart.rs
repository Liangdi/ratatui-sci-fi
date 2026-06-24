//! **HBarChart** — horizontal category-comparison bars (PRD §3 条形图).
//!
//! A sci-fi status readout that compares `N` categories with horizontal bars:
//! each category gets its own row, laid out top-down, with a left-aligned label
//! followed by a bar whose fill width is the category's current value
//! (`0.0..=1.0`). Each bar's color shifts with its level — nominal → warn →
//! alert — so a low bar reads as a warning at a glance, exactly like
//! [`EnergyGauge`](crate::EnergyGauge).
//!
//! ## Spec
//! - One row per category, top-down, capped at `area.height` rows. Row layout:
//!   `[label_width][bar...]`, where the bar region width is
//!   `area.width - label_width` (guarded against 0).
//! - Filled cell count = `round(value * bar_width)`, filled from the left. The
//!   remaining cells in the bar region render the empty glyph.
//! - Color shifts with level: `value ≥ 0.6` → ok (`HBar.ok`), `0.3..0.6` →
//!   warn (`HBar.warn`), `< 0.3` → alert (`HBar.alert`) — the same thresholds as
//!   [`EnergyGauge`](crate::EnergyGauge). Empty cells use `HBar.empty`, labels
//!   use `HBar.label`.
//! - No fractional/transition glyph: the bar simply rounds to a cell count
//!   (unlike [`SpectrumBars`](crate::SpectrumBars), which has smooth tops). This
//!   keeps a category-comparison readout crisp and unambiguous.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]; per-category eased values + targets + a tick
//!   clock live in [`HBarChartState`]. The value **eases** toward its target each
//!   tick, so the widget self-animates.
//! - Two data-feed modes:
//!     1. **External**: the app calls [`HBarChartState::set_value`] to pin a
//!        category's value.
//!     2. **Self-generated (demo mode)**: the app calls [`HBarChartState::tick`]
//!        each frame and the state wanders each target along a per-index sine
//!        (distinct phase per row), then eases the value toward it.
//! - Drawn cell-by-cell directly into the [`Buffer`], with every color routed
//!   through the theme's [`Stylesheet`](crate::Theme::stylesheet) cascade: the
//!   `HBar` / `HBar.ok`|`HBar.warn`|`HBar.alert` / `HBar.empty` / `HBar.label`
//!   rules drive the colors. Because every rule is `var(--…)`-backed off the
//!   same palette, the rendered colors are byte-identical to reading the palette
//!   directly.
//! - The inline percentage is deliberately skipped: it would overlap filled
//!   cells on short bars and clutter a comparison readout. Bars stay clean.
//!
//! # Example
//!
//! ```no_run
//! use ratatui_sci_fi::{HBarChart, HBarChartState, Theme};
//!
//! let mut state = HBarChartState::new(4);
//! let chart = HBarChart::new()
//!     .categories(["ALPHA", "BETA", "GAMMA", "DELTA"])
//!     .label_width(6)
//!     .theme(Theme::Cyberpunk);
//! // In the event loop: state.tick(); each frame, then render the widget.
//! ```
//!
//! [`EnergyGauge`]: crate::EnergyGauge

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

/// Easing factor applied each tick: `value += (target - value) * EASE`.
///
/// `0.18` gives a smooth, slightly snappy sci-fi motion that settles in roughly
/// ~20 ticks. Deterministic — no RNG.
pub const EASE: f64 = 0.18;

/// Visual form of an [`HBarChart`] row.
///
/// Selects the glyph pair used for filled vs. empty bar cells; colors stay on
/// the CSS cascade (`HBar` / `HBar.ok`|`HBar.warn`|`HBar.alert` / `HBar.empty`),
/// untouched by this enum. The [`HBarShape::Cell`] default reproduces
/// [`EnergyGauge`](crate::EnergyGauge)'s reactor-cell `▰`/`▱` look.
///
/// Every glyph is Unicode width-1 (see convention #5 at the crate root),
/// keeping the per-cell bar math valid.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HBarShape {
    /// `▰` filled, `▱` empty — the reactor-cell look (matches
    /// [`EnergyGauge`](crate::EnergyGauge)'s default).
    #[default]
    Cell,
    /// `█` filled, `▒` empty — solid block bar.
    Block,
    /// `#` filled, `-` empty — plain ASCII bar.
    Ascii,
}

impl HBarShape {
    /// The glyph for a filled bar cell.
    #[must_use]
    pub const fn filled(self) -> &'static str {
        match self {
            Self::Cell => "▰",
            Self::Block => "█",
            Self::Ascii => "#",
        }
    }

    /// The glyph for an empty bar cell.
    #[must_use]
    pub const fn empty(self) -> &'static str {
        match self {
            Self::Cell => "▱",
            Self::Block => "▒",
            Self::Ascii => "-",
        }
    }

    /// The `(filled, empty)` glyph pair for this shape.
    #[must_use]
    pub const fn pair(self) -> (&'static str, &'static str) {
        (self.filled(), self.empty())
    }
}

/// A horizontal category-comparison bar chart.
///
/// Immutable config lives here (`categories`, `label_width`, `shape`, `theme`);
/// everything that changes per frame (eased values, targets, tick clock) lives
/// in [`HBarChartState`].
#[derive(Debug, Clone)]
pub struct HBarChart {
    /// Category labels, drawn left-aligned in the first `label_width` columns.
    /// Default `["ALPHA", "BETA", "GAMMA", "DELTA"]`.
    pub categories: Vec<String>,
    /// Number of leading columns reserved for the label (clamped ≥1). Default `6`.
    pub label_width: u16,
    /// Glyph-pair form (filled/empty glyphs). Defaults to [`HBarShape::Cell`].
    pub shape: HBarShape,
    /// Theme whose palette drives the colors via CSS cascade. Default
    /// [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Default for HBarChart {
    fn default() -> Self {
        Self {
            categories: vec!["ALPHA".to_string(), "BETA".to_string(), "GAMMA".to_string(), "DELTA".to_string()],
            label_width: 6,
            shape: HBarShape::default(),
            theme: Theme::Cyberpunk,
        }
    }
}

impl HBarChart {
    /// Build a chart with default config (4 categories, Cell shape, Cyberpunk).
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the category list (builder). Any `Into<String>` items accepted.
    #[must_use]
    pub fn categories(mut self, cats: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.categories = cats.into_iter().map(|c| c.into()).collect();
        self
    }

    /// Set the number of leading columns reserved for the label (clamped ≥1).
    #[must_use]
    pub fn label_width(mut self, w: u16) -> Self {
        self.label_width = w.max(1);
        self
    }

    /// Set the glyph-pair form (see [`HBarShape`]). Builder.
    #[must_use]
    pub fn shape(mut self, shape: HBarShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme whose palette drives colors. Builder.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Number of configured categories.
    pub fn category_count(&self) -> usize {
        self.categories.len()
    }
}

/// Mutable state for [`HBarChart`].
///
/// Holds a per-category eased `value` (what's drawn), its `target`, and a tick
/// clock. The app's event loop calls [`tick`](Self::tick) once per frame (demo
/// mode) or pins values via [`set_value`](Self::set_value) (external mode).
///
/// Easing is deterministic (no RNG): `value += (target - value) * `[`EASE`].
#[derive(Debug, Clone)]
pub struct HBarChartState {
    /// Per-category eased display values in `0.0..=1.0`.
    values: Vec<f64>,
    /// Per-category targets the values ease toward (`0.0..=1.0`).
    targets: Vec<f64>,
    /// Monotonic tick counter (drives the demo wander sine + tests).
    tick_count: u64,
}

impl Default for HBarChartState {
    fn default() -> Self {
        Self::new(4)
    }
}

impl HBarChartState {
    /// Build state for `n` categories. Eased values start at `0.0`; each row's
    /// target is seeded distinctly (`0.3 + 0.5 * i/(n-1)`, so bars stagger
    /// visually in demo mode). `n` is clamped to at least 1.
    #[must_use]
    pub fn new(n: usize) -> Self {
        let n = n.max(1);
        let (values, targets): (Vec<f64>, Vec<f64>) = (0..n)
            .map(|i| {
                let v = 0.0;
                let denom = if n > 1 { (n - 1) as f64 } else { 1.0 };
                let t = 0.3 + 0.5 * ((i as f64) / denom);
                (v, t)
            })
            .unzip();
        Self { values, targets, tick_count: 0 }
    }

    /// Advance the simulation by one tick (demo mode).
    ///
    /// 1. Bump the tick clock (wrapping).
    /// 2. Wander each target along a per-index sine with a distinct phase per
    ///    row so neighboring bars don't align: `target = 0.5 + 0.45 *
    ///    sin(tick_count * 0.03 + i * 0.7)`.
    /// 3. Ease each value toward its target by [`EASE`]:
    ///    `value += (target - value) * `[`EASE`].
    pub fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
        let t = self.tick_count as f64;
        for (i, (val, tgt)) in self.values.iter_mut().zip(self.targets.iter_mut()).enumerate() {
            // Per-row phase so bars don't move in lockstep.
            let phase = (i as f64) * 0.7;
            *tgt = 0.5 + 0.45 * (t * 0.03 + phase).sin();
            *tgt = clamp_unit(*tgt);
            *val += (*tgt - *val) * EASE;
            *val = clamp_unit(*val);
        }
    }

    /// Pin category `i` to `v` (clamped to `0.0..=1.0`), setting BOTH the eased
    /// value and the target. Use this for external control. Out-of-range indices
    /// are ignored (no panic).
    pub fn set_value(&mut self, i: usize, v: f64) {
        let Some((val, tgt)) = self.values.get_mut(i).zip(self.targets.get_mut(i)) else {
            return;
        };
        let clamped = clamp_unit(v);
        *val = clamped;
        *tgt = clamped;
    }

    /// The eased display value for category `i` (`0.0` if out of range).
    pub fn value(&self, i: usize) -> f64 {
        self.values.get(i).copied().unwrap_or(0.0)
    }

    /// The current target for category `i` (`0.0` if out of range).
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

impl StatefulWidget for HBarChart {
    type State = HBarChartState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Guard zero-size areas — nothing to draw.
        if area.width == 0 || area.height == 0 {
            return;
        }

        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();

        // Pre-resolve the level + label + empty styles once per render via a
        // single shared ComputeScratch (crate convention #2).
        let ok_style = sheet
            .compute_with(&NodeRef::new("HBar").classes(&["ok"]), None, &mut scratch)
            .to_style();
        let warn_style = sheet
            .compute_with(&NodeRef::new("HBar").classes(&["warn"]), None, &mut scratch)
            .to_style();
        let alert_style = sheet
            .compute_with(&NodeRef::new("HBar").classes(&["alert"]), None, &mut scratch)
            .to_style();
        let empty_style = sheet
            .compute_with(&NodeRef::new("HBar").classes(&["empty"]), None, &mut scratch)
            .to_style();
        let label_cascade = sheet
            .compute_with(&NodeRef::new("HBar").classes(&["label"]), None, &mut scratch)
            .to_style();
        // Fallback for the label style when the cascade has no fg.
        let palette = self.theme.palette();
        let label_style = if label_cascade.fg.is_some() {
            label_cascade
        } else {
            Style::default().fg(palette.fg.color())
        };

        let label_width = self.label_width;
        // Bar region: everything to the right of the label columns.
        let bar_width = area.width.saturating_sub(label_width);

        let cat_count = self.categories.len();
        let rows = (area.height as usize).min(cat_count);

        for row in 0..rows {
            let y = area.y + row as u16;
            let mut x = area.x;
            let right = area.x + area.width;

            // --- Label cells: left-align the category label into label_width cols.
            let label: String = self.categories[row].chars().take(label_width as usize).collect();
            let label_chars: Vec<char> = label.chars().collect();
            for col in 0..label_width {
                if x >= right {
                    break;
                }
                let glyph = label_chars.get(col as usize).copied().unwrap_or(' ');
                buf[(x, y)].set_char(glyph).set_style(label_style);
                x += 1;
            }

            // --- Bar cells (only if there's room for a bar).
            if bar_width == 0 {
                continue;
            }
            let value = clamp_unit(state.value(row));
            let level_style = match level_class(value) {
                "ok" => ok_style,
                "warn" => warn_style,
                _ => alert_style,
            };
            let filled = (value * (bar_width as f64)).round() as u16;
            let filled = filled.min(bar_width);
            for col in 0..bar_width {
                if x >= right {
                    break;
                }
                let (glyph, style) = if col < filled {
                    (self.shape.filled(), level_style)
                } else {
                    (self.shape.empty(), empty_style)
                };
                buf[(x, y)].set_symbol(glyph).set_style(style);
                x += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    const W: u16 = 24;
    const H: u16 = 8;

    /// Resolve the level color a given value should render with, via the
    /// `HBar.<level>` cascade node.
    fn level_color(theme: Theme, value: f64) -> Color {
        let sheet = theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let cls = level_class(value);
        sheet
            .compute_with(&NodeRef::new("HBar").classes(&[cls]), None, &mut scratch)
            .to_style()
            .fg
            .unwrap()
    }

    fn cell_symbol(buf: &Buffer, x: u16, y: u16) -> &str {
        buf[(x, y)].symbol()
    }

    /// Render the widget with the given state into a fresh buffer and return it.
    fn render(
        widget: HBarChart,
        state: &mut HBarChartState,
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
        let widget = HBarChart::new().theme(Theme::Cyberpunk);
        let mut state = HBarChartState::new(4);
        for _ in 0..20 {
            state.tick();
        }
        let buf = render(widget, &mut state, W, H);
        assert!(non_blank(&buf) > 0, "expected non-blank cells after rendering");
    }

    #[test]
    fn zero_area_does_not_panic() {
        let widget = HBarChart::new().theme(Theme::DeepSpace);
        let mut state = HBarChartState::new(4);
        let mut buf = Buffer::empty(Rect::ZERO);
        // Must be a no-op, not a panic.
        StatefulWidget::render(widget, Rect::ZERO, &mut buf, &mut state);
    }

    #[test]
    fn tick_advances_clock_and_eases_values() {
        // Pin a target by using set_value to a known spot, then tick and watch
        // the value ease toward it. set_value pins both value and target, so to
        // observe easing we manually move the target via the demo wander: seed
        // value low, tick, and confirm value moved off 0.
        let n = 3;
        let mut state = HBarChartState::new(n);
        let before_clock = state.tick_count();
        let before_v0 = state.value(0);
        state.tick();
        assert_eq!(state.tick_count(), before_clock + 1, "tick should advance the clock");
        let after_v0 = state.value(0);
        // In demo mode the target wanders off-seed and the value eases toward it;
        // value should have moved away from its seeded 0.0.
        assert!(
            after_v0 > before_v0,
            "value should ease toward target after a tick: {} -> {}",
            before_v0,
            after_v0
        );

        // Easing should move the value by ~EASE of the gap to target.
        // Reconstruct: before tick, target_0 after wander is
        // 0.5 + 0.45*sin(1*0.03 + 0). value started at 0.
        let t_after = 1.0_f64;
        let phase = 0.0_f64;
        let expected_target = clamp_unit(0.5 + 0.45 * (t_after * 0.03 + phase).sin());
        let gap = expected_target - 0.0;
        let expected_delta = gap * EASE;
        assert!(
            (after_v0 - expected_delta).abs() < 1e-9,
            "eased value should be ~EASE*gap: got {}, expected {}",
            after_v0,
            expected_delta
        );
    }

    #[test]
    fn set_value_clamps_and_sets() {
        let mut state = HBarChartState::new(2);
        // Over-range clamps to 1.0.
        state.set_value(0, 999.0);
        assert_eq!(state.value(0), 1.0);
        assert_eq!(state.target(0), 1.0);
        // Negative clamps to 0.0.
        state.set_value(0, -5.0);
        assert_eq!(state.value(0), 0.0);
        assert_eq!(state.target(0), 0.0);
        // A mid value is set as-is on both value and target.
        state.set_value(0, 0.42);
        assert!((state.value(0) - 0.42).abs() < 1e-9);
        assert!((state.target(0) - 0.42).abs() < 1e-9);
        // Out-of-range index is ignored (no panic).
        state.set_value(99, 0.7);
        assert_eq!(state.value(99), 0.0);
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
    fn full_value_fills_row() {
        // value 1.0 → every cell in the bar region is the filled glyph.
        let widget = HBarChart::new().categories(["A"]).label_width(1).theme(Theme::Cyberpunk);
        let mut state = HBarChartState::new(1);
        state.set_value(0, 1.0);
        // area width 9: 1 label col + 8 bar cols.
        let buf = render(widget, &mut state, 9, 1);
        let filled = HBarShape::Cell.filled();
        for x in 1..9 {
            assert_eq!(
                cell_symbol(&buf, x, 0),
                filled,
                "bar cell {x} should be filled"
            );
        }
    }

    #[test]
    fn zero_value_empties_row() {
        // value 0.0 → no filled cells in the bar region.
        let widget = HBarChart::new().categories(["A"]).label_width(1).theme(Theme::Cyberpunk);
        let mut state = HBarChartState::new(1);
        state.set_value(0, 0.0);
        let buf = render(widget, &mut state, 9, 1);
        let filled = HBarShape::Cell.filled();
        for x in 1..9 {
            assert_ne!(
                cell_symbol(&buf, x, 0),
                filled,
                "bar cell {x} should not be filled"
            );
        }
    }

    #[test]
    fn shape_variants_use_right_glyphs() {
        // Cell → ▰/▱, Block → █/▒, Ascii → #/-.
        for (shape, filled_g, empty_g) in [
            (HBarShape::Cell, "▰", "▱"),
            (HBarShape::Block, "█", "▒"),
            (HBarShape::Ascii, "#", "-"),
        ] {
            let widget =
                HBarChart::new().categories(["A"]).label_width(1).shape(shape).theme(Theme::Cyberpunk);
            let mut state = HBarChartState::new(1);
            // value 0.5 over 8 bar cols → 4 filled, 4 empty.
            state.set_value(0, 0.5);
            let buf = render(widget, &mut state, 9, 1);
            // First bar cell is filled.
            assert_eq!(
                cell_symbol(&buf, 1, 0),
                filled_g,
                "shape {:?} should use filled glyph {}",
                shape,
                filled_g
            );
            // Last bar cell is empty.
            assert_eq!(
                cell_symbol(&buf, 8, 0),
                empty_g,
                "shape {:?} should use empty glyph {}",
                shape,
                empty_g
            );
        }
    }

    #[test]
    fn builder_setters_work() {
        let w = HBarChart::new()
            .label_width(8)
            .shape(HBarShape::Block)
            .theme(Theme::Weyland);
        assert_eq!(w.label_width, 8);
        assert_eq!(w.shape, HBarShape::Block);
        assert_eq!(w.theme, Theme::Weyland);
    }

    #[test]
    fn default_is_cyberpunk() {
        let w = HBarChart::default();
        assert_eq!(w.theme, Theme::Cyberpunk);
        assert_eq!(w.label_width, 6);
        assert_eq!(w.categories.len(), 4);
    }

    #[test]
    fn default_shape_is_cell() {
        let w = HBarChart::default();
        assert_eq!(w.shape, HBarShape::Cell);
    }

    #[test]
    fn categories_builder_replaces_list() {
        let w = HBarChart::new().categories(["X", "Y", "Z"]);
        assert_eq!(w.categories, vec!["X".to_string(), "Y".to_string(), "Z".to_string()]);
        assert_eq!(w.category_count(), 3);
    }

    #[test]
    fn label_renders_on_the_left() {
        // label_width 6, category "ALPHA" → first 5 cells are A L P H A.
        let widget = HBarChart::new().categories(["ALPHA"]).label_width(6).theme(Theme::Cyberpunk);
        let mut state = HBarChartState::new(1);
        state.set_value(0, 0.0);
        let buf = render(widget, &mut state, 16, 1);
        assert_eq!(cell_symbol(&buf, 0, 0), "A");
        assert_eq!(cell_symbol(&buf, 1, 0), "L");
        assert_eq!(cell_symbol(&buf, 4, 0), "A");
        // 6th label col (index 5) is a space (label shorter than width).
        assert_eq!(cell_symbol(&buf, 5, 0), " ");
        // Bar starts at index 6.
        let empty = HBarShape::Cell.empty();
        assert_eq!(cell_symbol(&buf, 6, 0), empty);
    }

    #[test]
    fn label_truncates_to_label_width() {
        // A long label is truncated to label_width chars.
        let widget = HBarChart::new()
            .categories(["VERYLONGCATEGORY"])
            .label_width(4)
            .theme(Theme::Cyberpunk);
        let mut state = HBarChartState::new(1);
        state.set_value(0, 0.0);
        let buf = render(widget, &mut state, 12, 1);
        assert_eq!(cell_symbol(&buf, 0, 0), "V");
        assert_eq!(cell_symbol(&buf, 1, 0), "E");
        assert_eq!(cell_symbol(&buf, 2, 0), "R");
        assert_eq!(cell_symbol(&buf, 3, 0), "Y");
        // Bar starts at index 4 — no label bleed.
        let empty = HBarShape::Cell.empty();
        assert_eq!(cell_symbol(&buf, 4, 0), empty);
    }

    #[test]
    fn rows_capped_to_area_height() {
        // 4 categories but only 2 rows of height → only 2 rows drawn (no panic,
        // no out-of-bounds writes).
        let widget = HBarChart::new().theme(Theme::Cyberpunk);
        let mut state = HBarChartState::new(4);
        for _ in 0..5 {
            state.tick();
        }
        let buf = render(widget, &mut state, 20, 2);
        // Just assert it didn't panic and drew something.
        assert!(non_blank(&buf) > 0);
    }

    #[test]
    fn bar_width_zero_does_not_panic() {
        // label_width == area.width → bar_width 0; only labels draw.
        let widget = HBarChart::new().categories(["A"]).label_width(4).theme(Theme::Cyberpunk);
        let mut state = HBarChartState::new(1);
        state.set_value(0, 1.0);
        let buf = render(widget, &mut state, 4, 1);
        // Label cell present, no panic.
        assert_eq!(cell_symbol(&buf, 0, 0), "A");
    }

    #[test]
    fn state_shorter_than_categories_uses_zero() {
        // State has 2 entries but widget has 4 categories: rows 2,3 read value 0.
        let widget = HBarChart::new().theme(Theme::Cyberpunk);
        let mut state = HBarChartState::new(2);
        // row 0 gets a value; rows beyond the state vector default to 0.0.
        state.set_value(0, 1.0);
        let buf = render(widget, &mut state, 20, 4);
        // Row 0 bar (after 6 label cols) is filled; row 2 bar is empty (value 0).
        let filled = HBarShape::Cell.filled();
        let empty = HBarShape::Cell.empty();
        assert_eq!(cell_symbol(&buf, 6, 0), filled, "row 0 should be full");
        assert_eq!(cell_symbol(&buf, 6, 2), empty, "row 2 (no state) should be empty");
    }
}
