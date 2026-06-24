//! **ComboBox** — an input field with a dropdown.
//!
//! A free-text input that can also pop a list of options to pick from — the
//! [`crate::TextInput`] + [`crate::Dropdown`] hybrid. Type to edit `value`;
//! `Enter` opens the list (or commits the hovered option), `Up`/`Down` move
//! the hover, `Esc` closes.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]: `options` are configuration; the text + open
//!   flag + hover live in [`ComboBoxState`].
//! - The widget renders into the given `area` — collapsed it uses one row
//!   (the value + `▾`); expanded it lists the options beneath. The app enlarges
//!   the area (and `Clear`s beneath) when open, as with [`crate::Dropdown`].
//! - Styling reuses the `Input` (value text) and `List` / `List.selected`
//!   (options) cascade nodes.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{ComboBox, ComboBoxState, Theme};
//!
//! let cb = ComboBox::new(["ALPHA", "BETA", "GAMMA"]).theme(Theme::DeepSpace);
//! let mut state = ComboBoxState::new();
//! ```

use ratatui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Caret shown when the dropdown is closed.
pub const CARET: char = '▾';

/// A sci-fi combo box.
///
/// Build with [`ComboBox::new`] (an iterator of option labels).
#[derive(Debug, Clone)]
pub struct ComboBox {
    /// The selectable options.
    pub options: Vec<String>,
    /// Theme.
    pub theme: Theme,
}

impl ComboBox {
    /// Create a combo box from an iterator of options.
    pub fn new(options: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            options: options.into_iter().map(Into::into).collect(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the theme.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Apply a key event.
    pub fn handle_key(&self, state: &mut ComboBoxState, key: KeyEvent) {
        let n = self.options.len();
        match key.code {
            KeyCode::Char(c) if key.modifiers.is_empty() => state.value.push(c),
            KeyCode::Backspace => {
                state.value.pop();
            }
            KeyCode::Enter => {
                if state.open && n > 0 {
                    state.value = self.options[state.hover.min(n - 1)].clone();
                    state.open = false;
                } else {
                    state.open = true;
                    state.hover = 0;
                }
            }
            KeyCode::Esc => state.open = false,
            KeyCode::Up if state.open && n > 0 => state.hover = (state.hover + n - 1) % n,
            KeyCode::Down if state.open && n > 0 => state.hover = (state.hover + 1) % n,
            _ => {}
        }
    }
}

/// Mutable state for [`ComboBox`].
#[derive(Debug, Clone)]
pub struct ComboBoxState {
    /// The current text (typed or committed from a selection).
    pub value: String,
    /// Whether the option list is open.
    pub open: bool,
    /// Hovered option index while open.
    pub hover: usize,
}

impl Default for ComboBoxState {
    fn default() -> Self {
        Self::new()
    }
}

impl ComboBoxState {
    /// Create a closed, empty state.
    pub fn new() -> Self {
        Self {
            value: String::new(),
            open: false,
            hover: 0,
        }
    }
}

impl StatefulWidget for ComboBox {
    type State = ComboBoxState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }
        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let input_style =
            sheet.compute_with(&NodeRef::new("Input"), None, &mut scratch).to_style();
        let norm_style = sheet.compute_with(&NodeRef::new("List"), None, &mut scratch).to_style();
        let sel_style = sheet
            .compute_with(&NodeRef::new("List").classes(&["selected"]), None, &mut scratch)
            .to_style();
        let accent = self.theme.palette().accent.color();

        // Row 0: the value text + ▾.
        let row0 = area.y;
        let mut col = area.x;
        for ch in state.value.chars() {
            if col >= area.right() {
                break;
            }
            buf[(col, row0)].set_char(ch).set_style(input_style);
            col += 1;
        }
        if col < area.right() {
            buf[(col, row0)].set_char(CARET).set_fg(accent);
        }

        // Options beneath, when open.
        if state.open {
            let hover = if self.options.is_empty() { 0 } else { state.hover.min(self.options.len() - 1) };
            for (i, opt) in self.options.iter().enumerate() {
                let y = area.y + 1 + i as u16;
                if y >= area.bottom() {
                    break;
                }
                let is_hover = i == hover;
                let marker = if is_hover { '▸' } else { ' ' };
                let style = if is_hover { sel_style } else { norm_style };
                let mut c = area.x;
                if c < area.right() {
                    buf[(c, y)].set_char(marker).set_fg(accent);
                    c += 1;
                }
                for ch in opt.chars() {
                    if c >= area.right() {
                        break;
                    }
                    buf[(c, y)].set_char(ch).set_style(style);
                    c += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 16;
    const H: u16 = 6;

    fn render(options: &[&str], state: &mut ComboBoxState, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        StatefulWidget::render(
            ComboBox::new(options.iter().copied()).theme(theme),
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
    fn collapsed_shows_value_and_caret() {
        let mut s = ComboBoxState::new();
        s.value = "BETA".into();
        let buf = render(&["A", "B"], &mut s, Theme::Cyberpunk);
        let text = row_text(&buf, 0);
        assert!(text.contains("BETA"), "value shown: {text:?}");
        assert!(text.contains(CARET), "caret shown");
    }

    #[test]
    fn open_lists_options() {
        let mut s = ComboBoxState::new();
        s.open = true;
        let buf = render(&["ALPHA", "BETA"], &mut s, Theme::Cyberpunk);
        assert!(row_text(&buf, 1).contains("ALPHA"));
        assert!(row_text(&buf, 2).contains("BETA"));
    }

    #[test]
    fn handle_char_types_into_value() {
        let cb = ComboBox::new(["A"]);
        let mut s = ComboBoxState::new();
        cb.handle_key(&mut s, key(KeyCode::Char('x')));
        assert_eq!(s.value, "x");
    }

    #[test]
    fn handle_enter_opens_then_commits() {
        let cb = ComboBox::new(["A", "B"]);
        let mut s = ComboBoxState::new();
        cb.handle_key(&mut s, key(KeyCode::Enter));
        assert!(s.open);
        cb.handle_key(&mut s, key(KeyCode::Down));
        cb.handle_key(&mut s, key(KeyCode::Enter));
        assert!(!s.open);
        assert_eq!(s.value, "B", "committed hover becomes value");
    }

    #[test]
    fn handle_up_down_move_hover_when_open() {
        let cb = ComboBox::new(["A", "B", "C"]);
        let mut s = ComboBoxState { open: true, ..ComboBoxState::new() };
        cb.handle_key(&mut s, key(KeyCode::Down));
        assert_eq!(s.hover, 1);
        cb.handle_key(&mut s, key(KeyCode::Up));
        assert_eq!(s.hover, 0);
    }

    #[test]
    fn esc_closes() {
        let cb = ComboBox::new(["A"]);
        let mut s = ComboBoxState { open: true, ..ComboBoxState::new() };
        cb.handle_key(&mut s, key(KeyCode::Esc));
        assert!(!s.open);
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let mut state = ComboBoxState::new();
        StatefulWidget::render(ComboBox::new(["A"]), Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
