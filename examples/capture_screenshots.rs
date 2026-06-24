//! **Headless screenshot generator** — renders every example to looping GIFs
//! in `screenshot/`, no terminal required.
//!
//! Each example's real `App`/`draw` is driven off-screen through a
//! `TestBackend`: we tick the animation, clone the `Buffer`, rasterize every
//! cell (background fill + glyph outline via `ab_glyph`) into an RGBA frame,
//! write it to a PNG, and finally ask `ffmpeg` to stitch the frames into a
//! looping GIF. This reuses the *actual* example layouts (via `#[path]`), so
//! the art never drifts from what `cargo run --example <name>` shows.
//!
//! ```sh
//! # regenerate all README art
//! cargo run -p ratatui-sci-fi --example capture_screenshots
//! # …or just one
//! cargo run -p ratatui-sci-fi --example capture_screenshots -- dashboard
//! ```
//!
//! Needs `ffmpeg` on `$PATH`. Override the glyph font with
//! `RATATUI_SCIFI_FONT=/path/to/font.ttf`.

use std::fs;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::process::Command;

use ab_glyph::{Font, FontVec, Glyph, PxScale, ScaleFont, point};
use png::{BitDepth, ColorType, Encoder};
use ratatui::{Frame, Terminal, backend::TestBackend, buffer::Buffer, style::{Color, Modifier}};
use ratatui_sci_fi::Theme;

// Reuse the real example scenes — `pub App`/`draw`/`tick`/`cycle_theme` live in
// each example file. Including them as modules (not separate targets) lets us
// drive them headlessly without duplicating a line of layout.
#[allow(dead_code, unused_imports)]
#[path = "dashboard.rs"]
mod dashboard;
#[allow(dead_code, unused_imports)]
#[path = "matrix_rain.rs"]
mod matrix_rain;
#[allow(dead_code, unused_imports)]
#[path = "button.rs"]
mod button;
#[allow(dead_code, unused_imports)]
#[path = "widget_gallery.rs"]
mod widget_gallery;
#[allow(dead_code, unused_imports)]
#[path = "form_controls.rs"]
mod form_controls;
#[allow(dead_code, unused_imports)]
#[path = "hud_effects.rs"]
mod hud_effects;
#[allow(dead_code, unused_imports)]
#[path = "indicators.rs"]
mod indicators;
#[allow(dead_code, unused_imports)]
#[path = "agent_console.rs"]
mod agent_console;

// Re-export the example modules under the names the scene table uses.
use agent_console as scene_agent_console;
use button as scene_button;
use dashboard as scene_dashboard;
use matrix_rain as scene_matrix_rain;
use widget_gallery as scene_widget_gallery;
use form_controls as scene_form_controls;
use hud_effects as scene_hud_effects;
use indicators as scene_indicators;

/// Pixel geometry shared across every frame. `cell_w`/`cell_h` are derived from
/// the font's own metrics so box-drawing glyphs tile seamlessly.
#[derive(Clone, Copy)]
struct Metrics {
    cell_w: u32,
    cell_h: u32,
    /// Scale passed to `ab_glyph` (== line height in px).
    scale: f32,
    /// Distance from a cell's top edge to its text baseline, in px.
    ascent: f32,
}

impl Metrics {
    fn from_font(font: &FontVec, cell_h: u32) -> Self {
        let scale = PxScale::from(cell_h as f32);
        let scaled = font.as_scaled(scale);
        let advance = scaled.h_advance(font.glyph_id('M'));
        let ascent = scaled.ascent();
        Self {
            // Floor so adjacent box-drawing glyphs overlap by a hair rather
            // than leave a sub-pixel seam.
            cell_w: advance.floor().max(6.0) as u32,
            cell_h,
            scale: cell_h as f32,
            ascent,
        }
    }

    fn img_w(&self, cols: u16) -> u32 {
        self.cell_w * cols as u32
    }
    fn img_h(&self, rows: u16) -> u32 {
        self.cell_h * rows as u32
    }
}

/// The three faces the rasterizer draws with. No single shipped font covers
/// every glyph the widgets emit, so we assemble a small chain:
/// - `regular` / `bold`: a monospace face with Braille + box-drawing + block
///   glyph coverage (Adwaita Mono here) — this is what makes the radar, gauges
///   and biometric chart actually paint.
/// - `fallback`: a second face covering what the primary omits — chiefly the
///   half-width katakana that drives the matrix rain (Noto Sans Mono CJK).
struct Fonts<'a> {
    regular: &'a FontVec,
    bold: &'a FontVec,
    fallback: &'a FontVec,
}

/// Resolve a ratatui `Color` to sRGB bytes, falling back to `default` for
/// `Reset`. Themes are 24-bit so the named/256 entries are just safety nets.
fn resolve_color(c: Color, default: [u8; 3]) -> [u8; 3] {
    match c {
        Color::Reset => default,
        Color::Rgb(r, g, b) => [r, g, b],
        Color::Black => [0, 0, 0],
        Color::Red => [170, 0, 0],
        Color::Green => [0, 170, 0],
        Color::Yellow => [170, 85, 0],
        Color::Blue => [0, 0, 170],
        Color::Magenta => [170, 0, 170],
        Color::Cyan => [0, 170, 170],
        Color::Gray => [170, 170, 170],
        Color::DarkGray => [85, 85, 85],
        Color::LightRed => [255, 85, 85],
        Color::LightGreen => [85, 255, 85],
        Color::LightYellow => [255, 255, 85],
        Color::LightBlue => [85, 85, 255],
        Color::LightMagenta => [255, 85, 255],
        Color::LightCyan => [85, 255, 255],
        Color::White => [255, 255, 255],
        Color::Indexed(i) => ansi256(i),
    }
}

/// xterm 256-color palette entry for index `i`.
fn ansi256(i: u8) -> [u8; 3] {
    match i {
        0..=15 => resolve_color(named16(i), [128, 128, 128]),
        232..=255 => {
            let v = 8 + (i - 232) as u32 * 10;
            [v as u8, v as u8, v as u8]
        }
        16..=231 => {
            let i = (i - 16) as u32;
            let r = i / 36;
            let g = (i / 6) % 6;
            let b = i % 6;
            let c = |v: u32| if v == 0 { 0 } else { 55 + v * 40 };
            [c(r) as u8, c(g) as u8, c(b) as u8]
        }
    }
}

fn named16(i: u8) -> Color {
    [
        Color::Black,
        Color::Red,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::Gray,
        Color::DarkGray,
        Color::LightRed,
        Color::LightGreen,
        Color::LightYellow,
        Color::LightBlue,
        Color::LightMagenta,
        Color::LightCyan,
        Color::White,
    ][i as usize]
}

/// Paint a solid `w`×`h` rectangle at `(x,y)` in opaque RGBA.
fn fill_rect(buf: &mut [u8], stride: u32, x: u32, y: u32, w: u32, h: u32, [r, g, b]: [u8; 3]) {
    let row_start = (y * stride + x) as usize * 4;
    let row_bytes = w as usize * 4;
    for ry in 0..h {
        let off = row_start + (ry * stride) as usize * 4;
        for px in 0..w {
            let o = off + px as usize * 4;
            buf[o] = r;
            buf[o + 1] = g;
            buf[o + 2] = b;
            buf[o + 3] = 255;
        }
    }
    let _ = row_bytes; // (kept for clarity; per-pixel writes above)
}

/// Blend a single glyph pixel of color `rgb` with anti-alias coverage `a`.
fn blend(buf: &mut [u8], stride: u32, w: u32, h: u32, x: i32, y: i32, a: f32, [r, g, b]: [u8; 3]) {
    if (0..w as i32).contains(&x) && (0..h as i32).contains(&y) {
        let o = (y as u32 * stride + x as u32) as usize * 4;
        let ia = 1.0 - a;
        buf[o] = (buf[o] as f32 * ia + r as f32 * a) as u8;
        buf[o + 1] = (buf[o + 1] as f32 * ia + g as f32 * a) as u8;
        buf[o + 2] = (buf[o + 2] as f32 * ia + b as f32 * a) as u8;
        buf[o + 3] = 255;
    }
}

/// Rasterize a whole `Buffer` to an RGBA byte vector. Two passes: backgrounds
/// first (so a glyph bleeding into its neighbour lands on the right colour),
/// then glyphs. Missing glyphs fall back to `fallback`.
fn buffer_to_rgba(
    buf: &Buffer,
    m: Metrics,
    fonts: &Fonts<'_>,
    default_bg: [u8; 3],
    default_fg: [u8; 3],
) -> Vec<u8> {
    let cols = buf.area.width as u32;
    let rows = buf.area.height as u32;
    let stride = m.img_w(buf.area.width);
    let img_h = m.img_h(buf.area.height);
    let mut rgba = vec![0u8; (stride * img_h * 4) as usize];

    // Pass 1 — backgrounds.
    for r in 0..rows {
        for c in 0..cols {
            let cell = &buf.content[(r * cols + c) as usize];
            let bg = resolve_color(cell.bg, default_bg);
            fill_rect(&mut rgba, stride, c * m.cell_w, r * m.cell_h, m.cell_w, m.cell_h, bg);
        }
    }

    // Pass 2 — glyphs (alpha-blended over the backgrounds).
    for r in 0..rows {
        let baseline = r as f32 * m.cell_h as f32 + m.ascent;
        for c in 0..cols {
            let cell = &buf.content[(r * cols + c) as usize];
            if cell.symbol().chars().all(|ch| ch.is_whitespace()) {
                continue;
            }
            let fg = resolve_color(cell.fg, default_fg);
            let bold = cell.modifier.contains(Modifier::BOLD);
            let pen_x = c as f32 * m.cell_w as f32;
            for ch in cell.symbol().chars() {
                draw_char(&mut rgba, stride, stride, img_h, fonts, ch, pen_x, baseline, fg, m.scale, bold);
            }
        }
    }

    rgba
}

/// Outline + rasterize one code point at `(pen_x, baseline)`.
fn draw_char(
    rgba: &mut [u8],
    stride: u32,
    w: u32,
    h: u32,
    fonts: &Fonts<'_>,
    ch: char,
    pen_x: f32,
    baseline: f32,
    rgb: [u8; 3],
    scale: f32,
    bold: bool,
) {
    // Real bold when a bold face exists, else the regular face. Either way fall
    // back to the CJK/symbol face for glyphs the primary omits.
    let base = if bold { fonts.bold } else { fonts.regular };
    let base_id = base.glyph_id(ch);
    let (use_font, gid) = if base_id.0 != 0 {
        (base, base_id)
    } else {
        (fonts.fallback, fonts.fallback.glyph_id(ch))
    };
    let gid = if gid.0 == 0 { use_font.glyph_id(' ') } else { gid };

    let glyph = Glyph { scale: PxScale::from(scale), position: point(pen_x, baseline), id: gid };
    let Some(outlined) = use_font.outline_glyph(glyph) else { return };
    let bounds = outlined.px_bounds();
    let min_x = bounds.min.x;
    let min_y = bounds.min.y;

    outlined.draw(|gx, gy, cov| {
        let x = min_x as i32 + gx as i32;
        let y = min_y as i32 + gy as i32;
        let a = cov.min(1.0);
        if a <= 0.0 {
            return;
        }
        blend(rgba, stride, w, h, x, y, a, rgb);
        // Faux bold — smear one px right so weight reads at thumbnail size.
        if bold && a > 0.35 {
            blend(rgba, stride, w, h, x + 1, y, a, rgb);
        }
    });
}

/// Encode an RGBA buffer to a PNG file.
fn write_png(path: &Path, rgba: &[u8], w: u32, h: u32) -> std::io::Result<()> {
    let file = fs::File::create(path)?;
    let mut enc = Encoder::new(BufWriter::new(file), w, h);
    enc.set_color(ColorType::Rgba);
    enc.set_depth(BitDepth::Eight);
    let mut writer = enc.write_header()?;
    writer.write_image_data(rgba)?;
    Ok(())
}

/// `ffmpeg` PNG sequence → looping GIF via a two-pass global palette. Flat neon
/// colors quantize cleanly to ≤256 colours, so this stays small and animates
/// everywhere (IDE previews, terminals, github) without the multi-MB weight of
/// a true-colour APNG. Requires ffmpeg on `$PATH`.
fn ffmpeg_to_gif(frames_dir: &Path, _frames: u32, fps: u32, out: &Path) -> std::io::Result<()> {
    let input = frames_dir.join("f_%04d.png");
    let fps = fps.to_string();
    let filter =
        "[0:v] split [a][b]; \
         [a] palettegen=max_colors=256:reserve_transparent=0:stats_mode=diff [p]; \
         [b][p] paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle [out]";
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "warning",
            "-y",
            "-framerate",
            &fps,
            "-i",
            &input.to_string_lossy(),
            "-filter_complex",
            filter,
            "-map",
            "[out]",
            "-r",
            &fps,
            "-loop",
            "0",
            &out.to_string_lossy(),
        ])
        .status()?;
    if !status.success() {
        return Err(std::io::Error::other(format!("ffmpeg exited {status}")));
    }
    Ok(())
}

/// Read the first existing font file in `cands` as an owned `ab_glyph` font.
fn load_first(cands: &[Option<PathBuf>]) -> std::io::Result<FontVec> {
    for path in cands.iter().flatten() {
        if path.exists() {
            let data = fs::read(path)?;
            return FontVec::try_from_vec(data)
                .map_err(|e| std::io::Error::other(format!("bad font {}: {e}", path.display())));
        }
    }
    Err(std::io::Error::other(format!(
        "no font found among {cands:?}; set RATATUI_SCIFI_FONT=/path/to/font.ttf",
    )))
}

fn home_path(rel: &str) -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(rel))
}

/// Monospace face with the broadest sci-fi glyph coverage (Braille + box-drawing
/// + block + arrows). `RATATUI_SCIFI_FONT` wins; otherwise prefer Adwaita Mono.
fn primary_candidates() -> Vec<Option<PathBuf>> {
    vec![
        std::env::var("RATATUI_SCIFI_FONT").ok().map(PathBuf::from),
        Some(PathBuf::from("/usr/share/fonts/adwaita-mono-fonts/AdwaitaMono-Regular.ttf")),
        home_path(".local/share/fonts/DroidSansMNerdFont-Regular.otf"),
        Some(PathBuf::from("/usr/share/fonts/google-droid-sans-mono-fonts/DroidSansMono.ttf")),
        Some(PathBuf::from("/usr/share/fonts/liberation-mono-fonts/LiberationMono-Regular.ttf")),
    ]
}

/// Bold weight of the primary face; degrades to the regular face (faux bold) if
/// no bold variant is installed.
fn bold_candidates() -> Vec<Option<PathBuf>> {
    vec![
        Some(PathBuf::from("/usr/share/fonts/adwaita-mono-fonts/AdwaitaMono-Bold.ttf")),
        Some(PathBuf::from("/usr/share/fonts/adwaita-mono-fonts/AdwaitaMono-Regular.ttf")),
        Some(PathBuf::from("/usr/share/fonts/google-droid-sans-mono-fonts/DroidSansMono.ttf")),
        home_path(".local/share/fonts/DroidSansMNerdFont-Regular.otf"),
    ]
}

/// Secondary face covering glyphs the primary omits — chiefly the half-width
/// katakana (U+FF65–FF9D) that drives the matrix rain, plus CJK.
fn fallback_candidates() -> Vec<Option<PathBuf>> {
    vec![
        Some(PathBuf::from("/usr/share/fonts/google-noto-sans-mono-cjk-vf-fonts/NotoSansMonoCJK-VF.ttc")),
        home_path(".local/share/fonts/DroidSansMNerdFont-Regular.otf"),
        Some(PathBuf::from("/usr/share/fonts/adwaita-mono-fonts/AdwaitaMono-Regular.ttf")),
    ]
}

/// Drive one example scene headlessly and emit `screenshot/<name>.gif`.
fn run_scene<A>(
    name: &str,
    cols: u16,
    rows: u16,
    warmup: u32,
    frames: u32,
    fps: u32,
    theme_every: u32,
    m: Metrics,
    fonts: &Fonts<'_>,
    new_app: impl FnOnce() -> A,
    mut draw: impl FnMut(&mut Frame<'_>, &mut A),
    mut tick: impl FnMut(&mut A),
    mut cycle_theme: impl FnMut(&mut A),
    theme_of: impl Fn(&A) -> Theme,
) -> std::io::Result<PathBuf> {
    let mut term = Terminal::new(TestBackend::new(cols, rows)).unwrap();
    let mut app = new_app();
    for _ in 0..warmup {
        tick(&mut app);
    }

    let frames_dir = PathBuf::from(format!("/tmp/rsf_frames_{name}"));
    let _ = fs::remove_dir_all(&frames_dir);
    fs::create_dir_all(&frames_dir)?;

    for i in 0..frames {
        if i > 0 && theme_every > 0 && i % theme_every == 0 {
            cycle_theme(&mut app);
        }
        term.draw(|f| draw(f, &mut app)).unwrap();
        let buf = term.backend().buffer().clone();

        let palette = theme_of(&app).palette();
        let default_bg = resolve_color(palette.bg.color(), [10, 10, 18]);
        let default_fg = resolve_color(palette.fg.color(), [200, 200, 210]);

        let rgba = buffer_to_rgba(&buf, m, fonts, default_bg, default_fg);
        let path = frames_dir.join(format!("f_{i:04}.png"));
        write_png(&path, &rgba, m.img_w(cols), m.img_h(rows))?;

        tick(&mut app);
    }

    let out = PathBuf::from("screenshot").join(format!("{name}.gif"));
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    ffmpeg_to_gif(&frames_dir, frames, fps, &out)?;
    let _ = fs::remove_dir_all(&frames_dir);

    let kb = fs::metadata(&out)?.len() / 1024;
    println!("  {name:<16} → {} ({} frames, {}×{} cells, {kb} KB)", out.display(), frames, cols, rows);
    Ok(out)
}

fn main() -> std::io::Result<()> {
    // Optional allow-list: `capture_screenshots dashboard widget_gallery`.
    let want: Vec<String> = std::env::args().skip(1).collect();
    let selected = |name: &str| want.is_empty() || want.iter().any(|w| w == name);

    println!("Loading glyph fonts…");
    let regular = load_first(&primary_candidates())?;
    let bold = load_first(&bold_candidates())?;
    let fallback = load_first(&fallback_candidates())?;
    let fonts = Fonts { regular: &regular, bold: &bold, fallback: &fallback };
    let metrics = Metrics::from_font(&regular, 22);
    let fps = 18;

    println!("Rendering to screenshot/ ({}px cells):", metrics.cell_w);

    // The radar sweep trail + biometric rolling window only advance once the
    // boot intro finishes (frame > BOOT_TICKS = 120), so the dashboard needs a
    // long warm-up to pre-fill both before the first captured frame.
    if selected("dashboard") {
        run_scene(
            "dashboard",
            100,
            30,
            235, // well past boot + ~115 widget ticks: radar trail + bio window full
            120,
            fps,
            42, // ~3 themes per loop — slower cycle, less palette pressure
            metrics,
            &fonts,
            scene_dashboard::App::new,
            scene_dashboard::draw,
            scene_dashboard::App::tick,
            scene_dashboard::App::cycle_theme,
            |a: &scene_dashboard::App| a.theme(),
        )?;
    }

    // Gallery ticks every widget from frame 0, so a short warm-up suffices.
    if selected("widget_gallery") {
        run_scene(
            "widget_gallery",
            110,
            40,
            75,
            100,
            fps,
            50,
            metrics,
            &fonts,
            scene_widget_gallery::App::new,
            scene_widget_gallery::draw,
            scene_widget_gallery::App::tick,
            scene_widget_gallery::App::cycle_theme,
            |a: &scene_widget_gallery::App| a.theme(),
        )?;
    }

    // Form controls: the 5 interactive widgets in their initial (un-driven)
    // state — capture_screenshots only ticks, never sends keys, so values stay
    // at their defaults while the theme cycles.
    if selected("form_controls") {
        run_scene(
            "form_controls",
            70,
            24,
            10,
            80,
            fps,
            40,
            metrics,
            &fonts,
            scene_form_controls::App::new,
            scene_form_controls::draw,
            scene_form_controls::App::tick,
            scene_form_controls::App::cycle_theme,
            |a: &scene_form_controls::App| a.theme(),
        )?;
    }

    // HUD effects: typewriter / marquee / digital clock — ambient, tick-driven.
    if selected("hud_effects") {
        run_scene(
            "hud_effects",
            70,
            16,
            15,
            90,
            fps,
            40,
            metrics,
            &fonts,
            scene_hud_effects::App::new,
            scene_hud_effects::draw,
            scene_hud_effects::App::tick,
            scene_hud_effects::App::cycle_theme,
            |a: &scene_hud_effects::App| a.theme(),
        )?;
    }

    // Indicators: status LED / countdown / progress / collapsible panel.
    if selected("indicators") {
        run_scene(
            "indicators",
            70,
            24,
            15,
            90,
            fps,
            40,
            metrics,
            &fonts,
            scene_indicators::App::new,
            scene_indicators::draw,
            scene_indicators::App::tick,
            scene_indicators::App::cycle_theme,
            |a: &scene_indicators::App| a.theme(),
        )?;
    }

    if selected("matrix_rain") {
        run_scene(
            "matrix_rain",
            84,
            26,
            55, // let the columns populate before the first frame
            90,
            fps,
            30,
            metrics,
            &fonts,
            scene_matrix_rain::App::new,
            scene_matrix_rain::App::draw,
            scene_matrix_rain::App::tick,
            scene_matrix_rain::App::cycle_theme,
            |a: &scene_matrix_rain::App| a.theme(),
        )?;
    }

    // AI Agent Console — boot into the Console scene (fast-forwarded past boot +
    // login by `new_app`), then keep the chat alive by transmitting a scripted
    // line every ~34 frames so the GIF shows streaming markdown replies.
    if selected("agent_console") {
        let frame = std::cell::Cell::new(0u32);
        let script = ["status report", "ORACLE, threat board?", "ATLAS, ETA to jump?", "VEX, stand down"];
        run_scene(
            "agent_console",
            120,
            34,
            0, // warm-up happens in new_app (fast-forward)
            170,
            fps,
            55, // cycle themes slowly so every palette shows up
            metrics,
            &fonts,
            || {
                let mut a = scene_agent_console::App::new();
                a.fast_forward_to_console();
                a
            },
            |f, a| scene_agent_console::draw(f, a),
            |a| {
                a.tick();
                let i = frame.get();
                frame.set(i + 1);
                if i > 0 && i % 34 == 0 && !a.chat_streaming() {
                    a.transmit(script[((i / 34) as usize) % script.len()]);
                }
            },
            scene_agent_console::App::cycle_theme,
            |a: &scene_agent_console::App| a.theme(),
        )?;
    }

    // Static showcase — cycle through all 8 themes so the GIF shows every shape
    // variant in every palette. `tick` is a no-op (the layout doesn't animate).
    if selected("button") {
        run_scene(
            "button",
            92,
            26,
            1,
            24, // 8 themes × 3 frames
            fps,
            3, // new theme every 3 frames
            metrics,
            &fonts,
            scene_button::App::new,
            scene_button::draw,
            scene_button::App::tick,
            scene_button::App::cycle_theme,
            |a: &scene_button::App| a.theme(),
        )?;
    }

    println!("Done.");
    Ok(())
}
