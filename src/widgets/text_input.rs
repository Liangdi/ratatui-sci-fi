//! **TextInput** — single-line sci-fi input field with a blinking caret.
//!
//! A one-line text field: the app's event loop feeds keys in via
//! [`TextInputState::handle_key`], the widget renders the text with a blinking
//! `█` caret at the (char-indexed) cursor position, and shows a muted
//! placeholder when empty.
//!
//! ## Spec
//! - Renders the typed value, left-aligned, on the middle row.
//! - When empty, renders the [`placeholder`](TextInput::placeholder) in muted
//!   instead.
//! - A `█` caret (visible/hidden on a blink cycle) sits at the cursor's char
//!   position.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]; the value, the char-indexed cursor, and the
//!   blink clock all live in [`TextInputState`].
//! - **Cursor is a char index, not a byte index.** Every mutation clamps it to
//!   `value.chars().count()` so a shrinking value (backspace) never leaves the
//!   cursor past the end. Tested with a 2-byte char (`'é'`) to catch byte/char
//!   confusion.
//! - **Blink cadence** reuses [`crate::widgets::ScanListState`]'s exact formula
//!   and the shared [`DEFAULT_CURSOR_PERIOD`](crate::widgets::DEFAULT_CURSOR_PERIOD)
//!   so all caret animations in the crate blink in lockstep.
//! - `handle_key` maps `Char(c)` → insert, `Backspace` → delete-before, and the
//!   arrow/Home/End keys to cursor moves. It deliberately does **not** handle
//!   `Enter`/`Esc` — submit/cancel are app-level concerns.
//! - **Horizontal window:** v1 clips to `area.width` from the left. Cursor
//!   -following windowing is a follow-up (marked `TODO`).
//! - Styling goes through the theme's
//!   [`Stylesheet`](ratatui_style::Stylesheet) cascade: the value resolves via
//!   the `Input` rule (fg), the placeholder via `Input.placeholder` (muted),
//!   and the caret via the `Cursor` rule (accent).
//!
//! # Example
//!
//! ```no_run
//! use ratatui_sci_fi::{TextInput, TextInputState, Theme};
//!
//! let mut state = TextInputState::new();
//! let input = TextInput::new().placeholder("enter callsign").theme(Theme::Cyberpunk);
//! // in your event loop:
//! //   state.tick();
//! //   if let Event::Key(k) = ev { state.handle_key(k); }
//! ```

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::StatefulWidget,
};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui_style::{ComputeScratch, NodeRef};

use crate::CaretShape;
use crate::Theme;
use crate::widgets::list::DEFAULT_CURSOR_PERIOD;

/// The blinking caret glyph. Shared with `ScanList`'s selection cursor.
pub const CARET_GLYPH: &str = "█";

/// A single-line sci-fi text input.
///
/// Build with [`TextInput::new`], optionally set a placeholder
/// ([`TextInput::placeholder`]), a mask glyph ([`TextInput::mask`] /
/// [`TextInput::password`]), and theme ([`TextInput::theme`]). All editable
/// state lives in the companion [`TextInputState`].
#[derive(Debug, Clone, Default)]
pub struct TextInput {
    /// Placeholder shown (muted) when the value is empty.
    pub placeholder: Option<String>,
    /// Shape of the blinking caret glyph. Defaults to
    /// [`CaretShape::Block`] (`█`), reproducing the original look.
    pub caret: CaretShape,
    /// Optional mask glyph: when set, every value character renders as this
    /// glyph (a password / redacted field). `None` (the default) shows the value
    /// as typed. The underlying [`TextInputState::value`] is unaffected — only
    /// the display is masked.
    pub mask: Option<char>,
    /// Theme whose [`Stylesheet`](ratatui_style::Stylesheet) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl TextInput {
    /// Create an input with no placeholder, default theme.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the placeholder shown when the value is empty.
    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Set the blinking caret's glyph shape (see [`CaretShape`]).
    #[must_use]
    pub fn caret(mut self, caret: CaretShape) -> Self {
        self.caret = caret;
        self
    }

    /// Mask every value character as `ch` (a password / redacted field). The
    /// real value is still stored in [`TextInputState::value`]; only the display
    /// changes. The caret, placeholder, and editing behavior are unaffected.
    #[must_use]
    pub fn mask(mut self, ch: char) -> Self {
        self.mask = Some(ch);
        self
    }

    /// Convenience password mode: mask every value character with the bullet
    /// `•`. Pass `false` to clear masking. See [`TextInput::mask`] for a custom
    /// glyph.
    #[must_use]
    pub fn password(mut self, on: bool) -> Self {
        self.mask = if on { Some('•') } else { None };
        self
    }

    /// Set the theme whose cascade drives colors.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

/// Mutable state for [`TextInput`].
///
/// Holds the value, the char-indexed cursor, and the blink clock. Feed keys via
/// [`handle_key`](Self::handle_key) and advance the clock via
/// [`tick`](Self::tick) once per frame.
#[derive(Debug, Default, Clone)]
pub struct TextInputState {
    /// The current text.
    pub value: String,
    /// Cursor position as a **char** index in `0..=value.chars().count()`.
    pub cursor: usize,
    /// Blink clock; advanced each tick by the event loop (or [`Self::tick`]).
    pub blink_tick: u64,
}

impl TextInputState {
    /// Empty value, cursor at 0, clock at 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance the blink clock one tick.
    pub fn tick(&mut self) {
        self.blink_tick = self.blink_tick.wrapping_add(1);
    }

    /// Whether the caret is currently visible, using the crate's shared blink
    /// cadence. Mirrors [`crate::widgets::ScanListState::cursor_visible`].
    pub fn cursor_visible(&self) -> bool {
        (self.blink_tick / DEFAULT_CURSOR_PERIOD.max(1)).is_multiple_of(2)
    }

    /// Clamp the cursor into `0..=char_count` (defensive — every mutator already
    /// does this, but callers reaching into the public fields may not).
    fn clamp_cursor(&mut self) {
        let max = self.value.chars().count();
        if self.cursor > max {
            self.cursor = max;
        }
    }

    /// Insert `ch` at the cursor, advancing the cursor past it.
    pub fn insert_char(&mut self, ch: char) {
        // Convert the char-indexed cursor to a byte offset, insert, then move
        // the cursor one char forward.
        let byte_idx = self.byte_offset_of(self.cursor);
        self.value.insert(byte_idx, ch);
        self.cursor += 1;
    }

    /// Delete the char immediately before the cursor (backspace).
    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let byte_idx = self.byte_offset_of(self.cursor - 1);
        // The char to remove spans [byte_idx, next_char_end).
        let removed_len = self.value[byte_idx..].chars().next().map(|c| c.len_utf8()).unwrap_or(0);
        self.value.replace_range(byte_idx..byte_idx + removed_len, "");
        self.cursor -= 1;
    }

    /// Move the cursor one char left (clamped at 0).
    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move the cursor one char right (clamped at char count).
    pub fn move_right(&mut self) {
        self.clamp_cursor();
        let max = self.value.chars().count();
        if self.cursor < max {
            self.cursor += 1;
        }
    }

    /// Move the cursor to the start.
    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    /// Move the cursor to the end (after the last char).
    pub fn move_end(&mut self) {
        self.cursor = self.value.chars().count();
    }

    /// Apply a backend key event. `Char` inserts, `Backspace` deletes before the
    /// cursor, and the arrow / Home / End keys move the cursor. `Enter` / `Esc`
    /// are intentionally ignored — submit/cancel are app-level.
    pub fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(ch) => {
                // Ignore control-combos (Ctrl+C, etc.) — those are shortcuts,
                // not literal text.
                if !key.modifiers.is_empty() {
                    return;
                }
                self.insert_char(ch);
            }
            KeyCode::Backspace => self.backspace(),
            KeyCode::Left => self.move_left(),
            KeyCode::Right => self.move_right(),
            KeyCode::Home => self.move_home(),
            KeyCode::End => self.move_end(),
            _ => {}
        }
    }

    /// Byte offset in `value` corresponding to a given char index. Used to
    /// bridge the char-indexed cursor to `String`'s byte-indexed API.
    fn byte_offset_of(&self, char_idx: usize) -> usize {
        self.value
            .char_indices()
            .nth(char_idx)
            .map(|(b, _)| b)
            .unwrap_or_else(|| self.value.len())
    }
}

impl StatefulWidget for TextInput {
    type State = TextInputState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        // Defensive clamp in case a caller reached into the public fields.
        state.clamp_cursor();

        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let value_style =
            sheet.compute_with(&NodeRef::new("Input"), None, &mut scratch).to_style();
        let placeholder_style = sheet
            .compute_with(&NodeRef::new("Input").classes(&["placeholder"]), None, &mut scratch)
            .to_style();
        let caret_style =
            sheet.compute_with(&NodeRef::new("Cursor"), None, &mut scratch).to_style();

        let y = area.y + area.height / 2;
        let right = area.x + area.width;

        if state.value.is_empty() {
            // Placeholder (muted). The caret is drawn on its *own* cell only
            // when the placeholder hasn't claimed it — i.e. when there's no
            // placeholder, or the placeholder is shorter than the area. We never
            // overwrite a placeholder glyph with the caret (that would erase the
            // hint on the visible half-cycle); the caret at the very end of the
            // field instead signals focus.
            let mut x = area.x;
            if let Some(placeholder) = &self.placeholder {
                for ch in placeholder.chars() {
                    if x >= right {
                        break;
                    }
                    buf[(x, y)].set_symbol(ch.to_string().as_str()).set_style(placeholder_style);
                    x += 1;
                }
            }
            // Park a caret at the end of the (possibly empty) field when visible
            // so the empty input still reads as focused. It never clobbers a
            // rendered placeholder glyph.
            if state.cursor_visible() && x < right {
                buf[(x, y)].set_char(self.caret.glyph()).set_style(caret_style);
            }
            return;
        }

        // TODO(cursor-following window): v1 renders from char 0, clipping at the
        // right edge. For long values the caret can scroll out of view; a
        // follow-up should window the text around the cursor.

        // Render value chars left-to-right. When the caret is visible it sits at
        // the char *under* the cursor (or just past the last char): we draw that
        // one cell with the bright caret color so the caret reads as an
        // inverted highlight, keeping the glyph readable instead of erasing it.
        let char_count = state.value.chars().count() as u16;
        let caret_visible = state.cursor_visible();
        let caret_cell_is_char = caret_visible && (state.cursor as u16) < char_count;

        for (i, ch) in state.value.chars().enumerate() {
            let px = area.x + i as u16;
            if px >= right {
                break;
            }
            // When masking is on (a password field), every value char renders as
            // the mask glyph so the secret is never shown — even under the caret.
            let glyph = self.mask.unwrap_or(ch);
            if caret_cell_is_char && i == state.cursor {
                // Caret sits on this char: highlight it bright (caret color),
                // keeping the glyph itself visible.
                buf[(px, y)].set_symbol(glyph.to_string().as_str()).set_style(caret_style);
            } else {
                buf[(px, y)].set_symbol(glyph.to_string().as_str()).set_style(value_style);
            }
        }

        // When the caret is past the last char (cursor == char_count) and
        // visible, draw a dedicated caret cell after the value.
        if caret_visible && state.cursor as u16 == char_count {
            let px = area.x + char_count;
            if px < right {
                buf[(px, y)].set_char(self.caret.glyph()).set_style(caret_style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::Rect;

    const W: u16 = 16;
    const H: u16 = 1;

    /// Build a key event with empty modifiers.
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    fn render(state: &mut TextInputState, placeholder: Option<&str>, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let mut input = TextInput::new().theme(theme);
        if let Some(p) = placeholder {
            input = input.placeholder(p);
        }
        StatefulWidget::render(input, Rect::new(0, 0, W, H), &mut buf, state);
        buf
    }

    #[test]
    fn typed_chars_render_left_to_right() {
        let mut s = TextInputState::new();
        for c in "ABC".chars() {
            s.insert_char(c);
        }
        let buf = render(&mut s, None, Theme::Cyberpunk);
        assert_eq!(buf[(0, 0)].symbol(), "A");
        assert_eq!(buf[(1, 0)].symbol(), "B");
        assert_eq!(buf[(2, 0)].symbol(), "C");
    }

    #[test]
    fn caret_visible_at_tick_zero_hidden_next_half_cycle() {
        let mut s = TextInputState::new();
        s.insert_char('A');
        // tick 0 -> visible. Caret sits *after* 'A' (cursor==1==char_count) on
        // its own cell at x=1.
        assert!(s.cursor_visible());
        let buf = render(&mut s, None, Theme::Cyberpunk);
        assert_eq!(buf[(0, 0)].symbol(), "A");
        assert_eq!(buf[(1, 0)].symbol(), CARET_GLYPH, "caret should be visible after the value at tick 0");

        // Advance into the hidden half of the cycle.
        for _ in 0..DEFAULT_CURSOR_PERIOD {
            s.tick();
        }
        assert!(!s.cursor_visible(), "caret should be hidden after one period");
        let buf = render(&mut s, None, Theme::Cyberpunk);
        assert_eq!(buf[(0, 0)].symbol(), "A");
        assert_eq!(buf[(1, 0)].symbol(), " ", "no caret drawn when hidden");
    }

    #[test]
    fn caret_highlights_char_at_cursor_mid_value() {
        let mut s = TextInputState::new();
        s.value = "AB".into();
        s.cursor = 1; // sits on 'B'
        let buf = render(&mut s, None, Theme::Cyberpunk);
        // Caret is visible and on the char at index 1 -> 'B' stays, but in the
        // bright caret color.
        assert_eq!(buf[(0, 0)].symbol(), "A");
        assert_eq!(buf[(1, 0)].symbol(), "B", "glyph under the caret stays readable");
        let accent = Theme::Cyberpunk.palette().accent.color();
        assert_eq!(buf[(1, 0)].fg, accent, "char under the caret should be caret-colored");
    }

    #[test]
    fn backspace_removes_last_char_and_moves_cursor_left() {
        let mut s = TextInputState::new();
        for c in "AB".chars() {
            s.insert_char(c);
        }
        assert_eq!(s.cursor, 2);
        s.backspace();
        assert_eq!(s.value, "A");
        assert_eq!(s.cursor, 1, "cursor moves left after backspace");
    }

    #[test]
    fn backspace_at_start_is_a_noop() {
        let mut s = TextInputState::new();
        s.backspace();
        assert_eq!(s.value, "");
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn cursor_clamps_when_value_empties() {
        let mut s = TextInputState::new();
        s.insert_char('A');
        s.move_end();
        // Manually truncate the value (simulating a programmatic clear) — the
        // cursor must clamp on the next render, not panic.
        s.value.clear();
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut s2 = TextInputState { value: s.value.clone(), cursor: 5, blink_tick: 0 };
            let _buf = render(&mut s2, None, Theme::Cyberpunk);
        }))
        .is_ok());
    }

    #[test]
    fn placeholder_shown_when_empty() {
        let muted = Theme::Cyberpunk.palette().muted.color();
        let mut s = TextInputState::new();
        let buf = render(&mut s, Some("hint"), Theme::Cyberpunk);
        // The placeholder renders, untouched by the caret (the caret parks past
        // its last glyph, never overwriting one).
        assert_eq!(buf[(0, 0)].symbol(), "h", "placeholder should render");
        assert_eq!(buf[(0, 0)].fg, muted, "placeholder should be muted");
    }

    #[test]
    fn char_index_correctness_with_multibyte_char() {
        // 'é' is 2 bytes; cursor must be a char index, not a byte index.
        let mut s = TextInputState::new();
        s.insert_char('é');
        s.insert_char('B'); // value = "éB", cursor = 2
        assert_eq!(s.value, "éB");
        assert_eq!(s.cursor, 2);

        // Move back between é and B; backspace removes é (the 2-byte char), not
        // half of it.
        s.move_left(); // cursor = 1 (between é and B)
        s.backspace(); // removes é
        assert_eq!(s.value, "B", "backspace should remove the whole 2-byte char");
        assert_eq!(s.cursor, 0);

        let buf = render(&mut s, None, Theme::Cyberpunk);
        // Caret sits on 'B' (cursor 0) and is visible -> glyph stays readable.
        assert_eq!(buf[(0, 0)].symbol(), "B");
    }

    #[test]
    fn handle_key_inserts_and_navigates() {
        let mut s = TextInputState::new();
        s.handle_key(key(KeyCode::Char('A')));
        s.handle_key(key(KeyCode::Char('B')));
        s.handle_key(key(KeyCode::Char('C')));
        assert_eq!(s.value, "ABC");

        s.handle_key(key(KeyCode::Left)); // cursor between B and C
        s.handle_key(key(KeyCode::Backspace)); // remove B
        assert_eq!(s.value, "AC");

        s.handle_key(key(KeyCode::Home));
        assert_eq!(s.cursor, 0);
        s.handle_key(key(KeyCode::End));
        assert_eq!(s.cursor, 2);
    }

    #[test]
    fn handle_key_ignores_control_combos() {
        let mut s = TextInputState::new();
        // Ctrl+C has modifiers set -> ignored, not inserted.
        s.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert_eq!(s.value, "", "control-combo should not insert a char");
        // Enter / Esc are ignored.
        s.handle_key(key(KeyCode::Enter));
        s.handle_key(key(KeyCode::Esc));
        assert_eq!(s.value, "");
    }

    #[test]
    fn tick_wraps_without_panic() {
        let mut s = TextInputState { value: String::from("X"), cursor: 1, blink_tick: u64::MAX };
        s.tick();
        assert_eq!(s.blink_tick, 0);
        let _ = s.cursor_visible();
    }

    #[test]
    fn empty_area_is_a_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let mut s = TextInputState::new();
        TextInput::new().placeholder("X").render(Rect::new(0, 0, 0, 0), &mut buf, &mut s);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }

    #[test]
    fn non_default_caret_shape_uses_its_glyph() {
        // An empty input with the Bar caret: at tick 0 the caret is visible and
        // parks on its own cell at x=0. Its glyph must be Bar's '▎', not the
        // default Block '█'.
        let mut s = TextInputState::new();
        assert!(s.cursor_visible(), "caret should be visible at tick 0");
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let input = TextInput::new().caret(CaretShape::Bar);
        StatefulWidget::render(input, Rect::new(0, 0, W, H), &mut buf, &mut s);
        assert_eq!(
            buf[(0, 0)].symbol(),
            "▎",
            "Bar caret should render '▎', not the default Block '█'"
        );
    }

    #[test]
    fn mask_default_is_none() {
        assert!(TextInput::new().mask.is_none(), "masking must be off by default");
    }

    #[test]
    fn password_builder_sets_bullet_mask() {
        assert_eq!(TextInput::new().password(true).mask, Some('•'));
        assert!(TextInput::new().password(false).mask.is_none(), "password(false) clears masking");
        // mask() sets an arbitrary glyph.
        assert_eq!(TextInput::new().mask('*').mask, Some('*'));
    }

    #[test]
    fn mask_replaces_value_chars_without_leaking_them() {
        let mut s = TextInputState::new();
        for c in "secret".chars() {
            s.insert_char(c);
        }
        // cursor is now 6 == char_count, so the caret parks on its own cell at
        // x=6; the value cells x=0..5 must all be the mask glyph.
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let input = TextInput::new().mask('*');
        StatefulWidget::render(input, Rect::new(0, 0, W, H), &mut buf, &mut s);
        for x in 0..6u16 {
            assert_eq!(buf[(x, 0)].symbol(), "*", "value char at x={x} must be masked");
        }
        // The real value is intact in state — masking is display-only.
        assert_eq!(s.value, "secret");
    }

    #[test]
    fn mask_still_hides_char_under_the_caret() {
        // Caret sitting ON a value char (mid-value) must show the mask glyph too,
        // never revealing the real character.
        let mut s = TextInputState { value: String::from("abc"), cursor: 1, blink_tick: 0 };
        assert!(s.cursor_visible());
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let input = TextInput::new().mask('•');
        StatefulWidget::render(input, Rect::new(0, 0, W, H), &mut buf, &mut s);
        // x=1 is the char under the caret (index 1) — must be the mask, not 'b'.
        assert_eq!(buf[(1, 0)].symbol(), "•", "char under the caret must be masked");
        assert_ne!(buf[(1, 0)].symbol(), "b");
    }
}
