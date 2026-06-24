//! **Graph** — a node-and-edge topology diagram.
//!
//! Draws a network of nodes (positions in normalized `0..1` space) connected
//! by edges — the system-topology / constellation-route view. Nodes and edges
//! are rasterized onto a Braille sub-pixel canvas, so a Graph stays crisp at
//! any size.
//!
//! ## Implementation notes
//! - Stateless [`Widget`]: `nodes`/`edges`/`shape` are configuration; the
//!   topology is static, so there is no `GraphState`.
//! - Nodes are placed at `(x, y) * (pixel_width, pixel_height)`; edges are
//!   drawn between node centers with Bresenham's line algorithm. Both are
//!   packed into per-cell Braille glyphs. Colors come off the
//!   [`Palette`](crate::Palette): nodes = `accent`, edges = `muted`.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{Graph, GraphShape, Theme};
//!
//! // A triangle: three nodes, three edges.
//! let g = Graph::new([(0.2, 0.8), (0.8, 0.8), (0.5, 0.1)])
//!     .edges([(0, 1), (1, 2), (2, 0)])
//!     .shape(GraphShape::Cross)
//!     .theme(Theme::DeepSpace);
//! ```

use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};

use crate::Theme;

/// Visual form of a [`Graph`]'s nodes.
///
/// Selects how many sub-pixels a node covers; colors stay on the palette
/// (nodes `accent`), untouched by this enum.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GraphShape {
    /// A single sub-pixel dot per node — the default.
    #[default]
    Dot,
    /// A 5-pixel cross (`+`) per node.
    Cross,
}

/// A sci-fi network topology graph.
///
/// Build with [`Graph::new`] (an iterator of `(x, y)` node positions in
/// `0..1`), then add [`Graph::edges`] (index pairs) and set the theme.
#[derive(Debug, Clone)]
pub struct Graph {
    /// Node positions as `(x, y)` in `0..=1`.
    pub nodes: Vec<(f32, f32)>,
    /// Edges as `(from_index, to_index)` pairs into `nodes`.
    pub edges: Vec<(usize, usize)>,
    /// Node glyph form. Defaults to [`GraphShape::Dot`].
    pub shape: GraphShape,
    /// Theme whose [`Palette`](crate::Palette) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Graph {
    /// Create a graph from an iterator of node `(x, y)` positions (`0..=1`).
    pub fn new(nodes: impl IntoIterator<Item = (f32, f32)>) -> Self {
        Self {
            nodes: nodes.into_iter().collect(),
            edges: Vec::new(),
            shape: GraphShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the edges (`(from, to)` index pairs).
    #[must_use]
    pub fn edges(mut self, edges: impl IntoIterator<Item = (usize, usize)>) -> Self {
        self.edges = edges.into_iter().collect();
        self
    }

    /// Set the node glyph form (see [`GraphShape`]).
    #[must_use]
    pub fn shape(mut self, shape: GraphShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the graph.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

impl Widget for Graph {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() || self.nodes.is_empty() {
            return;
        }
        // Braille sub-pixel grid.
        let pw = (area.width as usize).saturating_mul(2);
        let ph = (area.height as usize).saturating_mul(4);
        if pw == 0 || ph == 0 {
            return;
        }

        // Two parallel lit-pixel grids: edges (muted) under, nodes (accent) over.
        let mut lit_edge = vec![false; pw * ph];
        let mut lit_node = vec![false; pw * ph];

        // Map nodes to pixel coords (clamp into the grid).
        let node_px: Vec<(i64, i64)> = self
            .nodes
            .iter()
            .map(|(nx, ny)| {
                let x = (*nx).clamp(0.0, 1.0) * (pw - 1) as f32;
                let y = (*ny).clamp(0.0, 1.0) * (ph - 1) as f32;
                (x.round() as i64, y.round() as i64)
            })
            .collect();

        // Draw edges (Bresenham).
        for &(a, b) in &self.edges {
            if a < node_px.len() && b < node_px.len() {
                for (x, y) in line_points(node_px[a].0, node_px[a].1, node_px[b].0, node_px[b].1) {
                    set_pixel(&mut lit_edge, pw, ph, x, y);
                }
            }
        }

        // Draw nodes.
        for &(x, y) in &node_px {
            set_pixel(&mut lit_node, pw, ph, x, y);
            if matches!(self.shape, GraphShape::Cross) {
                // 4-neighborhood cross.
                set_pixel(&mut lit_node, pw, ph, x + 1, y);
                set_pixel(&mut lit_node, pw, ph, x - 1, y);
                set_pixel(&mut lit_node, pw, ph, x, y + 1);
                set_pixel(&mut lit_node, pw, ph, x, y - 1);
            }
        }

        let node_style = Style::new().fg(self.theme.palette().accent.color());
        let edge_style = Style::new().fg(self.theme.palette().muted.color());

        // Pack each cell's 2×4 sub-pixels into one Braille glyph. A cell with any
        // node pixels takes the node style; otherwise an edge cell takes muted.
        for row in 0..area.height {
            for col in 0..area.width {
                let mut node_bits = 0u8;
                let mut edge_bits = 0u8;
                for sy in 0..4u16 {
                    for sx in 0..2u16 {
                        let px = (col * 2 + sx) as usize;
                        let py = (row * 4 + sy) as usize;
                        let bit = 1u8 << (sx + sy * 2);
                        if lit_node[py * pw + px] {
                            node_bits |= bit;
                        }
                        if lit_edge[py * pw + px] {
                            edge_bits |= bit;
                        }
                    }
                }
                if node_bits != 0 {
                    let ch = braille(node_bits);
                    buf[(area.x + col, area.y + row)].set_char(ch).set_style(node_style);
                } else if edge_bits != 0 {
                    let ch = braille(edge_bits);
                    buf[(area.x + col, area.y + row)].set_char(ch).set_style(edge_style);
                }
            }
        }
    }
}

/// Bresenham's line algorithm between two pixel endpoints.
fn line_points(x0: i64, y0: i64, x1: i64, y1: i64) -> Vec<(i64, i64)> {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;
    let mut x = x0;
    let mut y = y0;
    let mut pts = Vec::new();
    loop {
        pts.push((x, y));
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }
    pts
}

/// Set a single pixel in `grid` (bounds-checked; out-of-range is a no-op).
fn set_pixel(grid: &mut [bool], pw: usize, ph: usize, x: i64, y: i64) {
    if x >= 0 && y >= 0 {
        let (xu, yu) = (x as usize, y as usize);
        if xu < pw && yu < ph {
            grid[yu * pw + xu] = true;
        }
    }
}

/// A Braille glyph from its 8-bit dot bitmap.
fn braille(bits: u8) -> char {
    char::from_u32(0x2800 + bits as u32).unwrap_or(' ')
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 30;
    const H: u16 = 12;

    fn render(nodes: &[(f32, f32)], edges: &[(usize, usize)], shape: GraphShape, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        Graph::new(nodes.iter().copied())
            .edges(edges.iter().copied())
            .shape(shape)
            .theme(theme)
            .render(Rect::new(0, 0, W, H), &mut buf);
        buf
    }

    fn lit_count(buf: &Buffer) -> usize {
        let mut n = 0;
        for x in 0..W {
            for y in 0..H {
                if buf[(x, y)].symbol() != " " {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn renders_nodes_and_edge() {
        // Two nodes + one edge lights at least a few cells.
        let buf = render(&[(0.1, 0.5), (0.9, 0.5)], &[(0, 1)], GraphShape::Dot, Theme::Cyberpunk);
        assert!(lit_count(&buf) > 0, "graph with an edge should light cells");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        Graph::new([(0.5, 0.5)]).render(Rect::new(0, 0, 0, 0), &mut buf);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }

    #[test]
    fn no_nodes_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        Graph::new([]).render(Rect::new(0, 0, W, H), &mut buf);
        assert_eq!(lit_count(&buf), 0, "no nodes → nothing drawn");
    }

    #[test]
    fn cross_shape_lights_more_than_dot() {
        // A single node: Cross (5 pixels) lights ≥ as many cells as Dot (1).
        let dot = render(&[(0.5, 0.5)], &[], GraphShape::Dot, Theme::Cyberpunk);
        let cross = render(&[(0.5, 0.5)], &[], GraphShape::Cross, Theme::Cyberpunk);
        assert!(lit_count(&cross) >= lit_count(&dot), "Cross ≥ Dot lit cells");
    }

    #[test]
    fn node_uses_accent_edge_uses_muted() {
        // Two widely-spaced nodes joined by an edge: somewhere in the middle the
        // trace is pure edge (muted), at a node it's accent.
        let accent = Theme::Cyberpunk.palette().accent.color();
        let muted = Theme::Cyberpunk.palette().muted.color();
        let buf = render(&[(0.05, 0.5), (0.95, 0.5)], &[(0, 1)], GraphShape::Dot, Theme::Cyberpunk);
        let mut fgs = std::collections::HashSet::new();
        for x in 0..W {
            for y in 0..H {
                if buf[(x, y)].symbol() != " " {
                    fgs.insert(buf[(x, y)].fg);
                }
            }
        }
        assert!(fgs.contains(&accent), "nodes carry --accent: {fgs:?}");
        assert!(fgs.contains(&muted), "edges carry --muted: {fgs:?}");
    }

    #[test]
    fn invalid_edge_index_does_not_panic() {
        // Edge references a non-existent node — must skip, not panic.
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        Graph::new([(0.5, 0.5)]).edges([(0, 99)]).render(Rect::new(0, 0, W, H), &mut buf);
        // The node still renders.
        assert!(lit_count(&buf) > 0);
    }

    #[test]
    fn triangle_renders() {
        let buf = render(
            &[(0.2, 0.8), (0.8, 0.8), (0.5, 0.1)],
            &[(0, 1), (1, 2), (2, 0)],
            GraphShape::Cross,
            Theme::Cyberpunk,
        );
        assert!(lit_count(&buf) > 5, "a triangle lights a healthy number of cells");
    }
}
