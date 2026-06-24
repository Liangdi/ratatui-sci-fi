//! Standalone **Matrix digital-rain** demo — a full-screen animated backdrop.
//!
//! Cycles the theme on `t`, quits on `q` / `Esc`.
//!
//! ```sh
//! cargo run -p ratatui-sci-fi --example matrix_rain
//! ```

use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{Frame, Terminal, backend::CrosstermBackend};
use ratatui_sci_fi::{MatrixRain, MatrixRainState, Theme};

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

/// Drives the rain's animation clock and the active theme. Kept as a small
/// `pub` struct (with a `draw` entry point) so the headless screenshot harness
/// can render it off-screen without a real terminal.
pub struct App {
    state: MatrixRainState,
    theme_idx: usize,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        Self { state: MatrixRainState::default(), theme_idx: 0 }
    }

    pub fn theme(&self) -> Theme {
        THEMES[self.theme_idx]
    }

    /// Cycle to the next theme — mirrors the `t` key.
    pub fn cycle_theme(&mut self) {
        self.theme_idx = (self.theme_idx + 1) % THEMES.len();
    }

    pub fn tick(&mut self) {
        self.state.tick();
    }

    pub fn draw(f: &mut Frame<'_>, app: &mut App) {
        let rain = MatrixRain::new().density(0.9).speed(0.6).theme(app.theme());
        f.render_stateful_widget(rain, f.area(), &mut app.state);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut terminal = setup()?;
    let mut app = App::new();

    loop {
        terminal.draw(|f| App::draw(f, &mut app))?;
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
