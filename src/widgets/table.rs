//! **Table** — a sci-fi styled data table.
//!
//! A header row over body rows, with automatic per-column widths and an
//! optional zebra-stripe backdrop — the readout grid. It's the themed
//! alternative to ratatui's built-in `Table` for when the whole UI should share
//! one palette.
//!
//! ## Spec
//! - Header row: `accent`, bold.
//! - Body rows: `fg` on the theme background; [`Zebra`](TableShape::Zebra)
//!   paints odd rows `panel` for striping.
//! - Column width = the widest cell in that column (header or body), plus
//!   2 cells of padding.
//!
//! ## Implementation notes
//! - Stateless [`Widget`]: headers/rows are configuration.
//! - Colors off the [`Palette`](crate::Palette).
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{Table, TableShape, Theme};
//!
//! let t = Table::new(["SYS", "STATUS"], [["LIFE", "OK"], ["NAV", "WARN"]])
//!     .shape(TableShape::Zebra)
//!     .theme(Theme::Weyland);
//! ```

use ratatui::{buffer::Buffer, layout::Rect, style::{Modifier, Style}, widgets::Widget};

use crate::Theme;

/// Per-cell padding (left + right) added to each column's content width.
const COL_PAD: u16 = 2;

/// Visual form of a [`Table`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TableShape {
    /// Odd body rows get a `panel` backdrop.
    #[default]
    Zebra,
    /// No striping.
    Plain,
}

/// A sci-fi data table.
///
/// Build with [`Table::new`] (headers, rows). Rows may be ragged — missing
/// cells render blank.
#[derive(Debug, Clone)]
pub struct Table {
    /// Column headers.
    pub headers: Vec<String>,
    /// Body rows (each a vector of cell strings).
    pub rows: Vec<Vec<String>>,
    /// Striping form. Defaults to [`TableShape::Zebra`].
    pub shape: TableShape,
    /// Theme whose [`Palette`](crate::Palette) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Table {
    /// Create a table from headers and rows.
    pub fn new(
        headers: impl IntoIterator<Item = impl Into<String>>,
        rows: impl IntoIterator<Item = impl IntoIterator<Item = impl Into<String>>>,
    ) -> Self {
        Self {
            headers: headers.into_iter().map(Into::into).collect(),
            rows: rows
                .into_iter()
                .map(|r| r.into_iter().map(Into::into).collect())
                .collect(),
            shape: TableShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the striping form (see [`TableShape`]).
    #[must_use]
    pub fn shape(mut self, shape: TableShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the table.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Width (in cells) of each column: max content width + padding.
    fn col_widths(&self) -> Vec<u16> {
        let mut widths: Vec<u16> = self
            .headers
            .iter()
            .map(|h| h.chars().count() as u16 + COL_PAD)
            .collect();
        for row in &self.rows {
            for (c, cell) in row.iter().enumerate() {
                if c < widths.len() {
                    widths[c] = widths[c].max(cell.chars().count() as u16 + COL_PAD);
                }
            }
        }
        widths
    }
}

impl Widget for Table {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() || self.headers.is_empty() {
            return;
        }

        let p = self.theme.palette();
        let header_style = Style::new().fg(p.accent.color()).add_modifier(Modifier::BOLD);
        let cell_style = Style::new().fg(p.fg.color());
        let zebra_bg = p.panel.color();
        let zebra = matches!(self.shape, TableShape::Zebra);

        let col_w = self.col_widths();

        // Header row.
        let mut x = area.x;
        for (c, w) in col_w.iter().enumerate() {
            let cx = x + 1; // left pad
            for (gi, ch) in self.headers[c].chars().enumerate() {
                let px = cx + gi as u16;
                if px >= area.right() {
                    break;
                }
                buf[(px, area.y)].set_char(ch).set_style(header_style);
            }
            x += *w;
        }

        // Body rows.
        for (ri, row) in self.rows.iter().enumerate() {
            let y = area.y + 1 + ri as u16;
            if y >= area.bottom() {
                break;
            }
            let bg = if zebra && ri % 2 == 1 { Some(zebra_bg) } else { None };
            let mut x = area.x;
            for (c, w) in col_w.iter().enumerate() {
                let cell = row.get(c).map(String::as_str).unwrap_or("");
                let cx = x + 1;
                for (gi, ch) in cell.chars().enumerate() {
                    let px = cx + gi as u16;
                    if px >= area.right() {
                        break;
                    }
                    let mut s = cell_style;
                    if let Some(bg) = bg {
                        s = s.bg(bg);
                    }
                    buf[(px, y)].set_char(ch).set_style(s);
                }
                // Pad the rest of the cell to the column width with the row bg.
                if let Some(bg) = bg {
                    let cell_w = cell.chars().count() as u16;
                    let mut px = cx + cell_w;
                    let col_end = (x + w).min(area.right());
                    while px < col_end {
                        buf[(px, y)].set_style(cell_style.bg(bg));
                        px += 1;
                    }
                }
                x += *w;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 30;
    const H: u16 = 6;

    fn render(headers: &[&str], rows: &[&[&str]], shape: TableShape, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        Table::new(headers.iter().copied(), rows.iter().map(|r| r.iter().copied()))
            .shape(shape)
            .theme(theme)
            .render(Rect::new(0, 0, W, H), &mut buf);
        buf
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..W).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()
    }

    #[test]
    fn renders_header_and_rows() {
        let buf = render(&["SYS", "STATUS"], &[&["LIFE", "OK"], &["NAV", "WARN"]], TableShape::Plain, Theme::Cyberpunk);
        assert!(row_text(&buf, 0).contains("SYS") && row_text(&buf, 0).contains("STATUS"));
        assert!(row_text(&buf, 1).contains("LIFE") && row_text(&buf, 1).contains("OK"));
        assert!(row_text(&buf, 2).contains("NAV") && row_text(&buf, 2).contains("WARN"));
    }

    #[test]
    fn header_is_accent_bold() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let buf = render(&["SYS"], &[&["X"]], TableShape::Plain, Theme::Cyberpunk);
        let hx = (0..W).find(|&x| buf[(x, 0)].symbol() == "S").expect("'S' present");
        assert_eq!(buf[(hx, 0)].fg, accent, "header is --accent");
        assert!(buf[(hx, 0)].modifier.contains(Modifier::BOLD), "header is bold");
    }

    #[test]
    fn zebra_paints_odd_row_panel() {
        let panel = Theme::Cyberpunk.palette().panel.color();
        let buf = render(&["A"], &[&["r0"], &["r1"], &["r2"]], TableShape::Zebra, Theme::Cyberpunk);
        // Row 1 (index 1, odd) has panel bg; rows 0 and 2 don't.
        assert_eq!(buf[(1, 2)].bg, panel, "odd body row (table row 1) is panel-bg");
        assert_ne!(buf[(1, 1)].bg, panel, "even body row is not striped");
        assert_ne!(buf[(1, 3)].bg, panel, "even body row 2 is not striped");
    }

    #[test]
    fn plain_has_no_striping() {
        let panel = Theme::Cyberpunk.palette().panel.color();
        let buf = render(&["A"], &[&["r0"], &["r1"]], TableShape::Plain, Theme::Cyberpunk);
        assert_ne!(buf[(1, 2)].bg, panel, "Plain has no panel striping");
    }

    #[test]
    fn ragged_rows_render_blank_missing_cells() {
        // Row 1 has only one cell; the second column is blank, no panic.
        let buf = render(&["A", "B"], &[&["x"]], TableShape::Plain, Theme::Cyberpunk);
        assert!(row_text(&buf, 1).contains('x'));
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        Table::new(["A"], [["x"]]).render(Rect::new(0, 0, 0, 0), &mut buf);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
