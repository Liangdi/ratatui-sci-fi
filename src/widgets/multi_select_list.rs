//! **MultiSelectList** — a multi-select checklist.
//!
//! A vertical list where each item can be independently toggled — the
//! [`crate::Checkbox`] list (just as [`crate::RadioGroup`] is the
//! [`crate::Toggle`] list). A cursor marks the focused item; `Up`/`Down` move
//! it and `Space` flips the item's check.
//!
//! ## Spec
//! ```text
//! ▸ [✓] SHIELDS       (cursor + checked)
//!   [ ] CLOAK
//!   [✓] AUTOAIM
//! ```
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]: `items` are configuration; `cursor` + the
//!   per-item `checked` mask live in [`MultiSelectListState`].
//! - Key handling lives on the widget (it needs the item count):
//!   `list.handle_key(&mut state, key)`.
//! - Styling reuses the `Toggle.on` (checked, ok + bold) / `Toggle.off`
//!   (unchecked, muted) cascade nodes; the cursor's `▸` marker is `accent`.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{MultiSelectList, MultiSelectListState, MultiSelectShape, Theme};
//!
//! let list = MultiSelectList::new(["SHIELDS", "CLOAK", "AUTOAIM"]).theme(Theme::DeepSpace);
//! let mut state = MultiSelectListState::new(3);
//! ```

use ratatui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Cursor marker glyph.
pub const CURSOR: char = '▸';
/// Check mark glyph for the [`MultiSelectShape::Check`] default.
pub const MARK_CHECK: char = '✓';

/// Visual form of a [`MultiSelectList`]'s check mark.
///
/// Selects the checked/unchecked glyph pair; colors stay on the CSS cascade
/// (reusing `Toggle.on` / `Toggle.off`), untouched by this enum.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MultiSelectShape {
    /// `✓` checked, blank unchecked — the default.
    #[default]
    Check,
    /// `✕` checked, `·` unchecked.
    Cross,
    /// `■` checked, `□` unchecked.
    Block,
}

impl MultiSelectShape {
    /// The mark glyph for the given checked state.
    #[must_use]
    pub const fn mark(self, checked: bool) -> char {
        match (self, checked) {
            (Self::Check, true) => MARK_CHECK,
            (Self::Check, false) => ' ',
            (Self::Cross, true) => '✕',
            (Self::Cross, false) => '·',
            (Self::Block, true) => '■',
            (Self::Block, false) => '□',
        }
    }
}

/// A sci-fi multi-select list.
///
/// Build with [`MultiSelectList::new`] (an iterator of item labels).
#[derive(Debug, Clone)]
pub struct MultiSelectList {
    /// Item labels, top to bottom.
    pub items: Vec<String>,
    /// Mark-glyph form. Defaults to [`MultiSelectShape::Check`].
    pub shape: MultiSelectShape,
    /// Theme whose [`Stylesheet`](ratatui_style::Stylesheet) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl MultiSelectList {
    /// Create a list from an iterator of item labels.
    pub fn new(items: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            items: items.into_iter().map(Into::into).collect(),
            shape: MultiSelectShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the mark-glyph form (see [`MultiSelectShape`]).
    #[must_use]
    pub fn shape(mut self, shape: MultiSelectShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the list.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Apply a key event: `Up`/`Down` move the cursor (wrapping), `Space`
    /// toggles the item at the cursor. Other keys are ignored. A no-op when
    /// there are no items.
    pub fn handle_key(&self, state: &mut MultiSelectListState, key: KeyEvent) {
        let n = self.items.len();
        if n == 0 {
            return;
        }
        match key.code {
            KeyCode::Up | KeyCode::Left => state.cursor = (state.cursor + n - 1) % n,
            KeyCode::Down | KeyCode::Right => state.cursor = (state.cursor + 1) % n,
            KeyCode::Char(' ') => state.toggle(state.cursor.min(n - 1)),
            _ => {}
        }
    }
}

/// Mutable state for [`MultiSelectList`].
///
/// `cursor` is the focused item index; `checked` is the per-item toggle mask.
/// Both clamp on render; `checked` auto-grows to cover the item count.
#[derive(Debug, Clone)]
pub struct MultiSelectListState {
    /// Index of the focused (cursor) item.
    pub cursor: usize,
    /// Per-item checked flag.
    pub checked: Vec<bool>,
}

impl MultiSelectListState {
    /// Create a state with `n` unchecked items, cursor at 0.
    pub fn new(n: usize) -> Self {
        Self {
            cursor: 0,
            checked: vec![false; n],
        }
    }

    /// Flip the check on item `i` (no-op if out of range).
    pub fn toggle(&mut self, i: usize) {
        if let Some(c) = self.checked.get_mut(i) {
            *c = !*c;
        }
    }

    /// Whether item `i` is checked (false if out of range).
    pub fn is_checked(&self, i: usize) -> bool {
        self.checked.get(i).copied().unwrap_or(false)
    }
}

impl Default for MultiSelectListState {
    fn default() -> Self {
        Self::new(0)
    }
}

impl StatefulWidget for MultiSelectList {
    type State = MultiSelectListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() || self.items.is_empty() {
            return;
        }
        let n = self.items.len();
        let cursor = state.cursor.min(n - 1);
        // Grow the mask if the item list outgrew it.
        if state.checked.len() < n {
            state.checked.resize(n, false);
        }

        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let on_style = sheet
            .compute_with(&NodeRef::new("Toggle").classes(&["on"]), None, &mut scratch)
            .to_style();
        let off_style = sheet
            .compute_with(&NodeRef::new("Toggle").classes(&["off"]), None, &mut scratch)
            .to_style();
        let accent = self.theme.palette().accent.color();

        for (i, item) in self.items.iter().enumerate() {
            let y = area.y + i as u16;
            if y >= area.bottom() {
                break;
            }
            let is_cursor = i == cursor;
            let checked = state.is_checked(i);
            let mark = self.shape.mark(checked);
            let mark_style = if checked { on_style } else { off_style };

            let mut col = area.x;
            // Cursor marker (▸) or blank, accent-colored when the cursor is here.
            if col < area.right() {
                let glyph = if is_cursor { CURSOR } else { ' ' };
                buf[(col, y)].set_char(glyph).set_fg(accent);
                col += 1;
            }
            // `[mark]`.
            if col < area.right() {
                buf[(col, y)].set_char('[').set_style(mark_style);
                col += 1;
            }
            if col < area.right() {
                buf[(col, y)].set_char(mark).set_style(mark_style);
                col += 1;
            }
            if col < area.right() {
                buf[(col, y)].set_char(']').set_style(mark_style);
                col += 1;
            }
            // Space + item text.
            if col < area.right() {
                col += 1;
            }
            for ch in item.chars() {
                if col >= area.right() {
                    break;
                }
                buf[(col, y)].set_char(ch).set_style(mark_style);
                col += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect, style::Modifier};

    const W: u16 = 20;
    const H: u16 = 6;

    fn render(items: &[&str], state: &mut MultiSelectListState, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        StatefulWidget::render(
            MultiSelectList::new(items.iter().copied()).theme(theme),
            Rect::new(0, 0, W, H),
            &mut buf,
            state,
        );
        buf
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..W).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()
    }

    const fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, ratatui::crossterm::event::KeyModifiers::NONE)
    }

    #[test]
    fn unchecked_items_show_blank_mark() {
        let mut s = MultiSelectListState::new(2);
        let buf = render(&["A", "B"], &mut s, Theme::Cyberpunk);
        assert!(row_text(&buf, 0).contains("[ ]"), "unchecked → [ ]");
        assert!(!row_text(&buf, 0).contains(MARK_CHECK));
    }

    #[test]
    fn checked_item_shows_mark_and_ok_color() {
        let ok = Theme::Cyberpunk.palette().ok.color();
        let mut s = MultiSelectListState::new(2);
        s.toggle(0);
        let buf = render(&["A", "B"], &mut s, Theme::Cyberpunk);
        assert!(row_text(&buf, 0).contains(MARK_CHECK), "checked → [✓]");
        // The mark cell (col 2, after "▸[") is ok + bold.
        assert_eq!(buf[(2, 0)].fg, ok);
        assert!(buf[(2, 0)].modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn cursor_marker_on_focused_row() {
        let mut s = MultiSelectListState::new(3);
        s.cursor = 1;
        let buf = render(&["A", "B", "C"], &mut s, Theme::Cyberpunk);
        assert!(row_text(&buf, 0).starts_with(' '), "row 0 not cursor → blank");
        assert!(row_text(&buf, 1).starts_with(CURSOR), "row 1 cursor → ▸");
    }

    #[test]
    fn handle_key_down_moves_cursor() {
        let list = MultiSelectList::new(["A", "B", "C"]);
        let mut s = MultiSelectListState::new(3);
        list.handle_key(&mut s, key(KeyCode::Down));
        assert_eq!(s.cursor, 1);
        list.handle_key(&mut s, key(KeyCode::Down));
        list.handle_key(&mut s, key(KeyCode::Down));
        assert_eq!(s.cursor, 0, "Down wraps");
    }

    #[test]
    fn handle_key_space_toggles_cursor_item() {
        let list = MultiSelectList::new(["A", "B"]);
        let mut s = MultiSelectListState::new(2);
        assert!(!s.is_checked(0));
        list.handle_key(&mut s, key(KeyCode::Char(' ')));
        assert!(s.is_checked(0), "Space toggles item 0");
        list.handle_key(&mut s, key(KeyCode::Char(' ')));
        assert!(!s.is_checked(0), "Space toggles back");
    }

    #[test]
    fn mask_grows_to_item_count() {
        // State created with 0 items, then rendered against 2 → mask grows.
        let mut s = MultiSelectListState::new(0);
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        StatefulWidget::render(
            MultiSelectList::new(["A", "B"]),
            Rect::new(0, 0, W, H),
            &mut buf,
            &mut s,
        );
        assert_eq!(s.checked.len(), 2, "mask grew to 2");
        assert!(!s.is_checked(0) && !s.is_checked(1));
    }

    #[test]
    fn empty_items_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let mut state = MultiSelectListState::default();
        StatefulWidget::render(
            MultiSelectList::new(std::iter::empty::<&str>()),
            Rect::new(0, 0, W, H),
            &mut buf,
            &mut state,
        );
        assert_eq!(buf[(0, 0)].symbol(), " ");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let mut state = MultiSelectListState::new(1);
        StatefulWidget::render(MultiSelectList::new(["A"]), Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
