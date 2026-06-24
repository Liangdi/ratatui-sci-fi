//! **ScrollView** — a vertical scrollable text region.
//!
//! A viewport over a list of lines that's taller than the area — the log /
//! readout pane. `Up`/`Down`/`PageUp`/`PageDown`/`Home`/`End` scroll it, and a
//! scrollbar on the right marks the position.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]: `lines` are configuration; only `offset` is
//!   state, advanced by [`ScrollView::handle_key`] (which needs the viewport
//!   height — pass the area height you render into).
//! - Styling reuses the `Value` (fg) node for text; the scrollbar reads
//!   `accent` (thumb) / `muted` (track) off the palette.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{ScrollView, ScrollViewState, Theme};
//!
//! let view = ScrollView::new((0..50).map(|i| format!("line {i}"))).theme(Theme::DeepSpace);
//! let mut state = ScrollViewState::new();
//! // view.handle_key(&mut state, key, area.height);
//! ```

use ratatui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Thumb glyph for the [`ScrollViewShape::Bar`] default.
pub const THUMB: char = '●';
/// Track glyph for the [`ScrollViewShape::Bar`] default.
pub const TRACK: char = '│';

/// Visual form of a [`ScrollView`]'s scrollbar.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ScrollViewShape {
    /// A `│` track with a `●` thumb — the default.
    #[default]
    Bar,
    /// A `░` track with a `█` thumb.
    Block,
}

/// A sci-fi scrollable text view.
///
/// Build with [`ScrollView::new`] (an iterator of lines).
#[derive(Debug, Clone)]
pub struct ScrollView {
    /// The lines to scroll through.
    pub lines: Vec<String>,
    /// Scrollbar form. Defaults to [`ScrollViewShape::Bar`].
    pub shape: ScrollViewShape,
    /// Theme whose [`Stylesheet`](ratatui_style::Stylesheet) / [`Palette`](crate::Palette) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl ScrollView {
    /// Create a scroll view from an iterator of lines.
    pub fn new(lines: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            lines: lines.into_iter().map(Into::into).collect(),
            shape: ScrollViewShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the scrollbar form (see [`ScrollViewShape`]).
    #[must_use]
    pub fn shape(mut self, shape: ScrollViewShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the view.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Apply a key event. `viewport` is the visible row count (the area height
    /// you render into). `Up`/`Down` step one, `PageUp`/`PageDown` step a page,
    /// `Home`/`End` jump.
    pub fn handle_key(&self, state: &mut ScrollViewState, key: KeyEvent, viewport: u16) {
        let total = self.lines.len();
        let max = total.saturating_sub(viewport as usize);
        match key.code {
            KeyCode::Up => state.offset = state.offset.saturating_sub(1),
            KeyCode::Down => state.offset = state.offset.saturating_add(1).min(max),
            KeyCode::PageUp => state.offset = state.offset.saturating_sub(viewport as usize),
            KeyCode::PageDown => state.offset = state.offset.saturating_add(viewport as usize).min(max),
            KeyCode::Home => state.offset = 0,
            KeyCode::End => state.offset = max,
            _ => {}
        }
    }
}

/// Mutable state for [`ScrollView`].
///
/// `offset` is the index of the topmost visible line; clamped on render.
#[derive(Debug, Default, Clone)]
pub struct ScrollViewState {
    /// Index of the top visible line.
    pub offset: usize,
}

impl ScrollViewState {
    /// Create a state at offset 0.
    pub fn new() -> Self {
        Self::default()
    }
}

impl StatefulWidget for ScrollView {
    type State = ScrollViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }
        // Reserve one column on the right for the scrollbar.
        let text_w = area.width.saturating_sub(1);
        if text_w == 0 {
            return;
        }
        let viewport = area.height as usize;
        let total = self.lines.len();
        let max = total.saturating_sub(viewport);
        state.offset = state.offset.min(max);

        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let text_style = sheet.compute_with(&NodeRef::new("Value"), None, &mut scratch).to_style();
        let p = self.theme.palette();
        let thumb_color = p.accent.color();
        let track_color = p.muted.color();

        // Visible lines.
        for (i, line) in self.lines.iter().skip(state.offset).take(viewport).enumerate() {
            let y = area.y + i as u16;
            for (ci, ch) in line.chars().enumerate() {
                let px = area.x + ci as u16;
                if px >= area.x + text_w {
                    break;
                }
                buf[(px, y)].set_char(ch).set_style(text_style);
            }
        }

        // Scrollbar in the rightmost column.
        let sb_x = area.right() - 1;
        let pos = (state.offset * viewport.saturating_sub(1) / max.max(1)) as u16;
        for sy in 0..area.height {
            let is_thumb = sy == pos && total > viewport;
            let (glyph, color) = match (self.shape, is_thumb) {
                (ScrollViewShape::Bar, true) => (THUMB, thumb_color),
                (ScrollViewShape::Bar, false) => (TRACK, track_color),
                (ScrollViewShape::Block, true) => ('█', thumb_color),
                (ScrollViewShape::Block, false) => ('░', track_color),
            };
            buf[(sb_x, area.y + sy)].set_char(glyph).set_fg(color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 16;
    const H: u16 = 4;

    fn render(lines: &[&str], offset: usize, shape: ScrollViewShape, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let mut state = ScrollViewState { offset };
        StatefulWidget::render(
            ScrollView::new(lines.iter().copied()).shape(shape).theme(theme),
            Rect::new(0, 0, W, H),
            &mut buf,
            &mut state,
        );
        buf
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        // Text occupies columns 0..W-1 (last col is the scrollbar).
        (0..W - 1).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()
    }

    const fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, ratatui::crossterm::event::KeyModifiers::NONE)
    }

    #[test]
    fn shows_lines_from_offset() {
        let buf = render(&["a", "b", "c", "d", "e", "f", "g"], 2, ScrollViewShape::Bar, Theme::Cyberpunk);
        assert!(row_text(&buf, 0).starts_with('c'), "offset 2 → 'c' at top");
        assert!(row_text(&buf, 1).starts_with('d'));
    }

    #[test]
    fn handle_key_down_advances_offset() {
        let view = ScrollView::new(["a", "b", "c", "d", "e"]);
        let mut s = ScrollViewState::new();
        view.handle_key(&mut s, key(KeyCode::Down), H);
        assert_eq!(s.offset, 1);
    }

    #[test]
    fn handle_key_clamps_at_bottom() {
        let view = ScrollView::new(["a", "b", "c", "d", "e"]);
        let mut s = ScrollViewState { offset: 99 };
        view.handle_key(&mut s, key(KeyCode::End), H);
        // max offset = 5 - 4 = 1.
        assert_eq!(s.offset, 1);
    }

    #[test]
    fn scrollbar_present() {
        let buf = render(&["a", "b", "c", "d", "e"], 0, ScrollViewShape::Bar, Theme::Cyberpunk);
        // Rightmost column holds the track glyph.
        assert_eq!(buf[(W - 1, 1)].symbol(), TRACK.to_string().as_str(), "row 1 is track (row 0 is the thumb)");
    }

    #[test]
    fn thumb_moves_with_offset() {
        let at0 = render(&["a", "b", "c", "d", "e"], 0, ScrollViewShape::Bar, Theme::Cyberpunk);
        let at1 = render(&["a", "b", "c", "d", "e"], 1, ScrollViewShape::Bar, Theme::Cyberpunk);
        let thumb0 = (0..H).find(|&y| at0[(W - 1, y)].symbol() == THUMB.to_string().as_str());
        let thumb1 = (0..H).find(|&y| at1[(W - 1, y)].symbol() == THUMB.to_string().as_str());
        assert_ne!(thumb0, thumb1, "thumb moves with offset");
    }

    #[test]
    fn block_shape_uses_blocks() {
        let buf = render(&["a", "b", "c", "d", "e"], 0, ScrollViewShape::Block, Theme::Cyberpunk);
        assert_eq!(buf[(W - 1, 1)].symbol(), "░", "Block track is ░ (row 1; row 0 is the thumb)");
    }

    #[test]
    fn fits_when_lines_fewer_than_viewport() {
        // No scrollbar thumb needed when everything fits.
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let mut state = ScrollViewState::new();
        StatefulWidget::render(
            ScrollView::new(["only"]).theme(Theme::Cyberpunk),
            Rect::new(0, 0, W, H),
            &mut buf,
            &mut state,
        );
        // No thumb glyph anywhere (track only).
        let has_thumb = (0..H).any(|y| buf[(W - 1, y)].symbol() == THUMB.to_string().as_str());
        assert!(!has_thumb, "no thumb when content fits");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let mut state = ScrollViewState::new();
        StatefulWidget::render(ScrollView::new(["a"]), Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
