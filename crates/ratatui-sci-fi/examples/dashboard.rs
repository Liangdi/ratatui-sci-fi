//! **Sci-fi dashboard** — a composite HUD showcasing every widget in the crate:
//! a `BootSequence` intro, then a telemetry panel (`TargetLock` + `EnergyGauge`s),
//! a `SciFiRadar`, a `BiometricChart`, an event `ScanList`, a `GlitchText`
//! header, and theme switching.
//!
//! `t` cycles themes, `a` triggers an alert, `q` / `Esc` quits.
//!
//! Sound needs the `audio` feature:
//!
//! ```sh
//! cargo run -p ratatui-sci-fi --example dashboard --features audio
//! ```

use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::Style,
    widgets::{Block, Clear, Paragraph},
};
use ratatui_sci_fi::{
    AlertPopup, AlertPopupState, BiometricChart, BiometricChartState, Blip, BootSequence,
    BootSequenceState, EnergyGauge, GlitchText, GlitchTextState, ScanList, ScanListState,
    SciFiRadar, SciFiRadarState, TargetLock, Theme,
};
use ratatui_sci_fi::audio::Sound;

#[cfg(feature = "audio")]
use ratatui_sci_fi::audio::AudioSystem;

/// Procedural-sound wrapper so the example compiles **with or without** the
/// `audio` feature. With the feature on it drives an [`AudioSystem`] (and is a
/// silent no-op if no device was opened); with it off it's an inert shim.
#[cfg(feature = "audio")]
struct Sfx(Option<AudioSystem>);
#[cfg(feature = "audio")]
impl Sfx {
    fn new() -> Self {
        Self(AudioSystem::init())
    }
    fn start_ambient(&mut self) {
        if let Some(a) = &mut self.0 {
            a.start_ambient();
        }
    }
    fn play(&self, sound: Sound) {
        if let Some(a) = &self.0 {
            a.play(sound);
        }
    }
}

#[cfg(not(feature = "audio"))]
struct Sfx;
#[cfg(not(feature = "audio"))]
impl Sfx {
    fn new() -> Self {
        Self
    }
    fn start_ambient(&mut self) {}
    fn play(&self, _sound: Sound) {}
}

type Term = Terminal<CrosstermBackend<Stdout>>;

const THEMES: [Theme; 4] = [Theme::Cyberpunk, Theme::Fallout, Theme::Weyland, Theme::DeepSpace];

/// How many ticks the boot intro plays before the HUD is revealed.
const BOOT_TICKS: u64 = 120;

const BOOT_LINES: &[&str] = &[
    "[ OK ] MOTHERBOARD ........ ONLINE",
    "[ OK ] CPU CORE x8 ........ NOMINAL",
    "[ OK ] COOLANT LOOP ....... STABLE",
    "[ OK ] LIFE SUPPORT ....... GREEN",
    "[WARN] REACTOR ............ 78%",
    "[ OK ] NAV LINK ........... LOCKED",
    "[ OK ] SENSOR ARRAY ....... CALIBRATED",
    "[ OK ] AI CORE ............ AWAKE",
    "ERROR HULL INTEGRITY ...... 64%",
    "[ OK ] BOOT SEQUENCE ...... COMPLETE",
];

/// Event-log rows shown in the `ScanList`; also indexed by the Up/Down handler.
const LOG_ITEMS: &[&str] = &[
    "DOCK SEQUENCE OK",
    "RADAR SWEEP DONE",
    "HULL STRESS +12%",
    "LINK ESTABLISHED",
    "UNKNOWN SIGNATURE",
    "CALIBRATING GYRO",
];

struct App {
    frame: u64,
    theme_idx: usize,
    radar: SciFiRadarState,
    bio: BiometricChartState,
    log: ScanListState,
    title: GlitchTextState,
    boot: BootSequenceState,
    alert_state: AlertPopupState,
    alert_visible: bool,
    sfx: Sfx,
}

impl App {
    fn new() -> Self {
        let mut radar = SciFiRadarState::default();
        radar.push_blip(Blip::new(0.7, 0.60, 0.9));
        radar.push_blip(Blip::new(2.4, 0.42, 0.6));
        radar.push_blip(Blip::new(4.1, 0.82, 0.8));
        Self {
            frame: 0,
            theme_idx: 0,
            radar,
            bio: BiometricChartState::new(3, 60),
            log: ScanListState::default(),
            title: GlitchTextState::default(),
            boot: BootSequenceState::default(),
            alert_state: AlertPopupState::default(),
            alert_visible: false,
            sfx: Sfx::new(),
        }
    }

    fn theme(&self) -> Theme {
        THEMES[self.theme_idx]
    }

    fn tick(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        self.title.tick();
        self.boot.tick();
        if self.frame > BOOT_TICKS {
            // HUD just came online — confirmation chime.
            if self.frame == BOOT_TICKS + 1 {
                self.sfx.play(Sound::UiConfirm);
            }
            self.radar.tick();
            self.bio.tick();
            // Radar "嗵" once per revolution (~2π / 0.2 rad-per-tick ≈ 31 ticks).
            if (self.frame - BOOT_TICKS).is_multiple_of(31) {
                self.sfx.play(Sound::RadarEcho);
            }
        }
        self.log.tick();
        if self.alert_visible {
            // Drive the popup's show-flash blink.
            self.alert_state.tick();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut terminal = setup()?;
    let mut app = App::new();
    app.sfx.start_ambient();

    loop {
        terminal.draw(|f| draw(f, &mut app))?;
        app.tick();

        if event::poll(Duration::from_millis(60))?
            && let Event::Key(key) = event::read()?
        {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('t') => {
                    app.theme_idx = (app.theme_idx + 1) % THEMES.len();
                    app.sfx.play(Sound::UiTick);
                }
                // Toggle the alert popup — the siren fires the instant it appears.
                KeyCode::Char('a') => {
                    app.alert_visible = !app.alert_visible;
                    if app.alert_visible {
                        app.alert_state.flash(8);
                        app.sfx.play(Sound::AlertSiren);
                    }
                }
                KeyCode::Enter if app.alert_visible => app.alert_visible = false,
                // ScanList navigation — a tick per cursor move.
                KeyCode::Down => {
                    app.log.selected = (app.log.selected + 1) % LOG_ITEMS.len();
                    app.sfx.play(Sound::UiTick);
                }
                KeyCode::Up => {
                    app.log.selected =
                        (app.log.selected + LOG_ITEMS.len() - 1) % LOG_ITEMS.len();
                    app.sfx.play(Sound::UiTick);
                }
                _ => {}
            }
        }
    }

    teardown(&mut terminal)?;
    Ok(())
}

fn draw(f: &mut ratatui::Frame<'_>, app: &mut App) {
    let theme = app.theme();
    let area = f.area();

    // Root background.
    f.render_widget(Block::new().style(Style::new().bg(theme.palette().bg.color())), area);

    // Boot intro plays first, then the HUD.
    if app.frame <= BOOT_TICKS {
        let boot = BootSequence::new(BOOT_LINES.iter().copied()).ticks_per_line(7).theme(theme);
        f.render_stateful_widget(boot, area, &mut app.boot);
        return;
    }

    let outer = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(area);

    // Header: glitching title.
    let title = GlitchText::new("▶ SCI-FI HUD // ratatui-sci-fi ◀")
        .intensity(0.15)
        .theme(theme);
    f.render_stateful_widget(title, outer[0], &mut app.title);

    // Body: telemetry | radar | (vitals + event log).
    let body = Layout::horizontal([
        Constraint::Length(32),
        Constraint::Min(20),
        Constraint::Length(34),
    ])
    .split(outer[1]);

    telemetry_panel(f, theme, body[0], app.frame);
    f.render_stateful_widget(
        SciFiRadar::new().sweep_speed(0.2).theme(theme),
        body[1],
        &mut app.radar,
    );

    let right = Layout::vertical([Constraint::Length(10), Constraint::Min(1)]).split(body[2]);
    f.render_stateful_widget(
        BiometricChart::new(3).window(60).theme(theme),
        right[0],
        &mut app.bio,
    );
    let log = ScanList::new(LOG_ITEMS.iter().copied()).theme(theme);
    f.render_stateful_widget(log, right[1], &mut app.log);

    // Footer: hints.
    f.render_widget(
        Paragraph::new(" [↑↓] list   [t] theme   [a] alert   [q] exit")
            .style(Style::new().fg(theme.palette().muted.color())),
        outer[2],
    );

    // Alert popup overlay. The siren already fired in the key handler the
    // moment the popup was shown — this just paints it, centered, on top.
    if app.alert_visible {
        let popup_area = centered_rect(56, 7, area);
        f.render_widget(Clear, popup_area);
        let popup = AlertPopup::new("HULL BREACH — SEALING")
            .title(" ⚠ ALERT ")
            .theme(theme);
        f.render_stateful_widget(popup, popup_area, &mut app.alert_state);
    }
}

/// A rect `percent_x%` wide and `height` rows tall, centered in `area`.
fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    // Center vertically with a fixed-height middle band.
    let vert = Layout::vertical([Constraint::Min(0), Constraint::Length(height), Constraint::Min(0)])
        .split(area);
    // Center horizontally by percentage.
    let pad = 100u16.saturating_sub(percent_x) / 2;
    Layout::horizontal([
        Constraint::Percentage(pad),
        Constraint::Percentage(percent_x),
        Constraint::Percentage(pad),
    ])
    .split(vert[1])[1]
}

/// Left panel: a `TargetLock` HUD frame wrapping four oscillating `EnergyGauge`s.
fn telemetry_panel(f: &mut ratatui::Frame<'_>, theme: Theme, area: ratatui::layout::Rect, tick: u64) {
    let lock = TargetLock::new().title("TELEMETRY").theme(theme);
    let inner = lock.inner(area);
    f.render_widget(lock, area);

    let gauges: [(&str, f64); 4] = [
        ("CORE", 0.07),
        ("PWR", 0.05),
        ("HULL", 0.03),
        ("SHLD", 0.09),
    ];
    let rows = Layout::vertical([
        Constraint::Min(1),
        Constraint::Min(1),
        Constraint::Min(1),
        Constraint::Min(1),
    ])
    .split(inner);

    let t = tick as f64;
    for (i, (label, freq)) in gauges.iter().enumerate() {
        // Oscillate in ~0.25..0.95 so all three gauge colors get exercised.
        let ratio = 0.60 + (t * freq).sin() * 0.30;
        let gauge = EnergyGauge::new(ratio).label(*label).segments(16).theme(theme);
        f.render_widget(gauge, rows[i]);
    }
}

fn setup() -> io::Result<Term> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(stdout))
}

fn teardown(term: &mut Term) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
