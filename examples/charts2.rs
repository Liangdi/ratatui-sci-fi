//! **Charts 2** — the second sci-fi data-viz gallery: seven more animated chart
//! widgets, each self-animating every frame in demo mode. The counterpart to
//! `charts.rs` (which hosts the original seven). This example covers
//! [`CandlestickChart`] (OHLC market), [`TreeMap`] (allocations), [`AreaChart`]
//! (throughput), [`ActivityRings`] (goals), [`StripChart`] (multi-channel
//! vitals), [`RadialBarChart`] (sectors), and [`Compass`] (heading).
//!
//! Layout: three two-up panel rows — **MARKET / GOALS**, **THROUGHPUT /
//! SECTORS**, **VITALS / HEADING** — over a full-width **STORAGE** treemap.
//!
//! `t` cycles all eight themes · `s` cycles each widget's shape variant in
//! lockstep · `q` / `Esc` quits.
//!
//! ```sh
//! cargo run -p ratatui-sci-fi --example charts2
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
    ActivityRings, ActivityRingsState, AreaChart, AreaChartState, AreaShape, CandlestickChart,
    CandlestickChartState, CandlestickShape, Compass, CompassShape, CompassState, Panel,
    RadialBarChart, RadialBarState, RBarShape, RingShape, StripChart, StripChartState, StripShape,
    Theme, TreeMap, TreeMapState, TreeShape,
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

/// Every [`CandlestickShape`], in display order.
const CANDLE_SHAPES: [CandlestickShape; 3] =
    [CandlestickShape::Filled, CandlestickShape::Hollow, CandlestickShape::Bar];

/// Every [`TreeShape`], in display order.
const TREE_SHAPES: [TreeShape; 3] = [TreeShape::Flat, TreeShape::Slice, TreeShape::Brick];

/// Every [`AreaShape`], in display order.
const AREA_SHAPES: [AreaShape; 3] = [AreaShape::Solid, AreaShape::Fill, AreaShape::Line];

/// Every [`RingShape`], in display order.
const RING_SHAPES: [RingShape; 3] = [RingShape::Arc, RingShape::Track, RingShape::Tick];

/// Every [`StripShape`], in display order.
const STRIP_SHAPES: [StripShape; 3] = [StripShape::Line, StripShape::Fill, StripShape::Bar];

/// Every [`RBarShape`], in display order.
const RBAR_SHAPES: [RBarShape; 3] = [RBarShape::Bar, RBarShape::Arc, RBarShape::Needle];

/// Every [`CompassShape`], in display order.
const COMPASS_SHAPES: [CompassShape; 3] =
    [CompassShape::Needle, CompassShape::Arrow, CompassShape::Chevron];

/// Demo sizing constants.
const CANDLE_CAPACITY: usize = 32;
const AREA_WINDOW: usize = 64;
const RING_COUNT: usize = 3;
const STRIP_CHANNELS: usize = 4;
const STRIP_WINDOW: usize = 80;
const RBAR_COUNT: usize = 6;

pub struct App {
    theme_idx: usize,
    shape_idx: usize,
    candle: CandlestickChartState,
    tree: TreeMapState,
    area: AreaChartState,
    rings: ActivityRingsState,
    strip: StripChartState,
    rbar: RadialBarState,
    compass: CompassState,
}

impl App {
    pub fn new() -> Self {
        Self {
            theme_idx: 0,
            shape_idx: 0,
            candle: CandlestickChartState::new(CANDLE_CAPACITY),
            tree: TreeMapState::new(),
            area: AreaChartState::new(AREA_WINDOW),
            rings: ActivityRingsState::new(RING_COUNT),
            strip: StripChartState::new(STRIP_CHANNELS, STRIP_WINDOW),
            rbar: RadialBarState::new(RBAR_COUNT),
            compass: CompassState::new(),
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

    pub fn candle_shape(&self) -> CandlestickShape {
        CANDLE_SHAPES[self.shape_idx % CANDLE_SHAPES.len()]
    }

    pub fn tree_shape(&self) -> TreeShape {
        TREE_SHAPES[self.shape_idx % TREE_SHAPES.len()]
    }

    pub fn area_shape(&self) -> AreaShape {
        AREA_SHAPES[self.shape_idx % AREA_SHAPES.len()]
    }

    pub fn ring_shape(&self) -> RingShape {
        RING_SHAPES[self.shape_idx % RING_SHAPES.len()]
    }

    pub fn strip_shape(&self) -> StripShape {
        STRIP_SHAPES[self.shape_idx % STRIP_SHAPES.len()]
    }

    pub fn rbar_shape(&self) -> RBarShape {
        RBAR_SHAPES[self.shape_idx % RBAR_SHAPES.len()]
    }

    pub fn compass_shape(&self) -> CompassShape {
        COMPASS_SHAPES[self.shape_idx % COMPASS_SHAPES.len()]
    }

    /// Advance every widget's animation by one tick (demo mode).
    pub fn tick(&mut self) {
        self.candle.tick();
        self.tree.tick();
        self.area.tick();
        self.rings.tick();
        self.strip.tick();
        self.rbar.tick();
        self.compass.tick();
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
        Constraint::Length(9),
        Constraint::Length(1),
    ])
    .split(area);

    render_title(f, theme, outer[0]);
    render_pair(f, app, outer[1], render_candle, render_rings);
    render_pair(f, app, outer[2], render_area, render_rbar);
    render_pair(f, app, outer[3], render_strip, render_compass);
    render_tree(f, app, outer[4]);
    render_footer(f, app, outer[5]);
}

/// Centered header title.
fn render_title(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect) {
    let band = Layout::vertical([Constraint::Min(0), Constraint::Length(1), Constraint::Min(0)])
        .split(area)[1];
    let label = "▶  CHARTS 2 · MORE SCI-FI DATA VIZ  ◀";
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

/// The OHLC market candlestick chart.
fn render_candle(f: &mut ratatui::Frame<'_>, app: &mut App, area: Rect) {
    let theme = app.theme();
    let shape = app.candle_shape();
    let panel = Panel::new().title("MARKET").theme(theme);
    let content = panel.inner(area);
    f.render_widget(panel, area);
    f.render_stateful_widget(
        CandlestickChart::new().capacity(CANDLE_CAPACITY).shape(shape).theme(theme),
        content,
        &mut app.candle,
    );
}

/// The concentric multi-goal activity rings.
fn render_rings(f: &mut ratatui::Frame<'_>, app: &mut App, area: Rect) {
    let theme = app.theme();
    let shape = app.ring_shape();
    let panel = Panel::new().title("GOALS").theme(theme);
    let content = panel.inner(area);
    f.render_widget(panel, area);
    f.render_stateful_widget(
        ActivityRings::new().rings(RING_COUNT).shape(shape).theme(theme),
        content,
        &mut app.rings,
    );
}

/// The filled throughput area chart.
fn render_area(f: &mut ratatui::Frame<'_>, app: &mut App, area: Rect) {
    let theme = app.theme();
    let shape = app.area_shape();
    let panel = Panel::new().title("THROUGHPUT").theme(theme);
    let content = panel.inner(area);
    f.render_widget(panel, area);
    f.render_stateful_widget(
        AreaChart::new().window(AREA_WINDOW).shape(shape).theme(theme),
        content,
        &mut app.area,
    );
}

/// The polar sector bars.
fn render_rbar(f: &mut ratatui::Frame<'_>, app: &mut App, area: Rect) {
    let theme = app.theme();
    let shape = app.rbar_shape();
    let panel = Panel::new().title("SECTORS").theme(theme);
    let content = panel.inner(area);
    f.render_widget(panel, area);
    f.render_stateful_widget(
        RadialBarChart::new().bars(RBAR_COUNT).shape(shape).theme(theme),
        content,
        &mut app.rbar,
    );
}

/// The multi-channel vitals oscilloscope.
fn render_strip(f: &mut ratatui::Frame<'_>, app: &mut App, area: Rect) {
    let theme = app.theme();
    let shape = app.strip_shape();
    let panel = Panel::new().title("VITALS").theme(theme);
    let content = panel.inner(area);
    f.render_widget(panel, area);
    f.render_stateful_widget(
        StripChart::new()
            .channels(STRIP_CHANNELS)
            .window(STRIP_WINDOW)
            .shape(shape)
            .theme(theme),
        content,
        &mut app.strip,
    );
}

/// The heading compass.
fn render_compass(f: &mut ratatui::Frame<'_>, app: &mut App, area: Rect) {
    let theme = app.theme();
    let shape = app.compass_shape();
    let panel = Panel::new().title("HEADING").theme(theme);
    let content = panel.inner(area);
    f.render_widget(panel, area);
    f.render_stateful_widget(
        Compass::new().shape(shape).theme(theme),
        content,
        &mut app.compass,
    );
}

/// The full-width allocation treemap.
fn render_tree(f: &mut ratatui::Frame<'_>, app: &mut App, area: Rect) {
    let theme = app.theme();
    let shape = app.tree_shape();
    let panel = Panel::new().title("STORAGE").theme(theme);
    let content = panel.inner(area);
    f.render_widget(panel, area);
    f.render_stateful_widget(TreeMap::new().shape(shape).theme(theme), content, &mut app.tree);
}

fn render_footer(f: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let theme = app.theme();
    let name = format!("{:?}", theme);
    // Controls first so they survive clipping on narrow terminals; the
    // per-widget shape readout trails and may clip on the right.
    let line = format!(
        " [t] theme:{name:<11} [s] shape [q] quit   candle={:?} area={:?} rings={:?} strip={:?} rbar={:?} compass={:?} tree={:?}",
        app.candle_shape(),
        app.area_shape(),
        app.ring_shape(),
        app.strip_shape(),
        app.rbar_shape(),
        app.compass_shape(),
        app.tree_shape(),
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
