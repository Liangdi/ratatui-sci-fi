## [0.2.1] - 2026-06-24

### 🚀 Features

- *(widgets)* Add Checkbox/RadioGroup/Slider/NumberStepper/Dropdown form controls
- *(widgets)* Add Typewriter/Marquee/DigitalClock HUD effect widgets
- *(widgets)* Add StatusLED/CountdownTimer/CollapsiblePanel/ProgressBar indicators
- *(widgets)* Add Oscilloscope/StarMap/Graph data-viz widgets (Braille canvas)
- *(widgets)* Add ScanlineOverlay/Noise ambient overlay layer
- *(widgets)* Add BigText/Stat/KeyValue/Timeline/Table info-display widgets
- *(widgets)* Add SignalBars/BatteryIndicator/Thermometer indicators
- *(widgets)* Add MultiSelectList/TextArea multi-row input widgets
- *(widgets)* Add Breadcrumb/Tabs/ScrollView navigation widgets
- *(widgets)* Add Badge/Tooltip/Toast feedback widgets
- *(widgets)* Add PieChart/Speedometer/LineChart data-viz widgets
- *(widgets)* Add Barcode/ImageView visual widgets
- *(widgets)* Add VerticalSlider/ComboBox input widgets
- *(widgets)* Add PieChart Gapped + Graph Diamond shape variants
- *(examples)* Integrate DigitalClock + status strip into the dashboard
- *(examples)* Turn widget_gallery into a tabbed tour

### 🐛 Bug Fixes

- *(comm_log)* Bound history and cache chat markdown

### 🚜 Refactor

- *(widgets)* Extract capped_push and draw_centered_label

### 📚 Documentation

- Sync README to 0.2.0
- *(readme)* Refresh totals for the 15 new widgets (32 → 47)
- Consolidate the Unreleased changelog + round out the example list
- *(examples)* Clarify widget_gallery is the core-15 overview

### ⚡ Performance

- *(widgets)* Zero-alloc set_char and reuse ComputeScratch

### 🧪 Testing

- *(comm_log)* Add chat-style end-to-end integration test

### ⚙️ Miscellaneous Tasks

- Make the repo clippy-clean (fix all pre-existing lints)
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

### ⚙️ Miscellaneous Tasks

- Release ratatui-sci-fi version 0.2.0
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
