//! **Navigation** — breadcrumb, tabs, and a scroll view.
//!
//! Showcases the three navigation widgets: a [`Breadcrumb`] path, a [`Tabs`]
//! bar (switch with `←`/`→`), and a [`ScrollView`] whose contents change with
//! the active tab (scroll with `↑`/`↓`/`PgUp`/`PgDn`/`Home`/`End`).
//!
//! `←→` tabs · `↑↓/PgUp/PgDn/Home/End` scroll · `t` theme · `q` / `Esc` quits.
//!
//! ```sh
//! cargo run -p ratatui-sci-fi --example navigation
//! ```
//!
//! [`Breadcrumb`]: ratatui_sci_fi::Breadcrumb
//! [`Tabs`]: ratatui_sci_fi::Tabs
//! [`ScrollView`]: ratatui_sci_fi::ScrollView

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
    Breadcrumb, ScrollView, ScrollViewState, Tabs as SfTabs, TabsState, Theme,
};

type Term = Terminal<CrosstermBackend<Stdout>>;

const THEMES: [Theme; 4] = [Theme::Cyberpunk, Theme::Fallout, Theme::Weyland, Theme::DeepSpace];
const TITLE: &str = "▶  NAVIGATION  ◀";
const TAB_LABELS: [&str; 3] = ["SYSTEM", "SENSORS", "LOG"];

/// One block of scrollable lines per tab.
const SYSTEM_LOG: &[&str] = &[
    "kernel ......... online",
    "uptime ......... 14d 02h",
    "load ........... 0.42 0.38 0.31",
    "procs .......... 112 / 4096",
    "memory ......... 6.2G / 16G",
    "swap ........... 0K",
    "boot dev ....... /dev/nvme0",
    "firmware ....... v2.41",
    "scheduler ...... nomos",
    "watchdog ....... armed",
];
const SENSOR_LOG: &[&str] = &[
    "core temp ...... 41C",
    "coolant ........ 24C",
    "hull integ ..... 99.8%",
    "rad ............ 0.12 uSv",
    "o2 ............. 20.9%",
    "co2 ............ 0.04%",
    "press .......... 1.01 atm",
    "gravity ........ 0.98 g",
    "field str ...... nominal",
    "hull stress .... low",
];
const MISSION_LOG: &[&str] = &[
    "T-00:00  launch",
    "T-00:14  stage-1 sep",
    "T-02:31  orbital insert",
    "T-04:12  trans-mars burn",
    "T-18:40  course correction",
    "T-1d02h  cruise nominal",
    "T-3d11h  solar panel deploy",
    "T-5d07h  cruise nominal",
    "T-7d22h  approach burn",
    "T-8d01h  capture",
];

fn tab_log(idx: usize) -> &'static [&'static str] {
    match idx {
        1 => SENSOR_LOG,
        2 => MISSION_LOG,
        _ => SYSTEM_LOG,
    }
}

pub struct App {
    theme_idx: usize,
    tabs: TabsState,
    scroll: ScrollViewState,
    /// The ScrollView viewport height (set each render, read on key events).
    viewport: u16,
}

impl App {
    pub fn new() -> Self {
        Self {
            theme_idx: 0,
            tabs: TabsState::new(),
            scroll: ScrollViewState::new(),
            viewport: 10,
        }
    }

    pub fn theme(&self) -> Theme {
        THEMES[self.theme_idx]
    }

    pub fn cycle_theme(&mut self) {
        self.theme_idx = (self.theme_idx + 1) % THEMES.len();
    }

    /// No-op clock so the headless screenshot harness has a tick to call.
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
        app.tick();

        if event::poll(Duration::from_millis(60))?
            && let Event::Key(key) = event::read()?
        {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('t') => app.cycle_theme(),
                // Tab switching resets the scroll to the top of the new content.
                KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
                    SfTabs::new(TAB_LABELS).handle_key(&mut app.tabs, key);
                    app.scroll.offset = 0;
                }
                _ => ScrollView::new(tab_log(app.tabs.selected).iter().copied())
                    .handle_key(&mut app.scroll, key, app.viewport),
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
        Layout::vertical([Constraint::Length(3), Constraint::Length(3), Constraint::Min(1), Constraint::Length(1)])
            .split(area);

    render_title(f, theme, outer[0]);

    // Breadcrumb path reflecting the active tab.
    let active = TAB_LABELS[app.tabs.selected.min(TAB_LABELS.len() - 1)];
    f.render_widget(
        Breadcrumb::new(["BRIDGE", "CONSOLE", active]).theme(theme),
        vertically_centered(outer[1], 1),
    );

    // Tabs.
    f.render_stateful_widget(
        SfTabs::new(TAB_LABELS).theme(theme),
        outer[1],
        &mut app.tabs,
    );

    // Scroll view of the active tab's content.
    let body = outer[2];
    app.viewport = body.height;
    f.render_stateful_widget(
        ScrollView::new(tab_log(app.tabs.selected).iter().copied()).theme(theme),
        body,
        &mut app.scroll,
    );

    render_footer(f, theme, outer[3]);
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
    let footer = Paragraph::new(Line::from("←→ tabs · ↑↓/PgUp/PgDn/Home/End scroll · t theme · q quit"))
        .alignment(Alignment::Center)
        .style(Style::new().fg(muted));
    f.render_widget(footer, vertically_centered(area, 1));
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
