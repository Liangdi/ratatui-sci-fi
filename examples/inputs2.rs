//! **Inputs 2** — a vertical slider and a combo box.
//!
//! Showcases the two input-enhancement widgets: a [`VerticalSlider`] and a
//! [`ComboBox`]. `Tab` cycles focus; on the slider `↑/↓` adjust; on the combo
//! type / `Enter` (open or commit) / `↑↓` navigate.
//!
//! `Tab` focus · `t` theme · `q` / `Esc` quits.
//!
//! ```sh
//! cargo run -p ratatui-sci-fi --example inputs2
//! ```
//!
//! [`VerticalSlider`]: ratatui_sci_fi::VerticalSlider
//! [`ComboBox`]: ratatui_sci_fi::ComboBox

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
use ratatui_sci_fi::{ComboBox, ComboBoxState, Theme, VerticalSlider, VerticalSliderState};

type Term = Terminal<CrosstermBackend<Stdout>>;

const THEMES: [Theme; 4] = [Theme::Cyberpunk, Theme::Fallout, Theme::Weyland, Theme::DeepSpace];
const TITLE: &str = "▶  INPUTS 2  ◀";
const OPTIONS: [&str; 4] = ["ALPHA", "BETA", "GAMMA", "DELTA"];
const FOCUS_COUNT: usize = 2;

pub struct App {
    theme_idx: usize,
    focus: usize,
    slider: VerticalSliderState,
    combo: ComboBoxState,
}

impl App {
    pub fn new() -> Self {
        Self {
            theme_idx: 0,
            focus: 0,
            slider: VerticalSliderState { value: 0.4 },
            combo: ComboBoxState::new(),
        }
    }

    pub fn theme(&self) -> Theme {
        THEMES[self.theme_idx]
    }

    pub fn cycle_theme(&mut self) {
        self.theme_idx = (self.theme_idx + 1) % THEMES.len();
    }

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
                KeyCode::Tab => app.focus = (app.focus + 1) % FOCUS_COUNT,
                KeyCode::BackTab => app.focus = (app.focus + FOCUS_COUNT - 1) % FOCUS_COUNT,
                _ => match app.focus {
                    0 => VerticalSlider::new().handle_key(&mut app.slider, key),
                    1 => ComboBox::new(OPTIONS).handle_key(&mut app.combo, key),
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

    let cols = Layout::horizontal([Constraint::Length(10), Constraint::Min(20)]).split(outer[1]);
    slider_cell(f, theme, cols[0], app, app.focus == 0);
    combo_cell(f, theme, cols[1], app, app.focus == 1);

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
    let footer = Paragraph::new(Line::from("Tab focus · ↑↓ slider / combo · enter opens combo · t theme · q quit"))
        .alignment(Alignment::Center)
        .style(Style::new().fg(muted));
    f.render_widget(footer, vertically_centered(area, 1));
}

fn slider_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App, focused: bool) {
    let inner = labeled_cell(f, theme, area, "V-SLIDER", focused);
    f.render_stateful_widget(VerticalSlider::new().theme(theme), inner, &mut app.slider);
}

fn combo_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App, focused: bool) {
    let inner = labeled_cell(f, theme, area, "COMBO", focused);
    // When open, grow the area downward to fit the options.
    let area = if app.combo.open {
        Rect::new(inner.x, inner.y, inner.width, inner.height.max(OPTIONS.len() as u16 + 1))
    } else {
        inner
    };
    f.render_widget(Clear, area);
    f.render_stateful_widget(ComboBox::new(OPTIONS).theme(theme), area, &mut app.combo);
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
