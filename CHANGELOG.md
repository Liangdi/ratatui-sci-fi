## [Unreleased]

### 🚀 Features

- *(widgets)* Add 3 sci-fi chart widgets: `SpectrumBars` (animated vertical-bar spectrum analyzer), `RadialGauge` (eased reactor-core dial), and `HeatGrid` (sensor-array heatmap) — each a stateful `StatefulWidget` with self-generated demo mode + external feed, a `…Shape` glyph/geometry variant, and CSS-cascade theming (`Spectrum` / `Dial` / `Heat` nodes)
- *(examples)* Add dedicated `charts` example showcasing all three chart widgets with `t` theme + `s` shape-variant cycling

## [0.1.2] - 2026-06-15

### 🚜 Refactor

- Flatten workspace into a single root crate
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
