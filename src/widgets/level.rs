//! Shared [`Level`] type for status-bearing widgets.
//!
//! A handful of widgets color their output by a coarse status — a telemetry
//! readout, a reactor gauge, a toggle. They all share the same four-level
//! vocabulary, so the level lives here once and each widget maps it to the
//! cascade class name that resolves its color.
//!
//! ## Class mapping
//!
//! Each level maps to a stylesheet class that resolves to a `var(--…)` token off
//! the active palette:
//!
//! | Level     | CSS class | Palette token | Meaning        |
//! |-----------|-----------|---------------|----------------|
//! | `Normal`  | (base)    | `--fg`        | neutral        |
//! | `Ok`      | `ok`      | `--ok`        | nominal        |
//! | `Warn`    | `warn`    | `--warn`      | caution        |
//! | `Alert`   | `alert`   | `--alert`     | danger / error |

/// A coarse status level shared by status-bearing widgets.
///
/// See the [module docs](self) for the class/token mapping. Widgets turn a
/// `Level` into a cascade class name with [`Level::as_class`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Level {
    /// Neutral — resolves to the base node (theme foreground).
    #[default]
    Normal,
    /// Nominal / good — `.ok`.
    Ok,
    /// Caution — `.warn`.
    Warn,
    /// Danger / error — `.alert`.
    Alert,
}

impl Level {
    /// The cascade class name this level adds to its node type, or an empty
    /// slice for [`Level::Normal`] (which uses the bare node).
    ///
    /// Use it as the `classes` slice passed to
    /// [`ratatui_style::NodeRef::classes`]:
    /// `NodeRef::new("Value").classes(level.as_classes())`.
    pub fn as_classes(self) -> &'static [&'static str] {
        match self {
            Level::Normal => &[],
            Level::Ok => &["ok"],
            Level::Warn => &["warn"],
            Level::Alert => &["alert"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_has_no_class() {
        assert!(Level::Normal.as_classes().is_empty());
    }

    #[test]
    fn each_non_normal_level_has_one_named_class() {
        assert_eq!(Level::Ok.as_classes(), &["ok"]);
        assert_eq!(Level::Warn.as_classes(), &["warn"]);
        assert_eq!(Level::Alert.as_classes(), &["alert"]);
    }

    #[test]
    fn default_is_normal() {
        assert_eq!(Level::default(), Level::Normal);
    }
}
