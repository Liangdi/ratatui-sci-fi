//! **Dropdown** — sci-fi collapsible single-choice control.
//!
//! A collapsed `▾ OPTION` line that expands into a vertical option list — the
//! single-choice counterpart of [`crate::RadioGroup`] for when the options
//! should stay hidden until invoked. It is the hardest of the form controls
//! because it has two visual states (collapsed / expanded).
//!
//! ## Overlay protocol — the app owns the area
//!
//! The library never touches the terminal, so the widget cannot position its
//! own floating list. Instead it always renders **only into the `area` it is
//! given**, and switches its layout on `state.expanded`:
//!
//! - **Collapsed** (1 row): `▾ options[selected]` — render into a 1-row area.
//! - **Expanded** (`n` rows): every option on its own row, the hovered one
//!   highlighted — render into an `n`-row-tall area.
//!
//! When expanding, the **app** allocates the taller `Rect` and clears it first,
//! exactly like the [`crate::AlertPopup`] overlay in `widget_gallery.rs`:
//!
//! ```rust,ignore
//! let area = if dropdown.expanded {
//!     Rect { height: options.len() as u16, ..collapsed_area }
//! } else {
//!     collapsed_area
//! };
//! f.render_widget(ratatui::widgets::Clear, area); // erase what was beneath
//! f.render_stateful_widget(dropdown, area, &mut state);
//! ```
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]: `options` are immutable configuration on the
//!   widget struct (convention #3); `selected`/`expanded`/`hover` are mutable
//!   state, driven by [`Dropdown::handle_key`] or set directly by the app.
//! - Key handling lives on the **widget** because it needs the option count.
//! - Styling reuses the [`crate::ScanList`] `List` / `List.selected` cascade
//!   nodes — collapsed row + hovered row take `List.selected` (accent on
//!   panel), the rest take `List`. All `var(--…)`-driven off the palette.
//! - Rendered cell-by-cell, left-aligned. All glyphs are width-1.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{Dropdown, DropdownState, Theme};
//! use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
//!
//! let dd = Dropdown::new(["ALPHA", "BETA", "GAMMA"]).theme(Theme::DeepSpace);
//! let mut state = DropdownState::new();
//! dd.handle_key(&mut state, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)); // expand
//! ```

use ratatui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Glyph drawn as the collapsed caret, for the [`DropdownShape::Chevron`]
/// default.
pub const CARET: char = '▾';
/// Glyph drawn beside the hovered option when expanded, for the
/// [`DropdownShape::Chevron`] default.
pub const MARK: char = '▶';

/// Visual form of a [`Dropdown`]'s caret (collapsed) and mark (expanded hover).
///
/// Selects the `(caret, mark)` glyph pair; colors stay on the CSS cascade
/// (reusing `List` / `List.selected`), untouched by this enum. The
/// [`DropdownShape::Chevron`] default renders the original `▾` / `▶` look.
///
/// Every glyph is Unicode width-1 (see convention #5 at the crate root).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DropdownShape {
    /// Caret `▾`, mark `▶`.
    #[default]
    Chevron,
    /// Caret `▼`, mark `►`.
    Arrow,
    /// Caret `·`, mark `◉`.
    Dot,
}

impl DropdownShape {
    /// The collapsed-state caret glyph.
    #[must_use]
    pub const fn caret(self) -> char {
        match self {
            Self::Chevron => CARET,
            Self::Arrow => '▼',
            Self::Dot => '·',
        }
    }

    /// The expanded-state mark glyph (beside the hovered option).
    #[must_use]
    pub const fn mark(self) -> char {
        match self {
            Self::Chevron => MARK,
            Self::Arrow => '►',
            Self::Dot => '◉',
        }
    }
}

/// A sci-fi dropdown.
///
/// Build with [`Dropdown::new`] (an iterator of option labels), then set the
/// theme with [`Dropdown::theme`]. Selection and expand state live in
/// [`DropdownState`].
#[derive(Debug, Clone)]
pub struct Dropdown {
    /// Option labels, top to bottom.
    pub options: Vec<String>,
    /// Caret/mark-glyph form. Defaults to [`DropdownShape::Chevron`].
    pub shape: DropdownShape,
    /// Theme whose [`Stylesheet`](ratatui_style::Stylesheet) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Dropdown {
    /// Create a dropdown from an iterator of option labels, default theme.
    pub fn new(options: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            options: options.into_iter().map(Into::into).collect(),
            shape: DropdownShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the caret/mark-glyph form (see [`DropdownShape`]).
    #[must_use]
    pub fn shape(mut self, shape: DropdownShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the dropdown.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Apply a key event to `state`:
    /// - `Enter` toggles expand/collapse; on collapse it commits the hovered
    ///   option as the new selection.
    /// - `Up`/`Down` move the hover (only while expanded; wrapping).
    /// - `Esc` collapses without committing.
    ///
    /// Other keys (and all keys while collapsed except `Enter`) are ignored.
    /// A no-op when there are no options.
    pub fn handle_key(&self, state: &mut DropdownState, key: KeyEvent) {
        let n = self.options.len();
        if n == 0 {
            return;
        }
        match key.code {
            KeyCode::Enter => {
                if state.expanded {
                    state.selected = state.hover.min(n - 1);
                    state.expanded = false;
                } else {
                    state.expanded = true;
                    state.hover = state.selected.min(n - 1);
                }
            }
            KeyCode::Esc => state.expanded = false,
            KeyCode::Up if state.expanded => state.hover = (state.hover + n - 1) % n,
            KeyCode::Down if state.expanded => state.hover = (state.hover + 1) % n,
            _ => {}
        }
    }
}

/// Mutable state for [`Dropdown`].
///
/// `selected` is the committed choice (shown when collapsed); `hover` is the
/// highlighted row while expanded; `expanded` is the open/closed flag. Both
/// indices are clamped into range on render.
#[derive(Debug, Default, Clone)]
pub struct DropdownState {
    /// Index of the committed selection (shown collapsed). Clamped on render.
    pub selected: usize,
    /// Whether the option list is expanded.
    pub expanded: bool,
    /// Index of the hovered option while expanded. Clamped on render.
    pub hover: usize,
}

impl DropdownState {
    /// Create a collapsed state with selection at index 0.
    pub fn new() -> Self {
        Self::default()
    }
}

impl StatefulWidget for Dropdown {
    type State = DropdownState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width == 0 || area.height == 0 || self.options.is_empty() {
            return;
        }

        let n = self.options.len();
        let selected = state.selected.min(n - 1);
        let hover = state.hover.min(n - 1);

        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let normal_style = sheet.compute_with(&NodeRef::new("List"), None, &mut scratch).to_style();
        let selected_style = sheet
            .compute_with(&NodeRef::new("List").classes(&["selected"]), None, &mut scratch)
            .to_style();

        if !state.expanded {
            // Collapsed: one row, caret + the committed option, highlighted.
            write_row(buf, area.x, area.y, area.right(), self.shape.caret(), &self.options[selected], selected_style);
            return;
        }

        // Expanded: one option per row; the hovered row is marked + highlighted.
        for (i, opt) in self.options.iter().enumerate() {
            let y = area.y + i as u16;
            if y >= area.bottom() {
                break;
            }
            let is_hover = i == hover;
            let prefix = if is_hover { self.shape.mark() } else { ' ' };
            let style = if is_hover { selected_style } else { normal_style };
            write_row(buf, area.x, y, area.right(), prefix, opt, style);
        }
    }
}

/// Write `prefix + ' ' + text` starting at `(x, y)`, padding the rest of the row
/// (up to `right`, exclusive) with spaces so the row's background is continuous.
/// Stops at `right` so a long option never overshoots the area.
fn write_row(buf: &mut Buffer, x: u16, y: u16, right: u16, prefix: char, text: &str, style: ratatui::style::Style) {
    let mut col = x;
    if col < right {
        buf[(col, y)].set_char(prefix).set_style(style);
        col += 1;
    }
    if col < right {
        buf[(col, y)].set_char(' ').set_style(style);
        col += 1;
    }
    for ch in text.chars() {
        if col >= right {
            break;
        }
        buf[(col, y)].set_char(ch).set_style(style);
        col += 1;
    }
    while col < right {
        buf[(col, y)].set_symbol(" ").set_style(style);
        col += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    const W: u16 = 16;
    const H: u16 = 6;

    const fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn render(options: &[&str], theme: Theme, state: &mut DropdownState) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        StatefulWidget::render(
            Dropdown::new(options.iter().copied()).theme(theme),
            Rect::new(0, 0, W, H),
            &mut buf,
            state,
        );
        buf
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..W).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()
    }

    #[test]
    fn collapsed_shows_caret_and_selected_option() {
        let mut s = DropdownState { selected: 1, ..DropdownState::default() };
        let buf = render(&["ALPHA", "BETA", "GAMMA"], Theme::Cyberpunk, &mut s);
        let text = row_text(&buf, 0);
        assert!(text.contains(CARET), "collapsed shows caret: {text:?}");
        assert!(text.contains("BETA"), "collapsed shows selected option: {text:?}");
    }

    #[test]
    fn handle_enter_expands_then_commits_and_collapses() {
        let dd = Dropdown::new(["A", "B", "C"]);
        let mut s = DropdownState::new();
        dd.handle_key(&mut s, key(KeyCode::Enter));
        assert!(s.expanded, "Enter expands");
        // move hover to index 2, then Enter commits.
        dd.handle_key(&mut s, key(KeyCode::Down));
        dd.handle_key(&mut s, key(KeyCode::Down));
        dd.handle_key(&mut s, key(KeyCode::Enter));
        assert!(!s.expanded, "Enter collapses");
        assert_eq!(s.selected, 2, "committed hover becomes selected");
    }

    #[test]
    fn handle_up_down_moves_hover_when_expanded() {
        let dd = Dropdown::new(["A", "B", "C"]);
        let mut s = DropdownState { expanded: true, hover: 0, ..DropdownState::default() };
        dd.handle_key(&mut s, key(KeyCode::Down));
        assert_eq!(s.hover, 1);
        dd.handle_key(&mut s, key(KeyCode::Down));
        dd.handle_key(&mut s, key(KeyCode::Down));
        assert_eq!(s.hover, 0, "Down wraps");
        dd.handle_key(&mut s, key(KeyCode::Up));
        assert_eq!(s.hover, 2, "Up wraps backwards");
    }

    #[test]
    fn handle_up_down_ignored_when_collapsed() {
        let dd = Dropdown::new(["A", "B", "C"]);
        let mut s = DropdownState::default(); // collapsed
        dd.handle_key(&mut s, key(KeyCode::Down));
        assert_eq!(s.hover, 0, "Down is a no-op while collapsed");
    }

    #[test]
    fn handle_esc_collapses() {
        let dd = Dropdown::new(["A", "B"]);
        let mut s = DropdownState { expanded: true, ..DropdownState::default() };
        dd.handle_key(&mut s, key(KeyCode::Esc));
        assert!(!s.expanded);
    }

    #[test]
    fn expanded_renders_all_options_with_hover_marked() {
        let mut s = DropdownState { expanded: true, hover: 1, ..DropdownState::default() };
        let buf = render(&["ALPHA", "BETA", "GAMMA"], Theme::Cyberpunk, &mut s);
        // All three options render on their own row.
        assert!(row_text(&buf, 0).contains("ALPHA"));
        assert!(row_text(&buf, 1).contains("BETA"));
        assert!(row_text(&buf, 2).contains("GAMMA"));
        // The hovered row (1) carries the mark glyph; the others don't.
        assert!(row_text(&buf, 1).contains(MARK), "hovered row is marked");
        assert!(!row_text(&buf, 0).contains(MARK), "non-hovered row is not marked");
    }

    #[test]
    fn hover_uses_accent_and_panel_bg() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let panel = Theme::Cyberpunk.palette().panel.color();
        let mut s = DropdownState { expanded: true, hover: 0, ..DropdownState::default() };
        let buf = render(&["A", "B"], Theme::Cyberpunk, &mut s);
        // Column 0 of the hovered row 0 is the mark → accent on panel.
        assert_eq!(buf[(0, 0)].fg, accent, "hover mark fg should be --accent");
        assert_eq!(buf[(0, 0)].bg, panel, "hover row bg should be --panel");
    }

    #[test]
    fn collapsed_uses_accent_and_panel_bg() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let mut s = DropdownState::default();
        let buf = render(&["A", "B"], Theme::Cyberpunk, &mut s);
        assert_eq!(buf[(0, 0)].fg, accent, "collapsed caret should be --accent");
    }

    #[test]
    fn hover_out_of_range_clamped() {
        let mut s = DropdownState { expanded: true, hover: 99, ..DropdownState::default() };
        let buf = render(&["A", "B"], Theme::Cyberpunk, &mut s);
        // hover 99 clamps to 1 → row 1 is marked, not a panic.
        assert!(row_text(&buf, 1).contains(MARK));
    }

    #[test]
    fn empty_options_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let mut state = DropdownState::default();
        StatefulWidget::render(
            Dropdown::new(std::iter::empty::<&str>()),
            Rect::new(0, 0, W, H),
            &mut buf,
            &mut state,
        );
        assert_eq!(buf[(0, 0)].symbol(), " ");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let mut state = DropdownState::default();
        StatefulWidget::render(Dropdown::new(["A"]), Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }

    #[test]
    fn non_default_shape_changes_caret() {
        let mut s = DropdownState::default();
        let buf = render_with_shape(&["A"], Theme::Cyberpunk, &mut s, DropdownShape::Arrow);
        assert!(row_text(&buf, 0).contains('▼'), "Arrow caret is ▼");
        assert!(!row_text(&buf, 0).contains(CARET), "must not use the Chevron caret");
    }

    fn render_with_shape(options: &[&str], theme: Theme, state: &mut DropdownState, shape: DropdownShape) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        StatefulWidget::render(
            Dropdown::new(options.iter().copied()).shape(shape).theme(theme),
            Rect::new(0, 0, W, H),
            &mut buf,
            state,
        );
        buf
    }
}
