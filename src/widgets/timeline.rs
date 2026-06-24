//! **Timeline** — a vertical event log.
//!
//! A stack of timestamped events, each marked with a node glyph — the
//! "mission log" / audit trail. [`Plain`](TimelineShape::Plain) packs one event
//! per row; [`Connected`](TimelineShape::Connected) draws a `│` link between
//! nodes.
//!
//! ## Spec
//! ```text
//! ● 09:00 · BOOT       (Plain: one row each)
//! ● 09:05 · LOGIN
//! ```
//! ```text
//! ● 09:00 · BOOT       (Connected: │ links the nodes)
//! │
//! ● 09:05 · LOGIN
//! ```
//!
//! ## Implementation notes
//! - Stateless [`Widget`]: events are configuration.
//! - Node `●` is `accent`; the timestamp is `muted`; the event text is `fg` —
//!   all off the [`Palette`](crate::Palette).
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{Theme, Timeline, TimelineShape};
//!
//! let tl = Timeline::new([("09:00", "BOOT"), ("09:05", "LOGIN")])
//!     .shape(TimelineShape::Connected)
//!     .theme(Theme::Fallout);
//! ```

use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};

use crate::Theme;

/// Node glyph drawn at each event, for the default look.
pub const NODE: char = '●';

/// Visual form of a [`Timeline`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TimelineShape {
    /// One event per row, no inter-node link.
    #[default]
    Plain,
    /// A `│` connects each node to the next (two rows per event).
    Connected,
}

/// A sci-fi event timeline.
///
/// Build with [`Timeline::new`] (an iterator of `(time, event)` pairs).
#[derive(Debug, Clone)]
pub struct Timeline {
    /// `(time, event)` rows, top to bottom.
    pub events: Vec<(String, String)>,
    /// Node/link form. Defaults to [`TimelineShape::Plain`].
    pub shape: TimelineShape,
    /// Theme whose [`Palette`](crate::Palette) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Timeline {
    /// Create a timeline from an iterator of `(time, event)` pairs.
    pub fn new(events: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>) -> Self {
        Self {
            events: events.into_iter().map(|(t, e)| (t.into(), e.into())).collect(),
            shape: TimelineShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the node/link form (see [`TimelineShape`]).
    #[must_use]
    pub fn shape(mut self, shape: TimelineShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the timeline.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Draw one event's content (`time · event`) starting at column `x` on `y`.
    fn draw_event(&self, buf: &mut Buffer, x: u16, y: u16, right: u16, time: &str, event: &str) {
        let p = self.theme.palette();
        let muted = Style::new().fg(p.muted.color());
        let fg = Style::new().fg(p.fg.color());

        let mut col = x;
        // timestamp
        for ch in time.chars() {
            if col >= right {
                return;
            }
            buf[(col, y)].set_char(ch).set_style(muted);
            col += 1;
        }
        // " · " separator
        for ch in " · ".chars() {
            if col >= right {
                return;
            }
            buf[(col, y)].set_char(ch).set_style(muted);
            col += 1;
        }
        // event text
        for ch in event.chars() {
            if col >= right {
                return;
            }
            buf[(col, y)].set_char(ch).set_style(fg);
            col += 1;
        }
    }
}

impl Widget for Timeline {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        let accent = Style::new().fg(self.theme.palette().accent.color());
        let muted = Style::new().fg(self.theme.palette().muted.color());
        let content_x = area.x + 2; // node at col 0, space, content at col 2
        let right = area.right();

        let stride: u16 = if matches!(self.shape, TimelineShape::Connected) { 2 } else { 1 };

        for (i, (time, event)) in self.events.iter().enumerate() {
            let node_y = area.y + (i as u16) * stride;
            if node_y >= area.bottom() {
                break;
            }
            // Node.
            buf[(area.x, node_y)].set_char(NODE).set_style(accent);
            // Event content.
            self.draw_event(buf, content_x, node_y, right, time, event);
            // Link to the next node (Connected only, not after the last event).
            if matches!(self.shape, TimelineShape::Connected) && i + 1 < self.events.len() {
                let link_y = node_y + 1;
                if link_y < area.bottom() {
                    buf[(area.x, link_y)].set_char('│').set_style(muted);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 24;
    const H: u16 = 8;

    fn render(events: &[(&str, &str)], shape: TimelineShape, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        Timeline::new(events.iter().copied())
            .shape(shape)
            .theme(theme)
            .render(Rect::new(0, 0, W, H), &mut buf);
        buf
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..W).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()
    }

    #[test]
    fn plain_renders_node_time_event() {
        let buf = render(&[("09:00", "BOOT")], TimelineShape::Plain, Theme::Cyberpunk);
        let text = row_text(&buf, 0);
        assert!(text.contains(NODE), "node present: {text:?}");
        assert!(text.contains("09:00"), "time present");
        assert!(text.contains("BOOT"), "event present");
    }

    #[test]
    fn plain_one_row_per_event() {
        let buf = render(&[("09:00", "A"), ("09:05", "B")], TimelineShape::Plain, Theme::Cyberpunk);
        assert!(row_text(&buf, 0).contains("A"));
        assert!(row_text(&buf, 1).contains("B"));
        assert!(row_text(&buf, 2).trim().is_empty(), "no third row in Plain");
    }

    #[test]
    fn connected_has_link_line() {
        let buf = render(&[("09:00", "A"), ("09:05", "B")], TimelineShape::Connected, Theme::Cyberpunk);
        // Row 1 (between the two nodes) carries the │ link.
        assert_eq!(buf[(0, 1)].symbol(), "│", "Connected links nodes with │");
        // Nodes on rows 0 and 2.
        assert_eq!(buf[(0, 0)].symbol(), NODE.to_string().as_str());
        assert_eq!(buf[(0, 2)].symbol(), NODE.to_string().as_str());
    }

    #[test]
    fn connected_no_link_after_last() {
        let buf = render(&[("09:00", "A"), ("09:05", "B")], TimelineShape::Connected, Theme::Cyberpunk);
        // Row 3 (after the last node) has no link.
        assert_ne!(buf[(0, 3)].symbol(), "│", "no link after the last node");
    }

    #[test]
    fn node_is_accent() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let buf = render(&[("09:00", "A")], TimelineShape::Plain, Theme::Cyberpunk);
        assert_eq!(buf[(0, 0)].fg, accent, "node is --accent");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        Timeline::new([("0", "A")]).render(Rect::new(0, 0, 0, 0), &mut buf);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
