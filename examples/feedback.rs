//! **Feedback** — badges, a tooltip, and toasts.
//!
//! Showcases the three feedback widgets: a row of [`Badge`]es, a [`Tooltip`],
//! and a [`Toast`] you fire with `1` / `2` / `3` (Ok / Warn / Alert).
//!
//! `1/2/3` fire a toast · `t` theme · `q` / `Esc` quits.
//!
//! ```sh
//! cargo run -p ratatui-sci-fi --example feedback
//! ```
//!
//! [`Badge`]: ratatui_sci_fi::Badge
//! [`Tooltip`]: ratatui_sci_fi::Tooltip
//! [`Toast`]: ratatui_sci_fi::Toast

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
    widgets::{Block, Clear, Paragraph},
    Terminal,
};
use ratatui_sci_fi::{Badge, BadgeShape, Level, Theme, Toast, ToastState, Tooltip, TooltipShape};

type Term = Terminal<CrosstermBackend<Stdout>>;

const THEMES: [Theme; 4] = [Theme::Cyberpunk, Theme::Fallout, Theme::Weyland, Theme::DeepSpace];
const TITLE: &str = "▶  FEEDBACK  ◀";

pub struct App {
    theme_idx: usize,
    toast: ToastState,
}

impl App {
    pub fn new() -> Self {
        Self {
            theme_idx: 0,
            toast: ToastState::new(),
        }
    }

    pub fn theme(&self) -> Theme {
        THEMES[self.theme_idx]
    }

    pub fn cycle_theme(&mut self) {
        self.theme_idx = (self.theme_idx + 1) % THEMES.len();
    }

    pub fn tick(&mut self) {
        self.toast.tick();
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
                KeyCode::Char('1') => app.toast.show("SYSTEM NOMINAL", Level::Ok, 40),
                KeyCode::Char('2') => app.toast.show("FUEL LOW", Level::Warn, 40),
                KeyCode::Char('3') => app.toast.show("HULL BREACH", Level::Alert, 60),
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

    let body = Layout::vertical([Constraint::Length(3), Constraint::Length(3)]).split(outer[1]);
    badges_cell(f, theme, body[0]);
    tooltip_cell(f, theme, body[1]);

    render_footer(f, theme, outer[2]);

    // Toast overlay — bottom-centered, painted last so it floats.
    if app.toast.visible() {
        let w = (app.toast.message.chars().count() as u16 + 6).min(area.width);
        let pop = bottom_centered(w, 3, area);
        f.render_widget(Clear, pop);
        f.render_stateful_widget(Toast::new().theme(theme), pop, &mut app.toast);
    }
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
    let footer = Paragraph::new(Line::from("1 ok · 2 warn · 3 alert · t theme · q quit"))
        .alignment(Alignment::Center)
        .style(Style::new().fg(muted));
    f.render_widget(footer, vertically_centered(area, 1));
}

fn badges_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect) {
    let inner = labeled_cell(f, theme, area, "BADGES");
    let cols = Layout::horizontal([
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
    ])
    .split(inner);
    f.render_widget(
        Badge::new("ONLINE").level(Level::Ok).shape(BadgeShape::Filled).theme(theme),
        vertically_centered(cols[0], 1),
    );
    f.render_widget(
        Badge::new("STANDBY").level(Level::Warn).shape(BadgeShape::Filled).theme(theme),
        vertically_centered(cols[1], 1),
    );
    f.render_widget(
        Badge::new("OFFLINE").level(Level::Alert).shape(BadgeShape::Outlined).theme(theme),
        vertically_centered(cols[2], 1),
    );
    f.render_widget(
        Badge::new("IDLE").level(Level::Normal).shape(BadgeShape::Outlined).theme(theme),
        vertically_centered(cols[3], 1),
    );
}

fn tooltip_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect) {
    let inner = labeled_cell(f, theme, area, "TOOLTIP");
    // Tooltip pointing down at the (notional) subject beneath it.
    f.render_widget(
        Tooltip::new("reactor core").shape(TooltipShape::Pointer).theme(theme),
        vertically_centered(inner, 2),
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

/// A `width`×`height` rect centered horizontally, sitting one row above the
/// bottom edge of `area` (the toast's home).
fn bottom_centered(width: u16, height: u16, area: Rect) -> Rect {
    let y = area.bottom().saturating_sub(height + 1);
    let x = area.x + area.width.saturating_sub(width) / 2;
    Rect::new(x, y, width, height)
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
