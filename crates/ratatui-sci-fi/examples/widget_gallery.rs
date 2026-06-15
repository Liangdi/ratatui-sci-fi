//! **Widget gallery** — every widget in its own labelled cell, side by side.
//!
//! Where the `dashboard` example composites the widgets into a single HUD, this
//! one isolates each component in its own panel so you can see exactly what it
//! does and how it reacts. The 5×3 grid showcases all fifteen grid widgets,
//! grouped top→bottom by kind (basics → containers → charts → effects):
//!
//! ```text
//! ┌──────────────┬───────────────┬──────────────┐
//! │ BUTTONS      │ TOGGLE        │ VALUE        │   basics
//! ├──────────────┼───────────────┼──────────────┤
//! │ SPINNER      │ TEXT INPUT    │ DIVIDER      │   basics
//! ├──────────────┼───────────────┼──────────────┤
//! │ PANEL        │ TARGET LOCK   │ ENERGY GAUGE │   containers
//! ├──────────────┼───────────────┼──────────────┤
//! │ RADAR        │ BIOMETRICS    │ SCAN LIST    │   charts
//! ├──────────────┼───────────────┼──────────────┤
//! │ BOOT SEQUENCE│ MATRIX RAIN   │ GLITCH TEXT  │   effects
//! └──────────────┴───────────────┴──────────────┘
//! ```
//!
//! The `AlertPopup` is an overlay — press `a` to pop it.
//!
//! `←/→` move button focus · `↑/↓` move the list cursor · type into the text
//! input · `space` toggles the toggle · `t` cycles the four themes · `a` toggles
//! the alert popup · `q` / `Esc` quits.
//!
//! ```sh
//! cargo run -p ratatui-sci-fi --example widget_gallery
//! ```

use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::Style,
    widgets::{Block, Clear, Paragraph},
    Terminal,
};
use ratatui_sci_fi::{
    AlertPopup, AlertPopupState, BiometricChart, BiometricChartState, Blip, BootSequence,
    BootSequenceState, Button, Divider, EnergyGauge, GlitchText, GlitchTextState, Level,
    MatrixRain, MatrixRainState, Panel, ScanList, ScanListState, SciFiRadar, SciFiRadarState,
    Spinner, SpinnerState, TargetLock, TextInput, TextInputState, Theme, Toggle, Value,
};

type Term = Terminal<CrosstermBackend<Stdout>>;

const THEMES: [Theme; 4] = [Theme::Cyberpunk, Theme::Fallout, Theme::Weyland, Theme::DeepSpace];
const THEME_NAMES: [&str; 4] = ["Cyberpunk", "Fallout", "Weyland", "DeepSpace"];

/// Centered header title (ASCII + width-1 glyphs, so `chars().count()` is its
/// display width).
const TITLE: &str = "▶  SCI-FI WIDGET GALLERY  ◀";

/// Buttons shown in the BUTTONS cell; `button_focus` indexes into this.
const BUTTONS: [&str; 3] = ["ENGAGE", "SCAN", "ABORT"];

/// Rows shown in the SCAN LIST cell; `log.selected` indexes into this.
const LOG_ITEMS: [&str; 4] = ["PORT DOCK: OK", "SCAN COMPLETE", "TELEMETRY LIVE", "ANOMALY FOUND"];

/// Lines revealed one-by-one in the BOOT SEQUENCE cell. The widget itself
/// colors `[ OK ]` / `OK` lines green, `ERROR`/`FAIL` lines alert-red, and the
/// rest in the theme foreground — so this mix exercises all three.
const BOOT_LINES: [&str; 5] = [
    "BIOS v2.41  (c) W-Y INDUSTRIES",
    "[ OK ] CPU CORE x8 .... NOMINAL",
    "[ OK ] MEMORY 640K ..... OK",
    "[WARN] REACTOR ......... 78%",
    "ERROR HULL INTEGRITY .... 64%",
];

/// Ticks per revealed boot line. Drives the replay-cycle math in `tick`.
const BOOT_TPL: u64 = 9;
/// Extra ticks the finished boot log lingers before the reveal replays.
const BOOT_PAUSE: u64 = 45;

pub struct App {
    frame: u64,
    theme_idx: usize,
    button_focus: usize,
    toggle_on: bool,
    /// Drives the boot replay loop, independent of each state's own clock.
    boot_clock: u64,
    alert_visible: bool,
    title: GlitchTextState,
    glitch_a: GlitchTextState,
    glitch_b: GlitchTextState,
    radar: SciFiRadarState,
    bio: BiometricChartState,
    log: ScanListState,
    boot: BootSequenceState,
    rain: MatrixRainState,
    spinner: SpinnerState,
    input: TextInputState,
    alert: AlertPopupState,
}

impl App {
    pub fn new() -> Self {
        let mut radar = SciFiRadarState::default();
        radar.push_blip(Blip::new(0.7, 0.55, 0.9));
        radar.push_blip(Blip::new(2.4, 0.75, 0.7));
        radar.push_blip(Blip::new(4.1, 0.35, 0.8));
        Self {
            frame: 0,
            theme_idx: 0,
            button_focus: 0,
            toggle_on: true,
            boot_clock: 0,
            alert_visible: false,
            title: GlitchTextState::default(),
            glitch_a: GlitchTextState::default(),
            glitch_b: GlitchTextState::default(),
            radar,
            bio: BiometricChartState::new(3, 60),
            log: ScanListState::default(),
            boot: BootSequenceState::default(),
            rain: MatrixRainState::default(),
            spinner: SpinnerState::default(),
            input: TextInputState::default(),
            alert: AlertPopupState::default(),
        }
    }

    pub fn theme(&self) -> Theme {
        THEMES[self.theme_idx]
    }

    /// Cycle to the next theme — mirrors the `t` key, exposed for the headless
    /// screenshot harness so one looping capture can showcase every palette.
    pub fn cycle_theme(&mut self) {
        self.theme_idx = (self.theme_idx + 1) % THEMES.len();
    }

    /// Advance every widget's animation clock one tick.
    pub fn tick(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        self.title.tick();
        self.glitch_a.tick();
        self.glitch_b.tick();
        self.radar.tick();
        self.bio.tick();
        self.log.tick();
        self.rain.tick();
        self.spinner.tick();
        self.input.tick();

        // Boot reveal: advance, then replay after a pause so the cell never
        // sits idle.
        self.boot.tick();
        self.boot_clock = self.boot_clock.wrapping_add(1);
        let cycle = (BOOT_LINES.len() as u64) * BOOT_TPL + BOOT_PAUSE;
        if self.boot_clock >= cycle {
            self.boot_clock = 0;
            self.boot = BootSequenceState::default();
        }

        if self.alert_visible {
            self.alert.tick();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut terminal = setup()?;
    let mut app = App::new();

    loop {
        terminal.draw(|f| draw(f, &mut app))?;
        app.tick();

        if event::poll(Duration::from_millis(60))?
            && let Event::Key(key) = event::read()?
        {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('t') => app.theme_idx = (app.theme_idx + 1) % THEMES.len(),
                // Toggle the alert popup; arm its show-flash the moment it opens.
                KeyCode::Char('a') => {
                    app.alert_visible = !app.alert_visible;
                    if app.alert_visible {
                        app.alert.flash(8);
                    }
                }
                KeyCode::Enter if app.alert_visible => app.alert_visible = false,
                // `space` flips the toggle.
                KeyCode::Char(' ') if !app.alert_visible => app.toggle_on = !app.toggle_on,
                // Typing into the text input. Letters/digits/backspace/arrows
                // go to the field; navigation keys only when not already
                // claimed by another widget below.
                KeyCode::Char(c) if !app.alert_visible && c != ' ' => app.input.handle_key(key),
                KeyCode::Backspace if !app.alert_visible => app.input.handle_key(key),
                KeyCode::Left | KeyCode::Right if !app.alert_visible => app.input.handle_key(key),
                // ↑/↓ steer the list cursor — separate axis from the input.
                KeyCode::Up => {
                    app.log.selected = (app.log.selected + LOG_ITEMS.len() - 1) % LOG_ITEMS.len();
                }
                KeyCode::Down => app.log.selected = (app.log.selected + 1) % LOG_ITEMS.len(),
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

    let outer =
        Layout::vertical([Constraint::Length(3), Constraint::Min(1), Constraint::Length(1)])
            .split(area);

    // Header: glitching title, horizontally + vertically centered in its band.
    render_title(f, theme, outer[0], &mut app.title);

    // 5×3 grid of labelled cells.
    let rows = Layout::vertical([
        Constraint::Min(1),
        Constraint::Min(1),
        Constraint::Min(1),
        Constraint::Min(1),
        Constraint::Min(1),
    ])
    .split(outer[1]);
    let split3 = |row: Rect| {
        Layout::horizontal([Constraint::Min(1), Constraint::Min(1), Constraint::Min(1)]).split(row)
    };
    let r0 = split3(rows[0]);
    let r1 = split3(rows[1]);
    let r2 = split3(rows[2]);
    let r3 = split3(rows[3]);
    let r4 = split3(rows[4]);

    // Row 0 — basics.
    buttons_cell(f, theme, r0[0], app);
    toggle_cell(f, theme, r0[1], app);
    value_cell(f, theme, r0[2], app.frame);

    // Row 1 — basics.
    spinner_cell(f, theme, r1[0], app);
    input_cell(f, theme, r1[1], app);
    divider_cell(f, theme, r1[2]);

    // Row 2 — containers.
    panel_cell(f, theme, r2[0], app.frame);
    target_lock_cell(f, theme, r2[1], app);
    gauges_cell(f, theme, r2[2], app.frame);

    // Row 3 — charts.
    let radar_area = cell(f, theme, r3[0], "RADAR");
    f.render_stateful_widget(
        SciFiRadar::new().sweep_speed(0.18).theme(theme),
        radar_area,
        &mut app.radar,
    );

    let bio_area = cell(f, theme, r3[1], "BIOMETRICS");
    f.render_stateful_widget(
        BiometricChart::new(3).window(60).theme(theme),
        bio_area,
        &mut app.bio,
    );

    let list_area = cell(f, theme, r3[2], "SCAN LIST");
    f.render_stateful_widget(
        ScanList::new(LOG_ITEMS.iter().copied()).theme(theme),
        list_area,
        &mut app.log,
    );

    // Row 4 — effects.
    boot_cell(f, theme, r4[0], app);

    let rain_area = cell(f, theme, r4[1], "MATRIX RAIN");
    f.render_stateful_widget(
        MatrixRain::new().density(0.85).speed(0.5).theme(theme),
        rain_area,
        &mut app.rain,
    );

    glitch_cell(f, theme, r4[2], app);

    // Footer: keymap + active theme.
    f.render_widget(
        Paragraph::new(format!(
            " [←→] input  [↑↓] list  [space] toggle  [t] theme: {}  [a] alert  [q] quit",
            THEME_NAMES[app.theme_idx]
        ))
        .style(Style::new().fg(theme.palette().muted.color())),
        outer[2],
    );

    // Alert overlay, centered on top of everything.
    if app.alert_visible {
        let popup_area = centered_rect(60, 7, area);
        f.render_widget(Clear, popup_area);
        let popup = AlertPopup::new("SYSTEM ALERT — STANDBY").title(" ⚠ ALERT ").theme(theme);
        f.render_stateful_widget(popup, popup_area, &mut app.alert);
    }
}

/// Centered, glitching header title.
fn render_title(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, state: &mut GlitchTextState) {
    // Vertical middle row of the band.
    let band = Layout::vertical([Constraint::Min(0), Constraint::Length(1), Constraint::Min(0)])
        .split(area)[1];
    let title_w = TITLE.chars().count() as u16;
    let width = title_w.min(band.width);
    let x = band.x + band.width.saturating_sub(title_w) / 2;
    let title_area = Rect::new(x, band.y, width, 1);
    f.render_stateful_widget(
        GlitchText::new(TITLE).intensity(0.12).theme(theme),
        title_area,
        state,
    );
}

/// Render a labelled cell header (`▸ TITLE` in accent, padded with a muted
/// `─` rule) and return the content area beneath.
fn cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, title: &str) -> Rect {
    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).split(area);
    // The header is a `▸ TITLE` accent label punched through a muted rule —
    // exactly what `Divider` renders, so reuse it rather than hand-rolling
    // `Span`s.
    let label = format!("▸ {title}");
    f.render_widget(Divider::new().label(label).theme(theme), chunks[0]);
    chunks[1]
}

/// Three stacked buttons; the focused one (driven by `button_focus`) renders
/// in the energy-arrow style.
fn buttons_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let inner = cell(f, theme, area, "BUTTONS");
    let rows = Layout::vertical([Constraint::Min(1), Constraint::Min(1), Constraint::Min(1)])
        .split(inner);
    for (i, label) in BUTTONS.iter().enumerate() {
        let button = Button::new(*label).focused(i == app.button_focus).theme(theme);
        f.render_widget(button, rows[i]);
    }
}

/// A single toggle; `space` flips `toggle_on`.
fn toggle_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let inner = cell(f, theme, area, "TOGGLE");
    let rows = Layout::vertical([Constraint::Min(1), Constraint::Min(1)]).split(inner);
    f.render_widget(
        Toggle::new("SHIELDS").on(app.toggle_on).theme(theme),
        rows[0],
    );
    f.render_widget(
        Toggle::new("CLOAK").on(!app.toggle_on).theme(theme),
        rows[1],
    );
}

/// Three telemetry readouts, each cycling a different `Level` so all four
/// value colors are on screen.
fn value_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, frame: u64) {
    let inner = cell(f, theme, area, "VALUE");
    let rows = Layout::vertical([Constraint::Min(1), Constraint::Min(1), Constraint::Min(1)])
        .split(inner);

    let t = frame as f64;
    let hp = (82.0 + (t * 0.05).sin() * 6.0).round() as i32;
    let fuel = (47.0 + (t * 0.03).sin() * 8.0).round() as i32;
    let o2 = (21.0 + (t * 0.07).sin() * 8.0).round() as i32;

    f.render_widget(Value::new(format!("{hp}%")).label("HULL").state(Level::Ok).theme(theme), rows[0]);
    f.render_widget(
        Value::new(format!("{fuel}%")).label("FUEL").state(Level::Warn).theme(theme),
        rows[1],
    );
    f.render_widget(
        Value::new(format!("{o2}%")).label("O2").state(Level::Alert).theme(theme),
        rows[2],
    );
}

/// A spinner + label, advancing every tick.
fn spinner_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let inner = cell(f, theme, area, "SPINNER");
    let rows = Layout::vertical([Constraint::Min(1), Constraint::Min(1)]).split(inner);
    f.render_stateful_widget(Spinner::new().label("SYNC").theme(theme), rows[0], &mut app.spinner);
    f.render_stateful_widget(Spinner::new().label("DECRYPT").theme(theme), rows[1], &mut app.spinner);
}

/// A single-line text input. Type to edit; the caret blinks each tick.
fn input_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let inner = cell(f, theme, area, "TEXT INPUT");
    f.render_stateful_widget(
        TextInput::new().placeholder("enter callsign…").theme(theme),
        inner,
        &mut app.input,
    );
}

/// A bare `Divider` with a label, so you can see the widget itself.
fn divider_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect) {
    let inner = cell(f, theme, area, "DIVIDER");
    let rows = Layout::vertical([Constraint::Min(1), Constraint::Min(1), Constraint::Min(1)])
        .split(inner);
    f.render_widget(Divider::new().theme(theme), rows[0]);
    f.render_widget(Divider::new().label("SECTION A").theme(theme), rows[1]);
    f.render_widget(Divider::new().theme(theme), rows[2]);
}

/// A `Panel` (titled double-bordered container) wrapping a couple of `Value`
/// readouts — the basic-container use case.
fn panel_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, frame: u64) {
    let inner = cell(f, theme, area, "PANEL");
    let panel = Panel::new().title("STATUS").theme(theme);
    let content = panel.inner(inner);
    f.render_widget(panel, inner);

    let rows =
        Layout::vertical([Constraint::Min(1), Constraint::Min(1)]).split(content);
    let t = frame as f64;
    let pwr = (0.6 + (t * 0.04).sin() * 0.3).clamp(0.0, 1.0);
    let link = (40.0 + (t * 0.06).sin() * 30.0).round() as i32;
    f.render_widget(
        Value::new(format!("{:.0}%", pwr * 100.0))
            .label("PWR")
            .state(Level::Warn)
            .theme(theme),
        rows[0],
    );
    f.render_widget(
        Value::new(format!("{link}%")).label("LINK").state(Level::Ok).theme(theme),
        rows[1],
    );
}

/// Three gauges held in distinct level bands so all three gauge colors
/// (ok / warn / alert) are always on screen at once.
fn gauges_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, frame: u64) {
    let inner = cell(f, theme, area, "ENERGY GAUGE");
    let rows = Layout::vertical([Constraint::Min(1), Constraint::Min(1), Constraint::Min(1)])
        .split(inner);

    // (label, base, amplitude, frequency) — base±amp keeps each in its band.
    let gauges: [(&str, f64, f64, f64); 3] = [
        ("PWR", 0.80, 0.15, 0.08), // 0.65..0.95 → ok
        ("FUEL", 0.47, 0.10, 0.05), // 0.37..0.57 → warn
        ("O2", 0.16, 0.10, 0.13),  // 0.06..0.26 → alert
    ];
    let t = frame as f64;
    for (i, (label, base, amp, freq)) in gauges.iter().enumerate() {
        let ratio = (base + amp * (t * freq).sin()).clamp(0.0, 1.0);
        f.render_widget(
            EnergyGauge::new(ratio).label(*label).segments(14).theme(theme),
            rows[i],
        );
    }
}

/// Two glitching lines at different intensities to show the corruption range.
fn glitch_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let inner = cell(f, theme, area, "GLITCH TEXT");
    let rows = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Min(0)])
        .split(inner);
    f.render_stateful_widget(
        GlitchText::new("DECRYPT // SECTOR 7G").intensity(0.30).theme(theme),
        rows[0],
        &mut app.glitch_a,
    );
    f.render_stateful_widget(
        GlitchText::new("SIGNAL LOST").intensity(0.12).theme(theme),
        rows[1],
        &mut app.glitch_b,
    );
}

/// Boot log that reveals line-by-line, then loops.
fn boot_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let inner = cell(f, theme, area, "BOOT SEQUENCE");
    let boot = BootSequence::new(BOOT_LINES.iter().copied())
        .ticks_per_line(BOOT_TPL)
        .theme(theme);
    f.render_stateful_widget(boot, inner, &mut app.boot);
}

/// The `TargetLock` frame widget itself, shown framing a label + a focused
/// button in its `inner` content area.
fn target_lock_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, _app: &mut App) {
    let p = theme.palette();
    let inner = cell(f, theme, area, "TARGET LOCK");

    let lock = TargetLock::new().title("LOCK").theme(theme);
    let content = lock.inner(inner);
    f.render_widget(lock, inner);

    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).split(content);
    f.render_widget(
        Paragraph::new("◉ TARGET ACQUIRED").style(Style::new().fg(p.ok.color())),
        chunks[0],
    );
    f.render_widget(Button::new("FIRE").focused(true).theme(theme), chunks[1]);
}

/// A rect `percent_x%` wide and `height` rows tall, centered in `area`.
fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let vert =
        Layout::vertical([Constraint::Min(0), Constraint::Length(height), Constraint::Min(0)])
            .split(area);
    let pad = 100u16.saturating_sub(percent_x) / 2;
    Layout::horizontal([
        Constraint::Percentage(pad),
        Constraint::Percentage(percent_x),
        Constraint::Percentage(pad),
    ])
    .split(vert[1])[1]
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
