//! **KeyValue** — a label/value property list.
//!
//! A stack of `label … value` rows — the "properties / readouts" panel where
//! each line is a muted key on the left and its value flush right. It's the
//! many-row sibling of [`crate::Value`] (which shows one reading).
//!
//! ## Spec
//! - [`Plain`](KeyValueShape::Plain): `LABEL          VALUE` (key left, value right).
//! - [`Dotted`](KeyValueShape::Dotted): `LABEL ········ VALUE` (leaders between).
//!
//! ## Implementation notes
//! - Stateless [`Widget`]: the entries are configuration.
//! - Styling reuses the `Label` (muted) node for keys and the `Value` (fg) node
//!   for values — `var(--…)`-driven off the palette.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{KeyValue, KeyValueShape, Theme};
//!
//! let kv = KeyValue::new([("HULL", "82%"), ("FUEL", "47%"), ("O2", "21%")])
//!     .shape(KeyValueShape::Dotted)
//!     .theme(Theme::DeepSpace);
//! ```

use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};
use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Visual form of a [`KeyValue`] list.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum KeyValueShape {
    /// Key left, value right, nothing between.
    #[default]
    Plain,
    /// Key left, `·` leaders, value right.
    Dotted,
}

/// A sci-fi key/value property list.
///
/// Build with [`KeyValue::new`] (an iterator of `(key, value)` pairs), then set
/// the shape and theme.
#[derive(Debug, Clone)]
pub struct KeyValue {
    /// `(key, value)` rows, top to bottom.
    pub entries: Vec<(String, String)>,
    /// Leader form. Defaults to [`KeyValueShape::Plain`].
    pub shape: KeyValueShape,
    /// Theme whose [`Stylesheet`](ratatui_style::Stylesheet) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl KeyValue {
    /// Create a list from an iterator of `(key, value)` pairs.
    pub fn new(entries: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>) -> Self {
        Self {
            entries: entries
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
            shape: KeyValueShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the leader form (see [`KeyValueShape`]).
    #[must_use]
    pub fn shape(mut self, shape: KeyValueShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the list.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

impl Widget for KeyValue {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let key_style = sheet.compute_with(&NodeRef::new("Label"), None, &mut scratch).to_style();
        let val_style = sheet.compute_with(&NodeRef::new("Value"), None, &mut scratch).to_style();

        for (i, (key, val)) in self.entries.iter().enumerate() {
            let y = area.y + i as u16;
            if y >= area.bottom() {
                break;
            }

            // Key at the left edge.
            let key_w = key.chars().count() as u16;
            let key_end = area.x + key_w.min(area.width);
            for (gx, ch) in key.chars().enumerate() {
                let px = area.x + gx as u16;
                if px >= area.right() {
                    break;
                }
                buf[(px, y)].set_char(ch).set_style(key_style);
            }

            // Value flush right.
            let val_w = val.chars().count() as u16;
            let val_start = area.right().saturating_sub(val_w);
            for (vi, ch) in val.chars().enumerate() {
                let px = val_start + vi as u16;
                if px >= area.right() {
                    break;
                }
                buf[(px, y)].set_char(ch).set_style(val_style);
            }

            // Leaders between key and value (Dotted only).
            if matches!(self.shape, KeyValueShape::Dotted) {
                let mut lx = key_end + 1; // one space after the key
                while lx < val_start.saturating_sub(1) {
                    buf[(lx, y)].set_char('·').set_style(key_style);
                    lx += 1;
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
    const H: u16 = 5;

    fn render(entries: &[(&str, &str)], shape: KeyValueShape, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        KeyValue::new(entries.iter().copied())
            .shape(shape)
            .theme(theme)
            .render(Rect::new(0, 0, W, H), &mut buf);
        buf
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..W).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()
    }

    #[test]
    fn renders_key_and_value() {
        let buf = render(&[("HULL", "82%")], KeyValueShape::Plain, Theme::Cyberpunk);
        let text = row_text(&buf, 0);
        assert!(text.starts_with("HULL"), "key at left: {text:?}");
        assert!(text.trim_end().ends_with("82%"), "value at right: {text:?}");
    }

    #[test]
    fn plain_has_no_leaders() {
        let buf = render(&[("HULL", "82%")], KeyValueShape::Plain, Theme::Cyberpunk);
        assert!(!row_text(&buf, 0).contains('·'), "Plain has no dot leaders");
    }

    #[test]
    fn dotted_has_leaders() {
        let buf = render(&[("HULL", "82%")], KeyValueShape::Dotted, Theme::Cyberpunk);
        assert!(row_text(&buf, 0).contains('·'), "Dotted fills with · leaders");
    }

    #[test]
    fn key_is_muted_value_is_fg() {
        let muted = Theme::Cyberpunk.palette().muted.color();
        let fg = Theme::Cyberpunk.palette().fg.color();
        let buf = render(&[("HULL", "82%")], KeyValueShape::Plain, Theme::Cyberpunk);
        assert_eq!(buf[(0, 0)].fg, muted, "key is --muted");
        let val_x = (0..W).find(|&x| buf[(x, 0)].symbol() == "8").expect("'8' present");
        assert_eq!(buf[(val_x, 0)].fg, fg, "value is --fg");
    }

    #[test]
    fn multiple_rows_stack() {
        let buf = render(&[("A", "1"), ("B", "2")], KeyValueShape::Plain, Theme::Cyberpunk);
        assert!(row_text(&buf, 0).starts_with('A'));
        assert!(row_text(&buf, 1).starts_with('B'));
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        KeyValue::new([("A", "B")]).render(Rect::new(0, 0, 0, 0), &mut buf);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
