//! **HUD effects** — three auto-playing text/HUD widgets in one panel.
//!
//! Where `form_controls` showcases interactive inputs, this example isolates
//! the **ambient** effects — [`Typewriter`], [`Marquee`], [`DigitalClock`] —
//! that play on their own, driven only by the tick clock. The clock reads the
//! wall clock (UTC) each frame; the other two advance their own animation.
//!
//! `t` cycles theme · `q` / `Esc` quits.
//!
//! ```sh
//! cargo run -p ratatui-sci-fi --example hud_effects
//! ```
//!
//! [`Typewriter`]: ratatui_sci_fi::Typewriter
//! [`Marquee`]: ratatui_sci_fi::Marquee
//! [`DigitalClock`]: ratatui_sci_fi::DigitalClock

use std::io::{self, Stdout};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
    DigitalClock, DigitalClockState, Marquee, MarqueeState, Theme, Typewriter, TypewriterState,
};

type Term = Terminal<CrosstermBackend<Stdout>>;

const THEMES: [Theme; 4] = [Theme::Cyberpunk, Theme::Fallout, Theme::Weyland, Theme::DeepSpace];

/// Centered header title (ASCII + width-1 glyphs).
const TITLE: &str = "▶  HUD EFFECTS  ◀";

pub struct App {
    frame: u64,
    theme_idx: usize,
    typewriter: TypewriterState,
    marquee: MarqueeState,
    clock: DigitalClockState,
}

impl App {
    pub fn new() -> Self {
        Self {
            frame: 0,
            theme_idx: 0,
            typewriter: TypewriterState::new(),
            marquee: MarqueeState::new(),
            clock: DigitalClockState::new(),
        }
    }

    pub fn theme(&self) -> Theme {
        THEMES[self.theme_idx]
    }

    /// Cycle to the next theme — exposed for the headless screenshot harness.
    pub fn cycle_theme(&mut self) {
        self.theme_idx = (self.theme_idx + 1) % THEMES.len();
    }

    pub fn tick(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        self.typewriter.tick();
        self.marquee.tick();
        self.clock.tick();
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
        Constraint::Length(3), // typewriter
        Constraint::Length(3), // marquee
        Constraint::Length(7), // digital clock (5-row segment + border)
    ])
    .split(outer[1]);

    typewriter_cell(f, theme, body[0], app);
    marquee_cell(f, theme, body[1], app);
    clock_cell(f, theme, body[2], app);

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

fn typewriter_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let inner = labeled_cell(f, theme, area, "TYPEWRITER");
    f.render_stateful_widget(
        Typewriter::new("INITIALIZING SUBSYSTEMS // PLEASE STAND BY")
            .ticks_per_char(2)
            .theme(theme),
        inner,
        &mut app.typewriter,
    );
}

fn marquee_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let inner = labeled_cell(f, theme, area, "MARQUEE");
    f.render_stateful_widget(
        Marquee::new("LONG RANGE SCAN ACTIVE \u{00b7} SECTOR 7G \u{00b7} ANOMALY DETECTED")
            .speed(2)
            .theme(theme),
        inner,
        &mut app.marquee,
    );
}

fn clock_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let inner = labeled_cell(f, theme, area, "DIGITAL CLOCK");
    let (h, m, s) = utc_hms();
    f.render_stateful_widget(DigitalClock::new(h, m, s).theme(theme), inner, &mut app.clock);
}

/// Current UTC wall-clock time as (hours, minutes, seconds).
fn utc_hms() -> (u32, u32, u32) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let day = secs % 86400;
    ((day / 3600) as u32, ((day % 3600) / 60) as u32, (day % 60) as u32)
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
