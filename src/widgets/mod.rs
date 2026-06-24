//! Sci-fi widgets.
//!
//! All widgets follow the conventions documented at the crate root
//! ([`crate`]). Stateless widgets implement [`Widget`] (`render(self, area,
//! buf)`); stateful ones implement [`StatefulWidget`] with a separate
//! `…State` struct.
//!
//! [`Widget`]: ratatui::widgets::Widget
//! [`StatefulWidget`]: ratatui::widgets::StatefulWidget

pub mod activity_rings;
pub mod area_chart;
pub mod battery_indicator;
pub mod big_text;
pub mod biometric_chart;
pub mod boot_sequence;
pub mod button;
pub mod candlestick;
pub mod caret;
pub mod checkbox;
pub mod collapsible_panel;
pub mod comm_log;
pub mod compass;
pub mod countdown_timer;
pub mod digital_clock;
pub mod divider;
pub mod donut_chart;
pub mod dropdown;
pub mod gauge;
pub mod glitch_text;
pub mod graph;
pub mod heat_grid;
pub mod hbar_chart;
pub mod key_value;
pub mod level;
pub mod list;
pub mod marquee;
#[cfg(feature = "markdown")]
pub mod markdown;
pub mod matrix_rain;
pub mod noise;
pub mod number_stepper;
pub mod oscilloscope;
pub mod panel;
pub mod popup;
pub mod progress_bar;
pub mod radial_bar;
pub mod radial_gauge;
pub mod radio_group;
pub mod scanline_overlay;
pub mod scatter_plot;
pub mod scifi_radar;
pub mod signal_bars;
pub mod slider;
pub mod sparkline;
pub mod spectrum_bars;
pub mod spinner;
pub mod star_map;
pub mod stat;
pub mod status_led;
pub mod strip_chart;
pub mod table;
pub mod target_lock;
pub mod text_input;
pub mod thermometer;
pub mod timeline;
pub mod toggle;
pub mod tree_map;
pub mod typewriter;
pub mod util;
pub mod value;

pub use activity_rings::{ActivityRings, ActivityRingsState, RingShape};
pub use area_chart::{AreaChart, AreaChartState, AreaShape};
pub use battery_indicator::{BatteryIndicator, BatteryShape};
pub use big_text::{BigText, BigTextShape};
pub use biometric_chart::{BiometricChart, BiometricChartState};
pub use boot_sequence::{BootSequence, BootSequenceState};
pub use button::{Button, ButtonShape};
pub use candlestick::{CandlestickChart, CandlestickChartState, CandlestickShape, Ohlc};
pub use caret::CaretShape;
pub use checkbox::{Checkbox, CheckboxShape};
pub use collapsible_panel::{CollapsiblePanel, CollapsiblePanelState, CollapsibleShape};
pub use comm_log::{CommKind, CommLog, CommLogMessage, CommLogState, CommStyle};
pub use compass::{Compass, CompassShape, CompassState};
pub use countdown_timer::{CountdownTimer, CountdownTimerShape, CountdownTimerState};
pub use digital_clock::{DigitalClock, DigitalClockShape, DigitalClockState};
pub use divider::{Divider, DividerShape};
pub use donut_chart::{DonutChart, DonutChartState, DonutShape};
pub use dropdown::{Dropdown, DropdownState, DropdownShape};
pub use gauge::{EnergyGauge, GaugeShape};
pub use glitch_text::{GlitchShape, GlitchText, GlitchTextState};
pub use graph::{Graph, GraphShape};
pub use heat_grid::{HeatGrid, HeatGridState, HeatShape};
pub use hbar_chart::{HBarChart, HBarChartState, HBarShape};
pub use key_value::{KeyValue, KeyValueShape};
pub use level::Level;
pub use list::{ScanList, ScanListState};
pub use marquee::{Marquee, MarqueeShape, MarqueeState};
#[cfg(feature = "markdown")]
pub use markdown::{markdown_to_lines, Markdown};
pub use matrix_rain::{MatrixRain, MatrixRainState, MatrixShape};
pub use noise::{Noise, NoiseShape, NoiseState};
pub use number_stepper::{NumberStepper, NumberStepperState, NumberStepperShape};
pub use oscilloscope::{Oscilloscope, OscilloscopeShape, OscilloscopeState};
pub use panel::{Panel, PanelShape};
pub use popup::{AlertPopup, AlertPopupState, PopupShape};
pub use progress_bar::{ProgressBar, ProgressBarShape, ProgressBarState};
pub use radial_bar::{RadialBarChart, RadialBarState, RBarShape};
pub use radial_gauge::{DialShape, RadialGauge, RadialGaugeState};
pub use radio_group::{RadioGroup, RadioGroupState, RadioGroupShape};
pub use scanline_overlay::{ScanlineOverlay, ScanlineOverlayState, ScanlineShape};
pub use scatter_plot::{ScatterPlot, ScatterPlotState, ScatterShape};
pub use scifi_radar::{Blip, SciFiRadar, SciFiRadarState};
pub use signal_bars::{SignalBars, SignalBarsShape};
pub use slider::{Slider, SliderState, SliderShape};
pub use sparkline::{Sparkline, SparklineState, SparkShape};
pub use spectrum_bars::{SpectrumBars, SpectrumBarsState, SpectrumShape};
pub use spinner::{Spinner, SpinnerShape, SpinnerState};
pub use star_map::{StarMap, StarMapState, StarShape};
pub use stat::{Stat, StatShape, Trend};
pub use status_led::{LEDShape, StatusLED};
pub use strip_chart::{StripChart, StripChartState, StripShape};
pub use table::{Table, TableShape};
pub use target_lock::{TargetLock, TargetShape};
pub use text_input::{TextInput, TextInputState};
pub use thermometer::{Thermometer, ThermometerShape};
pub use timeline::{Timeline, TimelineShape};
pub use toggle::{Toggle, ToggleShape};
pub use tree_map::{TreeMap, TreeMapState, TreeShape};
pub use typewriter::{Typewriter, TypewriterShape, TypewriterState};
pub use value::Value;
