//! Shared render/animation helpers factored out of the per-widget code.
//!
//! These collapse boilerplate that was copy-pasted across the ~32 widgets:
//!
//! - [`capped_push`] — the rolling-window `push + drain(..overflow)` trim used
//!   by every history-keeping chart's `tick` / `push`.
//! - [`draw_centered_label`] — the "count chars, center, set each glyph" block
//!   shared by the labeled gauges and charts.
//!
//! Both assume the crate's width-1-glyph convention (crate-root docs, item 5):
//! a glyph's `chars().count()` equals its display width.

use ratatui::buffer::Buffer;
use ratatui::style::Color;

/// Append `value` to `buf`, then drop the oldest entries once it grows past
/// `cap`, leaving the newest `cap` items. The rolling-window trim shared by
/// every chart widget.
///
/// ```
/// # use ratatui_sci_fi::widgets::util::capped_push;
/// let mut buf = Vec::new();
/// for v in 0..5 { capped_push(&mut buf, v, 3); }
/// assert_eq!(buf, vec![2, 3, 4], "oldest dropped, newest retained");
/// ```
pub fn capped_push<T>(buf: &mut Vec<T>, value: T, cap: usize) {
    buf.push(value);
    let overflow = buf.len().saturating_sub(cap);
    if overflow > 0 {
        buf.drain(..overflow);
    }
}

/// Draw `label` horizontally centered within a `width`-cell span starting at
/// column `x`, on row `y`, clipped at column `right` (exclusive). Each glyph is
/// painted with `fg` / `bg`.
///
/// `width` is the span to center *within* — usually the widget's full
/// `area.width`, but some widgets center within a sub-span (e.g. a ring's
/// diameter); pass that span here. Assumes width-1 glyphs.
#[allow(clippy::too_many_arguments)]
pub fn draw_centered_label(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    right: u16,
    label: &str,
    fg: Color,
    bg: Color,
) {
    let label_len = label.chars().count() as u16;
    let start = x + width.saturating_sub(label_len) / 2;
    for (gx, ch) in (start..).zip(label.chars()) {
        if gx >= right {
            break;
        }
        buf[(gx, y)].set_char(ch).set_fg(fg).set_bg(bg);
    }
}
