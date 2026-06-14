//! Builds the per-theme [`ratatui_style::Stylesheet`].
//!
//! Each theme injects its palette as CSS custom properties (`:root { --… }`),
//! then the shared [`COMPONENT_CSS`] rules — which reference those tokens via
//! `var(--…)` — apply uniformly across all themes.

use std::sync::LazyLock;

use ratatui_style::Stylesheet;

use crate::themes::{Palette, Theme};

/// Shared component rules — token-driven, so they apply to every theme.
///
/// Only declarations proven valid against the `ratatui-style` 0.1.x CSS parser
/// are used here (`background`, `color`, `var(--…)`, `font-weight: bold`,
/// `padding: 1`, `border: double`). Widgets may extend this set as they verify
/// further properties.
const COMPONENT_CSS: &str = r#"
    Root        { background: var(--bg); }

    Frame       { color: var(--muted); border: double; padding: 1; }
    Frame.main  { color: var(--accent); }

    Label       { color: var(--muted); }
    Value       { color: var(--fg); }
    Value.ok    { color: var(--ok); }
    Value.warn  { color: var(--warn); }
    Value.alert { color: var(--alert); }

    Button        { color: var(--accent); }
    Button.focus  { color: var(--bg); background: var(--accent); font-weight: bold; }

    Bar         { color: var(--accent); }
    Bar.warn    { color: var(--warn); }
    Bar.alert   { color: var(--alert); }

    Scanline    { color: var(--muted); }
    Cursor      { color: var(--accent); }
    Alert       { color: var(--alert); border: double; }
"#;

/// The `:root` token block for a palette.
fn root_tokens(p: Palette) -> String {
    format!(
        "
        :root {{
            --accent: {accent};
            --accent2: {accent2};
            --bg: {bg};
            --panel: {panel};
            --fg: {fg};
            --muted: {muted};
            --ok: {ok};
            --warn: {warn};
            --alert: {alert};
        }}
        ",
        accent = p.accent.hex(),
        accent2 = p.accent2.hex(),
        bg = p.bg.hex(),
        panel = p.panel.hex(),
        fg = p.fg.hex(),
        muted = p.muted.hex(),
        ok = p.ok.hex(),
        warn = p.warn.hex(),
        alert = p.alert.hex(),
    )
}

fn build(p: Palette) -> Stylesheet {
    let css = format!("{}\n{}", root_tokens(p), COMPONENT_CSS);
    Stylesheet::parse(&css).expect("bundled sci-fi CSS must parse")
}

static CYBERPUNK: LazyLock<Stylesheet> = LazyLock::new(|| build(Theme::Cyberpunk.palette()));
static FALLOUT: LazyLock<Stylesheet> = LazyLock::new(|| build(Theme::Fallout.palette()));
static WEYLAND: LazyLock<Stylesheet> = LazyLock::new(|| build(Theme::Weyland.palette()));
static DEEP_SPACE: LazyLock<Stylesheet> = LazyLock::new(|| build(Theme::DeepSpace.palette()));

/// The cached stylesheet for `theme`.
pub fn stylesheet(theme: Theme) -> &'static Stylesheet {
    match theme {
        Theme::Cyberpunk => &CYBERPUNK,
        Theme::Fallout => &FALLOUT,
        Theme::Weyland => &WEYLAND,
        Theme::DeepSpace => &DEEP_SPACE,
    }
}
