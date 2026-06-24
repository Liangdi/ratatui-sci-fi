//! **ProgressBar** — continuous linear progress bar (with indeterminate mode).
//!
//! A single-row progress bar. In **determinate** mode (a `0.0..=1.0` ratio)
//! it fills left-to-right; in **indeterminate** mode (`ratio = None`) a lit
//! block scans back and forth — the "working, unknown duration" look.
//!
//! Unlike [`crate::EnergyGauge`] (segmented `▰▱`, threshold-colored, with a
//! percentage label), [`ProgressBar`] is a continuous fill with no label and an
//! indeterminate mode — the general-purpose progress indicator.
//!
//! ## Spec
//!
//! ```text
//!   ███████░░░░░     determinate, ratio 0.6
//!   ░░░███░░░░░░     indeterminate, block scanning
//! ```
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]: `ratio`/`shape` are configuration; only the
//!   animation clock (used by indeterminate mode) lives in [`ProgressBarState`].
//! - Colors come straight off the [`Palette`](crate::Palette): filled cells +
//!   the scanning block take `accent`, empty track takes `muted`.
//! - All glyphs are width-1.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{ProgressBar, ProgressBarState, Theme};
//!
//! let mut state = ProgressBarState::new();
//! let determinate = ProgressBar::new(Some(0.6)).theme(Theme::DeepSpace);
//! let indeterminate = ProgressBar::new(None).theme(Theme::DeepSpace); // scans on tick
//! ```

use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::StatefulWidget};

use crate::Theme;

/// Ticks per column the indeterminate block advances.
const INDETERMINATE_SPEED: u64 = 2;

/// Visual form of a [`ProgressBar`]'s fill/track glyphs.
///
/// Selects the `(filled, track)` glyph pair; colors stay on the palette,
/// untouched by this enum. The [`ProgressBarShape::Block`] default draws
/// `█`/`░`.
///
/// Every glyph is Unicode width-1 (see convention #5 at the crate root).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProgressBarShape {
    /// Filled `█`, track `░`.
    #[default]
    Block,
    /// Filled `▰`, track `▱`.
    Cell,
    /// Filled `=`, track `-`.
    Ascii,
}

impl ProgressBarShape {
    /// The filled-cell glyph.
    #[must_use]
    pub const fn filled(self) -> char {
        match self {
            Self::Block => '█',
            Self::Cell => '▰',
            Self::Ascii => '=',
        }
    }

    /// The empty-track glyph.
    #[must_use]
    pub const fn track(self) -> char {
        match self {
            Self::Block => '░',
            Self::Cell => '▱',
            Self::Ascii => '-',
        }
    }
}

/// A sci-fi progress bar.
///
/// Build with [`ProgressBar::new`], passing `Some(ratio)` for determinate fill
/// or `None` for indeterminate scanning. Set the theme with [`ProgressBar::theme`].
#[derive(Debug, Clone)]
pub struct ProgressBar {
    /// `Some(r)` (0.0..=1.0) fills determinately; `None` scans indefinitely.
    pub ratio: Option<f32>,
    /// Fill/track glyph form. Defaults to [`ProgressBarShape::Block`].
    pub shape: ProgressBarShape,
    /// Theme whose [`Palette`](crate::Palette) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl ProgressBar {
    /// Create a progress bar. `Some(ratio)` = determinate; `None` = indeterminate.
    pub fn new(ratio: Option<f32>) -> Self {
        Self {
            ratio,
            shape: ProgressBarShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the fill/track glyph form (see [`ProgressBarShape`]).
    #[must_use]
    pub fn shape(mut self, shape: ProgressBarShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the bar.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

/// Mutable state for [`ProgressBar`].
///
/// `tick` drives the indeterminate scan; the app advances it each frame (or
/// calls [`Self::tick`]). Unused in determinate mode.
#[derive(Debug, Default, Clone)]
pub struct ProgressBarState {
    /// Animation clock, advanced once per frame (indeterminate scan only).
    pub tick: u64,
}

impl ProgressBarState {
    /// Create a state at tick 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance the clock one tick.
    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }
}

impl StatefulWidget for ProgressBar {
    type State = ProgressBarState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }

        let p = self.theme.palette();
        let on = Style::new().fg(p.accent.color());
        let off = Style::new().fg(p.muted.color());
        let y = area.y + area.height / 2;
        let w = area.width;

        match self.ratio {
            Some(ratio) => {
                let filled = (ratio.clamp(0.0, 1.0) * w as f32).round() as u16;
                for i in 0..w {
                    let (g, s) = if i < filled { (self.shape.filled(), on) } else { (self.shape.track(), off) };
                    buf[(area.x + i, y)].set_char(g).set_style(s);
                }
            }
            None => {
                // Indeterminate: a lit block ping-pongs across the track.
                let block_w = (w / 4).max(1);
                let span = w.saturating_sub(block_w) as u64; // leftmost..rightmost block start
                let period = 2 * span;
                let phase = (state.tick / INDETERMINATE_SPEED) % period.max(1);
                let pos = if phase < span { phase } else { period - phase };
                let pos = (pos as u16).min(w.saturating_sub(block_w));
                for i in 0..w {
                    let in_block = i >= pos && i < pos + block_w;
                    let (g, s) = if in_block { (self.shape.filled(), on) } else { (self.shape.track(), off) };
                    buf[(area.x + i, y)].set_char(g).set_style(s);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    const W: u16 = 12;
    const H: u16 = 3;

    fn render(ratio: Option<f32>, tick: u64, theme: Theme, shape: ProgressBarShape) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let mut state = ProgressBarState { tick };
        StatefulWidget::render(
            ProgressBar::new(ratio).shape(shape).theme(theme),
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
    fn determinate_fills_proportionally() {
        // ratio 0.5, width 12 → 6 filled.
        let buf = render(Some(0.5), 0, Theme::Cyberpunk, ProgressBarShape::Block);
        let y = H / 2;
        for i in 0..6 {
            assert_eq!(buf[(i, y)].symbol(), "█", "cell {i} filled");
        }
        for i in 6..12 {
            assert_eq!(buf[(i, y)].symbol(), "░", "cell {i} empty track");
        }
    }

    #[test]
    fn determinate_clamps_over_one() {
        // ratio 2.0 → clamps to 1.0 → all filled.
        let buf = render(Some(2.0), 0, Theme::Cyberpunk, ProgressBarShape::Block);
        let text = row_text(&buf, H / 2);
        assert!(!text.contains('░'), "over-1 ratio fills everything: {text:?}");
    }

    #[test]
    fn determinate_filled_uses_accent() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let buf = render(Some(0.5), 0, Theme::Cyberpunk, ProgressBarShape::Block);
        assert_eq!(buf[(0, H / 2)].fg, accent, "filled cell is --accent");
    }

    #[test]
    fn determinate_track_uses_muted() {
        let muted = Theme::Cyberpunk.palette().muted.color();
        let buf = render(Some(0.5), 0, Theme::Cyberpunk, ProgressBarShape::Block);
        assert_eq!(buf[(11, H / 2)].fg, muted, "track cell is --muted");
    }

    #[test]
    fn indeterminate_block_moves_with_tick() {
        // block_w = 12/4 = 3, span = 9. tick 0 → block at [0,3); tick 10 (phase
        // 5) → block at [5,8). So cell 0 is filled at tick 0, track at tick 10.
        let at0 = render(None, 0, Theme::Cyberpunk, ProgressBarShape::Block);
        let at10 = render(None, 10, Theme::Cyberpunk, ProgressBarShape::Block);
        let y = H / 2;
        assert_eq!(at0[(0, y)].symbol(), "█", "block starts at the left edge at tick 0");
        assert_eq!(at10[(0, y)].symbol(), "░", "block has moved off cell 0 by tick 10");
    }

    #[test]
    fn indeterminate_ping_pongs_not_overflow() {
        // A large tick must wrap and keep the block in-bounds (no panic, all
        // cells are either fill or track).
        let buf = render(None, u64::MAX / 2, Theme::Cyberpunk, ProgressBarShape::Block);
        let text = row_text(&buf, H / 2);
        for ch in text.chars() {
            assert!(ch == '█' || ch == '░', "every cell is fill or track: {text:?}");
        }
    }

    #[test]
    fn cell_shape_uses_cell_glyphs() {
        let buf = render(Some(0.5), 0, Theme::Cyberpunk, ProgressBarShape::Cell);
        let y = H / 2;
        assert_eq!(buf[(0, y)].symbol(), "▰", "Cell filled is '▰'");
        assert_eq!(buf[(11, y)].symbol(), "▱", "Cell track is '▱'");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let mut state = ProgressBarState::new();
        StatefulWidget::render(
            ProgressBar::new(Some(0.5)),
            Rect::new(0, 0, 0, 0),
            &mut buf,
            &mut state,
        );
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }

    #[test]
    fn tick_wraps_without_panic() {
        let mut s = ProgressBarState { tick: u64::MAX };
        s.tick();
        assert_eq!(s.tick, 0);
    }
}
