//! **TextArea** — a multi-line text editor.
//!
//! The multi-line sibling of [`crate::TextInput`]: a buffer of lines with a
//! row/column cursor that supports typing, backspace, enter (new line), and
//! arrow-key navigation. The cursor blinks on the shared cadence.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]: the lines + cursor + blink clock live in
//!   [`TextAreaState`]; editing happens via [`TextAreaState::handle_key`]
//!   (the state owns the text, like [`crate::TextInputState`]).
//! - Styling reuses the `Input` (fg) node for text and the `Cursor` (accent)
//!   node for the caret.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{TextArea, TextAreaState, Theme};
//!
//! let mut state = TextAreaState::new();
//! let area = TextArea::new().theme(Theme::DeepSpace);
//! ```

use ratatui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;
use crate::widgets::list::DEFAULT_CURSOR_PERIOD;

/// Cursor glyph for the [`TextAreaShape::Block`] default.
pub const CURSOR_BLOCK: char = '█';
/// Cursor glyph for the [`TextAreaShape::Bar`] variant.
pub const CURSOR_BAR: char = '_';

/// Visual form of a [`TextArea`]'s caret.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TextAreaShape {
    /// `█` — the default.
    #[default]
    Block,
    /// `_`.
    Bar,
}

impl TextAreaShape {
    /// The caret glyph.
    #[must_use]
    pub const fn glyph(self) -> char {
        match self {
            Self::Block => CURSOR_BLOCK,
            Self::Bar => CURSOR_BAR,
        }
    }
}

/// A sci-fi multi-line text area.
///
/// Build with [`TextArea::new`], then set the theme. The text + cursor live in
/// [`TextAreaState`].
#[derive(Debug, Clone)]
pub struct TextArea {
    /// Caret-glyph form. Defaults to [`TextAreaShape::Block`].
    pub shape: TextAreaShape,
    /// Theme whose [`Stylesheet`](ratatui_style::Stylesheet) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Default for TextArea {
    fn default() -> Self {
        Self {
            shape: TextAreaShape::default(),
            theme: Theme::Cyberpunk,
        }
    }
}

impl TextArea {
    /// Create a text area, default shape/theme.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the caret-glyph form (see [`TextAreaShape`]).
    #[must_use]
    pub fn shape(mut self, shape: TextAreaShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the text area.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

/// Mutable state for [`TextArea`].
///
/// `lines` holds the text (one `String` per row); `row`/`col` are the cursor
/// (col is a **char** index into the current line); `blink_tick` drives the
/// caret. All are mutated by [`Self::handle_key`] or set directly by the app.
#[derive(Debug, Clone)]
pub struct TextAreaState {
    /// The text, one `String` per line.
    pub lines: Vec<String>,
    /// Cursor row.
    pub row: usize,
    /// Cursor column (char index into `lines[row]`).
    pub col: usize,
    /// Blink clock, advanced once per frame.
    pub blink_tick: u64,
}

impl Default for TextAreaState {
    fn default() -> Self {
        Self::new()
    }
}

impl TextAreaState {
    /// Create an empty state (one blank line, cursor at 0:0).
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            row: 0,
            col: 0,
            blink_tick: 0,
        }
    }

    /// Create a state seeded with `text` (split on `\n`).
    pub fn from_text(text: &str) -> Self {
        let lines: Vec<String> = if text.is_empty() {
            vec![String::new()]
        } else {
            text.split('\n').map(String::from).collect()
        };
        Self {
            lines,
            row: 0,
            col: 0,
            blink_tick: 0,
        }
    }

    /// The full text, lines joined by `\n`.
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// Advance the blink clock one tick.
    pub fn tick(&mut self) {
        self.blink_tick = self.blink_tick.wrapping_add(1);
    }

    /// Whether the caret is currently visible.
    pub fn cursor_visible(&self) -> bool {
        (self.blink_tick / DEFAULT_CURSOR_PERIOD.max(1)).is_multiple_of(2)
    }

    /// Clamp the cursor into range (row within lines, col within the line).
    fn clamp(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        if self.row >= self.lines.len() {
            self.row = self.lines.len() - 1;
        }
        let line_len = self.lines[self.row].chars().count();
        if self.col > line_len {
            self.col = line_len;
        }
    }

    /// Apply a key event: `Char` types, `Backspace` deletes, `Enter` breaks the
    /// line, arrows / `Home` / `End` move. Other keys are ignored.
    pub fn handle_key(&mut self, key: KeyEvent) {
        self.clamp();
        match key.code {
            KeyCode::Char(ch) if key.modifiers.is_empty() => self.insert_char(ch),
            KeyCode::Backspace => self.backspace(),
            KeyCode::Enter => self.split_line(),
            KeyCode::Left => self.move_left(),
            KeyCode::Right => self.move_right(),
            KeyCode::Up => self.move_vertically(-1),
            KeyCode::Down => self.move_vertically(1),
            KeyCode::Home => self.col = 0,
            KeyCode::End => {
                self.col = self.lines.get(self.row).map(|l| l.chars().count()).unwrap_or(0);
            }
            _ => {}
        }
    }

    fn insert_char(&mut self, ch: char) {
        let line = &mut self.lines[self.row];
        let mut chars: Vec<char> = line.chars().collect();
        chars.insert(self.col, ch);
        self.col += 1;
        *line = chars.into_iter().collect();
    }

    fn backspace(&mut self) {
        if self.col > 0 {
            let line = &mut self.lines[self.row];
            let mut chars: Vec<char> = line.chars().collect();
            chars.remove(self.col - 1);
            self.col -= 1;
            *line = chars.into_iter().collect();
        } else if self.row > 0 {
            // Merge the current line into the previous one.
            let cur = self.lines.remove(self.row);
            self.row -= 1;
            self.col = self.lines[self.row].chars().count();
            self.lines[self.row].push_str(&cur);
        }
    }

    fn split_line(&mut self) {
        let cur = self.lines.remove(self.row);
        let chars: Vec<char> = cur.chars().collect();
        let before: String = chars[..self.col].iter().collect();
        let after: String = chars[self.col..].iter().collect();
        self.lines.insert(self.row, before);
        self.lines.insert(self.row + 1, after);
        self.row += 1;
        self.col = 0;
    }

    fn move_left(&mut self) {
        if self.col > 0 {
            self.col -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.col = self.lines[self.row].chars().count();
        }
    }

    fn move_right(&mut self) {
        let line_len = self.lines[self.row].chars().count();
        if self.col < line_len {
            self.col += 1;
        } else if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = 0;
        }
    }

    fn move_vertically(&mut self, delta: i32) {
        let new_row = self.row as i32 + delta;
        if new_row < 0 || new_row as usize >= self.lines.len() {
            return;
        }
        self.row = new_row as usize;
        self.col = self.col.min(self.lines[self.row].chars().count());
    }
}

impl StatefulWidget for TextArea {
    type State = TextAreaState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }
        state.clamp();

        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let text_style = sheet.compute_with(&NodeRef::new("Input"), None, &mut scratch).to_style();
        let cursor_style =
            sheet.compute_with(&NodeRef::new("Cursor"), None, &mut scratch).to_style();
        let cursor_on = state.cursor_visible();
        let cursor_glyph = self.shape.glyph();

        for (i, line) in state.lines.iter().enumerate() {
            let y = area.y + i as u16;
            if y >= area.bottom() {
                break;
            }
            // Line text.
            for (ci, ch) in line.chars().enumerate() {
                let px = area.x + ci as u16;
                if px >= area.right() {
                    break;
                }
                buf[(px, y)].set_char(ch).set_style(text_style);
            }
            // Caret on the cursor row at col.
            if i == state.row && cursor_on {
                let cx = area.x + state.col as u16;
                if cx < area.right() {
                    buf[(cx, y)].set_char(cursor_glyph).set_style(cursor_style);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    const W: u16 = 20;
    const H: u16 = 5;

    const fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn render(state: &mut TextAreaState, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        StatefulWidget::render(TextArea::new().theme(theme), Rect::new(0, 0, W, H), &mut buf, state);
        buf
    }

    #[test]
    fn typing_inserts_chars() {
        let mut s = TextAreaState::new();
        for c in "HI".chars() {
            s.handle_key(key(KeyCode::Char(c)));
        }
        assert_eq!(s.text(), "HI");
    }

    #[test]
    fn enter_creates_newline() {
        let mut s = TextAreaState::from_text("AB");
        s.col = 1; // between A and B
        s.handle_key(key(KeyCode::Enter));
        assert_eq!(s.lines, vec!["A".to_string(), "B".to_string()]);
        assert_eq!(s.row, 1);
        assert_eq!(s.col, 0);
    }

    #[test]
    fn backspace_removes_char() {
        let mut s = TextAreaState::from_text("AB");
        s.col = 2;
        s.handle_key(key(KeyCode::Backspace));
        assert_eq!(s.text(), "A");
    }

    #[test]
    fn backspace_at_line_start_merges() {
        let mut s = TextAreaState::from_text("AB\nCD");
        s.row = 1;
        s.col = 0;
        s.handle_key(key(KeyCode::Backspace));
        assert_eq!(s.text(), "ABCD");
        assert_eq!(s.row, 0);
        assert_eq!(s.col, 2);
    }

    #[test]
    fn arrows_move_cursor() {
        let mut s = TextAreaState::from_text("AB\nCD");
        s.handle_key(key(KeyCode::Right)); // 0:1
        assert_eq!((s.row, s.col), (0, 1));
        s.handle_key(key(KeyCode::Right)); // 0:2 (end of "AB")
        assert_eq!((s.row, s.col), (0, 2));
        s.handle_key(key(KeyCode::Right)); // wrap to 1:0
        assert_eq!((s.row, s.col), (1, 0));
        s.handle_key(key(KeyCode::Up)); // back to 0:0 (col clamped)
        assert_eq!((s.row, s.col), (0, 0));
    }

    #[test]
    fn home_end() {
        let mut s = TextAreaState::from_text("AB");
        s.col = 1;
        s.handle_key(key(KeyCode::Home));
        assert_eq!(s.col, 0);
        s.handle_key(key(KeyCode::End));
        assert_eq!(s.col, 2);
    }

    #[test]
    fn renders_text_and_cursor() {
        let mut s = TextAreaState::from_text("HI");
        s.col = 2; // cursor at end of "HI"
        let buf = render(&mut s, Theme::Cyberpunk);
        assert_eq!(buf[(0, 0)].symbol(), "H");
        assert_eq!(buf[(1, 0)].symbol(), "I");
        assert_eq!(buf[(2, 0)].symbol(), "█", "cursor at end of 'HI'");
    }

    #[test]
    fn cursor_uses_accent_color() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let mut s = TextAreaState::from_text("A");
        s.col = 1; // cursor after "A"
        let buf = render(&mut s, Theme::Cyberpunk);
        assert_eq!(buf[(1, 0)].fg, accent, "cursor is --accent");
    }

    #[test]
    fn renders_multiple_lines() {
        let mut s = TextAreaState::from_text("AB\nCD");
        s.blink_tick = DEFAULT_CURSOR_PERIOD; // cursor off — don't overwrite row 0
        let buf = render(&mut s, Theme::Cyberpunk);
        assert_eq!(buf[(0, 0)].symbol(), "A");
        assert_eq!(buf[(0, 1)].symbol(), "C");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let mut state = TextAreaState::new();
        StatefulWidget::render(TextArea::new(), Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }

    #[test]
    fn tick_advances_blink() {
        let mut s = TextAreaState::new();
        s.tick();
        assert_eq!(s.blink_tick, 1);
    }
}
