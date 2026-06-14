//! Sci-fi widgets.
//!
//! All widgets follow the conventions documented at the crate root
//! ([`crate`]). Stateless widgets implement [`Widget`] (`render(self, area,
//! buf)`); stateful ones implement [`StatefulWidget`] with a separate
//! `…State` struct.
//!
//! [`Widget`]: ratatui::widgets::Widget
//! [`StatefulWidget`]: ratatui::widgets::StatefulWidget

pub mod boot_sequence;
pub mod biometric_chart;
pub mod button;
pub mod gauge;
pub mod glitch_text;
pub mod list;
pub mod matrix_rain;
pub mod popup;
pub mod scifi_radar;
pub mod target_lock;

pub use boot_sequence::{BootSequence, BootSequenceState};
pub use biometric_chart::{BiometricChart, BiometricChartState};
pub use button::Button;
pub use gauge::EnergyGauge;
pub use glitch_text::{GlitchText, GlitchTextState};
pub use list::{ScanList, ScanListState};
pub use matrix_rain::{MatrixRain, MatrixRainState};
pub use popup::{AlertPopup, AlertPopupState};
pub use scifi_radar::{Blip, SciFiRadar, SciFiRadarState};
pub use target_lock::TargetLock;
