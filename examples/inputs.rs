//! **Inputs** — a multi-select list and a multi-line editor.
//!
//! Showcases the two multi-row input widgets: a [`MultiSelectList`] (toggle
//! items with `Space`) and a [`TextArea`] (type / `Enter` for newline /
//! arrows to navigate). `Tab` cycles focus between them.
//!
//! `Tab` focus · `←→↑↓` move · `space` toggles (list) / type & `enter` (text)
//! · `t` theme · `q` / `Esc` quits.
//!
//! ```sh
//! cargo run -p ratatui-sci-fi --example inputs
//! ```
//!
//! [`MultiSelectList`]: ratatui_sci_fi::MultiSelectList
//! [`TextArea`]: ratatui_sci_fi::TextArea

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
use ratatui_sci_fi::{MultiSelectList, MultiSelectListState, TextArea, TextAreaState, Theme};

type Term = Terminal<CrosstermBackend<Stdout>>;

const THEMES: [Theme; 4] = [Theme::Cyberpunk, Theme::Fallout, Theme::Weyland, Theme::DeepSpace];
const TITLE: &str = "▶  INPUTS  ◀";
const OPTIONS: [&str; 4] = ["SHIELDS", "CLOAK", "AUTOAIM", "DOCKING"];
const FOCUS_COUNT: usize = 2;

pub struct App {
    theme_idx: usize,
    /// 0 = list, 1 = text area.
    focus: usize,
    multi: MultiSelectListState,
    text: TextAreaState,
}

impl App {
    pub fn new() -> Self {
        let mut text = TextAreaState::new();
        text.lines = vec!["// mission notes".into(), "rendezvous at 0.2c".into()];
        Self {
            theme_idx: 0,
            focus: 0,
            multi: MultiSelectListState::new(OPTIONS.len()),
            text,
        }
    }

    pub fn theme(&self) -> Theme {
        THEMES[self.theme_idx]
    }

    pub fn cycle_theme(&mut self) {
        self.theme_idx = (self.theme_idx + 1) % THEMES.len();
    }

    pub fn tick(&mut self) {
        self.text.tick();
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
                KeyCode::Tab => app.focus = (app.focus + 1) % FOCUS_COUNT,
                KeyCode::BackTab => app.focus = (app.focus + FOCUS_COUNT - 1) % FOCUS_COUNT,
                _ => match app.focus {
                    0 => MultiSelectList::new(OPTIONS).handle_key(&mut app.multi, key),
                    1 => app.text.handle_key(key),
                    _ => {}
                },
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

    let cols = Layout::horizontal([Constraint::Min(1), Constraint::Min(1)]).split(outer[1]);
    multi_cell(f, theme, cols[0], app, app.focus == 0);
    text_cell(f, theme, cols[1], app, app.focus == 1);

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
    let footer = Paragraph::new(Line::from("Tab focus · space toggles · enter newline · t theme · q quit"))
        .alignment(Alignment::Center)
        .style(Style::new().fg(muted));
    f.render_widget(footer, vertically_centered(area, 1));
}

fn multi_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App, focused: bool) {
    let inner = labeled_cell(f, theme, area, "MULTI-SELECT", focused);
    f.render_stateful_widget(
        MultiSelectList::new(OPTIONS).theme(theme),
        inner,
        &mut app.multi,
    );
}

fn text_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App, focused: bool) {
    let inner = labeled_cell(f, theme, area, "TEXT AREA", focused);
    f.render_stateful_widget(TextArea::new().theme(theme), inner, &mut app.text);
}

/// A bordered, titled cell; accent frame when `focused`.
fn labeled_cell(
    f: &mut ratatui::Frame<'_>,
    theme: Theme,
    area: Rect,
    title: &str,
    focused: bool,
) -> Rect {
    let p = theme.palette();
    let border = if focused { p.accent.color() } else { p.muted.color() };
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
