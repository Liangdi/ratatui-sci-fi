//! **Data viz** — oscilloscope, star map, and topology graph.
//!
//! Showcases the three Braille-canvas data widgets: a scrolling [`Oscilloscope`]
//! trace, a twinkling [`StarMap`], and a [`Graph`] of nodes and edges. All play
//! on their own; `t` cycles theme, `q` / `Esc` quits.
//!
//! ```sh
//! cargo run -p ratatui-sci-fi --example data_viz
//! ```
//!
//! [`Oscilloscope`]: ratatui_sci_fi::Oscilloscope
//! [`StarMap`]: ratatui_sci_fi::StarMap
//! [`Graph`]: ratatui_sci_fi::Graph

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
    Graph, GraphShape, Oscilloscope, OscilloscopeShape, OscilloscopeState, StarMap, StarMapState,
    Theme,
};

type Term = Terminal<CrosstermBackend<Stdout>>;

const THEMES: [Theme; 4] = [Theme::Cyberpunk, Theme::Fallout, Theme::Weyland, Theme::DeepSpace];
const TITLE: &str = "▶  DATA VIZ  ◀";

/// A small constellation topology for the Graph.
const NODES: [(f32, f32); 5] = [(0.5, 0.1), (0.1, 0.5), (0.9, 0.5), (0.3, 0.9), (0.7, 0.9)];
const EDGES: [(usize, usize); 6] = [(0, 1), (0, 2), (1, 2), (1, 3), (2, 4), (3, 4)];

pub struct App {
    frame: u64,
    theme_idx: usize,
    scope: OscilloscopeState,
    stars: StarMapState,
}

impl App {
    pub fn new() -> Self {
        Self {
            frame: 0,
            theme_idx: 0,
            scope: OscilloscopeState::new(),
            stars: StarMapState::new(),
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
        self.scope.tick();
        self.stars.tick();
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

    f.render_widget(Block::new().style(Style::new().bg(theme.palette().bg.color())), area);

    let outer =
        Layout::vertical([Constraint::Length(3), Constraint::Min(1), Constraint::Length(1)])
            .split(area);

    render_title(f, theme, outer[0]);

    let body = Layout::vertical([
        Constraint::Length(5), // oscilloscope
        Constraint::Length(7), // star map
        Constraint::Length(7), // graph
    ])
    .split(outer[1]);

    scope_cell(f, theme, body[0], app);
    star_cell(f, theme, body[1], app);
    graph_cell(f, theme, body[2]);

    render_footer(f, theme, outer[2]);
}

fn render_title(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect) {
    let accent = theme.palette().accent.color();
    let title = Paragraph::new(Line::from(TITLE))
        .alignment(Alignment::Center)
        .style(Style::new().fg(accent).add_modifier(Modifier::BOLD));
    f.render_widget(title, vertically_centered(area, 1));
}

fn render_footer(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect) {
    let muted = theme.palette().muted.color();
    let footer = Paragraph::new(Line::from("t theme · q quit"))
        .alignment(Alignment::Center)
        .style(Style::new().fg(muted));
    f.render_widget(footer, vertically_centered(area, 1));
}

fn scope_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let inner = labeled_cell(f, theme, area, "OSCILLOSCOPE");
    f.render_stateful_widget(
        Oscilloscope::new(OscilloscopeShape::Sine).freq(0.1).amplitude(0.85).theme(theme),
        inner,
        &mut app.scope,
    );
}

fn star_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let inner = labeled_cell(f, theme, area, "STAR MAP");
    f.render_stateful_widget(StarMap::new().density(8).theme(theme), inner, &mut app.stars);
}

fn graph_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect) {
    let inner = labeled_cell(f, theme, area, "TOPOLOGY");
    f.render_widget(
        Graph::new(NODES.iter().copied())
            .edges(EDGES.iter().copied())
            .shape(GraphShape::Cross)
            .theme(theme),
        inner,
    );
}

/// A bordered, titled cell (muted frame). Returns the inner area.
fn labeled_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, title: &str) -> Rect {
    let p = theme.palette();
    let border = p.muted.color();
    let block = Block::bordered()
        .title(format!(" {title} "))
        .border_style(Style::new().fg(border))
        .title_style(Style::new().fg(border).add_modifier(Modifier::BOLD));
    let inner = block.inner(area);
    f.render_widget(block, area);
    inner
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
