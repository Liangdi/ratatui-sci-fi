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
pub mod divider;
pub mod gauge;
pub mod glitch_text;
pub mod level;
pub mod list;
pub mod matrix_rain;
pub mod panel;
pub mod popup;
pub mod scifi_radar;
pub mod spinner;
pub mod target_lock;
pub mod text_input;
pub mod toggle;
pub mod value;

pub use biometric_chart::{BiometricChart, BiometricChartState};
pub use boot_sequence::{BootSequence, BootSequenceState};
pub use button::Button;
pub use divider::Divider;
pub use gauge::EnergyGauge;
pub use glitch_text::{GlitchText, GlitchTextState};
pub use level::Level;
pub use list::{ScanList, ScanListState};
pub use matrix_rain::{MatrixRain, MatrixRainState};
pub use panel::Panel;
pub use popup::{AlertPopup, AlertPopupState};
pub use scifi_radar::{Blip, SciFiRadar, SciFiRadarState};
pub use spinner::{Spinner, SpinnerState};
pub use target_lock::TargetLock;
pub use text_input::{TextInput, TextInputState};
pub use toggle::Toggle;
pub use value::Value;
