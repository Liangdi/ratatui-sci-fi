//! # ratatui-sci-fi
//!
//! Sci-fi themed widgets for the [Ratatui] TUI ecosystem: cyberpunk neon,
//! fallout terminals, Weyland consoles, deep-space HUDs — plus the bloodmoon
//! war-room, nebula hologram, arctic cryo-station, and monochrome sentinel
//! themes.
//!
//! [Ratatui]: https://ratatui.rs
//!
//! # Architecture
//!
//! - **Themes** ([`Theme`]) expose a [`Palette`] (raw ratatui `Color`s for
//!   direct drawing, e.g. on a `Canvas`) and a
//!   [`ratatui_style::Stylesheet`] (CSS cascade — the primary styling path).
//!   Both derive from one RGB source of truth, so they never drift.
//! - **Widgets** ([`widgets`]) are ordinary ratatui widgets. Stateless ones
//!   implement [`Widget`]; stateful ones implement [`StatefulWidget`] with a
//!   companion `…State` struct.
//! - **Audio** ([`audio`]) is a **catalog only** for now — sound-effect ids,
//!   descriptions, and asset filenames. The playback engine lands in a later
//!   phase.
//!
//! # Widget conventions — read before implementing a widget
//!
//! 1. **ratatui 0.30 trait model.** Implement the *stable* traits:
//!    - stateless: `impl Widget for Foo { fn render(self, area: Rect, buf: &mut Buffer) }`
//!    - stateful: `impl StatefulWidget for Foo { type State = FooState; fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) }`
//!    - `render` takes `self` **by value**. All animation/selection state lives
//!      in the `…State` struct, mutated by the app's event loop each tick.
//! 2. **Theming.** Prefer the stylesheet: get `&'static Stylesheet` via
//!    `theme.stylesheet()`, reuse one `ComputeScratch` across a whole frame,
//!    and compute a style with
//!    `sheet.compute_with(&NodeRef::new("Button").classes(&["focus"]), None, &mut scratch).to_style()`.
//!    For `Canvas` / shape drawing that needs a bare `Color`, use
//!    `theme.palette().accent.color()`.
//! 3. **Config vs state.** Immutable configuration (labels, dimensions, glyph
//!    sets) goes on the widget struct behind a `Foo::new(...)` builder.
//!    Everything the event loop mutates per tick (positions, RNG streams,
//!    blink phase) goes on `FooState`.
//! 4. **Deterministic tests.** Render into an offscreen `Buffer` and assert on
//!    its cells (no real terminal). See the ratatui-style `04_render` example
//!    for the buffer-dump pattern.
//! 5. **Shape variants (形态变体).** A widget may expose a `.shape(XxxShape)`
//!    builder (caret-bearing widgets use `.caret(CaretShape)`) to swap its glyph
//!    set, fill style, or border type. Shape enums are CONFIG — they live on the
//!    widget struct (per #3), never on `…State`. The `#[default]` variant MUST
//!    reproduce the widget's prior appearance byte-for-byte, so existing tests
//!    pass without modification. All variant glyphs MUST be Unicode width-1
//!    (never East Asian Width `W`/`F`) — frame/marker glyphs participate in
//!    centering math that assumes `chars().count() == display_width`. Colors
//!    stay on CSS: a shape variant affects glyphs/layout only, never the
//!    stylesheet. Keep existing `pub const` glyph tables as the default
//!    variant's backing data so the public API does not break.
//!
//! [`Widget`]: ratatui::widgets::Widget
//! [`StatefulWidget`]: ratatui::widgets::StatefulWidget

pub mod audio;
pub mod themes;
pub mod widgets;

pub use themes::{Palette, Rgb, Theme};
pub use widgets::*;
