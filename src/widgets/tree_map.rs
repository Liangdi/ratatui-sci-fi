//! **TreeMap** — hierarchical/flat proportional rectangle map (PRD §3
//! 树图/比例分布).
//!
//! A disk-usage / allocation style treemap: each entry `(label, weight)` gets a
//! rectangle whose area is proportional to its weight share of the total. Cells
//! are laid out one of three ways ([`TreeShape`]), filled solid with a per-cell
//! color, and the label is overlaid on the top row when the cell is large enough
//! to read it.
//!
//! ## Spec
//! - Each entry occupies a rectangle whose area ∝ `weight / total`.
//! - Three layout algorithms (see [`TreeShape`]):
//!     - [`Flat`](TreeShape::Flat) — row-strip: fill rows left-to-right, each
//!       cell width ∝ its weight share of the remaining row; wrap when the row
//!       would overflow.
//!     - [`Slice`](TreeShape::Slice) — slice-and-dice: alternate horizontal /
//!       vertical cuts, one slice per cell, sized by weight.
//!     - [`Brick`](TreeShape::Brick) — a basic squarified-ish layout: same as
//!       `Flat` but the row break is chosen to best match a roughly-square
//!       aspect.
//! - Up to 5 cell colors cycle through `Tree.cell0`..`Tree.cell4`; labels use
//!   `Tree.label`.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]; per-tick weights live in [`TreeMapState`],
//!   advanced by the app's event loop each tick.
//! - Drawn cell-by-cell directly into the [`Buffer`], like
//!   [`HeatGrid`](crate::HeatGrid): each cell rectangle is filled with a full
//!   block glyph (`█`) in its resolved color, then the label is overlaid on the
//!   top row when it fits. Every color is routed through the theme's
//!   [`Stylesheet`](crate::Theme::stylesheet) cascade: `Tree` /
//!   `Tree.cell0`..`Tree.cell4` / `Tree.label`.
//! - The shape enum is **config** (per convention #5); it selects the layout
//!   algorithm only — colors stay on CSS. All glyphs are Unicode width-1.
//! - Demo [`TreeMapState::tick`] wanders each weight on a per-index sine (no
//!   RNG), keeping weights `≥ 0.1`.
//!
//! # Example
//!
//! ```no_run
//! use ratatui_sci_fi::{TreeMap, TreeMapState, TreeShape, Theme};
//!
//! let mut state = TreeMapState::new();
//! let tree = TreeMap::new().shape(TreeShape::Brick).theme(Theme::Cyberpunk);
//! // In the event loop: state.tick(); each frame, then render the widget.
//! ```

use crate::Theme;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::StatefulWidget,
};
use ratatui_style::{ComputeScratch, NodeRef};

/// Full-block fill glyph used for every cell rectangle (width-1).
const FILL: &str = "█";

/// Layout algorithm for a [`TreeMap`].
///
/// This is **config** (per crate convention #5): it lives on the widget struct,
/// is `#[derive(Copy)]`, and has a `#[default]`. It affects the layout
/// algorithm only — colors stay on the CSS cascade (`Tree` / `Tree.cell0..4` /
/// `Tree.label`), untouched by this enum. The [`TreeShape::Flat`] default gives
/// the canonical disk-usage row-strip look.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TreeShape {
    /// Row-strip layout — fill rows left-to-right, each cell width ∝ its weight
    /// share of the remaining row; wrap to the next row when the current row
    /// would overflow. This is the default.
    #[default]
    Flat,
    /// Slice-and-dice — alternate splitting the remaining rectangle
    /// horizontally / vertically by weight, one slice per cell.
    Slice,
    /// Squarified-ish — same as [`Flat`](Self::Flat) but each row's cell count
    /// is chosen to best match a roughly-square aspect ratio (the worst aspect
    /// ratio in the row is minimized against a target of 1.0).
    Brick,
}

/// A proportional rectangle map (disk-usage / allocation style treemap).
///
/// Immutable config lives here (`shape`, `theme`); everything that changes per
/// frame lives in [`TreeMapState`].
#[derive(Debug, Clone)]
pub struct TreeMap {
    /// Layout algorithm (see [`TreeShape`]). Defaults to [`TreeShape::Flat`].
    pub shape: TreeShape,
    /// Theme whose palette drives colors via the CSS cascade. Default
    /// [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Default for TreeMap {
    fn default() -> Self {
        Self { shape: TreeShape::Flat, theme: Theme::Cyberpunk }
    }
}

impl TreeMap {
    /// Build a treemap with default config (Flat layout).
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the layout algorithm (see [`TreeShape`]). Builder.
    #[must_use]
    pub fn shape(mut self, s: TreeShape) -> Self {
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

/// Mutable state for [`TreeMap`].
///
/// Holds the `(label, weight)` entries (weight `≥ 0`) and a monotonic tick
/// counter that drives the demo-mode weight wander. The app advances it every
/// frame via [`Self::tick`] (demo mode) or edits entries directly via
/// [`Self::set`] / [`Self::push`] / [`Self::remove`] (external mode).
#[derive(Debug, Clone)]
pub struct TreeMapState {
    /// `(label, weight)` entries, in display order. Weights are kept `≥ 0`.
    entries: Vec<(String, f64)>,
    /// Monotonic tick counter driving the demo-mode weight wander.
    tick: u64,
}

impl Default for TreeMapState {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeMapState {
    /// Build a state seeded with ~6 default entries.
    pub fn new() -> Self {
        let entries = vec![
            ("CORE".to_string(), 5.0),
            ("SENSOR".to_string(), 3.0),
            ("COMMS".to_string(), 2.0),
            ("NAV".to_string(), 2.5),
            ("PWR".to_string(), 4.0),
            ("AUX".to_string(), 1.0),
        ];
        Self { entries, tick: 0 }
    }

    /// Advance the treemap by one tick (demo mode).
    ///
    /// Each entry's weight wanders on a per-index sine around its seed value, so
    /// the proportional layout shifts gently over time. Weights are clamped to
    /// `≥ 0.1` so no entry collapses to zero (which would make it vanish). No
    /// RNG.
    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        let t = self.tick as f64;
        for (i, (_label, w)) in self.entries.iter_mut().enumerate() {
            // Distinct angular frequency + phase per index, scaled by the
            // entry's own magnitude so large entries wander more in absolute
            // terms. Baseline the wander off the seed value read at tick 0.
            let base = 1.0 + 0.5 * (i as f64);
            let phase = (i as f64) * 0.9;
            let wander = base * (0.35 * t + phase).sin() + 0.5 * (0.13 * t + phase).cos();
            *w = (*w + wander * 0.15).max(0.1);
        }
    }

    /// Replace entry `i`'s label + weight (weight clamped to `≥ 0`).
    /// Out-of-range `i` is silently ignored.
    pub fn set(&mut self, i: usize, label: impl Into<String>, weight: f64) {
        if let Some(e) = self.entries.get_mut(i) {
            e.0 = label.into();
            e.1 = weight.max(0.0);
        }
    }

    /// Append a new `(label, weight)` entry (weight clamped to `≥ 0`).
    pub fn push(&mut self, label: impl Into<String>, weight: f64) {
        self.entries.push((label.into(), weight.max(0.0)));
    }

    /// Remove entry `i`. Out-of-range `i` is silently ignored.
    pub fn remove(&mut self, i: usize) {
        if i < self.entries.len() {
            self.entries.remove(i);
        }
    }

    /// Remove every entry.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Borrow entry `i`, or `None` if out of range.
    pub fn entry(&self, i: usize) -> Option<&(String, f64)> {
        self.entries.get(i)
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether there are zero entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Sum of all weights (`0.0` if empty).
    pub fn total(&self) -> f64 {
        self.entries.iter().map(|(_, w)| *w).sum()
    }

    /// Current tick counter (mainly for diagnostics).
    pub fn tick_count(&self) -> u64 {
        self.tick
    }
}

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

/// A computed cell rectangle plus the entry index it belongs to.
struct CellRect {
    rect: Rect,
    entry: usize,
}

impl TreeMap {
    /// Compute the layout: one [`CellRect`] per entry, covering `area`, sized by
    /// weight share under the configured [`TreeShape`]. Zero-size cells are
    /// dropped; all rects are clamped to `area`.
    fn layout(&self, area: Rect, weights: &[f64]) -> Vec<CellRect> {
        let total: f64 = weights.iter().copied().sum();
        if total <= 0.0 || area.width == 0 || area.height == 0 {
            return Vec::new();
        }
        match self.shape {
            TreeShape::Flat => layout_flat(area, weights, total, false),
            TreeShape::Brick => layout_flat(area, weights, total, true),
            TreeShape::Slice => layout_slice(area, weights, total),
        }
    }
}

/// Row-strip layout used by both [`Flat`](TreeShape::Flat) and
/// [`Brick`](TreeShape::Brick). When `squarify` is false, a row breaks the
/// moment the next cell would overflow the row's remaining width (plain strip).
/// When `squarify` is true, the row break is chosen to minimize the worst aspect
/// ratio of the cells in the row against a target of 1.0 — a basic squarified
/// heuristic.
fn layout_flat(area: Rect, weights: &[f64], _total: f64, squarify: bool) -> Vec<CellRect> {
    let mut out = Vec::with_capacity(weights.len());
    let area_w = area.width as f64;
    let area_h = area.height as f64;
    if area_w < 1.0 || area_h < 1.0 {
        return out;
    }

    // Work in f64 with an offset origin, then clamp to u16 at the end.
    let mut remaining_top = 0.0_f64; // y offset from area.y, grows downward
    let mut idx = 0usize;
    let n = weights.len();

    while idx < n && remaining_top < area_h - 0.5 {
        // Remaining weight from idx onward.
        let rem_total: f64 = weights[idx..].iter().sum();
        if rem_total <= 0.0 {
            break;
        }
        let rem_h = area_h - remaining_top;
        // Choose how many entries go in this row.
        let row_count = if squarify {
            pick_row_count(&weights[idx..], rem_total, rem_h, area_w)
        } else {
            // Plain strip: greedily add cells until the next would overflow.
            pick_strip_count(&weights[idx..], rem_total, area_w)
        };
        let row_count = row_count.max(1).min(n - idx);

        // Row height ∝ (sum of row weights) / (remaining total), times rem_h.
        let row_weight: f64 = weights[idx..idx + row_count].iter().sum();
        let row_h = if rem_total > 0.0 {
            (row_weight / rem_total * rem_h).round().max(1.0)
        } else {
            1.0
        };
        // Don't overshoot the bottom.
        let row_h = row_h.min(area_h - remaining_top).max(1.0);

        // Lay out the row's cells left-to-right, width ∝ weight share.
        let mut x = 0.0_f64;
        for k in 0..row_count {
            let w = weights[idx + k];
            let frac = if row_weight > 0.0 { w / row_weight } else { 0.0 };
            let mut cell_w = (frac * area_w).round();
            if k == row_count - 1 {
                // Last cell in the row snaps to the right edge.
                cell_w = (area_w - x).round();
            }
            cell_w = cell_w.max(1.0).min(area_w - x);
            let r = Rect::new(
                area.x + x.round() as u16,
                area.y + remaining_top.round() as u16,
                cell_w as u16,
                row_h as u16,
            );
            if r.width > 0 && r.height > 0 {
                out.push(CellRect { rect: r, entry: idx + k });
            }
            x += cell_w;
        }

        remaining_top += row_h;
        idx += row_count;
    }

    out
}

/// Greedy strip row count: keep adding cells while the next cell's width (≥1)
/// still fits in the remaining row width.
fn pick_strip_count(row: &[f64], row_total: f64, area_w: f64) -> usize {
    let mut count = 0usize;
    let mut x = 0.0_f64;
    for w in row {
        let frac = if row_total > 0.0 { *w / row_total } else { 0.0 };
        let cell_w = (frac * area_w).round().max(1.0);
        if x + cell_w > area_w + 0.5 && count > 0 {
            break;
        }
        x += cell_w;
        count += 1;
    }
    count.max(1)
}

/// Squarified row count: add cells to the row as long as the worst aspect ratio
/// in the row improves (gets closer to 1.0). This is the classic squarify
/// worst-aspect test.
///
/// The row spans the full available width `area_w` (cells) and height `row_h`
/// (cells). Within the row, cell `i` has width `(w_i / row_sum) * area_w` and
/// height `row_h`; its aspect ratio is
/// `max(area_w * w_i / (row_sum * row_h), row_sum * row_h / (area_w * w_i))`.
/// Note `row_h` is proportional to `row_sum` for this row (both scale with the
/// row's share of the remaining area), so the comparison is well-defined.
fn pick_row_count(row: &[f64], _row_total: f64, row_h: f64, area_w: f64) -> usize {
    if row.is_empty() {
        return 0;
    }
    let area_w = area_w.max(1.0);
    let row_h = row_h.max(1.0);
    // Running sum of weights added to the row so far.
    let mut added = 0.0_f64;
    let mut best_count = 1usize;
    let mut best_worst = f64::INFINITY;
    for (k, &w) in row.iter().enumerate() {
        added += w.max(0.0);
        if added <= 0.0 {
            continue;
        }
        let count = k + 1;
        let mut worst = 0.0_f64;
        for wj in row.iter().take(count).map(|w| w.max(0.0)) {
            let cw = (wj / added) * area_w; // cell width in cells
            if cw <= 0.0 {
                continue;
            }
            let ar = (row_h / cw).max(cw / row_h);
            if ar > worst {
                worst = ar;
            }
        }
        if worst <= best_worst {
            best_worst = worst;
            best_count = count;
        } else {
            // Adding this cell made the worst aspect worse — stop here.
            break;
        }
    }
    best_count.max(1)
}

/// Slice-and-dice: recursively split the remaining rectangle, alternating the
/// cut axis, one slice per cell sized by weight share.
fn layout_slice(area: Rect, weights: &[f64], total: f64) -> Vec<CellRect> {
    let mut out = Vec::with_capacity(weights.len());
    if area.width == 0 || area.height == 0 || total <= 0.0 || weights.is_empty() {
        return out;
    }
    // Work in integer cell space; alternate axis each cell.
    let mut cur = area;
    // Start axis: cut along the longer dimension.
    let mut horizontal = area.width >= area.height;
    let rem: Vec<f64> = weights.to_vec();
    let n = rem.len();

    for (i, &w) in rem.iter().enumerate() {
        let rem_total: f64 = rem[i..].iter().sum();
        if rem_total <= 0.0 || cur.width == 0 || cur.height == 0 {
            break;
        }
        let frac = (w / rem_total).clamp(0.0, 1.0);
        let cell = if i == n - 1 {
            // Last cell takes the whole remaining rect.
            cur
        } else if horizontal {
            // Split horizontally: cut along width.
            let cut = ((frac * cur.width as f64).round() as u16).clamp(1, cur.width);
            let cell = Rect::new(cur.x, cur.y, cut, cur.height);
            cur = Rect::new(cur.x + cut, cur.y, cur.width - cut, cur.height);
            cell
        } else {
            // Split vertically: cut along height.
            let cut = ((frac * cur.height as f64).round() as u16).clamp(1, cur.height);
            let cell = Rect::new(cur.x, cur.y, cur.width, cut);
            cur = Rect::new(cur.x, cur.y + cut, cur.width, cur.height - cut);
            cell
        };
        if cell.width > 0 && cell.height > 0 {
            out.push(CellRect { rect: cell, entry: i });
        }
        // Alternate axis for the next cut.
        horizontal = !horizontal;
    }

    out
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

impl StatefulWidget for TreeMap {
    type State = TreeMapState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // 1. Guard zero-size areas.
        if area.width == 0 || area.height == 0 {
            return;
        }
        // 2. No entries or zero total → nothing to draw.
        if state.is_empty() || state.total() <= 0.0 {
            return;
        }

        let sheet = self.theme.stylesheet();
        let palette = self.theme.palette();
        let mut scratch = ComputeScratch::new();

        // 3. Pre-resolve the 5 cell colors (one cascade compute each) + label.
        let cell_classes = ["cell0", "cell1", "cell2", "cell3", "cell4"];
        let fallbacks = [
            palette.accent.color(),
            palette.accent2.color(),
            palette.ok.color(),
            palette.warn.color(),
            palette.alert.color(),
        ];
        let mut cell_colors: [Color; 5] = [Color::Reset; 5];
        for i in 0..5 {
            cell_colors[i] = sheet
                .compute_with(
                    &NodeRef::new("Tree").classes(&[cell_classes[i]]),
                    None,
                    &mut scratch,
                )
                .to_style()
                .fg
                .unwrap_or_else(|| fallbacks[i]);
        }
        let label_color = sheet
            .compute_with(&NodeRef::new("Tree").classes(&["label"]), None, &mut scratch)
            .to_style()
            .fg
            .unwrap_or_else(|| palette.fg.color());

        // 4. Compute the layout from a snapshot of the current weights.
        let weights: Vec<f64> = state.entries.iter().map(|(_, w)| *w).collect();
        let cells = self.layout(area, &weights);

        // 5. Fill each cell rectangle, then overlay its label if it fits.
        for cr in &cells {
            let color = cell_colors[cr.entry % 5];
            let fill_style = Style::default().fg(color).bg(color);
            let r = cr.rect;
            let right = r.x + r.width;
            let bottom = r.y + r.height;

            // Fill every terminal cell in the rectangle with a full block.
            let mut y = r.y;
            while y < bottom {
                let mut x = r.x;
                while x < right {
                    buf[(x, y)].set_symbol(FILL).set_style(fill_style);
                    x += 1;
                }
                y += 1;
            }

            // Overlay the label on the top row, left-aligned, if it fits.
            if let Some((label, _w)) = state.entries.get(cr.entry) {
                let label_chars: Vec<char> = label.chars().collect();
                // Need at least label_len + 1 columns and at least 1 row.
                if r.height >= 1 && (label_chars.len() as u16) < r.width {
                    for (i, ch) in label_chars.iter().enumerate() {
                        let px = r.x + i as u16;
                        if px >= right {
                            break;
                        }
                        // Keep the cell color as bg; set the label char's fg to
                        // the label color for readability.
                        buf[(px, r.y)]
                            .set_symbol(ch.to_string().as_str())
                            .set_fg(label_color)
                            .set_bg(color);
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
    const H: u16 = 12;

    /// Render the widget into a fresh buffer and return it.
    fn render(state: &mut TreeMapState, widget: TreeMap, width: u16, height: u16) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        StatefulWidget::render(widget, Rect::new(0, 0, width, height), &mut buf, state);
        buf
    }

    /// Count non-blank cells (symbol != single space).
    fn non_blank(buf: &Buffer) -> usize {
        buf.content.iter().filter(|c| c.symbol() != " ").count()
    }

    #[test]
    fn renders_without_panicking_after_ticks() {
        let mut state = TreeMapState::new();
        for _ in 0..10 {
            state.tick();
        }
        let buf = render(&mut state, TreeMap::new(), W, H);
        assert!(non_blank(&buf) > 0, "treemap should draw something after ticks");
    }

    #[test]
    fn zero_area_does_not_panic() {
        let mut state = TreeMapState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        // Must be a no-op, not a panic.
        StatefulWidget::render(
            TreeMap::new(),
            Rect::new(0, 0, 0, 0),
            &mut buf,
            &mut state,
        );
    }

    #[test]
    fn tick_advances_and_changes_weights() {
        let mut state = TreeMapState::new();
        let before: Vec<f64> = state.entries.iter().map(|(_, w)| *w).collect();
        // Tick several times and assert at least one weight changed.
        let mut changed = false;
        for _ in 0..20 {
            state.tick();
            let now: Vec<f64> = state.entries.iter().map(|(_, w)| *w).collect();
            for (a, b) in before.iter().zip(now.iter()) {
                if (a - b).abs() > 1e-9 {
                    changed = true;
                }
            }
            if changed {
                break;
            }
        }
        assert!(changed, "at least one weight should change after ticks");
        // Total stays positive.
        assert!(state.total() > 0.0, "total must stay > 0 after ticks");
        // Weights stay >= 0.1.
        for (_, w) in &state.entries {
            assert!(*w >= 0.1, "weight must stay >= 0.1, got {w}");
        }
    }

    #[test]
    fn empty_state_renders_nothing() {
        let mut state = TreeMapState::new();
        state.clear();
        assert!(state.is_empty());
        let buf = render(&mut state, TreeMap::new(), W, H);
        assert_eq!(non_blank(&buf), 0, "empty state should render nothing");
    }

    #[test]
    fn set_and_push_work() {
        let mut state = TreeMapState::new();
        let len0 = state.len();
        state.set(0, "CORE-X", 9.0);
        assert_eq!(state.entry(0).unwrap().0, "CORE-X");
        assert!((state.entry(0).unwrap().1 - 9.0).abs() < 1e-9);
        state.push("EXTRA", 2.0);
        assert_eq!(state.len(), len0 + 1);
        assert_eq!(state.entry(len0).unwrap().0, "EXTRA");
        // set clamps negative weight to 0.
        state.set(1, "Z", -5.0);
        assert!((state.entry(1).unwrap().1 - 0.0).abs() < 1e-9);
    }

    #[test]
    fn remove_works() {
        let mut state = TreeMapState::new();
        let len0 = state.len();
        state.remove(0);
        assert_eq!(state.len(), len0 - 1);
        // Out-of-range remove is a no-op.
        state.remove(999);
        assert_eq!(state.len(), len0 - 1);
    }

    #[test]
    fn total_and_len() {
        let mut state = TreeMapState::new();
        let n = state.len();
        assert!(n >= 5);
        assert!(state.total() > 0.0);
        state.clear();
        assert_eq!(state.len(), 0);
        assert_eq!(state.total(), 0.0);
    }

    #[test]
    fn shape_variants_render_without_panicking() {
        for shape in [TreeShape::Flat, TreeShape::Slice, TreeShape::Brick] {
            let mut state = TreeMapState::new();
            for _ in 0..5 {
                state.tick();
            }
            let buf = render(&mut state, TreeMap::new().shape(shape), W, H);
            assert!(
                non_blank(&buf) > 0,
                "shape {shape:?} should render non-blank cells"
            );
        }
    }

    #[test]
    fn builder_setters_work() {
        let w = TreeMap::new().shape(TreeShape::Brick).theme(Theme::Weyland);
        assert_eq!(w.shape, TreeShape::Brick);
        assert_eq!(w.theme, Theme::Weyland);
    }

    #[test]
    fn default_is_cyberpunk() {
        let w = TreeMap::default();
        assert_eq!(w.theme, Theme::Cyberpunk);
    }

    #[test]
    fn default_shape_is_flat() {
        let w = TreeMap::default();
        assert_eq!(w.shape, TreeShape::Flat);
    }

    #[test]
    fn default_state_seeds_entries() {
        let state = TreeMapState::default();
        assert!(state.len() >= 5);
        assert!(state.total() > 0.0);
        assert_eq!(state.tick_count(), 0);
    }

    #[test]
    fn layout_covers_area() {
        // The union of cell rects should cover most of the area (allow a little
        // slack for integer rounding on the last row/cell).
        let widget = TreeMap::new().shape(TreeShape::Flat);
        let weights = [5.0_f64, 3.0, 2.0, 2.5, 4.0, 1.0];
        let area = Rect::new(0, 0, W, H);
        let cells = widget.layout(area, &weights);
        assert!(!cells.is_empty());
        let mut covered: u64 = 0;
        for cr in &cells {
            covered += (cr.rect.width as u64) * (cr.rect.height as u64);
        }
        let total_cells = (W as u64) * (H as u64);
        // Cover at least 90% of the area (rounding may leave a sliver).
        assert!(
            covered * 10 >= total_cells * 9,
            "cells should cover most of the area: covered={covered} total={total_cells}"
        );
        // Every cell is inside the area.
        for cr in &cells {
            let r = cr.rect;
            assert!(r.x >= area.x);
            assert!(r.y >= area.y);
            assert!(r.x + r.width <= area.x + area.width);
            assert!(r.y + r.height <= area.y + area.height);
        }
    }

    #[test]
    fn layout_slice_covers_area() {
        let widget = TreeMap::new().shape(TreeShape::Slice);
        let weights = [5.0_f64, 3.0, 2.0, 2.5, 4.0, 1.0];
        let area = Rect::new(0, 0, W, H);
        let cells = widget.layout(area, &weights);
        assert!(!cells.is_empty());
        let mut covered: u64 = 0;
        for cr in &cells {
            covered += (cr.rect.width as u64) * (cr.rect.height as u64);
        }
        let total_cells = (W as u64) * (H as u64);
        assert!(
            covered * 10 >= total_cells * 9,
            "slice cells should cover most of the area: covered={covered} total={total_cells}"
        );
    }

    #[test]
    fn label_overlays_when_cell_is_large_enough() {
        // One big entry whose label fits in a wide cell.
        let mut state = TreeMapState::new();
        state.clear();
        state.push("CORE", 100.0);
        state.push("X", 1.0);
        let buf = render(&mut state, TreeMap::new(), W, H);
        // The label "CORE" (4 chars) should appear somewhere on the top row.
        let top_row: String = (0..W).map(|x| buf[(x, 0)].symbol().to_string()).collect();
        assert!(
            top_row.contains('C'),
            "label char 'C' should render on the top row: {top_row:?}"
        );
    }

    #[test]
    fn zero_weights_render_nothing() {
        let mut state = TreeMapState::new();
        // Force every weight to zero (clamp keeps them at 0).
        let n = state.len();
        for i in 0..n {
            state.set(i, "Z", 0.0);
        }
        assert_eq!(state.total(), 0.0);
        let buf = render(&mut state, TreeMap::new(), W, H);
        assert_eq!(non_blank(&buf), 0, "zero-total state should render nothing");
    }

    #[test]
    fn render_across_many_ticks_does_not_panic() {
        let mut state = TreeMapState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        for shape in [TreeShape::Flat, TreeShape::Slice, TreeShape::Brick] {
            for _ in 0..50 {
                state.tick();
                let widget = TreeMap::new().shape(shape).theme(Theme::Fallout);
                StatefulWidget::render(
                    widget,
                    Rect::new(0, 0, W, H),
                    &mut buf,
                    &mut state,
                );
            }
        }
        assert!(non_blank(&buf) > 0);
    }
}
