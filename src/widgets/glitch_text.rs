//! **GlitchText** — intermittent character-substitution glitch (PRD §3 故障文本).
//!
//! ## Spec
//! - Randomly, briefly replaces individual characters in the text, evoking
//!   signal interference, bad tracks, or decode failure.
//!
//! ## Implementation notes
//! - Stateful [`StatefulWidget`]; a glitch mask + countdown live in
//!   [`GlitchTextState`]. Every few ticks, re-roll which positions glitch and
//!   from what pool; non-glitch ticks show the clean text.
//! - RNG: a small xorshift32 PRNG is kept in [`GlitchTextState`] (fixed seed in
//!   [`Default`]) so test output is fully deterministic.
//! - **Styling.** Colors are sourced through the theme's [`Stylesheet`]
//!   cascade rather than read off the [`Palette`](crate::Palette) directly:
//!   clean characters resolve via the `Glitch` rule (`var(--fg)`) and glitched
//!   characters via `Glitch.corrupt` (`var(--alert)`, the danger color — the
//!   glitch reads as corruption / signal failure). Because both rules are
//!   `var(--…)`-driven off the same palette, the rendered colors are unchanged
//!   from the previous direct-palette reads; swap `--alert` for `--accent2`
//!   (or add a new rule) if a calmer "static" feel is ever wanted.
//!
//! # Example
//!
//! ```no_run
//! use ratatui_sci_fi::{GlitchText, GlitchTextState, Theme};
//!
//! let mut state = GlitchTextState::default();
//! let widget = GlitchText::new("DECRYPTING")
//!     .intensity(0.25)
//!     .theme(Theme::Cyberpunk);
//! // in your event loop: state.tick(); each frame before render.
//! ```

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::StatefulWidget,
};
use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Pool of width-1 glyphs used to replace characters during a glitch burst.
///
/// Half-width katakana + ASCII noise glyphs — the classic "corrupted decode"
/// look. All members are single cell wide so they slot cleanly into one
/// [`Buffer`] cell.
pub const GLYPH_POOL: &[char] = &[
    'ﾊ', 'ﾐ', 'ﾋ', 'ｰ', 'ｳ', 'ｼ', 'ﾅ', 'ﾓ', 'ﾆ', 'ｻ', 'ﾜ', 'ﾂ', '#', '%', '&', '!', '?', '0', '1',
    '<', '>', '*', '+', '=', '~',
];

/// Visual form of a [`GlitchText`]'s corruption glyphs.
///
/// Selects the pool of width-1 glyphs drawn during a glitch burst. Colors stay
/// on the CSS cascade (`Glitch` / `Glitch.corrupt`), untouched by this enum.
/// The [`GlitchShape::KatakanaNoise`] default returns the existing
/// [`GLYPH_POOL`] const — reproducing the original "corrupted decode" look
/// byte-for-byte, so existing tests pass unchanged.
///
/// Every member glyph is Unicode width-1 (see convention #5 at the crate
/// root), so each corruption glyph slots cleanly into one [`Buffer`] cell.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GlitchShape {
    /// Half-width katakana + ASCII noise — the original look. Returns the
    /// existing [`GLYPH_POOL`] const.
    #[default]
    KatakanaNoise,
    /// Binary noise — only `'0'` and `'1'`.
    Binary,
    /// Hexadecimal noise — `0-9` and `A-F`.
    Hex,
}

impl GlitchShape {
    /// The glyph pool this shape draws corruption glyphs from.
    #[must_use]
    pub const fn pool(self) -> &'static [char] {
        match self {
            Self::KatakanaNoise => GLYPH_POOL,
            Self::Binary => &['0', '1'],
            Self::Hex => &[
                '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F',
            ],
        }
    }
}

/// How many [`GlitchTextState::tick`] calls elapse between glitch re-rolls.
///
/// A re-roll happens every `REROLL_PERIOD` ticks: the mask of glitched positions
/// and the glyph each shows are recomputed and then held stable (for
/// [`BURST_LIFE`] ticks) so a burst "freezes" briefly rather than flickering
/// every frame.
pub const REROLL_PERIOD: u32 = 6;

/// How many ticks a freshly rolled glitch burst stays visible before the text
/// returns to clean. Kept `< REROLL_PERIOD` so the burst fully fades before the
/// next re-roll.
pub const BURST_LIFE: u32 = 3;

/// Default expected fraction of character positions that glitch during a burst.
pub const DEFAULT_INTENSITY: f32 = 0.2;

/// Fixed seed for [`GlitchTextState`]'s xorshift32 PRNG, so output is
/// deterministic across runs and tests.
pub const DEFAULT_SEED: u32 = 0x9E37_79B9;

/// Glitching text (PRD §3 故障文本).
///
/// Built from a string plus an optional [`intensity`](Self::intensity) (expected
/// fraction of glitched positions) and [`theme`](Self::theme) (default
/// [`Theme::Cyberpunk`]). All animation lives in the companion
/// [`GlitchTextState`], advanced by the app's event loop each tick.
#[derive(Debug, Clone)]
pub struct GlitchText {
    /// The text to display (and corrupt during glitches).
    pub text: String,
    /// Expected fraction of character positions glitched during a burst,
    /// in `0.0..=1.0`. Higher = denser corruption.
    pub intensity: f32,
    /// Glyph-pool form for corruption (see [`GlitchShape`]). Defaults to
    /// [`GlitchShape::KatakanaNoise`], the original katakana+ASCII look.
    pub shape: GlitchShape,
    /// Active theme; controls all colors via its [`Palette`](crate::Palette).
    pub theme: Theme,
}

impl Default for GlitchText {
    fn default() -> Self {
        Self {
            text: String::new(),
            intensity: DEFAULT_INTENSITY,
            shape: GlitchShape::default(),
            theme: Theme::default(),
        }
    }
}

impl GlitchText {
    /// Build a glitch widget from anything stringifiable.
    pub fn new(text: impl Into<String>) -> Self {
        Self::default().text(text)
    }

    /// Set the text (builder).
    #[must_use]
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    /// Set the glitch intensity — expected fraction of positions corrupted
    /// during a burst, clamped to `0.0..=1.0` (builder).
    #[must_use]
    pub fn intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity.clamp(0.0, 1.0);
        self
    }

    /// Set the glyph-pool form for corruption (see [`GlitchShape`]) (builder).
    #[must_use]
    pub fn shape(mut self, shape: GlitchShape) -> Self {
        self.shape = shape;
        self
    }

    /// Replace the theme (builder). Default is [`Theme::Cyberpunk`].
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

/// Mutable state for [`GlitchText`].
///
/// Holds a re-roll countdown, a tiny xorshift32 PRNG (fixed seed in
/// [`Default`] for deterministic tests), and the currently-active glitch burst
/// (which positions are corrupted and what glyph each shows). The app calls
/// [`tick`](Self::tick) once per frame; `render` is stable between ticks because
/// it only reads the stored burst.
#[derive(Debug, Clone)]
pub struct GlitchTextState {
    /// xorshift32 PRNG state. Always non-zero.
    prng: u32,
    /// Monotonic tick counter; a re-roll fires every `REROLL_PERIOD` ticks.
    tick: u32,
    /// Remaining ticks the current burst is visible for. `0` => show clean text.
    life: u32,
    /// Text length the current `burst` was rolled for. When the widget's text
    /// length changes between frames, the burst is re-rolled lazily on the next
    /// render so positions stay in range.
    burst_len: usize,
    /// `(position, glyph)` pairs currently corrupted. Stable between re-rolls.
    burst: Vec<(usize, char)>,
    /// Cached glyph pool the burst draws from. Synced from the widget's
    /// [`GlitchShape`] during render; defaults to [`GLYPH_POOL`] (the
    /// [`GlitchShape::KatakanaNoise`] default) so output is byte-identical.
    glyph_pool: &'static [char],
}

impl Default for GlitchTextState {
    fn default() -> Self {
        Self {
            prng: DEFAULT_SEED,
            tick: 0,
            life: 0,
            burst_len: 0,
            burst: Vec::new(),
            glyph_pool: GLYPH_POOL,
        }
    }
}

impl GlitchTextState {
    /// Advance the animation clock by one tick.
    ///
    /// Every `REROLL_PERIOD` ticks this schedules a fresh glitch burst (giving
    /// it [`BURST_LIFE`] ticks of visibility). The actual roll is materialized
    /// lazily in `render`, where the text length is known. Between re-rolls the
    /// burst counts down; at zero the text renders clean.
    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);

        // Count down any active burst.
        if self.life > 0 {
            self.life -= 1;
        }

        // Schedule a re-roll on the period boundary: clear the stored burst and
        // grant a fresh life budget. render() will repopulate `burst` for the
        // current text length on its next call.
        if self.tick.is_multiple_of(REROLL_PERIOD) {
            self.burst.clear();
            self.burst_len = 0;
            self.life = BURST_LIFE;
        }
    }

    /// Ensure `burst` holds a valid roll for a text of `len` chars at the given
    /// `intensity`, rolling a fresh one if the stored burst is stale (cleared by
    /// a re-roll, or rolled for a different length).
    fn ensure_burst(&mut self, len: usize, intensity: f32) {
        if self.burst_len == len && (!self.burst.is_empty() || intensity <= 0.0 || len == 0) {
            return;
        }
        self.roll_burst(len, intensity);
    }

    /// Roll a fresh burst: pick which of `len` positions glitch (each chosen
    /// independently with probability `intensity`) and assign each a random
    /// glyph from the cached glyph pool ([`self.glyph_pool`], synced from the
    /// widget's [`GlitchShape`]).
    fn roll_burst(&mut self, len: usize, intensity: f32) {
        self.burst.clear();
        self.burst_len = len;
        if len == 0 || intensity <= 0.0 {
            return;
        }
        let pool = self.glyph_pool;
        for pos in 0..len {
            if self.next_f32() < intensity {
                let glyph = pool[(self.next_u32() as usize) % pool.len()];
                self.burst.push((pos, glyph));
            }
        }
    }

    /// Next 32-bit pseudo-random value (xorshift32). State must be non-zero.
    #[inline]
    fn next_u32(&mut self) -> u32 {
        let mut x = self.prng;
        debug_assert!(x != 0, "xorshift32 state must be non-zero");
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.prng = x;
        x
    }

    /// Next float in `0.0..1.0`.
    #[inline]
    fn next_f32(&mut self) -> f32 {
        // High 24 bits into the mantissa => uniform in [0, 1).
        (self.next_u32() >> 8) as f32 / ((1u32 << 24) - 1) as f32
    }
}

impl StatefulWidget for GlitchText {
    type State = GlitchTextState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Guard zero-size areas.
        if area.width == 0 || area.height == 0 {
            return;
        }
        if self.text.is_empty() {
            return;
        }

        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let clean_style = sheet
            .compute_with(&NodeRef::new("Glitch"), None, &mut scratch)
            .to_style();
        let glitch_style = sheet
            .compute_with(&NodeRef::new("Glitch").classes(&["corrupt"]), None, &mut scratch)
            .to_style();

        // Pre-collect chars so we can index without re-borrowing `self.text`.
        let chars: Vec<char> = self.text.chars().collect();

        // Sync the glyph pool from the widget's shape into the state before any
        // burst (re-)roll. KatakanaNoise (default) caches GLYPH_POOL, so this
        // preserves byte-identical output for the default case.
        state.glyph_pool = self.shape.pool();

        // If a burst is active, ensure it is materialized for this text length.
        if state.life > 0 {
            state.ensure_burst(chars.len(), self.intensity);
        }

        let row_y = area.y;
        for (i, ch) in chars.iter().enumerate() {
            let col = area.x + i as u16;
            if col >= area.right() {
                break;
            }

            // Look up whether this position glitches this burst. Only glitched
            // when life > 0; clean text otherwise.
            let glyph = if state.life > 0 {
                state
                    .burst
                    .iter()
                    .find(|(p, _)| *p == i)
                    .map(|(_, g)| *g)
            } else {
                None
            };

            let (symbol, style) = match glyph {
                Some(g) => (g, glitch_style),
                None => (*ch, clean_style),
            };

            let cell = &mut buf[(col, row_y)];
            cell.set_char(symbol).set_style(style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    /// Helper: render the widget into a fresh buffer, returning it.
    fn render(text: &str, intensity: f32, theme: Theme, ticks: u32, width: u16, height: u16) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        let widget = GlitchText::new(text).intensity(intensity).theme(theme);
        let mut state = GlitchTextState::default();
        for _ in 0..ticks {
            state.tick();
        }
        StatefulWidget::render(widget, Rect::new(0, 0, width, height), &mut buf, &mut state);
        buf
    }

    /// Snapshot the symbols of row 0 across `width` cells.
    fn row_symbols(buf: &Buffer, width: u16) -> Vec<String> {
        (0..width).map(|x| buf[(x, 0)].symbol().to_string()).collect()
    }

    #[test]
    fn clean_render_uses_fg_and_original_text() {
        // 0 ticks: life == 0, no burst -> clean text.
        let buf = render("HELLO", 0.5, Theme::Cyberpunk, 0, 8, 1);
        let fg = Theme::Cyberpunk.palette().fg.color();
        assert_eq!(buf[(0, 0)].symbol(), "H");
        assert_eq!(buf[(4, 0)].symbol(), "O");
        assert_eq!(buf[(0, 0)].style().fg, Some(fg), "clean text uses palette.fg");
    }

    #[test]
    fn glitched_render_differs_from_clean() {
        // 1 tick: tick%REROLL_PERIOD(6)==1 -> NOT a re-roll boundary. So the
        // burst is *not* scheduled at tick 1. Use a tick count that lands on a
        // re-roll boundary (REROLL_PERIOD itself) to guarantee life>0.
        let clean = row_symbols(&render("HELLO", 1.0, Theme::Cyberpunk, 0, 5, 1), 5);
        // tick count == REROLL_PERIOD -> tick%PERIOD==0 -> re-roll scheduled.
        let glitched = row_symbols(&render("HELLO", 1.0, Theme::Cyberpunk, REROLL_PERIOD, 5, 1), 5);

        assert_eq!(clean, vec!["H", "E", "L", "L", "O"], "0 ticks renders clean");
        assert_ne!(
            glitched, clean,
            "intensity 1.0 after a re-roll boundary tick must corrupt"
        );
        // Every glitched cell must be a member of the glyph pool.
        for s in &glitched {
            let c = s.chars().next().expect("non-empty symbol");
            assert!(GLYPH_POOL.contains(&c), "{c:?} should be from GLYPH_POOL");
        }
    }

    #[test]
    fn glitched_cells_use_alert_color() {
        let buf = render("HELLO", 1.0, Theme::Cyberpunk, REROLL_PERIOD, 5, 1);
        let alert = Theme::Cyberpunk.palette().alert.color();
        assert_eq!(
            buf[(0, 0)].style().fg,
            Some(alert),
            "glitched cells use palette.alert"
        );
    }

    #[test]
    fn binary_shape_corrupts_to_zeroes_and_ones() {
        // The Binary shape restricts corruption glyphs to '0'/'1'. With
        // intensity 1.0 after a re-roll boundary tick, every cell must corrupt
        // and every corrupted glyph must be '0' or '1' — and the output must
        // differ from the clean text.
        let clean = row_symbols(&render("HELLO", 1.0, Theme::Cyberpunk, 0, 5, 1), 5);
        assert_eq!(clean, vec!["H", "E", "L", "L", "O"]);

        // Render with the Binary shape via a dedicated widget/state pair.
        let mut buf = Buffer::empty(Rect::new(0, 0, 5, 1));
        let widget = GlitchText::new("HELLO")
            .intensity(1.0)
            .shape(GlitchShape::Binary);
        let mut state = GlitchTextState::default();
        for _ in 0..REROLL_PERIOD {
            state.tick();
        }
        StatefulWidget::render(widget, Rect::new(0, 0, 5, 1), &mut buf, &mut state);
        let glitched = row_symbols(&buf, 5);

        assert_ne!(
            glitched, clean,
            "Binary shape after a re-roll boundary tick must corrupt"
        );
        for s in &glitched {
            let c = s.chars().next().expect("non-empty symbol");
            assert!(
                c == '0' || c == '1',
                "Binary shape may only emit '0'/'1', got {c:?}"
            );
        }
    }

    #[test]
    fn determinism_same_seed_same_output() {
        // Two independent states with the same seed + same tick count must
        // produce identical buffers.
        let a = row_symbols(&render("DECODE", 0.5, Theme::Weyland, REROLL_PERIOD, 6, 1), 6);
        let b = row_symbols(&render("DECODE", 0.5, Theme::Weyland, REROLL_PERIOD, 6, 1), 6);
        assert_eq!(a, b, "fixed seed => deterministic glitch output");
    }

    #[test]
    fn clean_returns_after_burst_life_expires() {
        // tick REROLL_PERIOD (6) -> re-roll, life=3 (visible at ticks 6,7,8;
        // tick 9 has life==0). Render happens *after* the tick loop, so:
        //   ticks=6 -> life=3 (visible)
        //   ticks=9 -> life counted down to 0 (clean)
        let burst = row_symbols(&render("HELLO", 1.0, Theme::Cyberpunk, REROLL_PERIOD, 5, 1), 5);
        let clean_again = row_symbols(&render("HELLO", 1.0, Theme::Cyberpunk, REROLL_PERIOD + BURST_LIFE, 5, 1), 5);
        assert_ne!(burst, vec!["H", "E", "L", "L", "O"], "burst at re-roll boundary corrupts");
        assert_eq!(
            clean_again,
            vec!["H", "E", "L", "L", "O"],
            "clean again once life hits 0"
        );
    }

    #[test]
    fn zero_size_area_is_noop() {
        // Zero width must not panic and must not write outside the area.
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let widget = GlitchText::new("X").intensity(1.0);
        let mut state = GlitchTextState::default();
        state.tick();
        StatefulWidget::render(widget, Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        // Buffer is empty/zero-sized; nothing to assert beyond "didn't panic".
    }

    #[test]
    fn empty_text_is_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 4, 1));
        let widget = GlitchText::new("").intensity(1.0);
        let mut state = GlitchTextState::default();
        state.tick();
        StatefulWidget::render(widget, Rect::new(0, 0, 4, 1), &mut buf, &mut state);
        assert_eq!(
            buf[(0, 0)].symbol(),
            " ",
            "untouched buffer cell stays a space"
        );
    }

    #[test]
    fn text_longer_than_area_is_clipped() {
        // Area width 3; text "ABCDEFGH" -> only first 3 cells written.
        let buf = render("ABCDEFGH", 0.0, Theme::DeepSpace, 0, 3, 1);
        assert_eq!(buf[(0, 0)].symbol(), "A");
        assert_eq!(buf[(1, 0)].symbol(), "B");
        assert_eq!(buf[(2, 0)].symbol(), "C");
    }

    #[test]
    fn intensity_zero_never_glitches() {
        // Even with life > 0, intensity 0 rolls an empty burst.
        let buf = render("HELLO", 0.0, Theme::Cyberpunk, REROLL_PERIOD, 5, 1);
        let syms = row_symbols(&buf, 5);
        assert_eq!(syms, vec!["H", "E", "L", "L", "O"], "intensity 0 never corrupts");
    }

    #[test]
    fn theme_builder_changes_alert_color() {
        let buf = render("HI", 1.0, Theme::Fallout, REROLL_PERIOD, 2, 1);
        let fallout_alert = Theme::Fallout.palette().alert.color();
        assert_eq!(buf[(0, 0)].style().fg, Some(fallout_alert));
    }

    #[test]
    fn tick_does_not_panic_on_long_run() {
        // wrapping_add should keep us safe across u32::MAX.
        let mut state = GlitchTextState::default();
        for _ in 0..1_000_000 {
            state.tick();
        }
        // Just assert it didn't panic and the prng stayed non-zero.
        assert_ne!(state.prng, 0);
    }

    #[test]
    fn burst_is_stable_between_ticks_within_life() {
        // Once rolled, the same burst should be shown across consecutive renders
        // until the next re-roll (no per-frame flicker).
        let mk = |ticks: u32| {
            let mut buf = Buffer::empty(Rect::new(0, 0, 5, 1));
            let widget = GlitchText::new("HELLO").intensity(1.0).theme(Theme::Cyberpunk);
            let mut state = GlitchTextState::default();
            for _ in 0..ticks {
                state.tick();
            }
            StatefulWidget::render(widget, Rect::new(0, 0, 5, 1), &mut buf, &mut state);
            row_symbols(&buf, 5)
        };
        // tick REROLL_PERIOD -> life=3. Renders at ticks PERIOD and PERIOD+1
        // (life still > 0, no new re-roll) must be identical.
        let a = mk(REROLL_PERIOD);
        let b = mk(REROLL_PERIOD + 1);
        assert_eq!(a, b, "burst is held stable between re-rolls");
    }
}
