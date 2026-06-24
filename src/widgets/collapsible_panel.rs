//! **CollapsiblePanel** — a titled container that folds to one row.
//!
//! A [`Panel`]-style frame whose body can be collapsed to a single header row
//! — the "disclosure" container for tucking detailed readouts behind a title.
//! Expanded it draws a full border + title; collapsed it draws just the header
//! (with a `▸`/`▾` marker), and [`CollapsiblePanel::inner`] returns a zero-area
//! rect so the app knows to skip rendering the body.
//!
//! ## Spec
//! - Collapsed: `▸ TITLE` on one row (no body).
//! - Expanded: a bordered frame titled `▾ TITLE`, with [`Self::inner`] giving
//!   the body area.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]: `title` is configuration; only `collapsed`
//!   is state, toggled by the app (e.g. on clicking/activating the header).
//! - Drawn with a plain `Block::bordered` (border = `--muted`, title =
//!   `--accent` bold) — the same vocabulary as the rest of the containers,
//!   read straight off the [`Palette`](crate::Palette).
//! - [`CollapsiblePanel::inner`] is a `&self` method taking `&state`; call it
//!   before [`StatefulWidget::render`] (which takes `self` by value).
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{CollapsiblePanel, CollapsiblePanelState, Theme};
//!
//! let panel = CollapsiblePanel::new("SENSORS").theme(Theme::DeepSpace);
//! let mut state = CollapsiblePanelState::default(); // expanded
//! // let body = panel.inner(area, &state); render your content there, then:
//! // StatefulWidget::render(panel, area, buf, &mut state);
//! ```

use ratatui::{buffer::Buffer, layout::Rect, style::{Modifier, Style}, widgets::{Block, StatefulWidget, Widget}};
use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::Theme;

/// Glyph drawn when collapsed, for the [`CollapsibleShape::Chevron`] default.
pub const MARK_COLLAPSED: char = '▸';
/// Glyph drawn when expanded, for the [`CollapsibleShape::Chevron`] default.
pub const MARK_EXPANDED: char = '▾';

/// Visual form of a [`CollapsiblePanel`]'s collapse/expand marker.
///
/// Selects the marker glyph pair; colors stay on the palette, untouched by
/// this enum. The [`CollapsibleShape::Chevron`] default draws `▸`/`▾`.
///
/// Every marker glyph is Unicode width-1 (see convention #5 at the crate root).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CollapsibleShape {
    /// `▸` collapsed, `▾` expanded — the original look.
    #[default]
    Chevron,
    /// `+` collapsed, `−` expanded.
    Plus,
    /// `►` collapsed, `▼` expanded.
    Arrow,
}

impl CollapsibleShape {
    /// The marker glyph for the given collapsed/expanded state.
    #[must_use]
    pub const fn marker(self, collapsed: bool) -> char {
        match (self, collapsed) {
            (Self::Chevron, true) => MARK_COLLAPSED,
            (Self::Chevron, false) => MARK_EXPANDED,
            (Self::Plus, true) => '+',
            (Self::Plus, false) => '−',
            (Self::Arrow, true) => '►',
            (Self::Arrow, false) => '▼',
        }
    }
}

/// A sci-fi collapsible panel.
///
/// Build with [`CollapsiblePanel::new`] (title), then set the theme with
/// [`CollapsiblePanel::theme`]. The collapse flag lives in
/// [`CollapsiblePanelState`].
#[derive(Debug, Clone)]
pub struct CollapsiblePanel {
    /// Header title text.
    pub title: String,
    /// Collapse/expand marker form. Defaults to [`CollapsibleShape::Chevron`].
    pub shape: CollapsibleShape,
    /// Theme whose [`Palette`](crate::Palette) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl CollapsiblePanel {
    /// Create a panel with the given title, expanded, default theme.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            shape: CollapsibleShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the marker-glyph form (see [`CollapsibleShape`]).
    #[must_use]
    pub fn shape(mut self, shape: CollapsibleShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the panel.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Toggle the collapse state on `Enter` (a convenience; the app can also
    /// flip `state.collapsed` directly).
    pub fn handle_key(&self, state: &mut CollapsiblePanelState, key: KeyEvent) {
        if let KeyCode::Enter = key.code {
            state.collapsed = !state.collapsed;
        }
    }

    /// The body area for app-rendered content. Zero-height when collapsed.
    /// Call before [`StatefulWidget::render`] (which consumes `self`).
    #[must_use]
    pub fn inner(&self, area: Rect, state: &CollapsiblePanelState) -> Rect {
        if state.collapsed {
            Rect::new(0, 0, 0, 0)
        } else {
            Block::bordered().inner(area)
        }
    }
}

/// Mutable state for [`CollapsiblePanel`].
#[derive(Debug, Default, Clone)]
pub struct CollapsiblePanelState {
    /// Whether the panel is folded to its header row.
    pub collapsed: bool,
}

impl CollapsiblePanelState {
    /// Create an expanded state (`collapsed = false`).
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggle collapsed/expanded.
    pub fn toggle(&mut self) {
        self.collapsed = !self.collapsed;
    }
}

impl StatefulWidget for CollapsiblePanel {
    type State = CollapsiblePanelState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }

        let p = self.theme.palette();
        let title_style = Style::new().fg(p.accent.color()).add_modifier(Modifier::BOLD);
        let border_style = Style::new().fg(p.muted.color());
        let marker = self.shape.marker(state.collapsed);

        if state.collapsed {
            // Single header row: `▸ TITLE`, left-aligned on row 0.
            let header = format!("{marker} {title}", title = self.title);
            buf.set_string(area.x, area.y, &header, title_style);
        } else {
            // Full bordered frame titled `▾ TITLE`.
            let block = Block::bordered()
                .title(format!(" {marker} {title} ", title = self.title))
                .border_style(border_style)
                .title_style(title_style);
            block.render(area, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 20;
    const H: u16 = 5;

    fn render(title: &str, collapsed: bool, theme: Theme, shape: CollapsibleShape) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let mut state = CollapsiblePanelState { collapsed };
        StatefulWidget::render(
            CollapsiblePanel::new(title).shape(shape).theme(theme),
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
    fn collapsed_renders_header_marker_only() {
        let buf = render("SENSORS", true, Theme::Cyberpunk, CollapsibleShape::Chevron);
        let header = row_text(&buf, 0);
        assert!(header.contains(MARK_COLLAPSED), "collapsed marker ▸: {header:?}");
        assert!(header.contains("SENSORS"), "title present: {header:?}");
        // No border drawn when collapsed (row 0 col 0 is the marker, not a corner).
        assert_ne!(buf[(0, 0)].symbol(), "┌", "no border corner when collapsed");
    }

    #[test]
    fn expanded_renders_border() {
        let buf = render("SENSORS", false, Theme::Cyberpunk, CollapsibleShape::Chevron);
        assert_eq!(buf[(0, 0)].symbol(), "┌", "expanded draws the top-left border corner");
        assert_eq!(buf[(W - 1, H - 1)].symbol(), "┘", "expanded draws the bottom-right corner");
    }

    #[test]
    fn expanded_header_has_expand_marker() {
        let buf = render("SENSORS", false, Theme::Cyberpunk, CollapsibleShape::Chevron);
        // The title row (row 0) carries the expanded marker.
        assert!(row_text(&buf, 0).contains(MARK_EXPANDED), "expanded marker ▾ in title");
    }

    #[test]
    fn marker_swaps_with_shape() {
        let plus = render("X", true, Theme::Cyberpunk, CollapsibleShape::Plus);
        assert!(row_text(&plus, 0).contains('+'), "Plus collapsed → '+'");
        assert!(!row_text(&plus, 0).contains(MARK_COLLAPSED), "must not use the Chevron marker");
    }

    #[test]
    fn inner_is_zero_when_collapsed() {
        let panel = CollapsiblePanel::new("X");
        let state = CollapsiblePanelState { collapsed: true };
        let inner = panel.inner(Rect::new(0, 0, W, H), &state);
        assert_eq!(inner.height, 0, "no body area when collapsed");
    }

    #[test]
    fn inner_is_nonzero_when_expanded() {
        let panel = CollapsiblePanel::new("X");
        let state = CollapsiblePanelState { collapsed: false };
        let inner = panel.inner(Rect::new(0, 0, W, H), &state);
        assert!(inner.height > 0 && inner.width > 0, "body area present when expanded");
        // Inner is the border-inset: 2 narrower and shorter than the area.
        assert_eq!(inner.width, W - 2);
        assert_eq!(inner.height, H - 2);
    }

    #[test]
    fn toggle_flips_state() {
        let mut s = CollapsiblePanelState::new();
        assert!(!s.collapsed);
        s.toggle();
        assert!(s.collapsed);
        s.toggle();
        assert!(!s.collapsed);
    }

    #[test]
    fn handle_key_enter_toggles() {
        let panel = CollapsiblePanel::new("X");
        let mut s = CollapsiblePanelState::new();
        let enter = KeyEvent::new(KeyCode::Enter, ratatui::crossterm::event::KeyModifiers::NONE);
        panel.handle_key(&mut s, enter);
        assert!(s.collapsed, "Enter collapses");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let mut state = CollapsiblePanelState::default();
        StatefulWidget::render(
            CollapsiblePanel::new("X"),
            Rect::new(0, 0, 0, 0),
            &mut buf,
            &mut state,
        );
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
