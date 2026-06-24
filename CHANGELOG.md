## [Unreleased]

### 🚀 Features

- *(widgets)* Add 40 new widgets across 10 categories:
  - form: Checkbox, RadioGroup, Slider, NumberStepper, Dropdown
  - HUD effects: Typewriter, Marquee, DigitalClock
  - indicators: StatusLED, CountdownTimer, CollapsiblePanel, ProgressBar, SignalBars, BatteryIndicator, Thermometer
  - data-viz: Oscilloscope, StarMap, Graph, PieChart, Speedometer, LineChart
  - ambient overlay: ScanlineOverlay, Noise
  - information: BigText, Stat, KeyValue, Timeline, Table
  - navigation: Breadcrumb, Tabs, ScrollView
  - feedback: Badge, Tooltip, Toast
  - input: MultiSelectList, TextArea, VerticalSlider, ComboBox
  - visual: Barcode, ImageView
- *(widgets)* Add PieChart `Gapped` and Graph `Diamond` shape variants
- *(examples)* Add 12 new examples — form_controls, hud_effects, indicators, data_viz, overlay, info_display, inputs, navigation, feedback, data_charts, visual, inputs2
- *(examples)* Extend the dashboard with DigitalClock (header) + a status strip (StatusLED + SignalBars + ProgressBar)

## [0.2.0] - 2026-06-15

### 🚀 Features

- *(widgets)* Add shape variants across the widget system
- *(button)* Add multi-row Pill / Framed shape variants
- *(widgets)* Add SpectrumBars / RadialGauge / HeatGrid chart widgets
- *(widgets)* Add Sparkline / DonutChart / HBarChart / ScatterPlot widgets
- *(widgets)* Add CommLog streaming comms feed
- *(widgets)* Add CandlestickChart / TreeMap / AreaChart / ActivityRings / StripChart / RadialBarChart / Compass
- Add markdown rendering, CommLog chat mode, and an AI agent console example

### 📚 Documentation

- *(examples)* Add a dedicated button shape-variant example
- *(agents)* Emphasize the no-branch-switch constraint
- *(examples)* Capture and embed the agent_console screenshot
## [0.1.2] - 2026-06-15

### 🚜 Refactor

- Flatten workspace into a single root crate

### ⚙️ Miscellaneous Tasks

- Release ratatui-sci-fi version 0.1.2
## [0.1.1] - 2026-06-15

### 🚀 Features

- 实现 sci-fi 组件库核心 — 主题 + 10 组件 + 音效引擎 + 示例
- 添加 widget_gallery 示例 — 3×3 网格展示全部 10 个组件
- *(themes)* Add 4 themes + extend component CSS to all widgets
- *(docs)* Headless screenshot generator + README demo GIFs
- *(widgets)* Add 7 basic widgets + migrate audio to rodio 0.22

### 🚜 Refactor

- *(widgets)* Drive Button + ScanList via stylesheet cascade
- *(widgets)* Migrate glitch/target/boot/gauge/biometric to cascade

### ⚙️ Miscellaneous Tasks

- Init
- Add CHANGELOG
- Update
- Release
