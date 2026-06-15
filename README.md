# ratatui-sci-fi

[![Rust](https://img.shields.io/badge/rust-edition%202024-orange)](https://www.rust-lang.org/)
[![ratatui](https://img.shields.io/badge/ratatui-0.30-red)](https://ratatui.rs)
[![Version](https://img.shields.io/badge/version-0.1.0-green)]()
[![License](https://img.shields.io/badge/license-MIT-blue)](#许可证)

**[English](README.en.md)** | 中文

> 为 [Ratatui](https://ratatui.rs) 生态打造的**科幻风格终端组件库**:赛博朋克霓虹、废土复古终端、《异形》工业控制台、深空 HUD —— 一套主题、一组特效组件、一套运行时合成的音效系统,帮你快速搭出沉浸感的 TUI。

---

## ✨ 特性

- **八大内置主题** —— Cyberpunk / Fallout / Weyland / DeepSpace / Bloodmoon / Nebula / Arctic / Sentinel,语义化调色板(`accent`/`bg`/`alert`/…),每个主题同时提供原生 `Color` 与基于 `ratatui-style` 的 CSS cascade 样式表。
- **17 个组件** —— 12 个风格统一的基础组件 + 5 个高感官的特效组件,全部按 ratatui 0.30 的 `Widget` / `StatefulWidget` 标准实现。
- **运行时合成音效** —— 零音频资产、零版权负担,6 个音效由纯 Rust 波形合成;`rodio` 后端,无设备时静默降级。
- **Markdown 对话流** —— `CommLog` 的 chat 样式把每条消息渲染成**带框卡片**(user/agent 靠右/靠左区分),正文走 [pulldown-cmark](https://crates.io/crates/pulldown-cmark) 的 CommonMark 渲染(标题 / 粗斜体 / `行内代码` / 代码块 / 列表 / 引用 / 分隔线),逐字流式出现 + 可滚动 + 滚动条,默认开启的 `markdown` feature。
- **后端无关渲染** —— 库通过 ratatui 的离屏 `Buffer` 渲染,不做任何终端 I/O;`crossterm` 作为正式依赖仅为 `TextInputState::handle_key` 提供按键事件类型(下游用 termion/termwiz 时可改用自己的事件循环)。
- **可测试** —— 所有组件都带离屏 `Buffer` 渲染单测,无需真实终端。

---

## 🖼️ 预览

运行自带示例(无需额外配置):

```sh
cargo run -p ratatui-sci-fi --example agent_console  # AI Agent 控制台(开机→登录→对话)
cargo run -p ratatui-sci-fi --example dashboard      # 综合仪表盘(全组件)
cargo run -p ratatui-sci-fi --example widget_gallery # 3×3 网格逐组件展示
cargo run -p ratatui-sci-fi --example matrix_rain    # 全屏数字雨
```

**`agent_console`** —— AI + 科幻集成示例:数字雨开机动效 → 操作员登录(代号 + 掩码口令 + 生物识别点缀 + 认证动画)→ Agent 控制台(左侧 Agent 花名册、中央 `CommLog` 对话流——**带框卡片 + Markdown 渲染**,Agent 回复逐字流式出现、右侧生命体征/负载/防御状态栏)。按 `h` 进入全屏可滚动的**对话历史**(LLM 模式 + 滚动条 + Markdown)。`↑↓` 选 Agent/滚动、`Enter` 发送、`a` 警报、`t` 切主题。

> `agent_console` 控制台场景版面示意:

```text
┌──────────────────────────────────────────────────────────────────┐
│ ▶  AEGIS // AI AGENT CONSOLE  ◀           OP LIANGDI  ● ONLINE     │
├──────────────┬───────────────────────────────────────┬────────────┤
│ ╔═AGENTS══╗ │ ─── NEXUS-7 // TACTICAL COORD ───      │ AGENT VITALS│
│ ║▶● NEXUS-7║ │ NEXUS-7 ▸ Vectors locked. Standing by█│ ╱╲╱╲___╱╲   │
│ ║ TACTICAL  ║ │ OPERATOR ▸ status report              │ SYSTEM LOAD │
│ ║ ◆ ORACLE  ║ │ NEXUS-7 ▸ Threat board is green…      │ CPU ▰▰▰▰▱   │
│ ║ ● ATLAS   ║ │                                       │ SECTOR SCAN │
│ ║ ▲ NAV     ║ │           ┌─ TRANSMIT ─┐              │    ◎ 扫描    │
│ ║ ■ VEX     ║ │           │ type…      │              │ SHIELDS ◉   │
│ ╚══════════╝ │           └────────────┘              │ CLOAK   ◇   │
└──────────────┴───────────────────────────────────────┴────────────┘
```


**`dashboard`** —— 综合科幻 HUD:开机序列 + 雷达扫描 / 能量槽 / 生命体征 / 事件列表,`t` 切换主题。

![dashboard 示例](screenshot/dashboard.gif)

**`widget_gallery`** —— 16 个组件各自独立展示(5×3 网格)。

![widget gallery 示例](screenshot/widget_gallery.gif)

**`matrix_rain`** —— 全屏数字雨背景。

![matrix rain 示例](screenshot/matrix_rain.gif)

> `dashboard` 的版面结构示意(上方为彩色动态实拍,下方为静态结构图):

```text
┌──────────────────────────────────────────────────────────────────┐
│ ▶ SCI-FI HUD // ratatui-sci-fi ◀                                  │
├──────────────┬───────────────────────┬────────────────────────────┤
│ ┏━TELEMETRY━┓ │      ◎ SCANNER        │  BIOMETRICS                │
│ ┃ CORE ▰▰▰▰▱│ │       . . ✛ .          │  ╱╲╱╲___╱╲╱╲              │
│ ┃ PWR  ▰▰▰▱▱│ │     .  ●     .         │                            │
│ ┃ HULL ▰▰▱▱▱│ │       . . . .          ├────────────────────────────┤
│ ┃ SHLD ▰▱▱▱▱│ │                       │ █ DOCK SEQUENCE OK         │
│ ┗━━━━━━━━━━┛ │                       │   RADAR SWEEP DONE         │
├──────────────┴───────────────────────┴────────────────────────────┤
│ [↑↓] list   [t] theme   [a] alert   [q] exit                       │
└──────────────────────────────────────────────────────────────────┘
```

---

## 📦 安装

```sh
cargo add ratatui-sci-fi
```

需要音效时,启用 `audio` feature(会引入 `rodio` + `cpal`,Linux 上需 ALSA/PulseAudio 开发库):

```sh
cargo add ratatui-sci-fi --features audio
```

`audio` 默认关闭 —— 只想要视觉的下游不会被迫引入音频原生依赖。

`markdown` 默认**开启**(引入 `pulldown-cmark`,驱动 `CommLog` 的 Markdown 对话卡片与 `Markdown` 组件)。不需要 Markdown 渲染时可关闭以精简依赖:

```sh
cargo add ratatui-sci-fi --no-default-features   # 只保留 Plain 文本流,不带 Markdown 解析器
```

---

## 🚀 快速开始

一个最小可运行程序:全屏显示一个深空主题的科幻雷达。

```rust
use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{Frame, Terminal, backend::CrosstermBackend};
use ratatui_sci_fi::{SciFiRadar, SciFiRadarState, Theme};

type Term = Terminal<CrosstermBackend<Stdout>>;

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    let mut state = SciFiRadarState::default();
    loop {
        terminal.draw(|f| ui(f, &mut state))?;
        state.tick(); // 每帧推进动画

        if event::poll(Duration::from_millis(60))?
            && let Event::Key(k) = event::read()?
            && matches!(k.code, KeyCode::Char('q') | KeyCode::Esc)
        {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

fn ui(f: &mut Frame, state: &mut SciFiRadarState) {
    f.render_stateful_widget(
        SciFiRadar::new().theme(Theme::DeepSpace),
        f.area(),
        state,
    );
}
```

---

## 🎨 主题

| 主题 | 核心色调 | 视觉意向 |
| :--- | :--- | :--- |
| **Cyberpunk**(默认) | 荧光粉 `#FF007F` / 霓虹蓝 `#00F0FF` | 赛博朋克、夜之城、霓虹 |
| **Fallout** | 荧光绿 `#33FF33` / 纯黑 | 废土、复古大型机、哔哔小子 |
| **Weyland** | 琥珀金 `#FFB000` / 暗红 | 《异形》工业感太空舱监视器 |
| **Deep Space** | 深邃蓝 `#0055FF` / 警报红 | 现代星际战舰、极简飞行 HUD |

获取主题:`Theme::Cyberpunk.palette()` 返回原生 `Color`;`Theme::Cyberpunk.stylesheet()` 返回基于 ratatui-style 的 `&'static Stylesheet`(CSS cascade,支持 `var(--token)`、类选择器)。两者派生自同一组 RGB,永不漂移。

> 主题色大多为 24-bit 真彩;在 8 色 / 不支持 `COLORTERM=truecolor` 的终端上会掉色(不报错)。

---

## 🧱 组件

### 基础组件
| 组件 | 说明 |
| :--- | :--- |
| `Button` | 未选中 `[ 确认 ]`,选中 `▶ 确认 ◀`(高亮反白 + 能量括号) |
| `EnergyGauge` | 反应堆能量槽,`▰▰▰▰▱▱▱▱` 分段,按阈值变色(ok/warn/alert) |
| `ScanList` | 扫描线分隔的列表,选中行高亮 + 闪烁光标(`█`) |
| `AlertPopup` | 双线警报红边框弹窗,弹出时短暂闪烁 |
| `TargetLock` | 四角断开括号 + 中心十字准星的 HUD 容器,带 `inner(area)` |
| `Panel` | 双线带标题的科幻容器窗框,CSS cascade 驱动,带 `inner(area)` |
| `Value` | 标签 + 带状态级别的读数(`.state(Level::Ok/Warn/Alert)` 变色) |
| `Divider` | 满宽分隔规则线,可选居中标签 `──── SEC ────` |
| `Spinner` | Braille 活动指示器 `⠋⠙⠹…`,每 tick 推进一格 |
| `Toggle` | 布尔开关 `[◉ SHIELDS · ENGAGED ]` / `[ ○ SHIELDS · STANDBY ]` |
| `TextInput` | 单行输入框,闪烁光标 + `handle_key(KeyEvent)` + 占位符,光标按 char 索引 |

### 特效组件
| 组件 | 说明 |
| :--- | :--- |
| `MatrixRain` | 黑客帝国数字雨,可配速度/密度,适合作大背景 |
| `GlitchText` | 随机短时字符替换,信号干扰 / 解码失败质感 |
| `BootSequence` | 开机逐行跑码 + 偶发屏幕闪烁 |
| `BiometricChart` | 多轨迹快速波动折线图(心率 / 能量 / 辐射) |
| `SciFiRadar` | Braille 圆形扫描 + 渐变衰减尾迹 + 可选 blips |

**组件约定**:无状态组件实现 `Widget`(`render(self, area, buf)`);有状态组件实现 `StatefulWidget`(`render(self, area, buf, &mut State)`)。动画状态在 `…State` 里,事件循环每帧调 `state.tick()`。每个组件都有 `.theme(Theme)` 构造器。

---

## 🔊 音效系统

音效由 [synth](src/audio/synth.rs) 模块**纯 Rust 合成**(无音频文件、无版权风险),播放由 `audio` feature 下的 [`AudioSystem`](src/audio/system.rs) 负责。

**目录**(`Sound` 枚举,始终可用、零依赖):

| 音效 | 文件名 | 说明 | 触发 |
| :--- | :--- | :--- | :--- |
| `AmbientHum` | `ambient_hum.wav` | 低频电流/风扇底噪 | 进入主界面循环 |
| `RadarEcho` | `radar_echo.wav` | 雷达每圈低沉"嗵——" | 雷达扫完一周 |
| `UiTick` | `ui_tick.wav` | 短促清脆电子音 | 光标在选项间移动 |
| `KeyboardClack` | `keyboard_clack.wav` | 复古键盘哒哒声 | 文本输入 |
| `UiConfirm` | `ui_confirm.wav` | 确认合成音 | 按钮确认 |
| `AlertSiren` | `alert_siren.wav` | 持续低频脉冲警报 | Error / 警告弹窗 |

> 文件名仅用于未来可能的资产路径;当前效果全部运行时合成。

**用法**(需 `audio` feature):

```rust
use ratatui_sci_fi::audio::{AudioSystem, Sound};

// 无音频设备时返回 None,程序照常运行(静默降级,绝不 panic)
if let Some(mut audio) = AudioSystem::init() {
    audio.start_ambient();        // 启动循环底噪
    audio.play(Sound::UiConfirm); // 触发一次确认音
    audio.set_volume(0.8);        // 0.0..=1.0
}
```

**事件 → 音效的推荐架构**:widget 不持有回调,由 app 层在事件循环里触发(参见 [dashboard 示例](examples/dashboard.rs):ScanList 移动 → `UiTick`,AlertPopup 弹出 → `AlertSiren`,雷达转一圈 → `RadarEcho`)。

---

## 🏗️ 架构

```text
ratatui-sci-fi/                  # 单 crate(库)
├── Cargo.toml                   # package + 依赖;`audio` feature 在此
├── src/
│   ├── lib.rs                   # 约定 + `pub use widgets::*` 根级再导出
│   ├── themes/                  # Palette / Theme / ratatui-style Stylesheet
│   ├── widgets/                 # 17 个组件(含 CommLog 对话流)
│   └── audio/                   # 目录(Sound/CATALOG)+ synth + AudioSystem
└── examples/
    ├── agent_console.rs         # AI Agent 控制台(开机→登录→对话流)
    ├── dashboard.rs             # 综合科幻仪表盘(全组件 + 音效)
    └── matrix_rain.rs           # 数字雨独立演示
```

- **双路径主题**:直接用 `palette()` 取 `Color`(适于 Canvas 直绘),或用 `stylesheet()` 走 CSS cascade(适于声明式样式)。同源 RGB,不漂移。
- **后端无关渲染**:库通过 ratatui 离屏 `Buffer` 渲染,不做终端 I/O。依赖 `ratatui` + `ratatui-style`,外加 `crossterm`(仅为 `TextInputState::handle_key` 提供按键事件类型;示例用其做终端 I/O)。

---

## 🗺️ 路线图

- [x] 四大主题 + 10 个组件
- [x] 运行时合成音效引擎(`audio` feature)
- [ ] 更多音色参数化(频率/时长可调)
- [x] 命名捕获的 demo 动图 / 截图(`screenshot/` + `capture_screenshots` 无头渲染示例,需 ffmpeg)
- [ ] 更多主题变体

---

## 🤝 贡献

欢迎 issue 与 PR。开发遵循 [AGENTS.md](AGENTS.md) 的约束(Rust 架构师视角、围绕本 crate 主题、不切换分支)。

---

## 📄 许可证

MIT。
