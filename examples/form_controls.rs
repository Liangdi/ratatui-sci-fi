//! **Form controls** — the five interactive form widgets in one drivable panel.
//!
//! Where `widget_gallery` showcases every widget in a static grid, this example
//! isolates the **interactive** controls — [`Checkbox`], [`RadioGroup`],
//! [`Slider`], [`NumberStepper`], [`Dropdown`] — so you can drive each one and
//! see its `handle_key` behavior.
//!
//! ```text
//! ┌─ CHECKBOX ──────────────────────┐
//! │          [✓] SHIELDS            │
//! ├─ RADIO ──────────────────────────── (focus frame is accent-colored) ─┐
//! │  ◉ ENGAGE                                                            │
//! │  ○ STANDBY                                                           │
//! │  ○ SAFE                                                              │
//! ├─ SLIDER ─────────────────────────┤
//! │  ════◉──────────  42%             │
//! ├─ STEPPER ────────────────────────┤
//! │           ◂ 42 ▸                  │
//! ├─ DROPDOWN ───────────────────────┤
//! │  ▾ BETA                           │   ← Enter pops a centered overlay
//! └──────────────────────────────────┘
//! ```
//!
//! `Tab` / `BackTab` cycle focus · `←/→/↑/↓` nudge the focused control ·
//! `Enter` opens/commits the dropdown · `space` toggles the checkbox · `t`
//! cycles theme · `q` / `Esc` quits.
//!
//! ```sh
//! cargo run -p ratatui-sci-fi --example form_controls
//! ```
//!
//! [`Checkbox`]: ratatui_sci_fi::Checkbox
//! [`RadioGroup`]: ratatui_sci_fi::RadioGroup
//! [`Slider`]: ratatui_sci_fi::Slider
//! [`NumberStepper`]: ratatui_sci_fi::NumberStepper
//! [`Dropdown`]: ratatui_sci_fi::Dropdown

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
use ratatui_sci_fi::{
    Checkbox, Dropdown, DropdownState, NumberStepper, NumberStepperState, RadioGroup,
    RadioGroupState, Slider, SliderState, Theme,
};

type Term = Terminal<CrosstermBackend<Stdout>>;

const THEMES: [Theme; 4] = [Theme::Cyberpunk, Theme::Fallout, Theme::Weyland, Theme::DeepSpace];

const RADIO_OPTS: [&str; 3] = ["ENGAGE", "STANDBY", "SAFE"];
const DROPDOWN_OPTS: [&str; 4] = ["ALPHA", "BETA", "GAMMA", "DELTA"];

const FOCUS_COUNT: usize = 5;
const FOCUS_LABELS: [&str; FOCUS_COUNT] = ["CHECKBOX", "RADIO", "SLIDER", "STEPPER", "DROPDOWN"];

/// Centered header title (ASCII + width-1 glyphs).
const TITLE: &str = "▶  FORM CONTROLS  ◀";

pub struct App {
    frame: u64,
    theme_idx: usize,
    /// Index of the focused control, `0..FOCUS_COUNT`.
    focus: usize,
    checkbox_on: bool,
    radio: RadioGroupState,
    slider: SliderState,
    stepper: NumberStepperState,
    dropdown: DropdownState,
}

impl App {
    pub fn new() -> Self {
        Self {
            frame: 0,
            theme_idx: 0,
            focus: 0,
            checkbox_on: true,
            radio: RadioGroupState::new(),
            slider: SliderState { value: 0.4 },
            stepper: NumberStepperState { value: 42 },
            dropdown: DropdownState::new(),
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
                // Tab / BackTab cycle focus between the five controls.
                KeyCode::Tab => app.focus = (app.focus + 1) % FOCUS_COUNT,
                KeyCode::BackTab => app.focus = (app.focus + FOCUS_COUNT - 1) % FOCUS_COUNT,
                _ => match app.focus {
                    // Checkbox: space flips it (it's stateless, so the app owns the bool).
                    0 if key.code == KeyCode::Char(' ') => app.checkbox_on = !app.checkbox_on,
                    1 => RadioGroup::new(RADIO_OPTS).handle_key(&mut app.radio, key),
                    2 => Slider::new().handle_key(&mut app.slider, key),
                    3 => NumberStepper::new().min(0).max(100).step(1).handle_key(&mut app.stepper, key),
                    4 => Dropdown::new(DROPDOWN_OPTS).handle_key(&mut app.dropdown, key),
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

    // Root background.
    f.render_widget(Block::new().style(Style::new().bg(theme.palette().bg.color())), area);

    let outer =
        Layout::vertical([Constraint::Length(3), Constraint::Min(1), Constraint::Length(1)])
            .split(area);

    render_title(f, theme, outer[0]);

    // Five fixed-height control cells, top to bottom.
    let body = Layout::vertical([
        Constraint::Length(3), // checkbox
        Constraint::Length(5), // radio (3 options)
        Constraint::Length(3), // slider
        Constraint::Length(3), // stepper
        Constraint::Length(3), // dropdown (collapsed)
    ])
    .split(outer[1]);

    checkbox_cell(f, theme, body[0], app);
    radio_cell(f, theme, body[1], app);
    slider_cell(f, theme, body[2], app);
    stepper_cell(f, theme, body[3], app);
    dropdown_cell(f, theme, body[4], app);

    // Dropdown expands as a centered overlay (the popup protocol: Clear, then
    // render into a taller Rect). Drawn last so it floats above the cells.
    if app.dropdown.expanded {
        let pop = centered_rect(50, DROPDOWN_OPTS.len() as u16, f.area());
        f.render_widget(Clear, pop);
        f.render_stateful_widget(
            Dropdown::new(DROPDOWN_OPTS).theme(theme),
            pop,
            &mut app.dropdown,
        );
    }

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
    let hint = "Tab focus · ←→↑↓ nudge · Enter dropdown · space checkbox · t theme · q quit";
    let footer = Paragraph::new(Line::from(hint))
        .alignment(Alignment::Center)
        .style(Style::new().fg(muted));
    f.render_widget(footer, vertically_centered(area, 1));
}

fn checkbox_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let inner = labeled_cell(f, theme, area, FOCUS_LABELS[0], app.focus == 0);
    f.render_widget(Checkbox::new("SHIELDS").checked(app.checkbox_on).theme(theme), inner);
}

fn radio_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let inner = labeled_cell(f, theme, area, FOCUS_LABELS[1], app.focus == 1);
    f.render_stateful_widget(RadioGroup::new(RADIO_OPTS).theme(theme), inner, &mut app.radio);
}

fn slider_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let inner = labeled_cell(f, theme, area, FOCUS_LABELS[2], app.focus == 2);
    f.render_stateful_widget(Slider::new().theme(theme), inner, &mut app.slider);
}

fn stepper_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let inner = labeled_cell(f, theme, area, FOCUS_LABELS[3], app.focus == 3);
    f.render_stateful_widget(
        NumberStepper::new().min(0).max(100).step(1).theme(theme),
        inner,
        &mut app.stepper,
    );
}

fn dropdown_cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let inner = labeled_cell(f, theme, area, FOCUS_LABELS[4], app.focus == 4);
    // Collapsed: the dropdown renders its current selection into the cell.
    // Expanded: the cell shows a placeholder while the overlay lists the options.
    if app.dropdown.expanded {
        let accent = theme.palette().accent.color();
        f.render_widget(
            Paragraph::new(Line::from("▾ open…"))
                .alignment(Alignment::Center)
                .style(Style::new().fg(accent)),
            inner,
        );
    } else {
        f.render_stateful_widget(Dropdown::new(DROPDOWN_OPTS).theme(theme), inner, &mut app.dropdown);
    }
}

/// A bordered, titled cell whose frame is accent-colored when `focused` (the
/// focus indicator for Tab cycling) and muted otherwise. Returns the inner area.
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

/// A rect `percent_x%` wide and `height` rows tall, centered in `area`.
fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let vert =
        Layout::vertical([Constraint::Min(0), Constraint::Length(height), Constraint::Min(0)])
            .split(area);
    let pad = 100u16.saturating_sub(percent_x) / 2;
    Layout::horizontal([
        Constraint::Percentage(pad),
        Constraint::Percentage(percent_x),
        Constraint::Percentage(pad),
    ])
    .split(vert[1])[1]
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
