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
    BootSequenceState, DigitalClock, DigitalClockState, Divider, EnergyGauge, GlitchText,
    GlitchTextState, Level, Panel, ProgressBar, ProgressBarState, ScanList, ScanListState,
    SciFiRadar, SciFiRadarState, SignalBars, StatusLED, Theme, Value,
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

const THEMES: [Theme; 8] = [
    Theme::Cyberpunk,
    Theme::Fallout,
    Theme::Weyland,
    Theme::DeepSpace,
    Theme::Bloodmoon,
    Theme::Nebula,
    Theme::Arctic,
    Theme::Sentinel,
];

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

pub struct App {
    frame: u64,
    theme_idx: usize,
    radar: SciFiRadarState,
    bio: BiometricChartState,
    log: ScanListState,
    title: GlitchTextState,
    boot: BootSequenceState,
    clock: DigitalClockState,
    progress: ProgressBarState,
    alert_state: AlertPopupState,
    alert_visible: bool,
    sfx: Sfx,
}

impl App {
    pub fn new() -> Self {
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
            clock: DigitalClockState::new(),
            progress: ProgressBarState::new(),
            alert_state: AlertPopupState::default(),
            alert_visible: false,
            sfx: Sfx::new(),
        }
    }

    pub fn theme(&self) -> Theme {
        THEMES[self.theme_idx]
    }

    /// Cycle to the next theme — mirrors the `t` key, exposed so the headless
    /// screenshot harness can showcase every palette in one looping capture.
    pub fn cycle_theme(&mut self) {
        self.theme_idx = (self.theme_idx + 1) % THEMES.len();
    }

    pub fn tick(&mut self) {
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
        self.clock.tick();
        self.progress.tick();
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

pub fn draw(f: &mut ratatui::Frame<'_>, app: &mut App) {
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
        Constraint::Length(3), // header
        Constraint::Min(1),    // body
        Constraint::Length(3), // status strip
        Constraint::Length(1), // footer
    ])
    .split(area);

    // Header: glitching title (left) + live digital clock (right).
    let header = Layout::horizontal([Constraint::Min(1), Constraint::Length(12)]).split(outer[0]);
    let title = GlitchText::new("▶ SCI-FI HUD // ratatui-sci-fi ◀")
        .intensity(0.15)
        .theme(theme);
    f.render_stateful_widget(title, header[0], &mut app.title);
    let (hh, mm, ss) = utc_hms();
    f.render_stateful_widget(DigitalClock::new(hh, mm, ss).theme(theme), header[1], &mut app.clock);

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

    let right =
        Layout::vertical([Constraint::Length(10), Constraint::Length(1), Constraint::Min(1)])
            .split(body[2]);
    f.render_stateful_widget(
        BiometricChart::new(3).window(60).theme(theme),
        right[0],
        &mut app.bio,
    );
    // A `Divider` rule with a centered label separates the vitits from the log.
    f.render_widget(Divider::new().label("EVENT LOG").theme(theme), right[1]);
    let log = ScanList::new(LOG_ITEMS.iter().copied()).theme(theme);
    f.render_stateful_widget(log, right[2], &mut app.log);

    // Status strip: link LED + signal bars + a cycling progress bar.
    status_row(f, theme, outer[2], app.frame, &mut app.progress);

    // Footer: hints.
    f.render_widget(
        Paragraph::new(" [↑↓] list   [t] theme   [a] alert   [q] exit")
            .style(Style::new().fg(theme.palette().muted.color())),
        outer[3],
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

/// Bottom status strip: a link LED, signal bars, and a cycling progress bar.
fn status_row(
    f: &mut ratatui::Frame<'_>,
    theme: Theme,
    area: Rect,
    frame: u64,
    progress: &mut ProgressBarState,
) {
    let cols = Layout::horizontal([
        Constraint::Length(16),
        Constraint::Length(10),
        Constraint::Min(16),
    ])
    .split(area);
    f.render_widget(StatusLED::new("LINK").level(Level::Ok).theme(theme), cols[0]);
    let signal = ((frame / 12) % 6) as u8;
    f.render_widget(SignalBars::new(signal).bars(5).theme(theme), cols[1]);
    let ratio = (frame % 100) as f32 / 100.0;
    f.render_stateful_widget(ProgressBar::new(Some(ratio)).theme(theme), cols[2], progress);
}

/// Current UTC wall-clock as (hours, minutes, seconds).
fn utc_hms() -> (u32, u32, u32) {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let day = secs % 86400;
    ((day / 3600) as u32, ((day % 3600) / 60) as u32, (day % 60) as u32)
}

/// Left panel: a `Panel` HUD frame wrapping four oscillating `EnergyGauge`s,
/// each preceded by a status `Value` readout. The `Panel` replaces the old
/// `TargetLock` wrapper here (TargetLock is still in the gallery for its
/// crosshair look); `Value` adds a labeled level-colored readout above each
/// gauge so a glance conveys both the trend and the status.
fn telemetry_panel(f: &mut ratatui::Frame<'_>, theme: Theme, area: ratatui::layout::Rect, tick: u64) {
    let panel = Panel::new().title("TELEMETRY").theme(theme);
    let inner = panel.inner(area);
    f.render_widget(panel, area);

    let gauges: [(&str, f64); 4] = [
        ("CORE", 0.07),
        ("PWR", 0.05),
        ("HULL", 0.03),
        ("SHLD", 0.09),
    ];
    // Each gauge gets a 1-row label + 1-row bar slot.
    let mut constraints = Vec::with_capacity(gauges.len() * 2);
    for _ in &gauges {
        constraints.push(Constraint::Length(1));
        constraints.push(Constraint::Length(1));
    }
    let rows = Layout::vertical(&constraints).split(inner);

    let t = tick as f64;
    for (i, (label, freq)) in gauges.iter().enumerate() {
        // Oscillate in ~0.25..0.95 so all three gauge colors get exercised.
        let ratio = 0.60 + (t * freq).sin() * 0.30;
        let pct = (ratio * 100.0).round() as i32;
        // Match the gauge's own level thresholds (≥0.6 ok, ≥0.3 warn, else alert).
        let level = if ratio >= 0.6 {
            Level::Ok
        } else if ratio >= 0.3 {
            Level::Warn
        } else {
            Level::Alert
        };
        f.render_widget(
            Value::new(format!("{pct}%")).label(*label).state(level).theme(theme),
            rows[i * 2],
        );
        f.render_widget(
            EnergyGauge::new(ratio).label(*label).segments(16).theme(theme),
            rows[i * 2 + 1],
        );
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
