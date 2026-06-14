//! RGB palettes for the four built-in sci-fi themes.
//!
//! [`Rgb`] is the single source of truth: both the ratatui [`Color`] it
//! produces and the CSS tokens injected into the [`ratatui_style`] stylesheet
//! are derived from these values, so the two paths never drift.

use ratatui::style::Color;

/// An RGB color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// The corresponding ratatui [`Color`].
    #[inline]
    pub const fn color(self) -> Color {
        Color::Rgb(self.0, self.1, self.2)
    }

    /// `#rrggbb`, as written into the `:root { --token: #… }` block.
    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

/// A complete sci-fi palette. Fields are **semantic**, not themed, so every
/// widget can refer to e.g. `palette.accent` regardless of which theme is
/// active.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Primary accent (the theme's signature color).
    pub accent: Rgb,
    /// Secondary accent for contrast / highlights.
    pub accent2: Rgb,
    /// App background.
    pub bg: Rgb,
    /// Panel / frame background.
    pub panel: Rgb,
    /// Primary foreground / text.
    pub fg: Rgb,
    /// Dimmed labels, scanlines, borders.
    pub muted: Rgb,
    /// "Ok" / nominal state.
    pub ok: Rgb,
    /// "Warn" state.
    pub warn: Rgb,
    /// "Alert" / danger state.
    pub alert: Rgb,
}

/// Cyberpunk — fluorescent pink / neon blue.
pub const CYBERPUNK: Palette = Palette {
    accent: Rgb(0xff, 0x00, 0x7f),
    accent2: Rgb(0x00, 0xf0, 0xff),
    bg: Rgb(0x08, 0x04, 0x14),
    panel: Rgb(0x14, 0x0a, 0x22),
    fg: Rgb(0xf0, 0xe6, 0xff),
    muted: Rgb(0x6a, 0x3a, 0x7a),
    ok: Rgb(0x39, 0xff, 0x14),
    warn: Rgb(0xff, 0xb0, 0x00),
    alert: Rgb(0xff, 0x20, 0x60),
};

/// Fallout Terminal — phosphor green on black.
pub const FALLOUT: Palette = Palette {
    accent: Rgb(0x33, 0xff, 0x33),
    accent2: Rgb(0x22, 0xcc, 0x22),
    bg: Rgb(0x00, 0x00, 0x00),
    panel: Rgb(0x05, 0x12, 0x05),
    fg: Rgb(0x33, 0xff, 0x33),
    muted: Rgb(0x12, 0x66, 0x12),
    ok: Rgb(0x33, 0xff, 0x33),
    warn: Rgb(0xcc, 0xdd, 0x22),
    alert: Rgb(0xff, 0x44, 0x44),
};

/// Weyland Console — amber gold / grey-white / dark red.
pub const WEYLAND: Palette = Palette {
    accent: Rgb(0xff, 0xb0, 0x00),
    accent2: Rgb(0xd0, 0xd0, 0xc8),
    bg: Rgb(0x0a, 0x08, 0x04),
    panel: Rgb(0x16, 0x10, 0x06),
    fg: Rgb(0xe6, 0xd8, 0xb0),
    muted: Rgb(0x70, 0x5a, 0x30),
    ok: Rgb(0xb0, 0xd0, 0x60),
    warn: Rgb(0xff, 0xb0, 0x00),
    alert: Rgb(0xb0, 0x20, 0x18),
};

/// Deep Space HUD — deep blue / white / alert red.
pub const DEEP_SPACE: Palette = Palette {
    accent: Rgb(0x00, 0x55, 0xff),
    accent2: Rgb(0x66, 0xaa, 0xff),
    bg: Rgb(0x02, 0x05, 0x12),
    panel: Rgb(0x06, 0x0e, 0x22),
    fg: Rgb(0xea, 0xf2, 0xff),
    muted: Rgb(0x34, 0x4a, 0x78),
    ok: Rgb(0x33, 0xdd, 0x88),
    warn: Rgb(0xff, 0xc0, 0x3a),
    alert: Rgb(0xff, 0x30, 0x30),
};
