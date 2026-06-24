//! **Tabs** — a sci-fi tab bar.
//!
//! A horizontal row of tabs with one selected — the navigation header. The
//! selected tab is `accent` (bold); its form depends on [`TabsShape`]:
//! [`Underline`](TabsShape::Underline) draws a rule beneath it, [`Bracket`]
//! (TabsShape::Bracket) wraps it in `[ … ]`, [`Arrow`](TabsShape::Arrow) in
//! `▶ … ◀`. `Left`/`Right` move the selection.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]: `labels` are configuration; only `selected`
//!   is state, advanced by [`Tabs::handle_key`].
//! - Styling reuses the `Value` (accent) and `Label` (muted) cascade nodes.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{Tabs as SfTabs, TabsShape, TabsState, Theme};
//!
//! let tabs = SfTabs::new(["STATUS", "SENSORS", "NAV"]).shape(TabsShape::Underline).theme(Theme::DeepSpace);
//! let mut state = TabsState::new();
//! ```

use ratatui::{buffer::Buffer, layout::Rect, style::Modifier, widgets::StatefulWidget};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Selected-tab left marker, for [`TabsShape::Arrow`].
pub const ARROW_L: char = '▶';
/// Selected-tab right marker, for [`TabsShape::Arrow`].
pub const ARROW_R: char = '◀';

/// Visual form of a selected [`Tabs`] entry.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TabsShape {
    /// A `─` rule beneath the selected tab — the default. Needs ≥2 rows.
    #[default]
    Underline,
    /// `[ … ]` around the selected tab.
    Bracket,
    /// `▶ … ◀` around the selected tab.
    Arrow,
}

impl TabsShape {
    /// The `(left, right)` marker pair around a selected tab's label.
    fn markers(self, selected: bool) -> (char, char) {
        if !selected {
            return (' ', ' ');
        }
        match self {
            Self::Underline => (' ', ' '),
            Self::Bracket => ('[', ']'),
            Self::Arrow => (ARROW_L, ARROW_R),
        }
    }
}

/// A sci-fi tab bar.
///
/// Build with [`Tabs::new`] (an iterator of tab labels).
#[derive(Debug, Clone)]
pub struct Tabs {
    /// Tab labels, left → right.
    pub labels: Vec<String>,
    /// Selected-tab form. Defaults to [`TabsShape::Underline`].
    pub shape: TabsShape,
    /// Theme whose [`Stylesheet`](ratatui_style::Stylesheet) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Tabs {
    /// Create a tab bar from an iterator of labels.
    pub fn new(labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            labels: labels.into_iter().map(Into::into).collect(),
            shape: TabsShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the selected-tab form (see [`TabsShape`]).
    #[must_use]
    pub fn shape(mut self, shape: TabsShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the tabs.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Apply a key event: `Left`/`Right` cycle the selection (wrapping). A
    /// no-op when there are no tabs.
    pub fn handle_key(&self, state: &mut TabsState, key: KeyEvent) {
        let n = self.labels.len();
        if n == 0 {
            return;
        }
        match key.code {
            KeyCode::Left => state.selected = (state.selected + n - 1) % n,
            KeyCode::Right => state.selected = (state.selected + 1) % n,
            KeyCode::Home => state.selected = 0,
            KeyCode::End => state.selected = n - 1,
            _ => {}
        }
    }
}

/// Mutable state for [`Tabs`].
///
/// `selected` is the active tab index, clamped on render.
#[derive(Debug, Default, Clone)]
pub struct TabsState {
    /// Index of the selected tab.
    pub selected: usize,
}

impl TabsState {
    /// Create a state with the first tab selected.
    pub fn new() -> Self {
        Self::default()
    }
}

impl StatefulWidget for Tabs {
    type State = TabsState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() || self.labels.is_empty() {
            return;
        }
        let n = self.labels.len();
        let selected = state.selected.min(n - 1);

        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let sel_style = sheet
            .compute_with(&NodeRef::new("Value"), None, &mut scratch)
            .to_style()
            .add_modifier(Modifier::BOLD);
        let dim_style = sheet.compute_with(&NodeRef::new("Label"), None, &mut scratch).to_style();
        let accent = self.theme.palette().accent.color();

        let tab_w = (area.width / n as u16).max(1);
        let label_row = area.y;
        let underline_row = area.y + area.height / 2 + 1;

        for (i, label) in self.labels.iter().enumerate() {
            let is_sel = i == selected;
            let tab_x = area.x + (i as u16) * tab_w;
            let (lm, rm) = self.shape.markers(is_sel);
            let content: String = format!("{lm}{label}{rm}");
            let cw = content.chars().count() as u16;
            let cx = tab_x + tab_w.saturating_sub(cw) / 2;
            let style = if is_sel { sel_style } else { dim_style };
            for (i, ch) in content.chars().enumerate() {
                let col = cx + i as u16;
                if col >= area.right() || col >= tab_x + tab_w {
                    break;
                }
                buf[(col, label_row)].set_char(ch).set_style(style);
            }
            // Underline shape: rule beneath the selected tab (if there's room).
            if matches!(self.shape, TabsShape::Underline)
                && is_sel
                && underline_row < area.bottom()
            {
                for x in tab_x..(tab_x + tab_w).min(area.right()) {
                    buf[(x, underline_row)].set_char('─').set_fg(accent);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect, style::Modifier};

    const W: u16 = 30;
    const H: u16 = 3;

    fn render(labels: &[&str], selected: usize, shape: TabsShape, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let mut state = TabsState { selected };
        StatefulWidget::render(
            Tabs::new(labels.iter().copied()).shape(shape).theme(theme),
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
    fn renders_all_labels() {
        let buf = render(&["STATUS", "NAV", "LOG"], 0, TabsShape::Bracket, Theme::Cyberpunk);
        let text = row_text(&buf, 0);
        assert!(text.contains("STATUS") && text.contains("NAV") && text.contains("LOG"));
    }

    #[test]
    fn selected_is_accent_bold() {
        let fg = Theme::Cyberpunk.palette().fg.color();
        let buf = render(&["A", "B"], 0, TabsShape::Bracket, Theme::Cyberpunk);
        let a_x = (0..W).find(|&x| buf[(x, 0)].symbol() == "A").expect("'A'");
        assert_eq!(buf[(a_x, 0)].fg, fg, "selected is fg/accent");
        assert!(buf[(a_x, 0)].modifier.contains(Modifier::BOLD), "selected is bold");
    }

    #[test]
    fn unselected_is_muted() {
        let muted = Theme::Cyberpunk.palette().muted.color();
        let buf = render(&["A", "B"], 0, TabsShape::Bracket, Theme::Cyberpunk);
        let b_x = (0..W).find(|&x| buf[(x, 0)].symbol() == "B").expect("'B'");
        assert_eq!(buf[(b_x, 0)].fg, muted, "unselected is muted");
    }

    #[test]
    fn underline_draws_rule_beneath_selected() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let buf = render(&["A", "B"], 0, TabsShape::Underline, Theme::Cyberpunk);
        let under = H / 2 + 1;
        // Tab 0's column range starts at 0; a ─ should appear beneath it.
        assert_eq!(buf[(0, under)].symbol(), "─");
        assert_eq!(buf[(0, under)].fg, accent, "underline is accent");
    }

    #[test]
    fn bracket_wraps_selected() {
        let buf = render(&["A", "B"], 0, TabsShape::Bracket, Theme::Cyberpunk);
        let text = row_text(&buf, 0);
        assert!(text.contains("[A]"), "selected tab bracketed: {text:?}");
    }

    #[test]
    fn arrow_wraps_selected() {
        let buf = render(&["A", "B"], 0, TabsShape::Arrow, Theme::Cyberpunk);
        let text = row_text(&buf, 0);
        assert!(text.contains(ARROW_L) && text.contains(ARROW_R), "arrow markers: {text:?}");
    }

    #[test]
    fn handle_key_right_cycles() {
        let tabs = Tabs::new(["A", "B", "C"]);
        let mut s = TabsState::new();
        let k = |c: KeyCode| KeyEvent::new(c, ratatui::crossterm::event::KeyModifiers::NONE);
        tabs.handle_key(&mut s, k(KeyCode::Right));
        assert_eq!(s.selected, 1);
        tabs.handle_key(&mut s, k(KeyCode::Right));
        tabs.handle_key(&mut s, k(KeyCode::Right));
        assert_eq!(s.selected, 0, "Right wraps");
    }

    #[test]
    fn selected_out_of_range_clamps() {
        let buf = render(&["A", "B"], 99, TabsShape::Bracket, Theme::Cyberpunk);
        // Doesn't panic; tab 1 (last) is the selected one.
        let b_x = (0..W).find(|&x| buf[(x, 0)].symbol() == "B").expect("'B'");
        let fg = Theme::Cyberpunk.palette().fg.color();
        assert_eq!(buf[(b_x, 0)].fg, fg, "out-of-range clamps to last");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let mut state = TabsState::new();
        StatefulWidget::render(Tabs::new(["A"]), Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
