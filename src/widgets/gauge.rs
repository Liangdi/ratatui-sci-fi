//! **EnergyGauge** — reactor-style segmented energy bar (PRD §3 进度条/条形图).
//!
//! A horizontal gauge made of `▰` (filled) / `▱` (empty) cells, like a reactor
//! readout: `▰▰▰▰▱▱▱▱`. The bar color shifts with the level — nominal → warn →
//! alert — so a glance is enough to tell whether the ship is about to lose
//! power.
//!
//! ## Spec
//! - Drop the standard block characters; use `▰` / `▱` glyphs to simulate a
//!   reactor's segmented energy cells.
//! - `ratio` (0.0..=1.0) is clamped, then mapped to a filled-segment count via
//!   `round(ratio * segments)`.
//! - Color shifts with level: `ratio ≥ 0.6` → ok (`palette.ok`), `0.3..0.6` →
//!   warn (`palette.warn`), `< 0.3` → alert (`palette.alert`).
//! - Optional left `label` (e.g. `PWR`) and a right-aligned percentage
//!   (e.g. ` 78%`).
//!
//! ## Implementation notes
//! - Stateless [`Widget`]; `ratio` is per-frame config.
//! - Drawn cell-by-cell directly into the [`Buffer`], with every color routed
//!   through the theme's [`Stylesheet`](crate::Theme::stylesheet) cascade: the
//!   `Gauge` / `Gauge.ok`|`Gauge.warn`|`Gauge.alert` rules drive the bar color,
//!   `Gauge.empty` drives the empty cells, and `Gauge.label` drives the label.
//!   Because every rule is `var(--…)`-backed off the same palette, the rendered
//!   colors are byte-identical to reading the palette directly.

use crate::Theme;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::Widget,
};
#[cfg(test)]
use ratatui::style::Color;
use ratatui_style::{ComputeScratch, NodeRef};

const FILLED: &str = "▰";
const EMPTY: &str = "▱";

/// A segmented reactor energy gauge.
///
/// ```rust
/// use ratatui_sci_fi::{EnergyGauge, Theme};
///
/// let gauge = EnergyGauge::new(0.78)
///     .label("PWR")
///     .segments(16)
///     .theme(Theme::Weyland);
/// ```
#[derive(Debug, Clone)]
pub struct EnergyGauge {
    /// Optional left-aligned label, e.g. `"PWR"`.
    pub label: Option<String>,
    /// Energy ratio, clamped to `0.0..=1.0` at render time.
    pub ratio: f64,
    /// Number of reactor cells in the bar. Default `16`.
    pub segments: u16,
    /// Theme whose palette drives the bar/label colors. Default [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Default for EnergyGauge {
    fn default() -> Self {
        Self { label: None, ratio: 0.0, segments: 16, theme: Theme::Cyberpunk }
    }
}

impl EnergyGauge {
    /// Create a gauge with the given `ratio` (clamped at render time).
    pub fn new(ratio: f64) -> Self {
        Self::default().ratio(ratio)
    }

    /// Attach a left-aligned label (e.g. `"PWR"`).
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the energy ratio. Values outside `0.0..=1.0` are clamped on render.
    #[must_use]
    pub fn ratio(mut self, ratio: f64) -> Self {
        self.ratio = ratio;
        self
    }

    /// Set the number of reactor segments.
    #[must_use]
    pub fn segments(mut self, segments: u16) -> Self {
        self.segments = segments;
        self
    }

    /// Set the theme whose palette drives colors.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Clamp `ratio` into `0.0..=1.0`.
    fn clamped_ratio(&self) -> f64 {
        self.ratio.clamp(0.0, 1.0)
    }

    /// Number of filled `▰` cells.
    fn filled_count(&self) -> u16 {
        let ratio = self.clamped_ratio();
        let seg = self.segments as f64;
        (ratio * seg).round() as u16
    }

    /// Map the current ratio to its level's CSS class name.
    fn level_class(&self) -> &'static str {
        let ratio = self.clamped_ratio();
        if ratio >= 0.6 {
            "ok"
        } else if ratio >= 0.3 {
            "warn"
        } else {
            "alert"
        }
    }

    /// Pick the bar color for the current level — resolved through the
    /// `Gauge.<level>` cascade node. Used by tests; render reuses the computed
    /// style directly.
    #[cfg(test)]
    fn bar_color(&self) -> Color {
        let sheet = self.theme.stylesheet();
        let cls = self.level_class();
        sheet.compute(&NodeRef::new("Gauge").classes(&[cls]), None).to_style().fg.unwrap()
    }
}

impl Widget for EnergyGauge {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let bar_style =
            sheet.compute_with(&NodeRef::new("Gauge").classes(&[self.level_class()]), None, &mut scratch).to_style();
        let label_style =
            sheet.compute_with(&NodeRef::new("Gauge").classes(&["label"]), None, &mut scratch).to_style();
        let empty_style =
            sheet.compute_with(&NodeRef::new("Gauge").classes(&["empty"]), None, &mut scratch).to_style();
        let gap_bg = sheet.compute_with(&NodeRef::new("Gauge"), None, &mut scratch).to_style().bg.unwrap();

        let y = area.y;
        let mut x = area.x;
        let right = area.x + area.width;

        // Left label, e.g. "PWR ".
        if let Some(label) = &self.label {
            let label_cell_count = label.chars().count() as u16;
            if x + label_cell_count <= right {
                for ch in label.chars() {
                    buf[(x, y)]
                        .set_symbol(ch.to_string().as_str())
                        .set_style(label_style);
                    x += 1;
                }
                // One-cell gap after the label, if there's room.
                if x < right {
                    buf[(x, y)].set_style(Style::default().bg(gap_bg));
                    x += 1;
                }
            }
        }

        // Segmented bar.
        let filled = self.filled_count();
        let segments = self.segments;
        for i in 0..segments {
            if x >= right {
                break;
            }
            let glyph = if i < filled { FILLED } else { EMPTY };
            let style = if i < filled { bar_style } else { empty_style };
            buf[(x, y)].set_symbol(glyph).set_style(style);
            x += 1;
        }

        // Right-aligned percentage, e.g. " 78%".
        let pct = (self.clamped_ratio() * 100.0).round() as u32;
        let pct_text = format!("{pct:>3}%");
        let pct_len = pct_text.chars().count() as u16;
        // Walk backwards from the right edge so the value hugs the right margin.
        if pct_len < area.width {
            let start_px = right.saturating_sub(pct_len);
            for (i, ch) in pct_text.chars().enumerate() {
                let px = start_px + i as u16;
                if px >= right {
                    break;
                }
                buf[(px, y)].set_symbol(ch.to_string().as_str()).set_style(bar_style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    const W: u16 = 24;
    const H: u16 = 1;

    fn cell_symbol(buf: &Buffer, x: u16, y: u16) -> &str {
        buf[(x, y)].symbol()
    }

    #[test]
    fn full_ratio_fills_every_segment() {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        EnergyGauge::new(1.0).segments(8).render(Rect::new(0, 0, W, H), &mut buf);
        for x in 0..8 {
            assert_eq!(cell_symbol(&buf, x, 0), "▰", "segment {x} should be filled");
        }
    }

    #[test]
    fn zero_ratio_empties_every_segment() {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        EnergyGauge::new(0.0).segments(8).render(Rect::new(0, 0, W, H), &mut buf);
        for x in 0..8 {
            assert_eq!(cell_symbol(&buf, x, 0), "▱", "segment {x} should be empty");
        }
    }

    #[test]
    fn filled_count_matches_round_ratio_times_segments() {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        // 0.5 * 8 = 4 filled.
        EnergyGauge::new(0.5).segments(8).render(Rect::new(0, 0, W, H), &mut buf);
        let filled = (0..8).filter(|&x| cell_symbol(&buf, x, 0) == "▰").count();
        assert_eq!(filled, 4);

        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        // round(0.34 * 8) = round(2.72) = 3 filled.
        EnergyGauge::new(0.34).segments(8).render(Rect::new(0, 0, W, H), &mut buf);
        let filled = (0..8).filter(|&x| cell_symbol(&buf, x, 0) == "▰").count();
        assert_eq!(filled, 3);
    }

    #[test]
    fn ratio_is_clamped() {
        // Over-range clamps to 1.0 → all filled.
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        EnergyGauge::new(5.0).segments(8).render(Rect::new(0, 0, W, H), &mut buf);
        assert_eq!(cell_symbol(&buf, 7, 0), "▰");

        // Negative clamps to 0.0 → all empty.
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        EnergyGauge::new(-1.0).segments(8).render(Rect::new(0, 0, W, H), &mut buf);
        assert_eq!(cell_symbol(&buf, 0, 0), "▱");
    }

    #[test]
    fn bar_color_shifts_with_level() {
        let palette = Theme::Cyberpunk.palette();

        // ok level
        let g = EnergyGauge::new(0.8).theme(Theme::Cyberpunk);
        assert_eq!(g.bar_color(), palette.ok.color());

        // warn level
        let g = EnergyGauge::new(0.45).theme(Theme::Cyberpunk);
        assert_eq!(g.bar_color(), palette.warn.color());

        // alert level
        let g = EnergyGauge::new(0.1).theme(Theme::Cyberpunk);
        assert_eq!(g.bar_color(), palette.alert.color());
    }

    #[test]
    fn filled_segment_uses_bar_color_empty_uses_muted() {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        EnergyGauge::new(0.5)
            .segments(8)
            .theme(Theme::Cyberpunk)
            .render(Rect::new(0, 0, W, H), &mut buf);

        let palette = Theme::Cyberpunk.palette();
        assert_eq!(buf[(0, 0)].fg, palette.warn.color(), "filled cell should be bar-colored (warn at 0.5)");
        assert_eq!(buf[(4, 0)].fg, palette.muted.color(), "empty cell should be muted");
    }

    #[test]
    fn label_renders_on_the_left() {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        EnergyGauge::new(0.5)
            .label("PWR")
            .segments(8)
            .render(Rect::new(0, 0, W, H), &mut buf);

        assert_eq!(cell_symbol(&buf, 0, 0), "P");
        assert_eq!(cell_symbol(&buf, 1, 0), "W");
        assert_eq!(cell_symbol(&buf, 2, 0), "R");
        // gap at x=3, then bar starts at x=4.
        assert_eq!(cell_symbol(&buf, 4, 0), "▰");
    }

    #[test]
    fn percentage_renders_right_aligned() {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        EnergyGauge::new(0.781)
            .segments(8)
            .render(Rect::new(0, 0, W, H), &mut buf);

        // round(78.1) = 78 → " 78%" occupies last 4 cells.
        assert_eq!(cell_symbol(&buf, W - 4, 0), " ");
        assert_eq!(cell_symbol(&buf, W - 3, 0), "7");
        assert_eq!(cell_symbol(&buf, W - 2, 0), "8");
        assert_eq!(cell_symbol(&buf, W - 1, 0), "%");
    }

    #[test]
    fn empty_area_is_a_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        EnergyGauge::new(0.5).render(Rect::new(0, 0, 0, 0), &mut buf);
        // Didn't panic — that's the contract.
    }

    #[test]
    fn theme_builder_is_applied() {
        let g = EnergyGauge::new(0.5).theme(Theme::Weyland);
        assert_eq!(g.theme, Theme::Weyland);
        assert_eq!(g.bar_color(), Theme::Weyland.palette().warn.color());
    }
}
