//! **BigText** — a 5×7 dot-matrix banner.
//!
//! Renders text on a 5×7 LED dot-matrix grid (one `█` block per lit dot) —
//! the scoreboard / countdown / title-banner look. Covers digits, `:` (for
//! big clock displays), and space; unknown characters render blank.
//!
//! ## Spec
//! - [`Glow`](BigTextShape::Glow): only the lit dots (`█`, `accent`) — clean.
//! - [`Grid`](BigTextShape::Grid): lit dots `█` (`accent`) over unlit `░`
//!   (`muted`) — the full matrix look.
//!
//! ## Implementation notes
//! - Stateless [`Widget`]: the text is configuration.
//! - Dot colors off the [`Palette`](crate::Palette). Each character is 5 wide
//!   × 7 tall, with a 1-cell gap between characters.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{BigText, BigTextShape, Theme};
//!
//! let bt = BigText::new("123").shape(BigTextShape::Grid).theme(Theme::Fallout);
//! ```

use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};

use crate::Theme;

/// Cells per character: 5 wide + 1 gap.
const CHAR_W: u16 = 6;
/// Dot-matrix height.
const CHAR_H: u16 = 7;

// 5×7 dot-matrix glyphs (`#` = lit). Index 0..=9 are digits, then `:` and ` `.
const G0: [&str; 7] = [" ### ", "#   #", "#   #", "#   #", "#   #", "#   #", " ### "];
const G1: [&str; 7] = ["  #  ", " ##  ", "  #  ", "  #  ", "  #  ", "  #  ", " ### "];
const G2: [&str; 7] = [" ### ", "#   #", "    #", "   # ", "  #  ", " #   ", "#####"];
const G3: [&str; 7] = [" ### ", "#   #", "    #", "  ## ", "    #", "#   #", " ### "];
const G4: [&str; 7] = ["   # ", "  ## ", " # # ", "#  # ", "#####", "   # ", "   # "];
const G5: [&str; 7] = ["#####", "#    ", "#### ", "    #", "    #", "#   #", " ### "];
const G6: [&str; 7] = [" ### ", "#    ", "#    ", "#### ", "#   #", "#   #", " ### "];
const G7: [&str; 7] = ["#####", "    #", "   # ", "  #  ", " #   ", "#    ", "#    "];
const G8: [&str; 7] = [" ### ", "#   #", "#   #", " ### ", "#   #", "#   #", " ### "];
const G9: [&str; 7] = [" ### ", "#   #", "#   #", " ####", "    #", "    #", " ### "];
const GCOLON: [&str; 7] = ["     ", "     ", "  #  ", "     ", "  #  ", "     ", "     "];
const GBLANK: [&str; 7] = ["     ", "     ", "     ", "     ", "     ", "     ", "     "];

/// The 5×7 glyph for `c`, or the blank glyph if unsupported.
fn glyph(c: char) -> &'static [&'static str; 7] {
    match c {
        '0' => &G0,
        '1' => &G1,
        '2' => &G2,
        '3' => &G3,
        '4' => &G4,
        '5' => &G5,
        '6' => &G6,
        '7' => &G7,
        '8' => &G8,
        '9' => &G9,
        ':' => &GCOLON,
        _ => &GBLANK,
    }
}

/// Visual form of [`BigText`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BigTextShape {
    /// Only lit dots (`█`, `accent`) — clean. The default.
    #[default]
    Glow,
    /// Lit dots `█` over unlit `░` — the full matrix.
    Grid,
}

/// A sci-fi dot-matrix banner.
///
/// Build with [`BigText::new`] (the text — digits / `:` / spaces render).
#[derive(Debug, Clone)]
pub struct BigText {
    /// The text to render.
    pub text: String,
    /// Glow vs grid form. Defaults to [`BigTextShape::Glow`].
    pub shape: BigTextShape,
    /// Theme whose [`Palette`](crate::Palette) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl BigText {
    /// Create a banner for `text`, default shape and theme.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            shape: BigTextShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the glow vs grid form (see [`BigTextShape`]).
    #[must_use]
    pub fn shape(mut self, shape: BigTextShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the banner.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

impl Widget for BigText {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        let p = self.theme.palette();
        let on = Style::new().fg(p.accent.color());
        let off = Style::new().fg(p.muted.color());
        let grid = matches!(self.shape, BigTextShape::Grid);

        let n = self.text.chars().count() as u16;
        if n == 0 {
            return;
        }
        let total_w = n * CHAR_W - 1; // no gap after the last char
        let start_x = area.x + area.width.saturating_sub(total_w.min(area.width)) / 2;
        let start_y = area.y + area.height.saturating_sub(CHAR_H.min(area.height)) / 2;

        for (ci, c) in self.text.chars().enumerate() {
            let rows = glyph(c);
            let char_x = start_x + (ci as u16) * CHAR_W;
            for (ry, row) in rows.iter().enumerate() {
                let py = start_y + ry as u16;
                if py >= area.bottom() {
                    break;
                }
                for (dx, dot) in row.chars().enumerate() {
                    let px = char_x + dx as u16;
                    if px >= area.right() {
                        break;
                    }
                    let lit = dot == '#';
                    if lit {
                        buf[(px, py)].set_char('█').set_style(on);
                    } else if grid {
                        buf[(px, py)].set_char('░').set_style(off);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 40;
    const H: u16 = 9;

    fn render(text: &str, shape: BigTextShape, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        BigText::new(text).shape(shape).theme(theme).render(Rect::new(0, 0, W, H), &mut buf);
        buf
    }

    fn lit_count(buf: &Buffer) -> usize {
        let mut n = 0;
        for x in 0..W {
            for y in 0..H {
                if buf[(x, y)].symbol() == "█" {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn renders_digits() {
        let buf = render("12", BigTextShape::Glow, Theme::Cyberpunk);
        assert!(lit_count(&buf) > 0, "digits light some cells");
    }

    #[test]
    fn glow_has_no_dim_dots() {
        let buf = render("8", BigTextShape::Glow, Theme::Cyberpunk);
        let has_dim = (0..W).any(|x| (0..H).any(|y| buf[(x, y)].symbol() == "░"));
        assert!(!has_dim, "Glow draws no unlit dots");
    }

    #[test]
    fn grid_has_dim_dots() {
        let buf = render("8", BigTextShape::Grid, Theme::Cyberpunk);
        let has_dim = (0..W).any(|x| (0..H).any(|y| buf[(x, y)].symbol() == "░"));
        assert!(has_dim, "Grid fills unlit dots with ░");
    }

    #[test]
    fn lit_dot_is_accent() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let buf = render("8", BigTextShape::Glow, Theme::Cyberpunk);
        let lit = (0..W)
            .flat_map(|x| (0..H).map(move |y| (x, y)))
            .find(|&(x, y)| buf[(x, y)].symbol() == "█")
            .expect("at least one lit dot");
        assert_eq!(buf[lit].fg, accent, "lit dot is --accent");
    }

    #[test]
    fn colon_renders() {
        // ":" has two lit dots; "0" has many more.
        let colon = render(":", BigTextShape::Glow, Theme::Cyberpunk);
        let zero = render("0", BigTextShape::Glow, Theme::Cyberpunk);
        assert!(lit_count(&colon) < lit_count(&zero), "colon is sparser than a digit");
        assert!(lit_count(&colon) > 0, "colon lights its two dots");
    }

    #[test]
    fn unknown_char_is_blank() {
        // 'A' is unsupported → renders no lit dots.
        let buf = render("A", BigTextShape::Glow, Theme::Cyberpunk);
        assert_eq!(lit_count(&buf), 0, "unsupported char renders blank");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        BigText::new("1").render(Rect::new(0, 0, 0, 0), &mut buf);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }
}
