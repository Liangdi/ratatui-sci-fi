//! **RadioGroup** — sci-fi mutually-exclusive choice list.
//!
//! A vertical list of options where exactly one is selected — the radio-button
//! counterpart to [`crate::ScanList`]. Where [`crate::ScanList`] highlights a
//! row with a blinking `█` cursor (all rows share one glyph), [`RadioGroup`]
//! marks the active option with a filled glyph (`◉`) and the rest with the
//! hollow form (`○`) — so selection is legible without any animation, and the
//! state needs no blink clock.
//!
//! ## Spec
//! - **Selected**: `◉ OPTION` — the filled mark, ok color, bold.
//! - **Unselected**: `○ OPTION` — the hollow mark, muted.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]: `options` are immutable configuration
//!   (convention #3, on the widget struct); only `selected` is mutable state,
//!   advanced by [`RadioGroup::handle_key`] or set directly by the app.
//! - Key handling lives on the **widget** (not the state) because it needs the
//!   option count: call `group.handle_key(&mut state, key)`.
//! - Styling reuses the [`crate::Toggle`] cascade node — selected → `Toggle.on`
//!   (ok + bold), unselected → `Toggle.off` (muted) — the same boolean
//!   vocabulary, `var(--…)`-driven off the active palette.
//! - Rendered cell-by-cell from `area.y` downward, one row per option. All
//!   glyphs are width-1.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{RadioGroup, RadioGroupState, Theme};
//! use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
//!
//! let group = RadioGroup::new(["ENGAGE", "STANDBY", "SAFE"]).theme(Theme::DeepSpace);
//! let mut state = RadioGroupState::new();
//! group.handle_key(&mut state, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
//! ```

use ratatui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Glyph drawn for the selected option, for the [`RadioGroupShape::Orb`]
/// default.
pub const MARK_ON: char = '◉';
/// Glyph drawn for an unselected option, for the [`RadioGroupShape::Orb`] /
/// [`RadioGroupShape::Bullet`] default hollow form.
pub const MARK_OFF: char = '○';

/// Visual form of a [`RadioGroup`]'s option mark.
///
/// Selects the selected/unselected glyph pair; colors stay on the CSS cascade
/// (reusing `Toggle.on` / `Toggle.off`), untouched by this enum. The
/// [`RadioGroupShape::Orb`] default renders the original `◉` / `○` look.
///
/// Every mark glyph is Unicode width-1 (see convention #5 at the crate root).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RadioGroupShape {
    /// `◉` selected, `○` unselected — the original look.
    #[default]
    Orb,
    /// `●` selected, `○` unselected.
    Bullet,
    /// `◆` selected, `◇` unselected.
    Diamond,
}

impl RadioGroupShape {
    /// The mark glyph for the given selected/unselected state.
    #[must_use]
    pub const fn mark(self, selected: bool) -> char {
        match (self, selected) {
            (Self::Orb, true) => MARK_ON,
            (Self::Orb, false) => MARK_OFF,
            (Self::Bullet, true) => '●',
            (Self::Bullet, false) => MARK_OFF,
            (Self::Diamond, true) => '◆',
            (Self::Diamond, false) => '◇',
        }
    }
}

/// A sci-fi radio group.
///
/// Build with [`RadioGroup::new`] (an iterator of option labels), then set the
/// theme with [`RadioGroup::theme`]. Selection lives in [`RadioGroupState`].
#[derive(Debug, Clone)]
pub struct RadioGroup {
    /// Option labels, top to bottom.
    pub options: Vec<String>,
    /// Mark-glyph form (selected/unselected indicator). Defaults to
    /// [`RadioGroupShape::Orb`].
    pub shape: RadioGroupShape,
    /// Theme whose [`Stylesheet`](ratatui_style::Stylesheet) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl RadioGroup {
    /// Create a radio group from an iterator of option labels, default theme.
    pub fn new(options: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            options: options.into_iter().map(Into::into).collect(),
            shape: RadioGroupShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the mark-glyph form (see [`RadioGroupShape`]).
    #[must_use]
    pub fn shape(mut self, shape: RadioGroupShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the group.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Apply a key event to `state`: `Up`/`Left` and `Down`/`Right` cycle the
    /// selection (wrapping), `Home`/`End` jump to the first/last option. Other
    /// keys are ignored. A no-op when there are no options.
    pub fn handle_key(&self, state: &mut RadioGroupState, key: KeyEvent) {
        let n = self.options.len();
        if n == 0 {
            return;
        }
        match key.code {
            KeyCode::Up | KeyCode::Left => state.selected = (state.selected + n - 1) % n,
            KeyCode::Down | KeyCode::Right => state.selected = (state.selected + 1) % n,
            KeyCode::Home => state.selected = 0,
            KeyCode::End => state.selected = n - 1,
            _ => {}
        }
    }
}

/// Mutable state for [`RadioGroup`].
///
/// `selected` is the index of the active option. It is clamped into range at
/// render time (so an out-of-range value or a shrinking option list never
/// panics), and advanced by [`RadioGroup::handle_key`] or set directly by the
/// app (like [`crate::ScanListState::selected`]).
#[derive(Debug, Default, Clone)]
pub struct RadioGroupState {
    /// Index of the selected option. Clamped to `options.len()-1` at render.
    pub selected: usize,
}

impl RadioGroupState {
    /// Create a state with selection at index 0.
    pub fn new() -> Self {
        Self::default()
    }
}

impl StatefulWidget for RadioGroup {
    type State = RadioGroupState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Nothing to draw if there's no space or no options.
        if area.width == 0 || area.height == 0 || self.options.is_empty() {
            return;
        }

        // Selection clamps into range so a stale index never panics.
        let selected = state.selected.min(self.options.len() - 1);

        // Row styles reuse the `Toggle.on` (ok + bold) / `Toggle.off` (muted)
        // cascade rules — the same boolean vocabulary, one ComputeScratch reused.
        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let on_style = sheet
            .compute_with(&NodeRef::new("Toggle").classes(&["on"]), None, &mut scratch)
            .to_style();
        let off_style = sheet
            .compute_with(&NodeRef::new("Toggle").classes(&["off"]), None, &mut scratch)
            .to_style();

        // One option per row, top to bottom, stopping at the area bottom.
        for (i, opt) in self.options.iter().enumerate() {
            let y = area.y + i as u16;
            if y >= area.bottom() {
                break;
            }

            let is_selected = i == selected;
            let row_style = if is_selected { on_style } else { off_style };
            let mark = self.shape.mark(is_selected);

            // Write cell-by-cell so styling is consistent and we never overshoot
            // the area: `[mark] option`, padded to the row end for a continuous
            // background on the selected row.
            let mut col = area.x;
            if col < area.right() {
                buf[(col, y)].set_char(mark).set_style(row_style);
                col += 1;
            }
            if col < area.right() {
                buf[(col, y)].set_char(' ').set_style(row_style);
                col += 1;
            }
            for ch in opt.chars() {
                if col >= area.right() {
                    break;
                }
                buf[(col, y)].set_char(ch).set_style(row_style);
                col += 1;
            }
            while col < area.right() {
                buf[(col, y)].set_symbol(" ").set_style(row_style);
                col += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect, style::Modifier};
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    const W: u16 = 16;
    const H: u16 = 6;

    /// Zero-modifier `KeyEvent` helper for tests.
    const fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn render(options: &[&str], theme: Theme, selected: usize) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let mut state = RadioGroupState { selected };
        StatefulWidget::render(
            RadioGroup::new(options.iter().copied()).theme(theme),
            Rect::new(0, 0, W, H),
            &mut buf,
            &mut state,
        );
        buf
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..W).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()
    }

    #[test]
    fn selected_row_filled_others_hollow() {
        let buf = render(&["ENGAGE", "STANDBY", "SAFE"], Theme::Cyberpunk, 1);
        assert!(row_text(&buf, 0).contains(MARK_OFF), "row 0 unselected → hollow");
        assert!(row_text(&buf, 1).contains(MARK_ON), "row 1 selected → filled");
        assert!(row_text(&buf, 2).contains(MARK_OFF), "row 2 unselected → hollow");
    }

    #[test]
    fn selected_mark_uses_ok_color_and_bold() {
        let ok = Theme::Cyberpunk.palette().ok.color();
        let buf = render(&["A", "B"], Theme::Cyberpunk, 0);
        // Column 0 of the selected row 0 carries the filled mark, --ok, bold.
        assert_eq!(buf[(0, 0)].fg, ok, "selected mark should be --ok");
        assert!(
            buf[(0, 0)].modifier.contains(Modifier::BOLD),
            "selected content should be bold via the cascade"
        );
    }

    #[test]
    fn unselected_row_uses_muted_color() {
        let muted = Theme::Cyberpunk.palette().muted.color();
        let buf = render(&["A", "B"], Theme::Cyberpunk, 0);
        assert_eq!(buf[(0, 1)].fg, muted, "unselected mark should be --muted");
    }

    #[test]
    fn handle_key_down_advances_then_wraps() {
        let group = RadioGroup::new(["A", "B", "C"]);
        let mut s = RadioGroupState::new();
        group.handle_key(&mut s, key(KeyCode::Down));
        assert_eq!(s.selected, 1);
        group.handle_key(&mut s, key(KeyCode::Down));
        assert_eq!(s.selected, 2);
        group.handle_key(&mut s, key(KeyCode::Down));
        assert_eq!(s.selected, 0, "Down wraps to 0 past the end");
    }

    #[test]
    fn handle_key_up_wraps_backwards() {
        let group = RadioGroup::new(["A", "B", "C"]);
        let mut s = RadioGroupState::new();
        group.handle_key(&mut s, key(KeyCode::Up));
        assert_eq!(s.selected, 2, "Up from 0 wraps to last");
    }

    #[test]
    fn handle_key_home_end() {
        let group = RadioGroup::new(["A", "B", "C"]);
        let mut s = RadioGroupState { selected: 1 };
        group.handle_key(&mut s, key(KeyCode::Home));
        assert_eq!(s.selected, 0);
        group.handle_key(&mut s, key(KeyCode::End));
        assert_eq!(s.selected, 2);
    }

    #[test]
    fn handle_key_noop_on_empty() {
        let group = RadioGroup::new(std::iter::empty::<&str>());
        let mut s = RadioGroupState { selected: 0 };
        group.handle_key(&mut s, key(KeyCode::Down));
        assert_eq!(s.selected, 0, "no options → no movement");
    }

    #[test]
    fn selected_out_of_range_clamped() {
        // selected=99 with 2 options must clamp to index 1, not panic.
        let buf = render(&["A", "B"], Theme::Cyberpunk, 99);
        assert!(row_text(&buf, 1).contains(MARK_ON), "out-of-range clamps to last");
        assert!(row_text(&buf, 0).contains(MARK_OFF));
    }

    #[test]
    fn empty_options_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let mut state = RadioGroupState::default();
        StatefulWidget::render(
            RadioGroup::new(std::iter::empty::<&str>()),
            Rect::new(0, 0, W, H),
            &mut buf,
            &mut state,
        );
        // Nothing written: the buffer's default cells stay as ' '.
        assert_eq!(buf[(0, 0)].symbol(), " ");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let mut state = RadioGroupState::default();
        StatefulWidget::render(RadioGroup::new(["A"]), Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }

    #[test]
    fn non_default_shape_uses_its_glyph() {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let mut state = RadioGroupState { selected: 0 };
        StatefulWidget::render(
            RadioGroup::new(["A"]).shape(RadioGroupShape::Diamond).theme(Theme::Cyberpunk),
            Rect::new(0, 0, W, H),
            &mut buf,
            &mut state,
        );
        assert!(row_text(&buf, 0).contains('◆'), "Diamond selected → ◆");
        assert!(!row_text(&buf, 0).contains(MARK_ON), "must not use the Orb glyph");
    }
}
