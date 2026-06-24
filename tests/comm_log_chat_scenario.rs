//! End-to-end render coverage for `CommLog`'s **chat** style — the
//! AI-agent-chatbox path the `agent_console` example is built on: bordered
//! markdown cards, the streaming typewriter caret, a scrollable transcript,
//! and the message cap that bounds long-running consoles.
//!
//! This is an integration test: it lives under `tests/` and links only the
//! crate's public API, so it guards the contract downstream apps depend on
//! rather than internal state. Gated on `markdown` (Chat needs the parser).

#![cfg(feature = "markdown")]

use ratatui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};

use ratatui_sci_fi::{CommKind, CommLog, CommLogMessage, CommLogState, CommStyle, Theme};

/// Render `state` as a scrollable chat transcript into a fresh `w`×`h` buffer.
fn render_chat(state: &mut CommLogState, w: u16, h: u16, theme: Theme) -> Buffer {
    let area = Rect::new(0, 0, w, h);
    let mut buf = Buffer::empty(area);
    CommLog::new()
        .style(CommStyle::Chat)
        .scrollbar(true)
        .theme(theme)
        .render(area, &mut buf, state);
    buf
}

/// A mixed-speaker transcript (agent / user / system) renders without panicking
/// and the first agent card carries its accent-colored rounded border.
#[test]
fn chat_transcript_renders_mixed_cards() {
    let accent = Theme::Cyberpunk.palette().accent.color();
    let mut s = CommLogState::new();
    s.push(CommLogMessage::new("NEXUS-7", "Vectors locked. Standing by.", CommKind::Agent));
    s.push(CommLogMessage::new("OPERATOR", "status report", CommKind::User));
    s.push(CommLogMessage::new("SYS", "link nominal", CommKind::System));
    let buf = render_chat(&mut s, 48, 16, Theme::Cyberpunk);
    // First (agent) card's top-left rounded corner sits at (0,0), accent.
    assert_eq!(buf[(0, 0)].symbol(), "╭");
    assert_eq!(buf[(0, 0)].fg, accent, "agent card border must be accent");
}

/// A streaming message, ticked once, shows the blinking accent caret — the
/// "agent typing itself in" effect the agent_console conversation relies on.
#[test]
fn chat_streaming_message_shows_caret() {
    let accent = Theme::Cyberpunk.palette().accent.color();
    let mut s = CommLogState::new();
    s.push(CommLogMessage::new("NEXUS-7", "ack", CommKind::Agent));
    s.push_streaming(CommLogMessage::new(
        "ORACLE",
        "Pattern match at ninety-four percent confidence.",
        CommKind::Agent,
    ));
    s.tick(1); // reveal one char of the streaming tail
    let buf = render_chat(&mut s, 48, 16, Theme::Cyberpunk);
    let has_caret = (0..16u16).any(|y| {
        (0..48u16).any(|x| buf[(x, y)].symbol() == "█" && buf[(x, y)].fg == accent)
    });
    assert!(has_caret, "streaming chat card should show the accent caret");
}

/// A transcript taller than the viewport overflows and draws a scrollbar
/// thumb, exactly like paging back through an LLM history.
#[test]
fn chat_long_transcript_shows_scrollbar() {
    let mut s = CommLogState::new();
    for i in 0..40u32 {
        s.push(CommLogMessage::new("NEXUS-7", format!("message number {i}"), CommKind::Agent));
    }
    let buf = render_chat(&mut s, 48, 8, Theme::Cyberpunk);
    let track_x = 47; // scrollbar track occupies the last column
    let has_thumb = (0..8u16).any(|y| buf[(track_x, y)].symbol() == "█");
    assert!(has_thumb, "overflowing transcript should draw a scrollbar thumb");
}

/// The message cap bounds a long-running console across frames: pushing well
/// past the cap, then rendering several frames (each applies the cap and
/// exercises the per-message markdown cache), trims back to the cap, keeps the
/// newest message, and never panics.
#[test]
fn chat_message_cap_bounds_history_across_frames() {
    let mut s = CommLogState::new();
    for i in 0..200u32 {
        s.push(CommLogMessage::new("A", format!("body {i}"), CommKind::Agent));
    }
    let area = Rect::new(0, 0, 48, 12);
    let widget = CommLog::new()
        .style(CommStyle::Chat)
        .scrollbar(true)
        .max_messages(50)
        .theme(Theme::Cyberpunk);
    for _ in 0..5 {
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf, &mut s);
    }
    assert_eq!(s.messages.len(), 50, "cap must trim history to max_messages");
    assert_eq!(
        s.messages.last().unwrap().body,
        "body 199",
        "newest message must survive the trim"
    );
}
