//! Sci-fi widgets.
//!
//! All widgets follow the conventions documented at the crate root
//! ([`crate`]). Stateless widgets implement [`Widget`] (`render(self, area,
//! buf)`); stateful ones implement [`StatefulWidget`] with a separate
//! `…State` struct.
//!
//! [`Widget`]: ratatui::widgets::Widget
//! [`StatefulWidget`]: ratatui::widgets::StatefulWidget

pub mod biometric_chart;
pub mod boot_sequence;
pub mod button;
pub mod caret;
pub mod divider;
pub mod gauge;
pub mod glitch_text;
pub mod heat_grid;
pub mod level;
pub mod list;
pub mod matrix_rain;
pub mod panel;
pub mod popup;
pub mod radial_gauge;
pub mod scifi_radar;
pub mod spectrum_bars;
pub mod spinner;
pub mod target_lock;
pub mod text_input;
pub mod toggle;
pub mod value;

pub use biometric_chart::{BiometricChart, BiometricChartState};
pub use boot_sequence::{BootSequence, BootSequenceState};
pub use button::{Button, ButtonShape};
pub use caret::CaretShape;
pub use divider::{Divider, DividerShape};
pub use gauge::{EnergyGauge, GaugeShape};
pub use glitch_text::{GlitchShape, GlitchText, GlitchTextState};
pub use heat_grid::{HeatGrid, HeatGridState, HeatShape};
pub use level::Level;
pub use list::{ScanList, ScanListState};
pub use matrix_rain::{MatrixRain, MatrixRainState, MatrixShape};
pub use panel::{Panel, PanelShape};
pub use popup::{AlertPopup, AlertPopupState, PopupShape};
pub use radial_gauge::{DialShape, RadialGauge, RadialGaugeState};
pub use scifi_radar::{Blip, SciFiRadar, SciFiRadarState};
pub use spectrum_bars::{SpectrumBars, SpectrumBarsState, SpectrumShape};
pub use spinner::{Spinner, SpinnerShape, SpinnerState};
pub use target_lock::{TargetLock, TargetShape};
pub use text_input::{TextInput, TextInputState};
pub use toggle::{Toggle, ToggleShape};
pub use value::Value;
