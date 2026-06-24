//! **Indicators** — status lights, countdown, progress, and a folding panel.
//!
//! Showcases the four indicator/container widgets: a row of [`StatusLED`]s at
//! every level, a [`CountdownTimer`], a [`ProgressBar`] in both determinate and
//! indeterminate modes, and a [`CollapsiblePanel`] you can fold with `c`.
//!
//! `c` toggles the panel · `t` cycles theme · `q` / `Esc` quits.
//!
//! ```sh
//! cargo run -p ratatui-sci-fi --example indicators
//! ```
//!
//! [`StatusLED`]: ratatui_sci_fi::StatusLED
//! [`CountdownTimer`]: ratatui_sci_fi::CountdownTimer
//! [`ProgressBar`]: ratatui_sci_fi::ProgressBar
//! [`CollapsiblePanel`]: ratatui_sci_fi::CollapsiblePanel

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
    CollapsiblePanel, CollapsiblePanelState, CountdownTimer, CountdownTimerState, Level,
    ProgressBar, ProgressBarState, StatusLED, Theme,
};

type Term = Terminal<CrosstermBackend<Stdout>>;

const THEMES: [Theme; 4] = [Theme::Cyberpunk, Theme::Fallout, Theme::Weyland, Theme::DeepSpace];
const TITLE: &str = "▶  INDICATORS  ◀";
/// Countdown ticks per remaining-second decrement (≈1 s at the 60 ms poll).
const COUNTDOWN_DEC_TICKS: u64 = 16;

pub struct App {
    frame: u64,
    theme_idx: usize,
    countdown: CountdownTimerState,
    progress: ProgressBarState,
    panel: CollapsiblePanelState,
}

impl App {
    pub fn new() -> Self {
        Self {
            frame: 0,
            theme_idx: 0,
            countdown: CountdownTimerState::new(30),
            progress: ProgressBarState::new(),
            panel: CollapsiblePanelState::new(),
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
        self.countdown.tick();
        self.progress.tick();
        // Decrement the countdown roughly once per second of wall time.
        if self.frame.is_multiple_of(COUNTDOWN_DEC_TICKS) && self.countdown.remaining > 0 {
            self.countdown.remaining -= 1;
        }
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
                KeyCode::Char('c') => app.panel.toggle(),
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
        Constraint::Length(3), // status LEDs
        Constraint::Length(3), // countdown
        Constraint::Length(5), // progress bars (determinate + indeterminate)
        Constraint::Length(7), // collapsible panel
    ])
    .split(outer[1]);

    status_cell(f, theme, body[0]);
    countdown_cell(f, theme, body[1], app);
    progress_cell(f, theme, body[2], app);
    panel_cell(f, theme, body[3], app);

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
    let footer = Paragraph::new(Line::from("c fold panel · t theme · q quit"))
        .alignment(Alignment::Center)
        .style(Style::new().fg(muted));
    f.render_widget(footer, vertically_centered(area, 1));
}

fn status_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect) {
    let inner = labeled_cell(f, theme, area, "STATUS");
    let cols = Layout::horizontal([
        Constraint::Min(1),
        Constraint::Min(1),
        Constraint::Min(1),
        Constraint::Min(1),
    ])
    .split(inner);
    let leds = [
        ("PWR", Level::Ok),
        ("NET", Level::Warn),
        ("HULL", Level::Alert),
        ("AUX", Level::Normal),
    ];
    for (i, (label, level)) in leds.iter().enumerate() {
        f.render_widget(StatusLED::new(*label).level(*level).theme(theme), cols[i]);
    }
}

fn countdown_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let inner = labeled_cell(f, theme, area, "COUNTDOWN");
    f.render_stateful_widget(CountdownTimer::new().theme(theme), inner, &mut app.countdown);
}

fn progress_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let inner = labeled_cell(f, theme, area, "PROGRESS");
    let rows = Layout::vertical([Constraint::Min(1), Constraint::Min(1)]).split(inner);
    // Determinate: a ratio that cycles 0..1.
    let ratio = (app.frame % 100) as f32 / 100.0;
    f.render_stateful_widget(
        ProgressBar::new(Some(ratio)).theme(theme),
        rows[0],
        &mut ProgressBarState::new(),
    );
    // Indeterminate: a scanning block.
    f.render_stateful_widget(ProgressBar::new(None).theme(theme), rows[1], &mut app.progress);
}

fn panel_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let panel = CollapsiblePanel::new("SENSORS").theme(theme);
    // Grab the body area (zero-height when collapsed) before render moves panel.
    let content = panel.inner(area, &app.panel);
    f.render_stateful_widget(panel, area, &mut app.panel);
    if !app.panel.collapsed {
        let ok = theme.palette().ok.color();
        let fg = theme.palette().fg.color();
        let lines = vec![
            Line::from("◉ REACTOR ........ NOMINAL").style(Style::new().fg(ok)),
            Line::from("◉ COOLANT ........ NOMINAL").style(Style::new().fg(ok)),
            Line::from("◉ O2 ............. NOMINAL").style(Style::new().fg(ok)),
            Line::from("  (press c to fold)").style(Style::new().fg(fg)),
        ];
        f.render_widget(Paragraph::new(lines), content);
    }
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
