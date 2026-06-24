//! **Info display** — the five information widgets together.
//!
//! Showcases [`BigText`], [`Stat`], [`KeyValue`], [`Timeline`], and [`Table`]
//! — the readout / dashboard family. `t` cycles theme, `q` / `Esc` quits.
//!
//! ```sh
//! cargo run -p ratatui-sci-fi --example info_display
//! ```
//!
//! [`BigText`]: ratatui_sci_fi::BigText
//! [`Stat`]: ratatui_sci_fi::Stat
//! [`KeyValue`]: ratatui_sci_fi::KeyValue
//! [`Timeline`]: ratatui_sci_fi::Timeline
//! [`Table`]: ratatui_sci_fi::Table

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
    BigText, BigTextShape, KeyValue, KeyValueShape, Stat, StatShape, Table, TableShape, Theme,
    Timeline, TimelineShape, Trend,
};

type Term = Terminal<CrosstermBackend<Stdout>>;

const THEMES: [Theme; 4] = [Theme::Cyberpunk, Theme::Fallout, Theme::Weyland, Theme::DeepSpace];
const TITLE: &str = "▶  INFO DISPLAY  ◀";

pub struct App {
    theme_idx: usize,
}

impl App {
    pub fn new() -> Self {
        Self { theme_idx: 0 }
    }

    pub fn theme(&self) -> Theme {
        THEMES[self.theme_idx]
    }

    pub fn cycle_theme(&mut self) {
        self.theme_idx = (self.theme_idx + 1) % THEMES.len();
    }

    /// No-op clock — all showcased widgets are stateless, so nothing advances.
    /// Exists so the headless screenshot harness's `run_scene` has a tick to call.
    pub fn tick(&mut self) {}
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

    // BigText banner on top; two columns beneath.
    let body =
        Layout::vertical([Constraint::Length(8), Constraint::Min(1)]).split(outer[1]);
    let lower = Layout::horizontal([Constraint::Min(1), Constraint::Min(1)]).split(body[1]);
    let left = Layout::vertical([Constraint::Min(1), Constraint::Min(1)]).split(lower[0]);
    let right = Layout::vertical([Constraint::Min(1), Constraint::Min(1)]).split(lower[1]);

    bigtext_cell(f, theme, body[0]);
    keyvalue_cell(f, theme, left[0]);
    timeline_cell(f, theme, left[1]);
    stat_cell(f, theme, right[0]);
    table_cell(f, theme, right[1]);

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

fn bigtext_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect) {
    let inner = labeled_cell(f, theme, area, "WIDGETS");
    f.render_widget(BigText::new("49").shape(BigTextShape::Grid).theme(theme), inner);
}

fn keyvalue_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect) {
    let inner = labeled_cell(f, theme, area, "METRICS");
    f.render_widget(
        KeyValue::new([("WIDGETS", "49"), ("TESTS", "620"), ("THEMES", "8"), ("EXAMPLES", "13")])
            .shape(KeyValueShape::Dotted)
            .theme(theme),
        inner,
    );
}

fn timeline_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect) {
    let inner = labeled_cell(f, theme, area, "CHANGELOG");
    f.render_widget(
        Timeline::new([
            ("0.1", "core + 10 widgets"),
            ("0.2", "16 chart widgets"),
            ("0.3", "form / HUD / data-viz"),
        ])
        .shape(TimelineShape::Connected)
        .theme(theme),
        inner,
    );
}

fn stat_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect) {
    let inner = labeled_cell(f, theme, area, "TESTS");
    f.render_widget(
        Stat::new("620", "PASSING").trend(Trend::Up).shape(StatShape::Card).theme(theme),
        inner,
    );
}

fn table_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect) {
    let inner = labeled_cell(f, theme, area, "BY CATEGORY");
    f.render_widget(
        Table::new(["CATEGORY", "COUNT"], [["basic+form+ind", "20"], ["effect", "10"], ["chart", "19"]])
            .shape(TableShape::Zebra)
            .theme(theme),
        inner,
    );
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
