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

/// Bloodmoon — crimson / ember on near-black red, war-room / alarm console.
pub const BLOODMOON: Palette = Palette {
    accent: Rgb(0xff, 0x33, 0x44),
    accent2: Rgb(0xff, 0x88, 0x55),
    bg: Rgb(0x0c, 0x02, 0x04),
    panel: Rgb(0x1c, 0x06, 0x08),
    fg: Rgb(0xff, 0xe2, 0xdc),
    muted: Rgb(0x80, 0x2e, 0x32),
    ok: Rgb(0x66, 0xee, 0x77),
    warn: Rgb(0xff, 0xc0, 0x3a),
    alert: Rgb(0xff, 0x22, 0x2a),
};

/// Nebula — violet / ice-cyan on indigo-black, iridescent holographic UI.
pub const NEBULA: Palette = Palette {
    accent: Rgb(0xbb, 0x66, 0xff),
    accent2: Rgb(0x66, 0xee, 0xff),
    bg: Rgb(0x07, 0x04, 0x12),
    panel: Rgb(0x14, 0x0c, 0x24),
    fg: Rgb(0xee, 0xe6, 0xff),
    muted: Rgb(0x5e, 0x48, 0x8c),
    ok: Rgb(0x55, 0xff, 0xaa),
    warn: Rgb(0xff, 0xcc, 0x44),
    alert: Rgb(0xff, 0x44, 0x88),
};

/// Arctic — aqua-teal / pale ice on cold black, cryo-lab / polar station HUD.
pub const ARCTIC: Palette = Palette {
    accent: Rgb(0x44, 0xee, 0xdd),
    accent2: Rgb(0xaa, 0xee, 0xff),
    bg: Rgb(0x02, 0x0a, 0x0c),
    panel: Rgb(0x06, 0x16, 0x1a),
    fg: Rgb(0xe6, 0xf6, 0xff),
    muted: Rgb(0x2e, 0x60, 0x6c),
    ok: Rgb(0x44, 0xff, 0x99),
    warn: Rgb(0xff, 0xd0, 0x44),
    alert: Rgb(0xff, 0x44, 0x66),
};

/// Sentinel — monochrome white / silver on charcoal, stealth / minimalist console.
pub const SENTINEL: Palette = Palette {
    accent: Rgb(0xe8, 0xe8, 0xec),
    accent2: Rgb(0x9a, 0x9a, 0xa6),
    bg: Rgb(0x04, 0x04, 0x06),
    panel: Rgb(0x10, 0x10, 0x14),
    fg: Rgb(0xd8, 0xd8, 0xde),
    muted: Rgb(0x4e, 0x4e, 0x58),
    ok: Rgb(0x66, 0xee, 0x88),
    warn: Rgb(0xff, 0xcc, 0x3a),
    alert: Rgb(0xff, 0x55, 0x55),
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Every shipped palette must expose a distinct signature `accent`, so
    /// themes never collide visually and a copy/paste typo is caught early.
    #[test]
    fn accents_are_pairwise_distinct() {
        let accents = [
            CYBERPUNK.accent,
            FALLOUT.accent,
            WEYLAND.accent,
            DEEP_SPACE.accent,
            BLOODMOON.accent,
            NEBULA.accent,
            ARCTIC.accent,
            SENTINEL.accent,
        ];
        for i in 0..accents.len() {
            for j in (i + 1)..accents.len() {
                assert_ne!(
                    accents[i], accents[j],
                    "duplicate accent between palettes #{i} and #{j}"
                );
            }
        }
    }

    /// `hex()` must round-trip back to the same `Color` for the token block.
    #[test]
    fn hex_roundtrips() {
        let rgb = BLOODMOON.accent;
        let parsed = u32::from_str_radix(&rgb.hex()[1..], 16).unwrap();
        let roundtrip = Rgb(
            ((parsed >> 16) & 0xff) as u8,
            ((parsed >> 8) & 0xff) as u8,
            (parsed & 0xff) as u8,
        );
        assert_eq!(rgb, roundtrip);
    }
}
