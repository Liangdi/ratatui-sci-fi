//! **Charts** — a dedicated gallery for the three sci-fi chart widgets:
//! [`SpectrumBars`] (animated spectrum analyzer), [`RadialGauge`] (reactor
//! dial), and [`HeatGrid`] (sensor-array heatmap). All three animate every
//! frame in their self-generated demo mode.
//!
//! Layout: the **SPECTRUM** panel (left) and the **REACTOR** dial (right) sit
//! on the top row; the **SENSOR ARRAY** heatmap fills the bottom.
//!
//! `t` cycles all eight themes · `s` cycles each widget's shape variant in
//! lockstep · `q` / `Esc` quits.
//!
//! ```sh
//! cargo run -p ratatui-sci-fi --example charts
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
    widgets::{Block, Paragraph},
    Terminal,
};
use ratatui_sci_fi::{
    DialShape, HeatGrid, HeatGridState, HeatShape, Panel, RadialGauge, RadialGaugeState,
    SpectrumBars, SpectrumBarsState, SpectrumShape, Theme,
};

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

/// Every [`SpectrumShape`], in display order.
const SPECTRUM_SHAPES: [SpectrumShape; 4] =
    [SpectrumShape::Bar, SpectrumShape::Block, SpectrumShape::Cell, SpectrumShape::Ascii];

/// Every [`DialShape`], in display order.
const DIAL_SHAPES: [DialShape; 3] = [DialShape::Needle, DialShape::Arc, DialShape::Tick];

/// Every [`HeatShape`], in display order.
const HEAT_SHAPES: [HeatShape; 4] =
    [HeatShape::Block, HeatShape::Shade, HeatShape::Ascii, HeatShape::Dot];

pub struct App {
    theme_idx: usize,
    shape_idx: usize,
    spectrum: SpectrumBarsState,
    dial: RadialGaugeState,
    heat: HeatGridState,
}

impl App {
    pub fn new() -> Self {
        Self {
            theme_idx: 0,
            shape_idx: 0,
            spectrum: SpectrumBarsState::new(16, 64),
            dial: RadialGaugeState::new(),
            heat: HeatGridState::new(7, 40),
        }
    }

    pub fn theme(&self) -> Theme {
        THEMES[self.theme_idx]
    }

    pub fn cycle_theme(&mut self) {
        self.theme_idx = (self.theme_idx + 1) % THEMES.len();
    }

    pub fn cycle_shape(&mut self) {
        self.shape_idx = self.shape_idx.wrapping_add(1);
    }

    /// The [`SpectrumShape`] for the current shape index.
    pub fn spectrum_shape(&self) -> SpectrumShape {
        SPECTRUM_SHAPES[self.shape_idx % SPECTRUM_SHAPES.len()]
    }

    /// The [`DialShape`] for the current shape index.
    pub fn dial_shape(&self) -> DialShape {
        DIAL_SHAPES[self.shape_idx % DIAL_SHAPES.len()]
    }

    /// The [`HeatShape`] for the current shape index.
    pub fn heat_shape(&self) -> HeatShape {
        HEAT_SHAPES[self.shape_idx % HEAT_SHAPES.len()]
    }

    /// Advance every widget's animation by one tick. Called once per frame so
    /// the spectrum, dial, and heatmap all move on their own (demo mode).
    pub fn tick(&mut self) {
        self.spectrum.tick();
        self.dial.tick();
        self.heat.tick();
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
        app.tick();
        terminal.draw(|f| draw(f, &mut app))?;

        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
        {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('t') => app.cycle_theme(),
                KeyCode::Char('s') => app.cycle_shape(),
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

    let outer = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(13),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(area);

    render_title(f, theme, outer[0]);
    render_top_row(f, app, outer[1]);
    render_heat(f, app, outer[2]);
    render_footer(f, app, outer[3]);
}

/// Centered header title.
fn render_title(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect) {
    let band = Layout::vertical([Constraint::Min(0), Constraint::Length(1), Constraint::Min(0)])
        .split(area)[1];
    let label = "▶  CHARTS · SCI-FI DATA VIZ  ◀";
    let w = label.chars().count() as u16;
    let x = band.x + band.width.saturating_sub(w) / 2;
    f.render_widget(
        Paragraph::new(label).style(Style::new().fg(theme.palette().accent.color())),
        Rect::new(x, band.y, w.min(band.width), 1),
    );
}

/// Top row: the spectrum analyzer (left) beside the reactor dial (right).
fn render_top_row(f: &mut ratatui::Frame<'_>, app: &mut App, area: Rect) {
    let cols =
        Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)]).split(area);
    render_spectrum(f, app, cols[0]);
    render_dial(f, app, cols[1]);
}

/// The animated vertical-bar spectrum analyzer.
fn render_spectrum(f: &mut ratatui::Frame<'_>, app: &mut App, area: Rect) {
    let theme = app.theme();
    let shape = app.spectrum_shape();
    let panel = Panel::new().title("SPECTRUM").theme(theme);
    let content = panel.inner(area);
    f.render_widget(panel, area);
    f.render_stateful_widget(
        SpectrumBars::new().shape(shape).theme(theme),
        content,
        &mut app.spectrum,
    );
}

/// The self-wandering reactor-core dial.
fn render_dial(f: &mut ratatui::Frame<'_>, app: &mut App, area: Rect) {
    let theme = app.theme();
    let shape = app.dial_shape();
    let panel = Panel::new().title("REACTOR").theme(theme);
    let content = panel.inner(area);
    f.render_widget(panel, area);
    f.render_stateful_widget(
        RadialGauge::new().shape(shape).label("CORE").theme(theme),
        content,
        &mut app.dial,
    );
}

/// The bottom sensor-array heatmap.
fn render_heat(f: &mut ratatui::Frame<'_>, app: &mut App, area: Rect) {
    let theme = app.theme();
    let shape = app.heat_shape();
    let panel = Panel::new().title("SENSOR ARRAY").theme(theme);
    let content = panel.inner(area);
    f.render_widget(panel, area);
    f.render_stateful_widget(
        HeatGrid::new().shape(shape).theme(theme),
        content,
        &mut app.heat,
    );
}

fn render_footer(f: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let theme = app.theme();
    let name = format!("{:?}", theme);
    let shapes = format!(
        "spec={:?} dial={:?} heat={:?}",
        app.spectrum_shape(),
        app.dial_shape(),
        app.heat_shape(),
    );
    f.render_widget(
        Paragraph::new(format!(
            " [t] theme: {name:<11} [s] shape: {shapes}   [q] quit"
        ))
        .style(Style::new().fg(theme.palette().muted.color())),
        area,
    );
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
