//! **AI Agent Console** — a multi-scene sci-fi integration: a boot sequence,
//! an operator login, and a live agent console where you talk to a roster of AI
//! agents whose replies type themselves in over a comms feed.
//!
//! Three scenes flow into one another, each compositing a different slice of the
//! crate:
//!
//! - **Boot** — a full-screen `MatrixRain` backdrop with a framed boot console
//!   centered over it: a glitching logo, a progress gauge that fills over the
//!   boot window, and a spinner cycling through boot phases.
//! - **Login** — `GlitchText` title flanked by a `BiometricChart`;
//!   two `TextInput` fields (callsign / passcode) with `Divider` underlines, an
//!   ENGAGE `Button`, and a `Spinner`-driven "authenticating" transition.
//! - **Console** — the main HUD: a custom agent roster (status-colored, sci-fi
//!   glyphs), a `CommLog` chat feed whose agent replies stream in
//!   character-by-character with a blinking caret, a `TargetLock`-framed transmit
//!   field, and a status column of `BiometricChart` / `EnergyGauge` / `Toggle`
//!   widgets.
//!
//! ```text
//!  BOOT ──▶ LOGIN ──▶ CONSOLE
//!                       │
//!                       ├── AGENTS roster (▲● ORACLE  ●)
//!                       ├── COMMS FEED (NEXUS-7 ▸ Vectors locked.█)
//!                       └── STATUS (vitals / load / defenses)
//! ```
//!
//! ## Keys
//!
//! - **Login**: `Tab` / `↑` / `↓` move field focus · type into the focused field ·
//!   `Enter` engage · `t` theme · `q` / `Esc` quit.
//! - **Console**: `↑` / `↓` pick an agent · type + `Enter` to transmit (the agent
//!   replies) · `a` alert popup · `h` open the full transcript · `t` theme ·
//!   `q` / `Esc` quit.
//! - **Chat (history)**: a full-page, scrollable user↔agent transcript (LLM chat
//!   mode) with a scrollbar — `↑` / `↓` / `PageUp` / `PageDown` scroll · type +
//!   `Enter` to send · `h` back to console · `t` theme · `q` / `Esc` quit.
//!
//! ```sh
//! cargo run -p ratatui-sci-fi --example agent_console
//! ```

use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};
use ratatui_sci_fi::{
    AlertPopup, AlertPopupState, BiometricChart, BiometricChartState, Button, ButtonShape,
    CommKind, CommLog, CommLogMessage, CommLogState, CommStyle, Divider, DividerShape, EnergyGauge,
    GlitchText, GlitchTextState, Level, MatrixRain, MatrixRainState, Panel, PanelShape, Spinner,
    SpinnerShape, SpinnerState, TargetLock, TextInput, TextInputState, Theme, Toggle, ToggleShape,
};

type Term = Terminal<CrosstermBackend<Stdout>>;

/// All eight themes, cycled with `t`.
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
const THEME_NAMES: [&str; 8] =
    ["Cyberpunk", "Fallout", "Weyland", "DeepSpace", "Bloodmoon", "Nebula", "Arctic", "Sentinel"];

/// Cycling phase labels shown beside the boot spinner — one phase per segment
/// of [`BOOT_TICKS`].
const BOOT_PHASES: &[&str] = &[
    "INITIALIZING KERNEL",
    "LINKING NEURAL MESH",
    "WAKING AGENT SUBSTRATE",
    "SYNCING QUANTUM UPLINK",
    "BOOT COMPLETE",
];
/// Ticks the boot intro runs before handing off to login.
const BOOT_TICKS: u64 = 70;

/// Lines cycled through while the operator "authenticates" after pressing ENGAGE.
const AUTH_LINES: &[&str] = &[
    "ESTABLISHING UPLINK",
    "VERIFYING BIOMETRIC SIGNATURE",
    "DECRYPTING ACCESS KEY",
    "SYNCING AGENT MESH",
    "AUTHORIZATION GRANTED",
];
/// Ticks the authenticating transition lasts.
const AUTH_TICKS: u64 = 80;

/// Rows scrolled per `PageUp` / `PageDown` in the Chat (history) scene.
const SCROLL_PAGE: usize = 8;

const TITLE_BOOT: &str = "▶  AEGIS // AI AGENT CONSOLE  ◀";
const TITLE_LOGIN: &str = "▶  IDENTITY AUTHORIZATION  ◀";

/// One AI agent on the roster.
struct Agent {
    name: &'static str,
    role: &'static str,
    /// Width-1 avatar glyph (crate convention #5).
    glyph: char,
    /// Roster status — drives the status-dot color.
    status: Level,
    /// Rotating canned replies; the operator's transmissions are answered from
    /// here so each agent has a distinct voice.
    replies: &'static [&'static str],
}

/// The agent roster. Status levels map to roster dot colors (ok / warn / alert).
const AGENTS: &[Agent] = &[
    Agent {
        name: "NEXUS-7",
        role: "TACTICAL COORD",
        glyph: '●',
        status: Level::Ok,
        replies: &[
            "**Vectors locked.** Standing by for your command, operator.",
            "Hostiles at bearing **047** — recommend evasive pattern *Delta*:\n- Shields front\n- Roll 15° port",
            "Threat board is **green**. We hold this sector.",
        ],
    },
    Agent {
        name: "ORACLE",
        role: "DATA SYNTHESIS",
        glyph: '◆',
        status: Level::Ok,
        replies: &[
            "Cross-referencing telemetry… pattern-match confidence **94%**. Run `analyze --deep` for detail.",
            "Three anomalies clustered near the relay — *statistically* not random.",
            "I've indexed the archive. Ask, and I will weigh it.",
        ],
    },
    Agent {
        name: "ATLAS",
        role: "ASTROGATION",
        glyph: '▲',
        status: Level::Ok,
        replies: &[
            "Course plotted. Drift within tolerance, Commander.",
            "Grav-shear ahead — adjusting burn by 0.3 delta-v.",
            "Next waypoint in six cycles. The lane is clear.",
        ],
    },
    Agent {
        name: "VEX",
        role: "RED TEAM",
        glyph: '■',
        status: Level::Warn,
        replies: &[
            "Their perimeter's soft. I'd punch through in twelve seconds.",
            "Don't ask how I got this. It's cleaner if you don't.",
            "I can crack it. You won't like the collateral.",
        ],
    },
    Agent {
        name: "SPECTRE",
        role: "GHOST PROTOCOL",
        glyph: '◑',
        status: Level::Warn,
        replies: &[
            "I'm already inside. They haven't noticed me.",
            "Shadows run deeper here than the schematics showed.",
            "Holding the channel. I go silent from here.",
        ],
    },
    Agent {
        name: "ECHO-3",
        role: "COMMS RELAY",
        glyph: '◐',
        status: Level::Alert,
        replies: &[
            "signal degrad— kssht — relay unstable...",
            "bouncing through three proxies, hold on... —kkk—",
            "if you can read this, the uplink is failing.",
        ],
    },
];

/// Which screen is active. Boot runs on a timer; the others are interactive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scene {
    Boot,
    Login,
    Console,
    /// Full-page, scrollable user↔agent transcript (LLM chat mode) — entered
    /// from the Console with `h`.
    Chat,
}

/// Console focus: either the agent roster (pick an agent) or the conversation
/// list (read / type / scroll). `Enter` on the roster drops into the chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ConsoleFocus {
    #[default]
    Roster,
    Chat,
}

impl ConsoleFocus {
    fn toggle(self) -> Self {
        match self {
            Self::Roster => Self::Chat,
            Self::Chat => Self::Roster,
        }
    }
}

pub struct App {
    frame: u64,
    theme_idx: usize,
    scene: Scene,
    // Shared animation clocks.
    title: GlitchTextState,
    spinner: SpinnerState,
    rain: MatrixRainState,
    // Login.
    login_user: TextInputState,
    login_key: TextInputState,
    /// 0 = callsign field, 1 = access-key field, 2 = ENGAGE button.
    login_focus: usize,
    authenticating: bool,
    auth_clock: u64,
    // Console.
    roster_select: usize,
    /// Which pane owns input in the Console scene (roster vs conversation).
    console_focus: ConsoleFocus,
    chat: CommLogState,
    chat_input: TextInputState,
    /// Per-agent reply cursor so each voice cycles through its `replies`.
    reply_idx: Vec<usize>,
    // Flair widgets reused across scenes.
    bio: BiometricChartState,
    // Alert popup.
    alert: AlertPopupState,
    alert_visible: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            frame: 0,
            // Boot straight into the Fallout (phosphor-green terminal) theme.
            theme_idx: THEMES.iter().position(|&t| t == Theme::Fallout).unwrap_or(0),
            scene: Scene::Boot,
            title: GlitchTextState::default(),
            spinner: SpinnerState::default(),
            rain: MatrixRainState::default(),
            login_user: TextInputState::default(),
            login_key: TextInputState::default(),
            login_focus: 0,
            authenticating: false,
            auth_clock: 0,
            roster_select: 0,
            console_focus: ConsoleFocus::default(),
            chat: CommLogState::new(),
            chat_input: TextInputState::default(),
            reply_idx: vec![0; AGENTS.len()],
            bio: BiometricChartState::new(3, 60),
            alert: AlertPopupState::default(),
            alert_visible: false,
        }
    }

    pub fn theme(&self) -> Theme {
        THEMES[self.theme_idx]
    }

    pub fn cycle_theme(&mut self) {
        self.theme_idx = (self.theme_idx + 1) % THEMES.len();
    }

    /// Fast-forward through boot + login so a headless capture lands in the
    /// Console scene with a seeded transcript. (Public for the screenshot
    /// harness, which can't drive real key events.)
    pub fn fast_forward_to_console(&mut self) {
        for _ in 0..BOOT_TICKS {
            self.tick();
        }
        self.login_user.value = "LIANGDI".into();
        self.engage();
        for _ in 0..AUTH_TICKS {
            self.tick();
        }
    }

    /// Type `msg` into the transmit field and send it (public for the screenshot
    /// harness).
    pub fn transmit(&mut self, msg: &str) {
        self.chat_input.value = msg.into();
        self.chat_input.cursor = self.chat_input.value.chars().count();
        self.send_message();
    }

    /// Whether the tail chat message is still streaming.
    pub fn chat_streaming(&self) -> bool {
        self.chat.is_streaming()
    }

    /// The operator's callsign (login_user), or a fallback before login.
    fn operator(&self) -> &str {
        let v = self.login_user.value.as_str();
        if v.is_empty() { "GUEST" } else { v }
    }

    /// Begin the authenticating transition (from the login ENGAGE button).
    fn engage(&mut self) {
        if self.login_user.value.trim().is_empty() || self.authenticating {
            return;
        }
        self.authenticating = true;
        self.auth_clock = 0;
    }

    /// Drop the just-typed operator line into the feed and queue a streaming
    /// reply from the selected agent.
    fn send_message(&mut self) {
        let body = self.chat_input.value.trim().to_string();
        if body.is_empty() {
            return;
        }
        let operator = self.operator().to_string();
        self.chat.push(CommLogMessage::new(operator, body, CommKind::User));
        // Reset the transmit field.
        self.chat_input.value.clear();
        self.chat_input.cursor = 0;

        // Queue the agent's reply, cycling through that agent's voice.
        let sel = self.roster_select.min(AGENTS.len() - 1);
        let idx = self.reply_idx[sel] % AGENTS[sel].replies.len();
        let reply = AGENTS[sel].replies[idx].to_string();
        self.reply_idx[sel] = self.reply_idx[sel].wrapping_add(1);
        self.chat
            .push_streaming(CommLogMessage::new(AGENTS[sel].name, reply, CommKind::Agent));
    }

    /// Seed the console's opening transcript when login completes. A multi-turn
    /// operator↔agent exchange gives the bottom-anchored console feed a tail to
    /// show and the full-page Chat scene enough history to scroll through.
    fn seed_console(&mut self) {
        self.chat.clear();
        let op = self.operator().to_string();
        // (kind, speaker, body) — `{op}` is substituted with the operator's name.
        let turns: &[(CommKind, &str, &str)] = &[
            (CommKind::Agent, "NEXUS-7", "Welcome back, **{op}**. All systems nominal. Awaiting orders."),
            (CommKind::User, "OPERATOR", "ORACLE, what's the read on that relay anomaly?"),
            (
                CommKind::Agent,
                "ORACLE",
                "Three **clustered** signatures near the relay — statistically *not* random. Cross-referencing telemetry via `telemetry --deep`.",
            ),
            (CommKind::User, "OPERATOR", "Threat assessment?"),
            (
                CommKind::Agent,
                "NEXUS-7",
                "Hostiles at bearing **047**. Recommend evasive pattern *Delta*:\n- Shields to front\n- Roll 15° port\n- Weapons **hot**",
            ),
            (
                CommKind::Agent,
                "ORACLE",
                "Pattern-match confidence **94%**. The cluster is converging on our vector.",
            ),
            (CommKind::User, "OPERATOR", "ATLAS, can we outrun them?"),
            (
                CommKind::Agent,
                "ATLAS",
                "Course plotted. Drift within tolerance — burn adjusted:\n```\nΔv = 0.30 m/s\nETA = 6 cycles\n```",
            ),
            (
                CommKind::Agent,
                "VEX",
                "Don't bother running. Their perimeter's *soft* — I'd punch through in `12s`.",
            ),
            (CommKind::User, "OPERATOR", "Hold, VEX. ORACLE, keep scanning. I want options."),
        ];
        // The opening line streams in; the rest land fully revealed so the
        // history is long enough to scroll the moment the Chat scene opens.
        for (i, (kind, speaker, body)) in turns.iter().enumerate() {
            let body = body.replace("{op}", &op);
            let speaker = if *speaker == "OPERATOR" { op.clone() } else { speaker.to_string() };
            let msg = CommLogMessage::new(speaker, body, *kind);
            if i == 0 {
                self.chat.push_streaming(msg);
            } else {
                self.chat.push(msg);
            }
        }
    }

    /// Advance every animation clock one tick and drive scene transitions.
    pub fn tick(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        self.title.tick();
        self.spinner.tick();
        self.bio.tick();
        self.login_user.tick();
        self.login_key.tick();
        self.chat_input.tick();
        // The comms feed unveils ~1 body char per tick (~16 cps at 60Hz).
        self.chat.tick(1);
        if self.alert_visible {
            self.alert.tick();
        }

        match self.scene {
            Scene::Boot => {
                self.rain.tick();
                if self.frame >= BOOT_TICKS {
                    self.scene = Scene::Login;
                }
            }
            Scene::Login => {
                if self.authenticating {
                    self.auth_clock = self.auth_clock.wrapping_add(1);
                    if self.auth_clock >= AUTH_TICKS {
                        self.authenticating = false;
                        self.scene = Scene::Console;
                        self.seed_console();
                    }
                }
            }
            Scene::Console | Scene::Chat => {}
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
            match app.scene {
                Scene::Boot => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('t') => app.cycle_theme(),
                    // Skip the boot intro on any other key.
                    _ => app.frame = app.frame.max(BOOT_TICKS),
                },
                Scene::Login if app.authenticating => match key.code {
                    // Lock input while authenticating; only quit/theme escape.
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('t') => app.cycle_theme(),
                    _ => {}
                },
                Scene::Login => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('t') => app.cycle_theme(),
                    KeyCode::Tab | KeyCode::Down => app.login_focus = (app.login_focus + 1) % 3,
                    KeyCode::Up => app.login_focus = (app.login_focus + 2) % 3,
                    KeyCode::Enter => app.engage(),
                    // Editing keys route to the focused field (0/1). `Char('q')`
                    // and `Char('t')` are claimed by the arms above, and the
                    // button (focus 2) ignores literal text.
                    KeyCode::Char(_)
                    | KeyCode::Backspace
                    | KeyCode::Left
                    | KeyCode::Right
                    | KeyCode::Home
                    | KeyCode::End if app.login_focus < 2 =>
                    {
                        let field =
                            if app.login_focus == 0 { &mut app.login_user } else { &mut app.login_key };
                        field.handle_key(key);
                    }
                    _ => {}
                },
                Scene::Console => match key.code {
                    // Top-level keys (work in either focus).
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('t') => app.cycle_theme(),
                    KeyCode::Char('a') => {
                        app.alert_visible = !app.alert_visible;
                        if app.alert_visible {
                            app.alert.flash(8);
                        }
                    }
                    // Open the full-page scrollable transcript (LLM chat mode).
                    KeyCode::Char('h') => app.scene = Scene::Chat,
                    // Toggle focus between the roster and the conversation.
                    KeyCode::Tab => app.console_focus = app.console_focus.toggle(),
                    // PageUp / PageDown scroll the feed in either focus.
                    KeyCode::PageUp => app.chat.scroll_up(SCROLL_PAGE),
                    KeyCode::PageDown => app.chat.scroll_down(SCROLL_PAGE),
                    // Roster focus: arrows pick an agent; Enter drops into chat.
                    KeyCode::Up if app.console_focus == ConsoleFocus::Roster => {
                        app.roster_select = (app.roster_select + AGENTS.len() - 1) % AGENTS.len();
                    }
                    KeyCode::Down if app.console_focus == ConsoleFocus::Roster => {
                        app.roster_select = (app.roster_select + 1) % AGENTS.len();
                    }
                    KeyCode::Enter if app.console_focus == ConsoleFocus::Roster => {
                        app.console_focus = ConsoleFocus::Chat;
                    }
                    // Chat focus: arrows line-scroll the feed; Enter sends;
                    // printable / editing keys go to the transmit field.
                    KeyCode::Up if app.console_focus == ConsoleFocus::Chat => {
                        app.chat.scroll_up(1)
                    }
                    KeyCode::Down if app.console_focus == ConsoleFocus::Chat => {
                        app.chat.scroll_down(1)
                    }
                    KeyCode::Enter
                        if !app.alert_visible && app.console_focus == ConsoleFocus::Chat =>
                    {
                        app.send_message();
                    }
                    KeyCode::Char(_)
                    | KeyCode::Backspace
                    | KeyCode::Left
                    | KeyCode::Right
                    | KeyCode::Home
                    | KeyCode::End
                        if !app.alert_visible && app.console_focus == ConsoleFocus::Chat =>
                    {
                        app.chat_input.handle_key(key);
                    }
                    _ => {}
                },
                // Full-page scrollable transcript. Vertical arrows / PageUp /
                // PageDown scroll history; horizontal arrows + printable chars
                // edit the transmit field; Enter sends; `h` returns to console.
                Scene::Chat => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('t') => app.cycle_theme(),
                    KeyCode::Char('h') => app.scene = Scene::Console,
                    KeyCode::PageUp => app.chat.scroll_up(SCROLL_PAGE),
                    KeyCode::PageDown => app.chat.scroll_down(SCROLL_PAGE),
                    KeyCode::Up => app.chat.scroll_up(1),
                    KeyCode::Down => app.chat.scroll_down(1),
                    KeyCode::Enter => app.send_message(),
                    KeyCode::Char(_)
                    | KeyCode::Backspace
                    | KeyCode::Left
                    | KeyCode::Right
                    | KeyCode::Home
                    | KeyCode::End =>
                    {
                        app.chat_input.handle_key(key);
                    }
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
    match app.scene {
        Scene::Boot => render_boot(f, app),
        Scene::Login => render_login(f, app),
        Scene::Console => render_console(f, app),
        Scene::Chat => render_chat_scene(f, app),
    }
}

// ─── Boot ────────────────────────────────────────────────────────────────────

/// The boot intro: a full-screen **digital-rain** backdrop with a translucent
/// boot console floating *in front of* it — the rain stays visible behind the
/// window (dimmed onto a dark "glass" tint), framed by a thick accent border,
/// with the glitch logo / progress gauge / phase spinner layered on top. Pure
/// sci-fi motion, no static boot log.
fn render_boot(f: &mut ratatui::Frame<'_>, app: &mut App) {
    let theme = app.theme();
    let p = theme.palette();
    let area = f.area();

    // 1. Full-screen digital-rain backdrop — the sci-fi "动效", the back layer.
    f.render_stateful_widget(
        MatrixRain::new().density(0.82).speed(0.6).theme(theme),
        area,
        &mut app.rain,
    );

    // 2. Translucent boot window floating over the rain. We deliberately do NOT
    //    use an opaque `Panel` (it would erase the rain behind the window).
    //    Instead we tint the window's interior onto the panel backdrop so the
    //    rain reads as "behind frosted glass", then frame it with a border-only
    //    block whose default style leaves that tint untouched.
    let win = centered(66, 13, area);
    let inner = Rect {
        x: win.x + 1,
        y: win.y + 1,
        width: win.width.saturating_sub(2),
        height: win.height.saturating_sub(2),
    };
    let buf = f.buffer_mut();
    for y in inner.top()..inner.bottom() {
        for x in inner.left()..inner.right() {
            // Keep the rain glyph (symbol already set above) but recolor it:
            // muted fg on the panel bg → dim rain visible behind the glass.
            buf[(x, y)].set_style(Style::new().fg(p.muted.color()).bg(p.panel.color()));
        }
    }
    f.render_widget(
        Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Thick)
            .border_style(Style::new().fg(p.accent.color()))
            .title(Line::from(" SYSTEM BOOT ").style(Style::new().fg(p.accent.color()))),
        win,
    );

    // 3. Content layered on top of the tinted glass.
    let rows = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(1), // glitch logo
        Constraint::Length(1), // subtitle
        Constraint::Length(1), // gap
        Constraint::Length(1), // progress gauge
        Constraint::Length(1), // gap
        Constraint::Length(1), // spinner + phase
        Constraint::Min(0),
    ])
    .split(inner);

    render_centered_title(
        f,
        theme,
        rows[1],
        "▶  AEGIS // AI AGENT CONSOLE  ◀",
        &mut app.title,
    );
    f.render_widget(
        Paragraph::new("NEURAL-LINK v3.7  //  INITIALIZING")
            .alignment(Alignment::Center)
            .style(Style::new().fg(p.accent.color())),
        rows[2],
    );

    // Progress gauge fills over the boot duration.
    let prog = (app.frame as f64 / BOOT_TICKS as f64).clamp(0.0, 1.0);
    f.render_widget(EnergyGauge::new(prog).label("BOOT").segments(28).theme(theme), rows[4]);

    // Cycling phase label beside the spinner.
    let step = (BOOT_TICKS / BOOT_PHASES.len() as u64).max(1);
    let phase = BOOT_PHASES[((app.frame / step) as usize) % BOOT_PHASES.len()];
    f.render_stateful_widget(
        Spinner::new().label(phase).shape(SpinnerShape::Braille).theme(theme),
        rows[6],
        &mut app.spinner,
    );
}

// ─── Login ───────────────────────────────────────────────────────────────────

fn render_login(f: &mut ratatui::Frame<'_>, app: &mut App) {
    let theme = app.theme();
    let area = f.area();

    let outer = Layout::vertical([Constraint::Length(7), Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    // Header band: glitch title | biometrics, for sci-fi flair.
    let header = Layout::horizontal([Constraint::Min(1), Constraint::Length(22)]).split(outer[0]);
    render_centered_title(f, theme, header[0], TITLE_LOGIN, &mut app.title);
    f.render_stateful_widget(
        BiometricChart::new(3).window(60).theme(theme),
        header[1],
        &mut app.bio,
    );

    // Centered login panel.
    let panel_area = centered(62, 19, outer[1]);
    let panel = Panel::new().title("IDENTITY AUTHORIZATION").shape(PanelShape::Double).theme(theme);
    let inner = panel.inner(panel_area);
    f.render_widget(panel, panel_area);
    render_login_form(f, theme, inner, app);

    // Footer hints.
    f.render_widget(
        Paragraph::new(format!(
            " [tab/↑↓] field  [enter] engage  [t] theme: {}  [q] quit",
            THEME_NAMES[app.theme_idx]
        ))
        .style(Style::new().fg(theme.palette().muted.color())),
        outer[2],
    );
}

fn render_login_form(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let p = theme.palette();
    let label_style = Style::new().fg(p.muted.color());
    let sub_style = Style::new().fg(p.accent.color());

    // Center the form block vertically with bookend spacers.
    let rows = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(1), // subtitle
        Constraint::Length(1), // gap
        Constraint::Length(1), // CALLSIGN label
        Constraint::Length(1), // callsign field
        Constraint::Length(1), // callsign underline
        Constraint::Length(1), // gap
        Constraint::Length(1), // ACCESS KEY label
        Constraint::Length(1), // key field
        Constraint::Length(1), // key underline
        Constraint::Length(1), // gap
        Constraint::Length(1), // button row
        Constraint::Length(1), // gap
        Constraint::Length(1), // status
        Constraint::Min(0),
    ])
    .split(area);

    f.render_widget(
        Paragraph::new("◉ NEURAL-LINK INTERFACE  //  v3.7 ◉")
            .alignment(Alignment::Center)
            .style(sub_style),
        rows[1],
    );

    f.render_widget(Paragraph::new("CALLSIGN").style(label_style), rows[3]);
    f.render_stateful_widget(
        TextInput::new().placeholder("enter callsign…").theme(theme),
        rows[4],
        &mut app.login_user,
    );
    f.render_widget(login_underline(theme, app.login_focus == 0), rows[5]);

    f.render_widget(Paragraph::new("PASSCODE").style(label_style), rows[7]);
    f.render_stateful_widget(
        TextInput::new().placeholder("enter passcode…").password(true).theme(theme),
        rows[8],
        &mut app.login_key,
    );
    f.render_widget(login_underline(theme, app.login_focus == 1), rows[9]);

    // ENGAGE button, centered in its row.
    let (label, focused) = if app.authenticating {
        ("◎ WORKING", false)
    } else {
        ("ENGAGE", app.login_focus == 2)
    };
    let btn = Button::new(label).focused(focused).shape(ButtonShape::Bracket).theme(theme);
    let btn_w = (label.chars().count() as u16) + 6;
    let btn_area = Rect {
        x: rows[11].x + rows[11].width.saturating_sub(btn_w) / 2,
        y: rows[11].y,
        width: btn_w.min(rows[11].width),
        height: 1,
    };
    f.render_widget(btn, btn_area);

    // Status row: spinner + auth progress while authenticating, else a hint.
    if app.authenticating {
        let step = AUTH_TICKS as usize / AUTH_LINES.len();
        let line = AUTH_LINES[(app.auth_clock as usize / step.max(1)) % AUTH_LINES.len()];
        f.render_stateful_widget(
            Spinner::new().label(line).shape(SpinnerShape::Braille).theme(theme),
            rows[13],
            &mut app.spinner,
        );
    } else {
        f.render_widget(
            Paragraph::new("› submit credentials to engage")
                .alignment(Alignment::Center)
                .style(label_style),
            rows[13],
        );
    }
}

/// A `Divider` rule beneath a login field: a solid accent line when focused, a
/// faint dotted line otherwise — doubles as the focus indicator.
fn login_underline(theme: Theme, focused: bool) -> Divider {
    Divider::new()
        .shape(if focused { DividerShape::Single } else { DividerShape::Dotted })
        .theme(theme)
}

// ─── Console ─────────────────────────────────────────────────────────────────

fn render_console(f: &mut ratatui::Frame<'_>, app: &mut App) {
    let theme = app.theme();
    let area = f.area();

    let outer =
        Layout::vertical([Constraint::Length(3), Constraint::Min(1), Constraint::Length(1)])
            .split(area);

    render_console_header(f, theme, outer[0], app);

    let body =
        Layout::horizontal([Constraint::Length(26), Constraint::Min(34), Constraint::Length(30)])
            .split(outer[1]);

    render_roster(f, theme, body[0], app);
    render_chat(f, theme, body[1], app);
    render_status(f, theme, body[2], app);

    let focus_label = if app.console_focus == ConsoleFocus::Roster { "AGENTS" } else { "COMMS" };
    f.render_widget(
        Paragraph::new(format!(
            " FOCUS:{}  [tab] switch  [↑↓] nav/scroll  [enter] select/send  [pgup/pgdn] scroll  [a] alert  [h] history  [t] theme: {}  [q] quit",
            focus_label, THEME_NAMES[app.theme_idx]
        ))
        .style(Style::new().fg(theme.palette().muted.color())),
        outer[2],
    );

    if app.alert_visible {
        let popup_area = centered(58, 7, area);
        f.render_widget(Clear, popup_area);
        let popup =
            AlertPopup::new("INTRUSION DETECTED — LOCKDOWN?").title(" ⚠ ALERT ").theme(theme);
        f.render_stateful_widget(popup, popup_area, &mut app.alert);
    }
}

fn render_console_header(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let p = theme.palette();
    let cols = Layout::horizontal([Constraint::Min(1), Constraint::Length(40)]).split(area);

    // Left: glitching console title.
    render_centered_title(f, theme, cols[0], TITLE_BOOT, &mut app.title);

    // Right: operator identity + live stardate, right-aligned.
    let operator = app.operator();
    let stardate = format!("SD {:.1}", app.frame as f64 / 10.0);
    let identity = Line::from(vec![
        Span::raw("OP "),
        Span::styled(operator, Style::new().fg(p.accent2.color())),
        Span::styled("  ● ONLINE  ", Style::new().fg(p.ok.color())),
        Span::styled(stardate, Style::new().fg(p.muted.color())),
    ]);
    // Vertically center the single line in the 3-row band.
    let band = Layout::vertical([Constraint::Min(0), Constraint::Length(1), Constraint::Min(0)])
        .split(cols[1])[1];
    f.render_widget(Paragraph::new(identity).alignment(Alignment::Right), band);
}

/// The agent roster: each agent is a 2-row block (glyph+name+status dot, then
/// role), with the selected block highlighted in accent.
fn render_roster(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let p = theme.palette();
    let title = if app.console_focus == ConsoleFocus::Roster { "◆ AGENTS" } else { "AGENTS" };
    let panel = Panel::new().title(title).shape(PanelShape::Double).theme(theme);
    let inner = panel.inner(area);
    f.render_widget(panel, area);

    let buf = f.buffer_mut();
    let selected = app.roster_select;
    let row_h: u16 = 2;
    for (i, agent) in AGENTS.iter().enumerate() {
        let y0 = inner.y + (i as u16) * row_h;
        if y0 + 1 >= inner.bottom() {
            break;
        }
        let is_sel = i == selected;
        let bg = if is_sel { p.accent.color() } else { p.panel.color() };
        // Fill the 2-row block background.
        for ry in 0..2u16 {
            for x in inner.x..inner.right() {
                buf[(x, y0 + ry)].set_style(Style::new().bg(bg));
            }
        }
        // Marker: ▶ on the selected row, space otherwise.
        let marker = if is_sel { '▶' } else { ' ' };
        let marker_style = Style::new().fg(if is_sel { p.bg.color() } else { p.muted.color() }).bg(bg);
        buf[(inner.x, y0)].set_char(marker).set_style(marker_style);

        // Avatar glyph.
        let glyph_style = Style::new().fg(if is_sel { p.bg.color() } else { p.accent.color() }).bg(bg);
        buf[(inner.x + 2, y0)].set_char(agent.glyph).set_style(glyph_style);

        // Name.
        let name_style = Style::new().fg(if is_sel { p.bg.color() } else { p.fg.color() }).bg(bg);
        let mut nx = inner.x + 4;
        for ch in agent.name.chars() {
            if nx >= inner.right() {
                break;
            }
            buf[(nx, y0)].set_char(ch).set_style(name_style);
            nx += 1;
        }

        // Role on the second row, muted (or dark on the selected accent bg).
        let role_style = Style::new().fg(if is_sel { p.panel.color() } else { p.muted.color() }).bg(bg);
        for (i, ch) in agent.role.chars().enumerate() {
            let rx = inner.x + 4 + i as u16;
            if rx >= inner.right() {
                break;
            }
            buf[(rx, y0 + 1)].set_char(ch).set_style(role_style);
        }

        // Status dot at the right edge, colored by status level.
        let dot_color = match agent.status {
            Level::Ok => p.ok.color(),
            Level::Warn => p.warn.color(),
            Level::Alert => p.alert.color(),
            Level::Normal => p.fg.color(),
        };
        let dot_x = inner.right().saturating_sub(2);
        if dot_x > nx {
            buf[(dot_x, y0)].set_char('●').set_style(Style::new().fg(dot_color).bg(bg));
        }
    }
}

/// The comms column: a labeled header (the addressed agent), the streaming
/// `CommLog` feed, and a `TargetLock`-framed transmit field.
fn render_chat(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let sel = app.roster_select.min(AGENTS.len() - 1);
    let agent = &AGENTS[sel];

    let cols = Layout::vertical([
        Constraint::Length(1), // header rule
        Constraint::Min(4),   // feed
        Constraint::Length(3), // transmit frame
    ])
    .split(area);

    f.render_widget(
        Divider::new().label(format!("{} // {}", agent.name, agent.role)).theme(theme),
        cols[0],
    );

    f.render_stateful_widget(
        CommLog::new().style(CommStyle::Chat).scrollbar(true).theme(theme),
        cols[1],
        &mut app.chat,
    );

    // Transmit field inside a TargetLock reticle.
    let tx_title = if app.console_focus == ConsoleFocus::Chat { "◆ TRANSMIT" } else { "TRANSMIT" };
    let lock = TargetLock::new().title(tx_title).theme(theme);
    let tx_inner = lock.inner(cols[2]);
    f.render_widget(lock, cols[2]);
    f.render_stateful_widget(
        TextInput::new().placeholder("type to address agent, enter to send…").theme(theme),
        tx_inner,
        &mut app.chat_input,
    );
}

/// Right-hand status column: agent vitals (a `BiometricChart` that grows to
/// fill the column), system load gauges, and two defense toggles.
fn render_status(f: &mut ratatui::Frame<'_>, theme: Theme, area: Rect, app: &mut App) {
    let cols = Layout::vertical([
        Constraint::Length(1), // "AGENT VITALS"
        Constraint::Min(6),    // biometric chart (grows to fill)
        Constraint::Length(1), // "SYSTEM LOAD"
        Constraint::Length(3), // three gauges
        Constraint::Length(2), // toggles
    ])
    .split(area);

    f.render_widget(Divider::new().label("AGENT VITALS").theme(theme), cols[0]);
    f.render_stateful_widget(
        BiometricChart::new(3).window(60).theme(theme),
        cols[1],
        &mut app.bio,
    );

    f.render_widget(Divider::new().label("SYSTEM LOAD").theme(theme), cols[2]);
    let gauges: [(&str, f64, f64, f64); 3] = [
        ("CPU", 0.62, 0.20, 0.08),
        ("MEM", 0.48, 0.15, 0.05),
        ("NET", 0.30, 0.25, 0.13),
    ];
    let t = app.frame as f64;
    for (i, (label, base, amp, freq)) in gauges.iter().enumerate() {
        let ratio = (base + amp * (t * freq).sin()).clamp(0.0, 1.0);
        let g = Rect { x: cols[3].x, y: cols[3].y + i as u16, width: cols[3].width, height: 1 };
        f.render_widget(EnergyGauge::new(ratio).label(*label).segments(14).theme(theme), g);
    }

    // Defense toggles — oscillate for visual life.
    let shields = (app.frame / 90).is_multiple_of(2);
    let tog = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(cols[4]);
    f.render_widget(Toggle::new("SHIELDS").on(shields).shape(ToggleShape::Orb).theme(theme), tog[0]);
    f.render_widget(
        Toggle::new("CLOAK").on(!shields).shape(ToggleShape::Diamond).theme(theme),
        tog[1],
    );
}

// ─── Chat (full-page history) ────────────────────────────────────────────────

/// The LLM-style conversation view: a tall, scrollable transcript with a
/// scrollbar, plus a transmit field. Same `app.chat` state as the console, now
/// rendered with `.scrollbar(true)` so the whole history pages back through.
fn render_chat_scene(f: &mut ratatui::Frame<'_>, app: &mut App) {
    let theme = app.theme();
    let area = f.area();
    let p = theme.palette();

    let outer = Layout::vertical([
        Constraint::Length(3), // header
        Constraint::Min(6),    // transcript
        Constraint::Length(3), // transmit
        Constraint::Length(1), // footer
    ])
    .split(area);

    // Header: glitching title left, scroll/back hint right.
    let header = Layout::horizontal([Constraint::Min(1), Constraint::Length(48)]).split(outer[0]);
    render_centered_title(f, theme, header[0], "▶  COMMS // FULL TRANSCRIPT  ◀", &mut app.title);
    let hint = Line::from(vec![
        Span::raw("[↑↓/pgup/pgdn] scroll  "),
        Span::styled("[h] back", Style::new().fg(p.accent.color())),
        Span::raw("  "),
        Span::styled("[enter] send", Style::new().fg(p.muted.color())),
    ]);
    let band =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1), Constraint::Min(0)]).split(header[1])
            [1];
    f.render_widget(Paragraph::new(hint).alignment(Alignment::Right), band);

    // Transcript panel + the scrollable CommLog (scrollbar mode).
    let sel = app.roster_select.min(AGENTS.len() - 1);
    let panel = Panel::new()
        .title(format!("TRANSCRIPT // addressed to {}", AGENTS[sel].name))
        .shape(PanelShape::Double)
        .theme(theme);
    let inner = panel.inner(outer[1]);
    f.render_widget(panel, outer[1]);
    f.render_stateful_widget(
        CommLog::new().style(CommStyle::Chat).scrollbar(true).theme(theme),
        inner,
        &mut app.chat,
    );

    // Transmit field (TargetLock, not Panel — Panel's padding would collapse a
    // 3-row frame to a zero-height interior and hide the input).
    let tx = TargetLock::new().title("TRANSMIT").theme(theme);
    let tx_inner = tx.inner(outer[2]);
    f.render_widget(tx, outer[2]);
    f.render_stateful_widget(
        TextInput::new().placeholder("message the agent — enter to send…").theme(theme),
        tx_inner,
        &mut app.chat_input,
    );

    // Footer: scroll position + keymap.
    let pos = if app.chat.at_bottom() { "▼ bottom" } else { "▴ scrolled up" };
    f.render_widget(
        Paragraph::new(format!(
            " {pos}   [↑↓/pgup/pgdn] scroll  [enter] send  [t] theme: {}  [h] back  [q] quit",
            THEME_NAMES[app.theme_idx]
        ))
        .style(Style::new().fg(p.muted.color())),
        outer[3],
    );
}

// ─── shared helpers ──────────────────────────────────────────────────────────

/// A glitching title, horizontally + vertically centered in `area`.
fn render_centered_title(
    f: &mut ratatui::Frame<'_>,
    theme: Theme,
    area: Rect,
    text: &str,
    state: &mut GlitchTextState,
) {
    let band = Layout::vertical([Constraint::Min(0), Constraint::Length(1), Constraint::Min(0)])
        .split(area)[1];
    let title_w = text.chars().count() as u16;
    let width = title_w.min(band.width);
    let x = band.x + band.width.saturating_sub(title_w) / 2;
    let title_area = Rect::new(x, band.y, width, 1);
    f.render_stateful_widget(GlitchText::new(text).intensity(0.12).theme(theme), title_area, state);
}

/// A rect `w` wide and `h` tall, centered in `area`.
fn centered(w: u16, h: u16, area: Rect) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    let vert =
        Layout::vertical([Constraint::Min(0), Constraint::Length(h), Constraint::Min(0)]).split(area);
    let hpad = area.width.saturating_sub(w) / 2;
    Layout::horizontal([
        Constraint::Length(hpad),
        Constraint::Length(w),
        Constraint::Length(area.width.saturating_sub(hpad).saturating_sub(w)),
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

#[cfg(test)]
mod tests {
    //! Headless smoke tests: render every scene to an off-screen buffer via
    //! `TestBackend` so we know `draw` never panics (including the streaming
    //! `CommLog` path and a shrunken terminal). No real terminal is touched.

    use super::*;
    use ratatui::backend::TestBackend;

    fn draw_scene(scene: Scene, width: u16, height: u16) {
        let backend = TestBackend::new(width, height);
        let mut term = Terminal::new(backend).unwrap();
        let mut app = App::new();
        app.scene = scene;
        // Exercise the console's streaming reply path: seed a greeting, then
        // transmit a message and let a few ticks unveil the agent's reply.
        // Chat reuses the same seeded transcript (now scrollable).
        if matches!(scene, Scene::Console | Scene::Chat) {
            app.seed_console();
            app.chat_input.value = "status report".into();
            app.send_message();
            // In Chat, also page up so the scrollbar/scroll path is exercised.
            if scene == Scene::Chat {
                app.chat.scroll_up(5);
            }
        }
        for _ in 0..40 {
            app.tick();
        }
        term.draw(|f| draw(f, &mut app)).unwrap();
    }

    #[test]
    fn renders_all_four_scenes_without_panic() {
        for scene in [Scene::Boot, Scene::Login, Scene::Console, Scene::Chat] {
            draw_scene(scene, 120, 40);
        }
    }

    #[test]
    fn renders_a_shrunken_terminal_without_panic() {
        // Tiny size stresses every layout's bounds + the manual roster writes.
        for scene in [Scene::Login, Scene::Console, Scene::Chat] {
            draw_scene(scene, 64, 20);
        }
    }
}
