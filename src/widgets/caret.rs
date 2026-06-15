//! Shared caret/cursor glyph shape.
//!
//! Used by [`crate::widgets::TextInput`] (the insertion caret) and
//! [`crate::widgets::ScanList`] (the selection cursor). It is the one shape
//! type shared across widgets; every other widget owns its own `…Shape` enum
//! because their glyph tables are structurally different.
//!
//! See convention #5 at the crate root: the [`CaretShape::Block`] default
//! reproduces the widgets' prior `█` caret byte-for-byte, and every glyph is
//! Unicode width-1.

/// Blinking caret glyph for text-input and list-cursor widgets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CaretShape {
    /// Full block `█` — the classic console caret (the prior default).
    #[default]
    Block,
    /// Left half-block `▎` — a thin insertion bar.
    Bar,
    /// Lower half-block `▁` — an underscore-style caret.
    Underscore,
    /// Upper half-block `▀` — a ceiling-style caret.
    Half,
}

impl CaretShape {
    /// The width-1 glyph this caret renders as.
    #[must_use]
    pub const fn glyph(self) -> char {
        match self {
            Self::Block => '█',
            Self::Bar => '▎',
            Self::Underscore => '▁',
            Self::Half => '▀',
        }
    }
}
