//! **ScanList** — list/table with scanline separators (PRD §3 列表与表格).
//!
//! ## Spec
//! - Faint horizontal scanline separates each row.
//! - The selected row is fully highlighted, with a blinking cursor glyph at
//!   the row head (`█` when visible, space when not).
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]; selection + blink phase live in
//!   [`ScanListState`], advanced by the event loop each tick.
//! - Blink cadence: toggle cursor every [`DEFAULT_CURSOR_PERIOD`] ticks
//!   (default 15) — pass a different value to [`ScanListState::cursor_visible`]
//!   to customize.
//! - Scanline approach (chosen & documented here): each item occupies **two**
//!   buffer rows — the text row, followed by a faint full-width `─` scanline
//!   row. Row styling goes through the theme's
//!   [`Stylesheet`](ratatui_style::Stylesheet) cascade: the text row queries
//!   the `List` node (overridden to `List.selected` — accent text on a panel
//!   background — for the selected row), and the scanline queries `List.scan`
//!   (muted). The rules are `var(--…)`-driven off the same palette, so the
//!   resolved colors match reading `palette()` directly.
//!
//! # Example
//!
//! ```no_run
//! use ratatui_sci_fi::{ScanList, ScanListState, Theme};
//!
//! let mut state = ScanListState::default();
//! let list = ScanList::new(["alpha", "beta", "gamma"]).theme(Theme::Cyberpunk);
//! // in your event loop: state.tick(); and adjust state.selected on key events.
//! ```

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::StatefulWidget,
};
use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Default blink period (in ticks) for the selection cursor.
///
/// The cursor is visible for `DEFAULT_CURSOR_PERIOD` ticks, then hidden for the
/// same span, repeating. Pass any other value to
/// [`ScanListState::cursor_visible`] to override.
pub const DEFAULT_CURSOR_PERIOD: u64 = 15;

/// Glyph drawn for the blinking selection cursor when it is visible.
pub const CURSOR_GLYPH: &str = "█";

/// Glyph drawn for the faint per-row scanline separator.
pub const SCANLINE_GLYPH: &str = "─";

/// A scanline-separated list (PRD §3).
///
/// Built from an iterator of (string) items and an optional [`Theme`]
/// (default [`Theme::Cyberpunk`]). Selection and blink animation live in the
/// companion [`ScanListState`], mutated by the app's event loop each tick.
#[derive(Debug, Clone, Default)]
pub struct ScanList {
    /// The rows to display, in order.
    pub items: Vec<String>,
    /// Active theme; controls all colors via its [`Palette`](crate::Palette).
    pub theme: Theme,
}

impl ScanList {
    /// Build a list from any iterator of stringifiable items.
    pub fn new(items: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self { items: items.into_iter().map(Into::into).collect(), theme: Theme::default() }
    }

    /// Replace the theme (builder). Default is [`Theme::Cyberpunk`].
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

/// Mutable state for [`ScanList`].
///
/// `selected` is the index of the highlighted row; `tick` is the animation
/// clock, advanced each frame by the app (or by [`Self::tick`]).
#[derive(Debug, Default, Clone)]
pub struct ScanListState {
    /// Index of the currently selected row. Clamped to `items.len()-1` at
    /// render time, so out-of-range values never panic on an empty list.
    pub selected: usize,
    /// Increments each tick; the app advances it (or call [`Self::tick`]).
    pub tick: u64,
}

impl ScanListState {
    /// Advance animation time by one tick.
    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }

    /// Whether the blinking cursor is currently visible.
    ///
    /// Uses the existing `.is_multiple_of(2)` cadence: the cursor is on for
    /// the first `period` ticks of each `2*period` cycle, then off.
    pub fn cursor_visible(&self, period: u64) -> bool {
        (self.tick / period.max(1)).is_multiple_of(2)
    }
}

impl StatefulWidget for ScanList {
    type State = ScanListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Nothing to draw if there's no space or no items.
        if area.width == 0 || area.height == 0 || self.items.is_empty() {
            return;
        }

        // Row styles come from the theme's stylesheet cascade — the `List`,
        // `List.selected`, and `List.scan` rules. One `ComputeScratch` is
        // reused across the three lookups (the documented draw-loop pattern).
        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let normal_style = sheet.compute_with(&NodeRef::new("List"), None, &mut scratch).to_style();
        let selected_style = sheet
            .compute_with(&NodeRef::new("List").classes(&["selected"]), None, &mut scratch)
            .to_style();
        let scan_style = sheet
            .compute_with(&NodeRef::new("List").classes(&["scan"]), None, &mut scratch)
            .to_style();

        // Each item reserves two rows: text + scanline. The last item's
        // scanline is drawn only if there's room.
        let row_stride: u16 = 2;

        // Clamp selection into range so an empty/shrinking list never panics.
        let selected = state.selected.min(self.items.len() - 1);
        let cursor_on = state.cursor_visible(DEFAULT_CURSOR_PERIOD);

        for (i, item) in self.items.iter().enumerate() {
            let text_y = area.y + (i as u16) * row_stride;
            // Stop once we run past the bottom of the area.
            if text_y >= area.bottom() {
                break;
            }

            let is_selected = i == selected;
            let row_style = if is_selected { selected_style } else { normal_style };

            // Cursor glyph occupies column 0 of a selected row; a leading
            // space otherwise keeps columns aligned across rows.
            let cursor = if is_selected && cursor_on { CURSOR_GLYPH } else { " " };

            // Write the cursor cell + text. We write cell-by-cell up to the
            // area width so styling is consistent and we never overshoot.
            let mut col = area.x;

            // Cursor cell (column 0 of the row, relative to area.x).
            if col < area.right() {
                let cell = &mut buf[(col, text_y)];
                cell.set_symbol(cursor).set_style(row_style);
                col += 1;
            }

            // Item text, char by char, filling the remainder of the row with
            // spaces so the background fill is continuous for selected rows.
            for ch in item.chars() {
                if col >= area.right() {
                    break;
                }
                let cell = &mut buf[(col, text_y)];
                cell.set_char(ch).set_style(row_style);
                col += 1;
            }
            while col < area.right() {
                let cell = &mut buf[(col, text_y)];
                cell.set_symbol(" ").set_style(row_style);
                col += 1;
            }

            // Faint scanline row directly beneath the text row (skip for the
            // last item if we'd overflow the area — keeps the bottom clean).
            let scan_y = text_y + 1;
            if scan_y < area.bottom() {
                let scan_ch = SCANLINE_GLYPH.chars().next().unwrap_or('─');
                let mut s_col = area.x;
                while s_col < area.right() {
                    let cell = &mut buf[(s_col, scan_y)];
                    cell.set_char(scan_ch).set_style(scan_style);
                    s_col += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    /// Helper: render a list into a fresh buffer and return it.
    fn render(
        items: &[&str],
        theme: Theme,
        selected: usize,
        tick: u64,
        width: u16,
        height: u16,
    ) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        let widget = ScanList::new(items.iter().copied()).theme(theme);
        let mut state = ScanListState { selected, tick };
        StatefulWidget::render(widget, Rect::new(0, 0, width, height), &mut buf, &mut state);
        buf
    }

    #[test]
    fn selected_row_head_shows_cursor_when_visible() {
        // tick 0 -> (0/15).is_multiple_of(2) == true -> cursor visible.
        let buf = render(&["alpha", "beta", "gamma"], Theme::Cyberpunk, 1, 0, 12, 6);
        // Selected row (index 1) lives at y = 1*2 = 2; its first cell is the cursor.
        assert_eq!(buf[(0, 2)].symbol(), CURSOR_GLYPH, "selected row head should be the cursor glyph");
    }

    #[test]
    fn selected_row_head_is_space_when_cursor_hidden() {
        // tick = DEFAULT_CURSOR_PERIOD (15) -> (15/15)=1, 1.is_multiple_of(2)==false -> hidden.
        let buf = render(&["alpha", "beta", "gamma"], Theme::Cyberpunk, 1, DEFAULT_CURSOR_PERIOD, 12, 6);
        assert_eq!(buf[(0, 2)].symbol(), " ", "cursor glyph should be hidden (space) this half-cycle");
    }

    #[test]
    fn selected_row_text_and_cursor_share_accent_fg() {
        let buf = render(&["alpha", "beta"], Theme::Cyberpunk, 0, 0, 12, 4);
        let accent = Theme::Cyberpunk.palette().accent.color();
        // Row 0, col 0 = cursor; col 1 = first char of "alpha".
        assert_eq!(buf[(0, 0)].symbol(), CURSOR_GLYPH);
        assert_eq!(buf[(0, 0)].style().fg, Some(accent));
        assert_eq!(buf[(1, 0)].symbol(), "a");
        assert_eq!(buf[(1, 0)].style().fg, Some(accent));
    }

    #[test]
    fn non_selected_rows_use_normal_fg() {
        let buf = render(&["alpha", "beta"], Theme::Cyberpunk, 1, 0, 12, 4);
        let fg = Theme::Cyberpunk.palette().fg.color();
        // Row 0 is not selected; its head cell is a space with normal fg.
        assert_eq!(buf[(0, 0)].symbol(), " ");
        assert_eq!(buf[(0, 0)].style().fg, Some(fg));
    }

    #[test]
    fn scanline_row_is_drawn_in_muted() {
        let buf = render(&["alpha", "beta"], Theme::Cyberpunk, 0, 0, 6, 4);
        let muted = Theme::Cyberpunk.palette().muted.color();
        // Beneath row 0 (y=0) is the scanline at y=1.
        assert_eq!(buf[(0, 1)].symbol(), SCANLINE_GLYPH);
        assert_eq!(buf[(0, 1)].style().fg, Some(muted));
        assert_eq!(buf[(3, 1)].symbol(), SCANLINE_GLYPH, "scanline spans the row width");
    }

    #[test]
    fn selected_out_of_range_is_clamped_not_panicked() {
        // selected=99 with only 2 items must clamp to index 1, not panic.
        let buf = render(&["alpha", "beta"], Theme::Cyberpunk, 99, 0, 8, 4);
        // Clamped selection is row 1 (y=2); its head should be the cursor.
        assert_eq!(buf[(0, 2)].symbol(), CURSOR_GLYPH);
        // Row 0 is now non-selected -> normal head.
        assert_eq!(buf[(0, 0)].symbol(), " ");
    }

    #[test]
    fn empty_list_renders_nothing_without_panicking() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 4));
        let widget = ScanList::new(std::iter::empty::<&str>()).theme(Theme::Fallout);
        let mut state = ScanListState { selected: 5, tick: 0 };
        // Should be a no-op; buffer stays empty.
        StatefulWidget::render(widget, Rect::new(0, 0, 10, 4), &mut buf, &mut state);
        // First cell remains the default space.
        assert_eq!(buf[(0, 0)].symbol(), " ");
    }

    #[test]
    fn theme_builder_changes_colors() {
        let buf = render(&["x"], Theme::Fallout, 0, 0, 4, 2);
        let fallout_accent = Theme::Fallout.palette().accent.color();
        assert_eq!(buf[(0, 0)].symbol(), CURSOR_GLYPH);
        assert_eq!(buf[(0, 0)].style().fg, Some(fallout_accent), "Fallout accent should be used");
    }

    #[test]
    fn cursor_visible_period_cadence() {
        let st = ScanListState { tick: 0, selected: 0 };
        // period 15: ticks 0..14 visible, 15..29 hidden, ...
        assert!(st.cursor_visible(15));
        assert!(!ScanListState { tick: 15, selected: 0 }.cursor_visible(15));
        assert!(ScanListState { tick: 30, selected: 0 }.cursor_visible(15));
    }
}
