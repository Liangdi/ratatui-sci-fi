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
///
/// The selectors below cover the whole widget roster. Note this is a
/// **contract** for widgets that adopt the cascade: today every widget still
/// reads [`Theme::palette`] directly, so these rules reserve the node-type and
/// state-class names (e.g. `Radar.threat`, `List.selected`) each widget will
/// query once it migrates to `compute_with`.
///
/// [`Theme::palette`]: crate::Theme::palette
const COMPONENT_CSS: &str = r#"
    Root        { background: var(--bg); }

    Frame       { color: var(--muted); border: double; padding: 1; }
    Frame.main  { color: var(--accent); }
    Frame.title { color: var(--accent); font-weight: bold; }

    Label       { color: var(--muted); }
    Value       { color: var(--fg); }
    Value.ok    { color: var(--ok); }
    Value.warn  { color: var(--warn); }
    Value.alert { color: var(--alert); }

    Button        { color: var(--accent); background: var(--bg); }
    Button.focus  { color: var(--bg); background: var(--accent); font-weight: bold; }

    /* Toggle — boolean control mirroring Button's two-state shape. Off reads as
       a quiet muted idle; on escalates to ok + bold. */
    Toggle        { color: var(--muted); }
    Toggle.on     { color: var(--ok); font-weight: bold; }
    Toggle.off    { color: var(--muted); }

    Bar         { color: var(--accent); }
    Bar.warn    { color: var(--warn); }
    Bar.alert   { color: var(--alert); }

    Scanline    { color: var(--muted); }
    Cursor      { color: var(--accent); }
    Alert       { color: var(--alert); border: double; }

    /* Text input — value text is fg; empty placeholder dims to muted; the
       caret reuses the `Cursor` rule above (accent). */
    Input             { color: var(--fg); }
    Input.placeholder { color: var(--muted); }

    /* Energy gauge — segmented energy bar (`Bar` above is the legacy alias).
       The base node supplies the gap-cell background; level/empty/label
       classes color the bar cells, empty cells, and the left label. */
    Gauge        { color: var(--accent); background: var(--bg); }
    Gauge.ok     { color: var(--ok); }
    Gauge.warn   { color: var(--warn); }
    Gauge.alert  { color: var(--alert); }
    Gauge.empty  { color: var(--muted); }
    Gauge.label  { color: var(--fg); }

    /* Biometric multi-trace chart — grid/axes use muted, block bg is panel,
       traces cycle tokens. */
    Biometric         { color: var(--muted); background: var(--panel); }
    Biometric.trace0  { color: var(--accent); }
    Biometric.trace1  { color: var(--accent2); }
    Biometric.trace2  { color: var(--ok); }
    Biometric.trace3  { color: var(--warn); }
    Biometric.trace4  { color: var(--alert); }

    /* Scan list — selected row gets an accent-on-panel highlight. */
    List          { color: var(--fg); }
    List.selected { color: var(--accent); background: var(--panel); }
    List.scan     { color: var(--muted); }

    /* Matrix rain — head uses accent, trail fades toward bg in the widget. */
    Matrix        { color: var(--accent); }

    /* Glitch text — clean glyphs use fg, corrupted glyphs use alert. */
    Glitch          { color: var(--fg); }
    Glitch.corrupt  { color: var(--alert); }

    /* Target lock HUD — brackets use accent, crosshair uses muted, locked
       state escalates to alert. */
    Target            { color: var(--accent); }
    Target.crosshair  { color: var(--muted); }
    Target.locked     { color: var(--alert); font-weight: bold; }

    /* Sci-fi radar — sweep accent, grid muted, friendly blips ok, threats alert. */
    Radar         { color: var(--accent); }
    Radar.grid    { color: var(--muted); }
    Radar.blip    { color: var(--ok); }
    Radar.threat  { color: var(--alert); }

    /* Boot sequence — line color follows its status class; flicker forces muted. */
    Boot          { color: var(--fg); }
    Boot.ok       { color: var(--ok); }
    Boot.warn     { color: var(--warn); }
    Boot.fail     { color: var(--alert); }
    Boot.flicker  { color: var(--muted); }

    /* Alert popup — panel interior, alert title, full-alert flash fill. */
    Popup         { color: var(--fg); background: var(--panel); border: double; }
    Popup.title   { color: var(--alert); font-weight: bold; }
    Popup.flash   { color: var(--alert); background: var(--alert); }
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
static BLOODMOON: LazyLock<Stylesheet> = LazyLock::new(|| build(Theme::Bloodmoon.palette()));
static NEBULA: LazyLock<Stylesheet> = LazyLock::new(|| build(Theme::Nebula.palette()));
static ARCTIC: LazyLock<Stylesheet> = LazyLock::new(|| build(Theme::Arctic.palette()));
static SENTINEL: LazyLock<Stylesheet> = LazyLock::new(|| build(Theme::Sentinel.palette()));

/// The cached stylesheet for `theme`.
pub fn stylesheet(theme: Theme) -> &'static Stylesheet {
    match theme {
        Theme::Cyberpunk => &CYBERPUNK,
        Theme::Fallout => &FALLOUT,
        Theme::Weyland => &WEYLAND,
        Theme::DeepSpace => &DEEP_SPACE,
        Theme::Bloodmoon => &BLOODMOON,
        Theme::Nebula => &NEBULA,
        Theme::Arctic => &ARCTIC,
        Theme::Sentinel => &SENTINEL,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every theme's stylesheet must parse. `stylesheet()` derefs the
    /// `LazyLock`, so this forces all eight `build()` calls — if any CSS rule
    /// is malformed the `.expect` inside `build` panics and fails the test.
    #[test]
    fn all_themes_parse() {
        for theme in [
            Theme::Cyberpunk,
            Theme::Fallout,
            Theme::Weyland,
            Theme::DeepSpace,
            Theme::Bloodmoon,
            Theme::Nebula,
            Theme::Arctic,
            Theme::Sentinel,
        ] {
            // Touch the sheet — panics here if the bundled CSS fails to parse.
            let _ = stylesheet(theme);
        }
    }

    /// Each theme must resolve to its own static sheet (catches a match arm
    /// accidentally aliasing another theme's sheet).
    #[test]
    fn each_theme_has_distinct_sheet() {
        let sheets = [
            stylesheet(Theme::Cyberpunk) as *const Stylesheet,
            stylesheet(Theme::Fallout) as *const Stylesheet,
            stylesheet(Theme::Weyland) as *const Stylesheet,
            stylesheet(Theme::DeepSpace) as *const Stylesheet,
            stylesheet(Theme::Bloodmoon) as *const Stylesheet,
            stylesheet(Theme::Nebula) as *const Stylesheet,
            stylesheet(Theme::Arctic) as *const Stylesheet,
            stylesheet(Theme::Sentinel) as *const Stylesheet,
        ];
        for i in 0..sheets.len() {
            for j in (i + 1)..sheets.len() {
                assert_ne!(
                    sheets[i], sheets[j],
                    "themes #{i} and #{j} share a stylesheet"
                );
            }
        }
    }
}
