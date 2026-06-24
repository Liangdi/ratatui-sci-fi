//! **ScanlineOverlay** — a CRT scanline + vignette pass over the whole screen.
//!
//! A full-area overlay (render it **last**, over everything else) that paints
//! the classic CRT look: a bright scanline sweeping top-to-bottom, plus an
//! optional darkened vignette around the edges. Unlike every other widget here
//! it is an *ambient layer*, not a control — drop it on the root area at the
//! end of `draw` to make the whole UI read as an old phosphor terminal.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]: only the sweep clock lives in
//!   [`ScanlineOverlayState`].
//! - The sweep paints one row's background `accent` (the bright scanline);
//!   the vignette paints edge cells' background `panel` (darker). Foreground
//!   glyphs are untouched, so content stays readable — only its backdrop
//!   shifts. This is the only way to "tint" cells in a terminal (no blending).
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{ScanlineOverlay, ScanlineOverlayState, ScanlineShape, Theme};
//!
//! let mut state = ScanlineOverlayState::new();
//! // at the end of draw, over the root area:
//! // f.render_stateful_widget(ScanlineOverlay::new(ScanlineShape::SweepAndVignette)
//! //     .theme(theme), f.area(), &mut state);
//! ```

use ratatui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};

use crate::Theme;

/// Ticks per row the sweep line advances.
const SWEEP_SPEED: u64 = 2;
/// Edge depth (in cells) the vignette darkens.
const VIGNETTE_RADIUS: u16 = 2;

/// Visual form of a [`ScanlineOverlay`].
///
/// - [`Sweep`](ScanlineShape::Sweep): just the moving bright scanline.
/// - [`SweepAndVignette`](ScanlineShape::SweepAndVignette): the sweep plus
///   darkened edges.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ScanlineShape {
    /// A moving bright scanline only.
    #[default]
    Sweep,
    /// The sweep plus a darkened vignette around the edges.
    SweepAndVignette,
}

/// A CRT scanline + vignette overlay.
///
/// Build with [`ScanlineOverlay::new`] (the shape), then set the theme. Render
/// it last, over the full screen area.
#[derive(Debug, Clone)]
pub struct ScanlineOverlay {
    /// Overlay form. Defaults to [`ScanlineShape::Sweep`].
    pub shape: ScanlineShape,
    /// Theme whose [`Palette`](crate::Palette) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl ScanlineOverlay {
    /// Create an overlay with the given shape, default theme.
    pub fn new(shape: ScanlineShape) -> Self {
        Self {
            shape,
            theme: Theme::Cyberpunk,
        }
    }

    /// Set the theme used for the overlay colors.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

/// Mutable state for [`ScanlineOverlay`].
///
/// `tick` drives the sweep; the app advances it each frame (or calls
/// [`Self::tick`]).
#[derive(Debug, Default, Clone)]
pub struct ScanlineOverlayState {
    /// Sweep clock, advanced once per frame.
    pub tick: u64,
}

impl ScanlineOverlayState {
    /// Create a state at tick 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance the sweep clock one tick.
    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }
}

impl StatefulWidget for ScanlineOverlay {
    type State = ScanlineOverlayState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }

        let accent = self.theme.palette().accent.color();
        let panel = self.theme.palette().panel.color();
        let vignette = matches!(self.shape, ScanlineShape::SweepAndVignette);

        // The sweep row cycles top→bottom→top.
        let sweep_row = ((state.tick / SWEEP_SPEED) % area.height as u64) as u16;

        for row in 0..area.height {
            for col in 0..area.width {
                let cell = &mut buf[(area.x + col, area.y + row)];
                if row == sweep_row {
                    // Bright scanline: accent backdrop over the whole row.
                    cell.set_bg(accent);
                } else if vignette {
                    // Vignette: darken cells within VIGNETTE_RADIUS of any edge.
                    let edge = col
                        .min(row)
                        .min(area.width - 1 - col)
                        .min(area.height - 1 - row);
                    if edge < VIGNETTE_RADIUS {
                        cell.set_bg(panel);
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

    const W: u16 = 12;
    const H: u16 = 8;

    fn render(shape: ScanlineShape, tick: u64, theme: Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        let mut state = ScanlineOverlayState { tick };
        StatefulWidget::render(
            ScanlineOverlay::new(shape).theme(theme),
            Rect::new(0, 0, W, H),
            &mut buf,
            &mut state,
        );
        buf
    }

    #[test]
    fn sweep_paints_one_row_accent() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let buf = render(ScanlineShape::Sweep, 0, Theme::Cyberpunk);
        // tick 0, speed 2 → sweep row 0. Every cell of row 0 is accent-bg.
        for col in 0..W {
            assert_eq!(buf[(col, 0)].bg, accent, "sweep row 0 col {col} is accent");
        }
        // Row 1 (not the sweep) is untouched.
        assert_ne!(buf[(0, 1)].bg, accent, "row 1 is not the sweep");
    }

    #[test]
    fn sweep_moves_with_tick() {
        let accent = Theme::Cyberpunk.palette().accent.color();
        let at0 = render(ScanlineShape::Sweep, 0, Theme::Cyberpunk);
        let at4 = render(ScanlineShape::Sweep, 4, Theme::Cyberpunk);
        // tick 4, speed 2 → sweep row 2.
        assert_eq!(at0[(0, 0)].bg, accent, "tick 0 → row 0");
        assert_eq!(at4[(0, 2)].bg, accent, "tick 4 → row 2");
        assert_ne!(at4[(0, 0)].bg, accent, "row 0 no longer swept at tick 4");
    }

    #[test]
    fn sweep_only_does_not_vignette() {
        // Sweep shape leaves edge cells' bg at the default (Reset).
        let buf = render(ScanlineShape::Sweep, 5, Theme::Cyberpunk);
        // Corner (0,0) — if it were the sweep row it'd be accent, so pick a tick
        // where row 0 isn't swept (tick 5, speed 2 → row 1).
        assert_eq!(buf[(0, 0)].bg, ratatui::style::Color::Reset, "no vignette in Sweep");
    }

    #[test]
    fn vignette_darkens_edges() {
        let panel = Theme::Cyberpunk.palette().panel.color();
        let buf = render(ScanlineShape::SweepAndVignette, 5, Theme::Cyberpunk);
        // Corner cell (0,0) is within the vignette radius → panel bg.
        assert_eq!(buf[(0, 0)].bg, panel, "corner darkened by vignette");
    }

    #[test]
    fn vignette_leaves_center_untouched() {
        // Center cell (6,4) is far from any edge → default bg.
        let buf = render(ScanlineShape::SweepAndVignette, 5, Theme::Cyberpunk);
        // Pick a tick where the sweep isn't on row 4 (tick 5 → row 1).
        assert_eq!(buf[(6, 4)].bg, ratatui::style::Color::Reset, "center untouched");
    }

    #[test]
    fn empty_area_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let mut state = ScanlineOverlayState::new();
        StatefulWidget::render(
            ScanlineOverlay::new(ScanlineShape::SweepAndVignette),
            Rect::new(0, 0, 0, 0),
            &mut buf,
            &mut state,
        );
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }

    #[test]
    fn tick_advances_clock() {
        let mut s = ScanlineOverlayState::new();
        s.tick();
        assert_eq!(s.tick, 1);
    }
}
