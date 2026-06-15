//! **Button shapes** — a dedicated gallery for every [`ButtonShape`] variant,
//! showing each in both its idle and focused state so the whole form range is
//! visible at once.
//!
//! Layout: the **INLINE** panel shows the five single-row marker shapes
//! (`Bracket` / `Angle` / `Chevron` / `Pipe` / `Arrow`); the **BOXED** panel
//! shows the two multi-row border shapes (`Pill` rounded, `Framed` square).
//! Each row pairs an idle button (left) with a focused one (right).
//!
//! `t` cycles all eight themes · `q` / `Esc` quits.
//!
//! ```sh
//! cargo run -p ratatui-sci-fi --example button
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
use ratatui_sci_fi::{Button, ButtonShape, Panel, Theme};

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

/// Every inline (single-row) shape, in display order.
const INLINE: [(ButtonShape, &str); 5] = [
    (ButtonShape::Bracket, "Bracket"),
    (ButtonShape::Angle, "Angle"),
    (ButtonShape::Chevron, "Chevron"),
    (ButtonShape::Pipe, "Pipe"),
    (ButtonShape::Arrow, "Arrow"),
];

/// Every boxed (multi-row) shape, in display order.
const BOXED: [(ButtonShape, &str); 2] =
    [(ButtonShape::Pill, "Pill"), (ButtonShape::Framed, "Framed")];

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

    /// No-op — the button showcase is static, but the screenshot harness ticks
    /// every scene between frames, so expose the hook.
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

    // Root background.
    f.render_widget(Block::new().style(Style::new().bg(theme.palette().bg.color())), area);

    let outer = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(9),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(area);

    render_title(f, theme, outer[0]);
    render_inline_section(f, theme, outer[1]);
    render_boxed_section(f, theme, outer[2]);
    render_footer(f, theme, outer[3]);
}

/// Centered header title.
fn render_title(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect) {
    let band = Layout::vertical([Constraint::Min(0), Constraint::Length(1), Constraint::Min(0)])
        .split(area)[1];
    let label = "▶  BUTTON · SHAPE VARIANTS  ◀";
    let w = label.chars().count() as u16;
    let x = band.x + band.width.saturating_sub(w) / 2;
    f.render_widget(
        Paragraph::new(label).style(Style::new().fg(theme.palette().accent.color())),
        Rect::new(x, band.y, w.min(band.width), 1),
    );
}

/// One row per inline shape: `name | idle button | focused button`.
fn render_inline_section(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect) {
    let panel = Panel::new().title("INLINE").theme(theme);
    let content = panel.inner(area);
    f.render_widget(panel, area);

    let rows = Layout::vertical([Constraint::Length(1); INLINE.len()]).split(content);
    let name_style = Style::new().fg(theme.palette().muted.color());

    for (i, (shape, name)) in INLINE.iter().enumerate() {
        let cols = Layout::horizontal([Constraint::Length(12), Constraint::Min(1), Constraint::Min(1)])
            .split(rows[i]);
        f.render_widget(Paragraph::new(*name).style(name_style), cols[0]);
        f.render_widget(Button::new("ENGAGE").shape(*shape).theme(theme), cols[1]);
        f.render_widget(
            Button::new("ENGAGE").focused(true).shape(*shape).theme(theme),
            cols[2],
        );
    }
}

/// One block per boxed shape (each gets a multi-row area for its border):
/// `name | idle box | focused box`.
fn render_boxed_section(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect) {
    let panel = Panel::new().title("BOXED").theme(theme);
    let content = panel.inner(area);
    f.render_widget(panel, area);

    let chunks = Layout::vertical([Constraint::Min(1); BOXED.len()]).split(content);
    let name_style = Style::new().fg(theme.palette().muted.color());

    for (i, (shape, name)) in BOXED.iter().enumerate() {
        let cols = Layout::horizontal([Constraint::Length(12), Constraint::Min(1), Constraint::Min(1)])
            .split(chunks[i]);
        f.render_widget(Paragraph::new(*name).style(name_style), cols[0]);
        f.render_widget(Button::new("ENGAGE").shape(*shape).theme(theme), cols[1]);
        f.render_widget(
            Button::new("ENGAGE").focused(true).shape(*shape).theme(theme),
            cols[2],
        );
    }
}

fn render_footer(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect) {
    let name = format!("{:?}", theme);
    f.render_widget(
        Paragraph::new(format!(
            " [t] theme: {name:<10}   idle button on the left · focused on the right   [q] quit"
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
