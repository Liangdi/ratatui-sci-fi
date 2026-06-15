//! **Charts** — the sci-fi data-viz gallery: seven animated chart widgets side
//! by side. The original three ([`SpectrumBars`] analyzer, [`RadialGauge`]
//! reactor dial, [`HeatGrid`] heatmap) plus four common chart types:
//! [`Sparkline`] (compact trend line), [`DonutChart`] (proportions),
//! [`HBarChart`] (horizontal comparison bars), and [`ScatterPlot`] (X/Y point
//! cloud). Every widget animates each frame in self-generated demo mode.
//!
//! Layout: three two-up rows of panels over the full-width sensor-array
//! heatmap — **SPECTRUM / REACTOR**, **TREND / ALLOCATIONS**,
//! **SUBSYSTEMS / CONTACTS**, then **SENSOR ARRAY**.
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
    DialShape, DonutChart, DonutChartState, DonutShape, HBarChart, HBarChartState, HBarShape,
    HeatGrid, HeatGridState, HeatShape, Panel, RadialGauge, RadialGaugeState, ScatterPlot,
    ScatterPlotState, ScatterShape, SparkShape, Sparkline, SparklineState, SpectrumBars,
    SpectrumBarsState, SpectrumShape, Theme,
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

/// Every [`SparkShape`], in display order.
const SPARK_SHAPES: [SparkShape; 3] = [SparkShape::Line, SparkShape::Block, SparkShape::Dot];

/// Every [`DonutShape`], in display order.
const DONUT_SHAPES: [DonutShape; 3] = [DonutShape::Arc, DonutShape::Thick, DonutShape::Tick];

/// Every [`HBarShape`], in display order.
const HBAR_SHAPES: [HBarShape; 3] = [HBarShape::Cell, HBarShape::Block, HBarShape::Ascii];

/// Every [`ScatterShape`], in display order.
const SCATTER_SHAPES: [ScatterShape; 3] =
    [ScatterShape::Dot, ScatterShape::Cross, ScatterShape::Ring];

/// Number of slices in the [`DonutChart`] demo.
const DONUT_SLICES: usize = 5;
/// Number of categories in the [`HBarChart`] demo.
const HBAR_ROWS: usize = 4;
/// Number of points in the [`ScatterPlot`] demo.
const SCATTER_POINTS: usize = 24;

pub struct App {
    theme_idx: usize,
    shape_idx: usize,
    spectrum: SpectrumBarsState,
    dial: RadialGaugeState,
    heat: HeatGridState,
    spark: SparklineState,
    donut: DonutChartState,
    hbar: HBarChartState,
    scatter: ScatterPlotState,
}

impl App {
    pub fn new() -> Self {
        Self {
            theme_idx: 0,
            shape_idx: 0,
            spectrum: SpectrumBarsState::new(16, 64),
            dial: RadialGaugeState::new(),
            heat: HeatGridState::new(7, 40),
            spark: SparklineState::new(64),
            donut: DonutChartState::new(DONUT_SLICES),
            hbar: HBarChartState::new(HBAR_ROWS),
            scatter: ScatterPlotState::new(SCATTER_POINTS),
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

    /// The [`SparkShape`] for the current shape index.
    pub fn spark_shape(&self) -> SparkShape {
        SPARK_SHAPES[self.shape_idx % SPARK_SHAPES.len()]
    }

    /// The [`DonutShape`] for the current shape index.
    pub fn donut_shape(&self) -> DonutShape {
        DONUT_SHAPES[self.shape_idx % DONUT_SHAPES.len()]
    }

    /// The [`HBarShape`] for the current shape index.
    pub fn hbar_shape(&self) -> HBarShape {
        HBAR_SHAPES[self.shape_idx % HBAR_SHAPES.len()]
    }

    /// The [`ScatterShape`] for the current shape index.
    pub fn scatter_shape(&self) -> ScatterShape {
        SCATTER_SHAPES[self.shape_idx % SCATTER_SHAPES.len()]
    }

    /// Advance every widget's animation by one tick. Called once per frame so
    /// all seven widgets move on their own (demo mode).
    pub fn tick(&mut self) {
        self.spectrum.tick();
        self.dial.tick();
        self.heat.tick();
        self.spark.tick();
        self.donut.tick();
        self.hbar.tick();
        self.scatter.tick();
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
        Constraint::Length(11),
        Constraint::Length(11),
        Constraint::Length(11),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(area);

    render_title(f, theme, outer[0]);
    render_pair(f, app, outer[1], render_spectrum, render_dial);
    render_pair(f, app, outer[2], render_spark, render_donut);
    render_pair(f, app, outer[3], render_hbar, render_scatter);
    render_heat(f, app, outer[4]);
    render_footer(f, app, outer[5]);
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

/// A two-up panel row: `left` (58%) beside `right` (42%).
fn render_pair(
    f: &mut ratatui::Frame<'_>,
    app: &mut App,
    area: Rect,
    left: fn(&mut ratatui::Frame<'_>, &mut App, Rect),
    right: fn(&mut ratatui::Frame<'_>, &mut App, Rect),
) {
    let cols =
        Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)]).split(area);
    left(f, app, cols[0]);
    right(f, app, cols[1]);
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

/// The compact single-value trend line.
fn render_spark(f: &mut ratatui::Frame<'_>, app: &mut App, area: Rect) {
    let theme = app.theme();
    let shape = app.spark_shape();
    let panel = Panel::new().title("TREND").theme(theme);
    let content = panel.inner(area);
    f.render_widget(panel, area);
    f.render_stateful_widget(
        Sparkline::new().shape(shape).theme(theme),
        content,
        &mut app.spark,
    );
}

/// The multi-slice proportional ring.
fn render_donut(f: &mut ratatui::Frame<'_>, app: &mut App, area: Rect) {
    let theme = app.theme();
    let shape = app.donut_shape();
    let panel = Panel::new().title("ALLOCATIONS").theme(theme);
    let content = panel.inner(area);
    f.render_widget(panel, area);
    f.render_stateful_widget(
        DonutChart::new().slices(DONUT_SLICES).shape(shape).theme(theme),
        content,
        &mut app.donut,
    );
}

/// The horizontal category-comparison bars.
fn render_hbar(f: &mut ratatui::Frame<'_>, app: &mut App, area: Rect) {
    let theme = app.theme();
    let shape = app.hbar_shape();
    let panel = Panel::new().title("SUBSYSTEMS").theme(theme);
    let content = panel.inner(area);
    f.render_widget(panel, area);
    f.render_stateful_widget(
        HBarChart::new()
            .categories(["ALPHA", "BETA", "GAMMA", "DELTA"])
            .label_width(6)
            .shape(shape)
            .theme(theme),
        content,
        &mut app.hbar,
    );
}

/// The X/Y point cloud.
fn render_scatter(f: &mut ratatui::Frame<'_>, app: &mut App, area: Rect) {
    let theme = app.theme();
    let shape = app.scatter_shape();
    let panel = Panel::new().title("CONTACTS").theme(theme);
    let content = panel.inner(area);
    f.render_widget(panel, area);
    f.render_stateful_widget(
        ScatterPlot::new().capacity(SCATTER_POINTS).shape(shape).theme(theme),
        content,
        &mut app.scatter,
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
    // Controls first so they survive clipping on narrow terminals; the
    // per-widget shape readout trails and may clip on the right.
    let line = format!(
        " [t] theme:{name:<11} [s] shape [q] quit   spec={:?} dial={:?} heat={:?} spark={:?} donut={:?} hbar={:?} scatter={:?}",
        app.spectrum_shape(),
        app.dial_shape(),
        app.heat_shape(),
        app.spark_shape(),
        app.donut_shape(),
        app.hbar_shape(),
        app.scatter_shape(),
    );
    f.render_widget(
        Paragraph::new(line).style(Style::new().fg(theme.palette().muted.color())),
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
