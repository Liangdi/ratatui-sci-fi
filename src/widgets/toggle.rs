//! **Toggle** — sci-fi boolean switch (PRD §3 基础组件).
//!
//! A single-row on/off control. It's the boolean sibling of [`Button`]: where
//! Button tracks transient focus, Toggle tracks a persistent on/off state — but
//! like Button, that state is per-frame configuration set by the app, not
//! animation, so Toggle is stateless.
//!
//! ## Spec
//! - **On**: `[◉ LABEL · ENGAGED ]` — the filled dot `◉`, accent color, bold;
//!   reads as an energized / armed indicator.
//! - **Off**: `[ ○ LABEL · STANDBY ]` — the hollow dot `○`, muted; reads as idle.
//!
//! ## Implementation notes
//! - Stateless [`Widget`]: `on` is per-frame configuration (like
//!   [`Button::focused`]), **not** animation state — so there is no `ToggleState`.
//! - Styling goes through the theme's
//!   [`Stylesheet`](ratatui_style::Stylesheet) cascade: the `Toggle.on` rule
//!   (ok + bold) or `Toggle.off` rule (muted) drives the whole content, mirroring
//!   `Button`/`Button.focus`. The colors are `var(--…)`-driven off the same
//!   palette.
//! - The label is horizontally centered in `area`; rendering targets the
//!   vertical middle row. All glyphs are width-1.
//!
//! # Example
//!
//! ```rust
//! use ratatui_sci_fi::{Toggle, Theme};
//!
//! let t = Toggle::new("SHIELDS").on(true).theme(Theme::DeepSpace);
//! ```

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::Widget,
};
use ratatui_style::{ComputeScratch, NodeRef};

use crate::Theme;

/// Filled indicator glyph for the on state.
pub const DOT_ON: &str = "◉";
/// Hollow indicator glyph for the off state.
pub const DOT_OFF: &str = "○";
/// Suffix appended when the toggle is on.
pub const SUFFIX_ON: &str = "ENGAGED";
/// Suffix appended when the toggle is off.
pub const SUFFIX_OFF: &str = "STANDBY";

/// Visual form of a [`Toggle`]'s indicator dot.
///
/// Selects the on/off dot glyph pair; colors stay on the CSS cascade
/// (`Toggle.on` / `Toggle.off`), untouched by this enum. The brackets and
/// suffixes are also unchanged across variants — only the dot glyph varies.
/// The [`ToggleShape::Orb`] default reproduces the original `◉` / `○` look
/// byte-for-byte, so existing tests pass unchanged.
///
/// Every dot glyph is Unicode width-1 (see convention #5 at the crate root),
/// keeping the toggle's `chars().count() == display_width` centering math
/// valid.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ToggleShape {
    /// Filled `◉` / hollow `○` — the original look.
    #[default]
    Orb,
    /// Filled `■` / hollow `□`.
    Block,
    /// Filled `◆` / hollow `◇`.
    Diamond,
    /// Filled `●` / hollow `○`.
    Bullet,
}

impl ToggleShape {
    /// The indicator dot glyph for the given on/off state.
    #[must_use]
    pub const fn dot(self, on: bool) -> char {
        match (self, on) {
            (Self::Orb, true) => '◉',
            (Self::Orb, false) => '○',
            (Self::Block, true) => '■',
            (Self::Block, false) => '□',
            (Self::Diamond, true) => '◆',
            (Self::Diamond, false) => '◇',
            (Self::Bullet, true) => '●',
            (Self::Bullet, false) => '○',
        }
    }
}

/// A sci-fi boolean toggle.
///
/// Build with [`Toggle::new`] (label), then set the on/off look with
/// [`Toggle::on`] and the theme with [`Toggle::theme`].
#[derive(Debug, Clone)]
pub struct Toggle {
    /// Visible label text.
    pub label: String,
    /// Whether the toggle is in its on (energized) state.
    pub on: bool,
    /// Dot-glyph form (on/off indicator). Defaults to
    /// [`ToggleShape::Orb`], the original filled/hollow look.
    pub shape: ToggleShape,
    /// Theme whose [`Stylesheet`](ratatui_style::Stylesheet) drives colors.
    /// Defaults to [`Theme::Cyberpunk`].
    pub theme: Theme,
}

impl Default for Toggle {
    fn default() -> Self {
        Self {
            label: String::new(),
            on: false,
            shape: ToggleShape::default(),
            theme: Theme::Cyberpunk,
        }
    }
}

impl Toggle {
    /// Create a toggle with the given label, off, default theme.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            on: false,
            shape: ToggleShape::default(),
            theme: Theme::Cyberpunk,
        }
    }

    /// Set whether the toggle renders in its on (energized) style.
    #[must_use]
    pub fn on(mut self, on: bool) -> Self {
        self.on = on;
        self
    }

    /// Set the dot-glyph form (see [`ToggleShape`]).
    #[must_use]
    pub fn shape(mut self, shape: ToggleShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set the theme used for coloring the toggle.
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Cascade class slice for the current state — `["on"]` or `["off"]`.
    fn state_classes(&self) -> &'static [&'static str] {
        if self.on { &["on"] } else { &["off"] }
    }
}

impl Widget for Toggle {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        // Style comes from the `Toggle.on` / `Toggle.off` cascade rules.
        let sheet = self.theme.stylesheet();
        let mut scratch = ComputeScratch::new();
        let style = sheet
            .compute_with(&NodeRef::new("Toggle").classes(self.state_classes()), None, &mut scratch)
            .to_style();

        let dot = self.shape.dot(self.on);
        let suffix = if self.on { SUFFIX_ON } else { SUFFIX_OFF };

        // Compose `[ dot LABEL · SUFFIX ]`.
        let content = format!("[ {dot} {label} · {suffix} ]", label = self.label);

        let row = area.y + area.height / 2;

        // Center `content` in `area`. Every glyph is width-1, so char count ==
        // display width.
        let content_width = content.chars().count() as u16;
        let available = area.width;
        let content_width = content_width.min(available);
        let x = area.x + available.saturating_sub(content_width) / 2;

        // Paint the full area background first so empty cells pick up the style,
        // then draw the centered content.
        buf.set_style(area, style);
        buf.set_string(x, row, content, style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, style::Modifier};

    const W: u16 = 24;
    const H: u16 = 3;

    fn render(toggle: Toggle) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, W, H));
        toggle.render(Rect::new(0, 0, W, H), &mut buf);
        buf
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..W).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()
    }

    #[test]
    fn on_shows_filled_dot_and_engaged_suffix() {
        let buf = render(Toggle::new("SHIELDS").on(true));
        let middle = H / 2;
        let text = row_text(&buf, middle);
        assert!(text.contains(DOT_ON), "on toggle should show the filled dot: {text:?}");
        assert!(text.contains(SUFFIX_ON), "on toggle should show ENGAGED: {text:?}");
        assert!(!text.contains(DOT_OFF));
    }

    #[test]
    fn off_shows_hollow_dot_and_standby_suffix() {
        let buf = render(Toggle::new("SHIELDS").on(false));
        let middle = H / 2;
        let text = row_text(&buf, middle);
        assert!(text.contains(DOT_OFF), "off toggle should show the hollow dot: {text:?}");
        assert!(text.contains(SUFFIX_OFF), "off toggle should show STANDBY: {text:?}");
        assert!(!text.contains(DOT_ON));
    }

    #[test]
    fn on_uses_ok_color_and_bold() {
        let ok = Theme::Cyberpunk.palette().ok.color();
        let buf = render(Toggle::new("X").on(true).theme(Theme::Cyberpunk));
        let middle = H / 2;
        // Find the filled dot and confirm it's ok-colored + bold.
        let dot_x = (0..W).find(|&x| buf[(x, middle)].symbol() == DOT_ON).expect("dot present");
        assert_eq!(buf[(dot_x, middle)].fg, ok, "on dot should be --ok");
        assert!(
            buf[(dot_x, middle)].modifier.contains(Modifier::BOLD),
            "on content should be bold via the cascade"
        );
    }

    #[test]
    fn off_uses_muted_color() {
        let muted = Theme::Cyberpunk.palette().muted.color();
        let buf = render(Toggle::new("X").on(false).theme(Theme::Cyberpunk));
        let middle = H / 2;
        let dot_x = (0..W).find(|&x| buf[(x, middle)].symbol() == DOT_OFF).expect("dot present");
        assert_eq!(buf[(dot_x, middle)].fg, muted, "off dot should be --muted");
    }

    #[test]
    fn label_present_in_both_states() {
        for on in [true, false] {
            let buf = render(Toggle::new("WARP").on(on));
            let text = row_text(&buf, H / 2);
            assert!(text.contains("WARP"), "label should render in state on={on}: {text:?}");
        }
    }

    #[test]
    fn content_is_centered() {
        let buf = render(Toggle::new("WARP").on(true));
        let text = row_text(&buf, H / 2);
        // Centered content is padded on the left.
        assert!(text.starts_with(' '), "content should be centered: {text:?}");
    }

    #[test]
    fn empty_area_is_a_noop() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        Toggle::new("X").on(true).render(Rect::new(0, 0, 0, 0), &mut buf);
        assert_eq!(*buf.area(), Rect::new(0, 0, 0, 0));
    }

    #[test]
    fn non_default_shape_changes_on_dot() {
        use super::ToggleShape;

        // The Block shape swaps the on dot from the default '◉' to '■'.
        let buf = render(Toggle::new("X").on(true).shape(ToggleShape::Block));
        let text = row_text(&buf, H / 2);
        assert!(
            text.contains('■'),
            "Block on should use the filled square dot '■': {text:?}"
        );
        assert!(
            !text.contains('◉'),
            "Block on must not use the default Orb dot '◉': {text:?}"
        );
    }
}
