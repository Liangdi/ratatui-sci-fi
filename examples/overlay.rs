//! **Overlay** — CRT scanlines + snow over live content.
//!
//! Demonstrates the ambient overlay layer: a [`Panel`] of telemetry readouts
//! is drawn first, then a [`Noise`] pass (toggle with `n`) and finally a
//! [`ScanlineOverlay`] paint the whole screen as a phosphor terminal. This is
//! the only widget family that renders *over* everything else.
//!
//! `n` toggles noise · `t` cycles theme · `q` / `Esc` quits.
//!
//! ```sh
//! cargo run -p ratatui-sci-fi --example overlay
//! ```
//!
//! [`Panel`]: ratatui_sci_fi::Panel
//! [`Noise`]: ratatui_sci_fi::Noise
//! [`ScanlineOverlay`]: ratatui_sci_fi::ScanlineOverlay

use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Paragraph},
    Terminal,
};
use ratatui_sci_fi::{
    Noise, NoiseState, Panel, ScanlineOverlay, ScanlineOverlayState, ScanlineShape, Theme, Value,
};

type Term = Terminal<CrosstermBackend<Stdout>>;

const THEMES: [Theme; 4] = [Theme::Cyberpunk, Theme::Fallout, Theme::Weyland, Theme::DeepSpace];
const TITLE: &str = "▶  CRT OVERLAY  ◀";

pub struct App {
    frame: u64,
    theme_idx: usize,
    show_noise: bool,
    scan: ScanlineOverlayState,
    noise: NoiseState,
}

impl App {
    pub fn new() -> Self {
        Self {
            frame: 0,
            theme_idx: 0,
            show_noise: true,
            scan: ScanlineOverlayState::new(),
            noise: NoiseState::new(),
        }
    }

    pub fn theme(&self) -> Theme {
        THEMES[self.theme_idx]
    }

    pub fn cycle_theme(&mut self) {
        self.theme_idx = (self.theme_idx + 1) % THEMES.len();
    }

    pub fn tick(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        self.scan.tick();
        self.noise.tick();
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
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
                KeyCode::Char('t') => app.cycle_theme(),
                KeyCode::Char('n') => app.show_noise = !app.show_noise,
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

    // 1. Background fill + content.
    f.render_widget(Block::new().style(Style::new().bg(theme.palette().bg.color())), area);
    let outer =
        Layout::vertical([Constraint::Length(3), Constraint::Min(1), Constraint::Length(1)])
            .split(area);
    render_title(f, theme, outer[0]);
    telemetry_panel(f, theme, outer[1], app.frame);
    render_footer(f, theme, outer[2], app.show_noise);

    // 2. Overlay pass — render LAST, over the whole screen. Noise first, then
    //    the scanline sweep on top.
    if app.show_noise {
        f.render_stateful_widget(
            Noise::new().intensity(0.30).theme(theme),
            area,
            &mut app.noise,
        );
    }
    f.render_stateful_widget(
        ScanlineOverlay::new(ScanlineShape::SweepAndVignette).theme(theme),
        area,
        &mut app.scan,
    );
}

fn render_title(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect) {
    let accent = theme.palette().accent.color();
    let title = Paragraph::new(Line::from(TITLE))
        .alignment(Alignment::Center)
        .style(Style::new().fg(accent).add_modifier(Modifier::BOLD));
    f.render_widget(title, vertically_centered(area, 1));
}

fn render_footer(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, noise_on: bool) {
    let muted = theme.palette().muted.color();
    let state = if noise_on { "ON" } else { "OFF" };
    let footer = Paragraph::new(Line::from(format!(
        "n noise [{state}] · t theme · q quit"
    )))
    .alignment(Alignment::Center)
    .style(Style::new().fg(muted));
    f.render_widget(footer, vertically_centered(area, 1));
}

/// A centered panel of telemetry readouts — the "signal" the overlay obscures.
fn telemetry_panel(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, frame: u64) {
    let panel = Panel::new().title("TRANSMISSION").theme(theme);
    let inner = panel.inner(area);
    f.render_widget(panel, area);

    let rows = Layout::vertical([
        Constraint::Min(1),
        Constraint::Min(1),
        Constraint::Min(1),
        Constraint::Min(1),
    ])
    .split(inner);

    let t = frame as f32 * 0.05;
    let pwr = (0.6 + t.sin() * 0.3).clamp(0.0, 1.0);
    let snr = (42.0 + (t * 1.3).sin() * 6.0).round() as i32;
    let rows_data: [(String, &str, ratatui_sci_fi::Level); 4] = [
        (format!("{:.0}%", pwr * 100.0), "PWR", ratatui_sci_fi::Level::Ok),
        (format!("{snr} dB"), "SNR", ratatui_sci_fi::Level::Warn),
        ("NOMINAL".to_string(), "LINK", ratatui_sci_fi::Level::Ok),
        ("STANDBY".to_string(), "AUX", ratatui_sci_fi::Level::Alert),
    ];
    for (i, (val, label, level)) in rows_data.iter().enumerate() {
        f.render_widget(
            Value::new(val.clone()).label(*label).state(*level).theme(theme),
            rows[i],
        );
    }
}

/// A `height`-tall slice vertically centered within `area`.
fn vertically_centered(area: Rect, height: u16) -> Rect {
    Layout::vertical([Constraint::Min(0), Constraint::Length(height), Constraint::Min(0)])
        .split(area)[1]
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
