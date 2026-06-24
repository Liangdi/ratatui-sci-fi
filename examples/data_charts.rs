//! **Data charts** — pie, speedometer, and line chart.
//!
//! Showcases the three data-viz additions: a [`PieChart`], a [`Speedometer`],
//! and an axis-labelled [`LineChart`]. `t` cycles theme, `q` / `Esc` quits.
//!
//! ```sh
//! cargo run -p ratatui-sci-fi --example data_charts
//! ```
//!
//! [`PieChart`]: ratatui_sci_fi::PieChart
//! [`Speedometer`]: ratatui_sci_fi::Speedometer
//! [`LineChart`]: ratatui_sci_fi::LineChart

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
use ratatui_sci_fi::{LineChart, PieChart, Speedometer, Theme};

type Term = Terminal<CrosstermBackend<Stdout>>;

const THEMES: [Theme; 4] = [Theme::Cyberpunk, Theme::Fallout, Theme::Weyland, Theme::DeepSpace];
const TITLE: &str = "▶  DATA CHARTS  ◀";

pub struct App {
    frame: u64,
    theme_idx: usize,
}

impl App {
    pub fn new() -> Self {
        Self { frame: 0, theme_idx: 0 }
    }

    pub fn theme(&self) -> Theme {
        THEMES[self.theme_idx]
    }

    pub fn cycle_theme(&mut self) {
        self.theme_idx = (self.theme_idx + 1) % THEMES.len();
    }

    pub fn tick(&mut self) {
        self.frame = self.frame.wrapping_add(1);
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
        Constraint::Length(9), // pie
        Constraint::Length(8), // speedometer
        Constraint::Length(8), // line chart
    ])
    .split(outer[1]);

    pie_cell(f, theme, body[0]);
    speedo_cell(f, theme, body[1], app.frame);
    line_cell(f, theme, body[2], app.frame);

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

fn pie_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect) {
    let inner = labeled_cell(f, theme, area, "PIE");
    f.render_widget(PieChart::new([3.0, 2.0, 1.5, 1.0]).theme(theme), inner);
}

fn speedo_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, frame: u64) {
    let inner = labeled_cell(f, theme, area, "SPEEDOMETER");
    let value = ((frame as f32 * 0.02).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
    f.render_widget(Speedometer::new(value).theme(theme), inner);
}

fn line_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, frame: u64) {
    let inner = labeled_cell(f, theme, area, "LINE");
    let t = frame as f32 * 0.1;
    let data: Vec<f32> = (0..40).map(|i| (t + i as f32 * 0.3).sin()).collect();
    f.render_widget(LineChart::new(data).theme(theme), inner);
}

/// A bordered, titled cell (muted frame). Returns the inner area.
fn labeled_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, title: &str) -> Rect {
    let border = theme.palette().muted.color();
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
