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
use ratatui::{Terminal, backend::CrosstermBackend};
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut terminal = setup()?;
    let mut state = MatrixRainState::default();
    let mut theme_idx = 0usize;

    loop {
        let theme = THEMES[theme_idx];
        terminal.draw(|f| {
            let rain = MatrixRain::new().density(0.9).speed(0.6).theme(theme);
            f.render_stateful_widget(rain, f.area(), &mut state);
        })?;
        state.tick();

        if event::poll(Duration::from_millis(60))?
            && let Event::Key(key) = event::read()?
        {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('t') => theme_idx = (theme_idx + 1) % THEMES.len(),
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
