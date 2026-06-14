//! Built-in sci-fi themes.
//!
//! Each theme exposes a [`Palette`] (raw ratatui `Color`s — for widgets that
//! draw directly, e.g. on a `Canvas`) and a [`ratatui_style::Stylesheet`]
//! (CSS-driven styling, the architecture's primary path). Both are derived
//! from the same RGB source of truth in [`palette`], so they never drift.

pub mod palette;
mod stylesheet;

pub use palette::{Palette, Rgb, CYBERPUNK, DEEP_SPACE, FALLOUT, WEYLAND};
pub use stylesheet::stylesheet;

/// The four built-in sci-fi themes.
///
/// `Cyberpunk` is the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    /// Fluorescent pink / neon blue — cyberpunk, night-city, hi-tech neon.
    #[default]
    Cyberpunk,
    /// Classic phosphor green on black — wasteland, retro mainframe, Pip-Boy.
    Fallout,
    /// Amber gold / grey-white / dark red — *Alien*-style industrial console.
    Weyland,
    /// Deep blue / white / alert red — modern starship, minimalist flight HUD.
    DeepSpace,
}

impl Theme {
    /// Raw color palette — use for direct `Canvas` / `Color` work.
    pub const fn palette(self) -> Palette {
        match self {
            Theme::Cyberpunk => CYBERPUNK,
            Theme::Fallout => FALLOUT,
            Theme::Weyland => WEYLAND,
            Theme::DeepSpace => DEEP_SPACE,
        }
    }

    /// CSS cascade stylesheet ([`ratatui_style`]). Widgets query it via
    /// `sheet.compute_with(&NodeRef::new("Type").classes(&[...]), None, &mut scratch)`.
    pub fn stylesheet(self) -> &'static ratatui_style::Stylesheet {
        stylesheet::stylesheet(self)
    }
}
