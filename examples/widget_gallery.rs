//! **Widget gallery** — a tabbed tour of the widgets.
//!
//! Four tabs (`←`/`→` to switch), each a 2×3 grid of labelled cells:
//! - **BASICS** — Button, Toggle, Value, Spinner, TextInput, Divider
//! - **FORMS** — Checkbox, RadioGroup, Slider, NumberStepper, Dropdown, ComboBox
//! - **INDICATORS** — StatusLED, SignalBars, BatteryIndicator, Thermometer, CountdownTimer, ProgressBar
//! - **CHARTS** — Oscilloscope, StarMap, Graph, PieChart, Speedometer, LineChart
//!
//! `←/→` switch tabs · `space` toggles · type into the input · `↑/↓` move the
//! radio cursor · `t` cycles themes · `q` / `Esc` quits.
//!
//! ```sh
//! cargo run -p ratatui-sci-fi --example widget_gallery
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
use ratatui_sci_fi::{
    BatteryIndicator, Button, ButtonShape, Checkbox, ComboBox, ComboBoxState, CountdownTimer,
    CountdownTimerState, Divider, DividerShape, Dropdown, DropdownState, GlitchText,
    GlitchTextState, Graph, Level, LineChart, NumberStepper, NumberStepperState, Oscilloscope,
    OscilloscopeShape, OscilloscopeState, PieChart, ProgressBar, ProgressBarState, RadioGroup,
    RadioGroupState, SignalBars, Slider, SliderState, Speedometer, Spinner, SpinnerShape,
    SpinnerState, StarMap, StarMapState, StatusLED, Tabs, TabsState, TextInput, TextInputState,
    Theme, Thermometer, Toggle, ToggleShape, Value,
};

type Term = Terminal<CrosstermBackend<Stdout>>;

const THEMES: [Theme; 4] = [Theme::Cyberpunk, Theme::Fallout, Theme::Weyland, Theme::DeepSpace];
const THEME_NAMES: [&str; 4] = ["Cyberpunk", "Fallout", "Weyland", "DeepSpace"];
const TITLE: &str = "▶  WIDGET GALLERY  ◀";
const BUTTONS: [&str; 3] = ["ENGAGE", "SCAN", "ABORT"];
const TAB_LABELS: [&str; 4] = ["BASICS", "FORMS", "INDICATORS", "CHARTS"];
const RADIO_OPTS: [&str; 3] = ["ENGAGE", "STANDBY", "SAFE"];

pub struct App {
    frame: u64,
    theme_idx: usize,
    button_focus: usize,
    toggle_on: bool,
    tabs: TabsState,
    title: GlitchTextState,
    spinner: SpinnerState,
    input: TextInputState,
    radio: RadioGroupState,
    slider: SliderState,
    number: NumberStepperState,
    dropdown: DropdownState,
    combo: ComboBoxState,
    countdown: CountdownTimerState,
    progress: ProgressBarState,
    scope: OscilloscopeState,
    stars: StarMapState,
}

impl App {
    pub fn new() -> Self {
        Self {
            frame: 0,
            theme_idx: 0,
            button_focus: 0,
            toggle_on: true,
            tabs: TabsState::new(),
            title: GlitchTextState::default(),
            spinner: SpinnerState::default(),
            input: TextInputState::default(),
            radio: RadioGroupState::new(),
            slider: SliderState { value: 0.4 },
            number: NumberStepperState { value: 42 },
            dropdown: DropdownState::new(),
            combo: ComboBoxState::new(),
            countdown: CountdownTimerState::new(30),
            progress: ProgressBarState::new(),
            scope: OscilloscopeState::new(),
            stars: StarMapState::new(),
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
        self.title.tick();
        self.spinner.tick();
        self.input.tick();
        self.countdown.tick();
        if self.frame.is_multiple_of(16) && self.countdown.remaining > 0 {
            self.countdown.remaining -= 1;
        }
        self.progress.tick();
        self.scope.tick();
        self.stars.tick();
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
                // ←/→ switch tabs.
                KeyCode::Left | KeyCode::Right => {
                    Tabs::new(TAB_LABELS).handle_key(&mut app.tabs, key);
                }
                KeyCode::Char(' ') => app.toggle_on = !app.toggle_on,
                // Typing into the text input (only meaningful on the BASICS tab).
                KeyCode::Char(c) if app.tabs.selected == 0 && c != ' ' => app.input.handle_key(key),
                KeyCode::Backspace if app.tabs.selected == 0 => app.input.handle_key(key),
                // ↑/↓ steer the radio cursor (FORMS tab).
                KeyCode::Up | KeyCode::Down if app.tabs.selected == 1 => {
                    RadioGroup::new(RADIO_OPTS).handle_key(&mut app.radio, key);
                }
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

    let outer = Layout::vertical([
        Constraint::Length(3), // title
        Constraint::Length(3), // tab bar
        Constraint::Min(1),    // page
        Constraint::Length(1), // footer
    ])
    .split(area);

    render_title(f, theme, outer[0], &mut app.title);
    f.render_stateful_widget(Tabs::new(TAB_LABELS).theme(theme), outer[1], &mut app.tabs);

    let page = outer[2];
    match app.tabs.selected.min(TAB_LABELS.len() - 1) {
        0 => basics_page(f, theme, page, app),
        1 => forms_page(f, theme, page, app),
        2 => indicators_page(f, theme, page, app),
        _ => charts_page(f, theme, page, app),
    }

    f.render_widget(
        Paragraph::new(format!(
            " [←→] tabs  [space] toggle  [↑↓] radio  type: input  [t] theme: {}  [q] quit",
            THEME_NAMES[app.theme_idx]
        ))
        .style(Style::new().fg(theme.palette().muted.color())),
        outer[3],
    );
}

/// Centered, glitching header title.
fn render_title(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, state: &mut GlitchTextState) {
    let band = Layout::vertical([Constraint::Min(0), Constraint::Length(1), Constraint::Min(0)])
        .split(area)[1];
    let title_w = TITLE.chars().count() as u16;
    let width = title_w.min(band.width);
    let x = band.x + band.width.saturating_sub(title_w) / 2;
    f.render_stateful_widget(
        GlitchText::new(TITLE).intensity(0.12).theme(theme),
        Rect::new(x, band.y, width, 1),
        state,
    );
}

/// A labelled cell header (`▸ TITLE` punched through a muted `─` rule);
/// returns the content area beneath it.
fn cell(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, title: &str) -> Rect {
    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).split(area);
    f.render_widget(Divider::new().label(format!("▸ {title}")).theme(theme), chunks[0]);
    chunks[1]
}

/// Split `area` into a 2×3 grid of six cells (row-major).
fn grid6(area: Rect) -> Vec<Rect> {
    let rows = Layout::vertical([Constraint::Min(1), Constraint::Min(1)]).split(area);
    let mut v = Vec::with_capacity(6);
    for row in rows.iter().copied() {
        for c in Layout::horizontal([Constraint::Min(1), Constraint::Min(1), Constraint::Min(1)])
            .split(row)
            .iter()
        {
            v.push(*c);
        }
    }
    v
}

fn v3(area: Rect) -> Vec<Rect> {
    Layout::vertical([Constraint::Min(1), Constraint::Min(1), Constraint::Min(1)]).split(area).to_vec()
}

// ── BASICS ──────────────────────────────────────────────────────────────────

fn basics_page(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let g = grid6(area);
    let shapes = [ButtonShape::Bracket, ButtonShape::Pill, ButtonShape::Framed];

    let b = cell(f, theme, g[0], "BUTTON");
    for (i, (r, label)) in v3(b).into_iter().zip(BUTTONS.iter()).enumerate() {
        f.render_widget(
            Button::new(*label).focused(i == app.button_focus).shape(shapes[i]).theme(theme),
            r,
        );
    }

    let t = cell(f, theme, g[1], "TOGGLE");
    let tr = v3(t);
    f.render_widget(Toggle::new("SHIELDS").on(app.toggle_on).theme(theme), tr[0]);
    f.render_widget(
        Toggle::new("CLOAK").on(!app.toggle_on).shape(ToggleShape::Diamond).theme(theme),
        tr[1],
    );

    let v = cell(f, theme, g[2], "VALUE");
    let vr = v3(v);
    let t0 = app.frame as f64;
    f.render_widget(
        Value::new(format!("{:.0}%", 80.0 + (t0 * 0.05).sin() * 6.0))
            .label("HULL")
            .state(Level::Ok)
            .theme(theme),
        vr[0],
    );
    f.render_widget(Value::new("47%").label("FUEL").state(Level::Warn).theme(theme), vr[1]);
    f.render_widget(Value::new("21%").label("O2").state(Level::Alert).theme(theme), vr[2]);

    let sp = cell(f, theme, g[3], "SPINNER");
    f.render_stateful_widget(
        Spinner::new().label("SYNC").shape(SpinnerShape::Braille).theme(theme),
        sp,
        &mut app.spinner,
    );

    let ti = cell(f, theme, g[4], "TEXT INPUT");
    f.render_stateful_widget(
        TextInput::new().placeholder("callsign…").theme(theme),
        ti,
        &mut app.input,
    );

    let d = cell(f, theme, g[5], "DIVIDER");
    let dr = v3(d);
    f.render_widget(Divider::new().shape(DividerShape::Single).theme(theme), dr[0]);
    f.render_widget(
        Divider::new().label("SECTION").shape(DividerShape::Double).theme(theme),
        dr[1],
    );
    f.render_widget(Divider::new().shape(DividerShape::Heavy).theme(theme), dr[2]);
}

// ── FORMS ───────────────────────────────────────────────────────────────────

fn forms_page(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let g = grid6(area);

    let cb = cell(f, theme, g[0], "CHECKBOX");
    let cr = v3(cb);
    f.render_widget(Checkbox::new("SHIELDS").checked(true).theme(theme), cr[0]);
    f.render_widget(Checkbox::new("CLOAK").checked(false).theme(theme), cr[1]);

    let rd = cell(f, theme, g[1], "RADIO");
    f.render_stateful_widget(RadioGroup::new(RADIO_OPTS).theme(theme), rd, &mut app.radio);

    let sl = cell(f, theme, g[2], "SLIDER");
    f.render_stateful_widget(Slider::new().theme(theme), sl, &mut app.slider);

    let ns = cell(f, theme, g[3], "STEPPER");
    f.render_stateful_widget(
        NumberStepper::new().min(0).max(100).theme(theme),
        ns,
        &mut app.number,
    );

    let dd = cell(f, theme, g[4], "DROPDOWN");
    f.render_stateful_widget(
        Dropdown::new(["ALPHA", "BETA", "GAMMA"]).theme(theme),
        dd,
        &mut app.dropdown,
    );

    let cb2 = cell(f, theme, g[5], "COMBO BOX");
    f.render_stateful_widget(
        ComboBox::new(["ALPHA", "BETA", "GAMMA"]).theme(theme),
        cb2,
        &mut app.combo,
    );
}

// ── INDICATORS ──────────────────────────────────────────────────────────────

fn indicators_page(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let g = grid6(area);

    let led = cell(f, theme, g[0], "STATUS LED");
    let lr = v3(led);
    f.render_widget(StatusLED::new("LINK").level(Level::Ok).theme(theme), lr[0]);
    f.render_widget(StatusLED::new("HULL").level(Level::Alert).theme(theme), lr[1]);

    let sg = cell(f, theme, g[1], "SIGNAL");
    f.render_widget(
        SignalBars::new(((app.frame / 12) % 6) as u8).bars(5).theme(theme),
        sg,
    );

    let bt = cell(f, theme, g[2], "BATTERY");
    f.render_widget(BatteryIndicator::new(0.7).theme(theme), bt);

    let th = cell(f, theme, g[3], "THERMOMETER");
    f.render_widget(Thermometer::new(0.6).theme(theme), th);

    let cd = cell(f, theme, g[4], "COUNTDOWN");
    f.render_stateful_widget(CountdownTimer::new().theme(theme), cd, &mut app.countdown);

    let pb = cell(f, theme, g[5], "PROGRESS");
    let pr = v3(pb);
    f.render_stateful_widget(
        ProgressBar::new(Some((app.frame % 100) as f32 / 100.0)).theme(theme),
        pr[0],
        &mut app.progress,
    );
    f.render_stateful_widget(
        ProgressBar::new(None).theme(theme),
        pr[1],
        &mut ProgressBarState::new(),
    );
}

// ── CHARTS ──────────────────────────────────────────────────────────────────

fn charts_page(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let g = grid6(area);

    let osc = cell(f, theme, g[0], "OSCILLOSCOPE");
    f.render_stateful_widget(
        Oscilloscope::new(OscilloscopeShape::Sine).freq(0.12).theme(theme),
        osc,
        &mut app.scope,
    );

    let sm = cell(f, theme, g[1], "STAR MAP");
    f.render_stateful_widget(StarMap::new().density(8).theme(theme), sm, &mut app.stars);

    let gr = cell(f, theme, g[2], "GRAPH");
    f.render_widget(
        Graph::new([(0.2, 0.8), (0.8, 0.8), (0.5, 0.1)])
            .edges([(0, 1), (1, 2), (2, 0)])
            .theme(theme),
        gr,
    );

    let pie = cell(f, theme, g[3], "PIE");
    f.render_widget(PieChart::new([3.0, 2.0, 1.0]).theme(theme), pie);

    let spd = cell(f, theme, g[4], "SPEEDOMETER");
    f.render_widget(
        Speedometer::new((app.frame as f32 * 0.02).sin() * 0.5 + 0.5).theme(theme),
        spd,
    );

    let lc = cell(f, theme, g[5], "LINE");
    let data: Vec<f32> = (0..40)
        .map(|i| (app.frame as f32 * 0.1 + i as f32 * 0.3).sin())
        .collect();
    f.render_widget(LineChart::new(data).theme(theme), lc);
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
