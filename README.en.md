# ratatui-sci-fi

[![Rust](https://img.shields.io/badge/rust-edition%202024-orange)](https://www.rust-lang.org/)
[![ratatui](https://img.shields.io/badge/ratatui-0.30-red)](https://ratatui.rs)
[![Version](https://img.shields.io/badge/version-0.1.0-green)]()
[![License](https://img.shields.io/badge/license-MIT-blue)](#license)

English | **[中文](README.md)**

> A **sci-fi themed widget collection for the [Ratatui](https://ratatui.rs) TUI ecosystem**: cyberpunk neon, wasteland retro terminals, *Alien*-style industrial consoles, deep-space HUDs — a set of themes, a set of effect widgets, and a runtime-synthesized audio system to help you build immersive terminal UIs fast.

---

## ✨ Features

- **Four built-in themes** — Cyberpunk / Fallout / Weyland / DeepSpace, with a semantic palette (`accent` / `bg` / `alert` / …). Each theme exposes both native ratatui `Color`s and a `ratatui-style` CSS-cascade stylesheet.
- **10 widgets** — 5 stylistically-consistent basic widgets + 5 high-sensory effect widgets, all implemented against the ratatui 0.30 `Widget` / `StatefulWidget` model.
- **Runtime-synthesized audio** — no audio assets, no licensing burden. Six sound effects are synthesized from pure-Rust waveforms; `rodio` backend degrades silently when no device is present.
- **Backend-agnostic** — the library itself depends on no terminal backend (examples use `crossterm`).
- **Testable** — every widget ships offscreen-`Buffer` unit tests; no real terminal needed.

---

## 🖼️ Preview

Run the bundled examples (no extra setup required):

```sh
cargo run -p ratatui-sci-fi --example dashboard      # composite HUD (all widgets)
cargo run -p ratatui-sci-fi --example widget_gallery # 3×3 grid, one widget per cell
cargo run -p ratatui-sci-fi --example matrix_rain    # full-screen digital rain
```

**`dashboard`** — a composite sci-fi HUD: boot sequence + radar sweep / energy gauges / biometrics / event log; press `t` to cycle themes.

![dashboard example](screenshot/dashboard.gif)

**`widget_gallery`** — every widget isolated in its own cell.

![widget gallery example](screenshot/widget_gallery.gif)

**`matrix_rain`** — a full-screen digital-rain backdrop.

![matrix rain example](screenshot/matrix_rain.gif)

> Structural sketch of the `dashboard` layout (the GIFs above are the real, animated capture):

```text
┌──────────────────────────────────────────────────────────────────┐
│ ▶ SCI-FI HUD // ratatui-sci-fi ◀                                  │
├──────────────┬───────────────────────┬────────────────────────────┤
│ ┏━TELEMETRY━┓ │      ◎ SCANNER        │  BIOMETRICS                │
│ ┃ CORE ▰▰▰▰▱│ │       . . ✛ .          │  ╱╲╱╲___╱╲╱╲              │
│ ┃ PWR  ▰▰▰▱▱│ │     .  ●     .         │                            │
│ ┃ HULL ▰▰▱▱▱│ │       . . . .          ├────────────────────────────┤
│ ┃ SHLD ▰▱▱▱▱│ │                       │ █ DOCK SEQUENCE OK         │
│ ┗━━━━━━━━━━┛ │                       │   RADAR SWEEP DONE         │
├──────────────┴───────────────────────┴────────────────────────────┤
│ [↑↓] list   [t] theme   [a] alert   [q] exit                       │
└──────────────────────────────────────────────────────────────────┘
```

---

## 📦 Installation

```sh
cargo add ratatui-sci-fi
```

For sound, enable the `audio` feature (pulls in `rodio` + `cpal`; on Linux you'll need ALSA/PulseAudio dev libraries):

```sh
cargo add ratatui-sci-fi --features audio
```

`audio` is **off by default** — consumers who only want visuals aren't forced to pull in native audio dependencies.

---

## 🚀 Quick start

A minimal, runnable program: a full-screen deep-space radar.

```rust
use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{Frame, Terminal, backend::CrosstermBackend};
use ratatui_sci_fi::{SciFiRadar, SciFiRadarState, Theme};

type Term = Terminal<CrosstermBackend<Stdout>>;

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    let mut state = SciFiRadarState::default();
    loop {
        terminal.draw(|f| ui(f, &mut state))?;
        state.tick(); // advance the animation each frame

        if event::poll(Duration::from_millis(60))?
            && let Event::Key(k) = event::read()?
            && matches!(k.code, KeyCode::Char('q') | KeyCode::Esc)
        {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

fn ui(f: &mut Frame, state: &mut SciFiRadarState) {
    f.render_stateful_widget(
        SciFiRadar::new().theme(Theme::DeepSpace),
        f.area(),
        state,
    );
}
```

---

## 🎨 Themes

| Theme | Core colors | Vibe |
| :--- | :--- | :--- |
| **Cyberpunk** (default) | Fluorescent pink `#FF007F` / neon blue `#00F0FF` | Cyberpunk, night-city, neon |
| **Fallout** | Phosphor green `#33FF33` / black | Wasteland, retro mainframe, Pip-Boy |
| **Weyland** | Amber gold `#FFB000` / dark red | *Alien*-style industrial console |
| **Deep Space** | Deep blue `#0055FF` / alert red | Modern starship, minimalist flight HUD |

Accessing a theme: `Theme::Cyberpunk.palette()` returns native `Color`s; `Theme::Cyberpunk.stylesheet()` returns a `&'static Stylesheet` from ratatui-style (CSS cascade, supports `var(--token)` and class selectors). Both derive from the same RGB source of truth — they never drift.

> Most theme colors are 24-bit truecolor; on 8-color terminals or terminals without `COLORTERM=truecolor` support, they'll fall back (no errors).

---

## 🧱 Widgets

### Basic
| Widget | Description |
| :--- | :--- |
| `Button` | Unfocused `[ CONFIRM ]`, focused `▶ CONFIRM ◀` (highlighted, inverted, energy brackets) |
| `EnergyGauge` | Reactor-style segmented bar, `▰▰▰▰▱▱▱▱`, color shifts by threshold (ok/warn/alert) |
| `ScanList` | Scanline-separated list; selected row highlighted with a blinking cursor (`█`) |
| `AlertPopup` | Double-line alert-red border, brief flash when shown |
| `TargetLock` | Corner-bracket + center-crosshair HUD container, with `inner(area)` |

### Effects
| Widget | Description |
| :--- | :--- |
| `MatrixRain` | Matrix digital rain; configurable speed/density, great as a backdrop |
| `GlitchText` | Random, short-lived character substitution — signal interference / decode-failure look |
| `BootSequence` | Line-by-line boot text + occasional screen flicker |
| `BiometricChart` | Multi-trace, fast-oscillating line chart (heart rate / energy / radiation) |
| `SciFiRadar` | Braille circular sweep with a fading trail and optional blips |

**Widget conventions**: stateless widgets implement `Widget` (`render(self, area, buf)`); stateful widgets implement `StatefulWidget` (`render(self, area, buf, &mut State)`). Animation lives in the `…State` struct, advanced each frame via `state.tick()`. Every widget has a `.theme(Theme)` builder.

---

## 🔊 Audio

Effects are **synthesized in pure Rust** by the [synth](crates/ratatui-sci-fi/src/audio/synth.rs) module (no audio files, no licensing risk); playback is handled by [`AudioSystem`](crates/ratatui-sci-fi/src/audio/system.rs) under the `audio` feature.

**Catalog** (the `Sound` enum; always available, zero-dependency):

| Sound | Filename | Description | Trigger |
| :--- | :--- | :--- | :--- |
| `AmbientHum` | `ambient_hum.wav` | Low-frequency electrical/fan hum | Loop when entering the main view |
| `RadarEcho` | `radar_echo.wav` | Low "boom" once per radar revolution | Radar completes a sweep |
| `UiTick` | `ui_tick.wav` | Short, crisp electronic blip | Cursor moves between options |
| `KeyboardClack` | `keyboard_clack.wav` | Retro mechanical clack | Text input |
| `UiConfirm` | `ui_confirm.wav` | Confirmation synth tone | Button confirm |
| `AlertSiren` | `alert_siren.wav` | Sustained low-frequency pulse siren | Error / alert popup |

> Filenames are reserved for a possible future asset path; all effects are currently synthesized at runtime.

**Usage** (requires the `audio` feature):

```rust
use ratatui_sci_fi::audio::{AudioSystem, Sound};

// Returns None when there's no audio device — the app then runs silently
// (graceful degradation; never panics).
if let Some(mut audio) = AudioSystem::init() {
    audio.start_ambient();        // start the looping bed
    audio.play(Sound::UiConfirm); // fire a one-shot
    audio.set_volume(0.8);        // 0.0..=1.0
}
```

**Recommended event → sound architecture**: widgets hold no callbacks; the app layer fires sounds in the event loop (see the [dashboard example](crates/ratatui-sci-fi/examples/dashboard.rs): ScanList navigation → `UiTick`, AlertPopup shown → `AlertSiren`, radar revolution → `RadarEcho`).

---

## 🏗️ Architecture

```text
ratatui-sci-fi/                  # Cargo workspace
├── Cargo.toml                   # [workspace] + shared deps
└── crates/ratatui-sci-fi/
    ├── Cargo.toml               # member crate; the `audio` feature lives here
    ├── src/
    │   ├── lib.rs               # conventions + `pub use widgets::*` re-exports
    │   ├── themes/              # Palette / Theme / ratatui-style Stylesheet
    │   ├── widgets/             # 10 widgets
    │   └── audio/               # catalog (Sound/CATALOG) + synth + AudioSystem
    └── examples/
        ├── dashboard.rs         # composite sci-fi dashboard (all widgets + audio)
        └── matrix_rain.rs       # standalone Matrix rain demo
```

- **Two theming paths**: use `palette()` for raw `Color`s (good for direct `Canvas` drawing), or `stylesheet()` for CSS-cascade styling (good for declarative styles). Same RGB source, no drift.
- **Backend-agnostic**: the library only depends on `ratatui` + `ratatui-style`; `crossterm` is a dev-dependency used by the examples.

---

## 🗺️ Roadmap

- [x] Four themes + 10 widgets
- [x] Runtime-synthesized audio engine (`audio` feature)
- [ ] Parameterize sound character (tunable frequency/duration)
- [x] Named demo GIFs / screenshots (`screenshot/` + the headless `capture_screenshots` example; needs ffmpeg)
- [ ] More theme variants

---

## 🤝 Contributing

Issues and PRs welcome. Development follows the constraints in [AGENTS.md](AGENTS.md) (Rust-architect perspective, scoped to this crate's theme, no branch switching).

---

## 📄 License

MIT.
