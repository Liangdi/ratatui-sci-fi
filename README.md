# ratatui-sci-fi

[![Rust](https://img.shields.io/badge/rust-edition%202024-orange)](https://www.rust-lang.org/)
[![ratatui](https://img.shields.io/badge/ratatui-0.30-red)](https://ratatui.rs)
[![Version](https://img.shields.io/badge/version-0.2.0-green)]()
[![License](https://img.shields.io/badge/license-MIT-blue)](#许可证)

**[English](README.en.md)** | 中文

> 为 [Ratatui](https://ratatui.rs) 生态打造的**科幻风格终端组件库**:赛博朋克霓虹、废土复古终端、《异形》工业控制台、深空 HUD —— 一套主题、一组特效组件、一套运行时合成的音效系统,帮你快速搭出沉浸感的 TUI。

---

## ✨ 特性

- **八大内置主题** —— Cyberpunk / Fallout / Weyland / DeepSpace / Bloodmoon / Nebula / Arctic / Sentinel,语义化调色板(`accent`/`bg`/`alert`/…),每个主题同时提供原生 `Color` 与基于 `ratatui-style` 的 CSS cascade 样式表。
- **70 个组件** —— 36 个基础 / 表单 / 指示 / 信息 / 导航 / 反馈组件 + 12 个高感官特效组件 + 22 个数据图表组件,全部按 ratatui 0.30 的 `Widget` / `StatefulWidget` 标准实现。
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
cargo run -p ratatui-sci-fi --example widget_gallery # 网格逐组件展示
cargo run -p ratatui-sci-fi --example charts         # 数据图表组件合集
cargo run -p ratatui-sci-fi --example button         # Button 形态变体(Pill / Framed)
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

![agent console 示例](screenshot/agent_console.gif)


**`dashboard`** —— 综合科幻 HUD:开机序列 + 雷达扫描 / 能量槽 / 生命体征 / 事件列表,`t` 切换主题。

![dashboard 示例](screenshot/dashboard.gif)

**`widget_gallery`** —— 全部组件各自独立展示(网格布局)。

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
| **Bloodmoon** | 血红 `#FF3344` / 余烬橙 `#FF8855` | 战情室、警报控制台 |
| **Nebula** | 紫罗兰 `#BB66FF` / 冰蓝 `#66EEFF` | 全息霓虹、星云 |
| **Arctic** | 青绿 `#44EEDD` / 冰白 `#AAEEFF` | 极地科考站、冷冻实验室 |
| **Sentinel** | 银白 `#E8E8EC` / 钢灰 `#9A9AA6` | 隐身、极简控制台 |

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
| `Checkbox` | 勾选框 `[✓] SHIELDS` / `[ ] SHIELDS`,与 Toggle 同构的无状态布尔控件 |
| `RadioGroup` | 单选组,选中 `◉`/未选 `○`,`handle_key` 上下循环选择 |
| `Slider` | 水平滑块 `════◉────── 42%`,归一化 0..1,左右步进,按阈值变色 |
| `NumberStepper` | 数字步进 `◂ 42 ▸`,`min/max/step` 可配,左右加减并 clamp |
| `Dropdown` | 下拉选择,折叠 `▾ BETA`,展开为 List 式浮层(app 控制 area + Clear) |
| `StatusLED` | 状态灯 `● LABEL`,按 `Level`(Ok/Warn/Alert/Normal)变色,无状态 |
| `CountdownTimer` | 倒计时 `MM:SS`,≤10s 紧急闪烁(Alert)、≤30s Warn,app 每秒减 remaining |
| `ProgressBar` | 线性进度条,`Some(ratio)` 确定填充 / `None` 不确定扫描(区别于分段 EnergyGauge) |
| `CollapsiblePanel` | 可折叠面板,折叠为单行标题 `▸`,展开为边框 + `inner(area,&state)` 内容区 |
| `KeyValue` | 键值属性列表,`label … value`(Plain / Dotted 点点引导) |
| `Stat` | 统计卡片,大数字(accent)+ 标签 + 趋势箭头(↑ok / ↓alert / →) |
| `Timeline` | 事件时间轴 `● time · event`(Plain / Connected 节点连线) |
| `Table` | sci-fi 表格,自动列宽 + accent 表头 + zebra 行(原生 Table 的主题皮肤版) |
| `BigText` | 5×7 点阵大字横幅(数字 / `:`),Glow 只亮段 / Grid 满网格 |
| `SignalBars` | 信号强度条 `▁▂▃▄▅`(Ascending 递增 / Equal 等高),前 level 根亮 |
| `BatteryIndicator` | 电池图标 `[████░░]▐` + 正极头,按 ratio 填充,<0.2 alert / <0.5 warn |
| `Thermometer` | 垂直温度计,底部球 `●` + 液柱按 ratio 上升(>0.8 alert / <0.2 ok) |
| `MultiSelectList` | 多选列表,`▸ [✓] item`,`Up/Down` 移光标,`Space` 切换勾选 |
| `TextArea` | 多行文本编辑,`Char` / `Backspace` / `Enter` / 方向键,闪烁光标(按 char 索引) |
| `Breadcrumb` | 面包屑路径 `item > item > current`,最后项 accent,分隔符可选(> / / ►) |
| `Tabs` | 标签页,selected accent 加粗 + 形态(Underline 下划线 / Bracket 括号 / Arrow 箭头) |
| `ScrollView` | 垂直滚动视图 + 滚动条,`Up/Down/PageUp/Down/Home/End` |
| `Badge` | 状态徽章,`Filled`(level 作背景色)/ `Outlined`(`[ text ]` level 色) |
| `Tooltip` | 悬停提示 `[ text ]`,`Pointer` 带底部 `▼` 指针 |
| `Toast` | 自动消失通知,`show`/`tick` 倒计时,居中浮层 + level 色边框 |

### 特效组件
| 组件 | 说明 |
| :--- | :--- |
| `MatrixRain` | 黑客帝国数字雨,可配速度/密度,适合作大背景 |
| `GlitchText` | 随机短时字符替换,信号干扰 / 解码失败质感 |
| `BootSequence` | 开机逐行跑码 + 偶发屏幕闪烁 |
| `BiometricChart` | 多轨迹快速波动折线图(心率 / 能量 / 辐射) |
| `SciFiRadar` | Braille 圆形扫描 + 渐变衰减尾迹 + 可选 blips |
| `Typewriter` | 逐字打字机,tick 驱动逐字 reveal + 闪烁光标(开机叙事 / AI 对白) |
| `Marquee` | 横向滚动跑马灯(告警 ticker),可配速度 / 方向,自动循环 |
| `DigitalClock` | 七段数码管时钟 `HH:MM:SS`(`█`/`░` 段位 + 闪烁冒号),空间不足降级为纯文本 |
| `ScanlineOverlay` | 全屏 CRT overlay:移动 accent 扫描线 + 可选暗角(叠在所有控件之上) |
| `Noise` | 全屏雪花噪点 overlay:`Snow` 每帧变 / `Static` 稳定,`intensity` 控制密度 |
| `Barcode` | 1D 条码,每字符 8-bit → 条(`█`×2)/ 空(×1)序列,可选 caption |
| `ImageView` | 多行 ASCII art 居中渲染(accent / muted / fg) |

### 数据图表组件(0.2.0 新增)
| 组件 | 说明 |
| :--- | :--- |
| `CommLog` | 对话流 / 聊天 feed,流式逐字出现 + 滚动条 + 可选 Markdown 卡片(Chat 样式) |
| `Markdown` | CommonMark 渲染(pulldown-cmark):标题 / 粗斜体 / 行内代码 / 代码块 / 列表 / 引用 |
| `ActivityRings` | 多目标同心进度环(Apple Watch 风格) |
| `AreaChart` | 单趋势曲线下的填充面积图 |
| `CandlestickChart` | 动画 OHLC 金融蜡烛图 |
| `Compass` | 航向 / 方位指示器 |
| `DonutChart` | 多切片比例环 |
| `HeatGrid` | 动画 2D 传感器阵列热力图 |
| `HBarChart` | 水平分类对比条形图 |
| `RadialBarChart` | 从中心向外辐射的极坐标条 |
| `RadialGauge` | 圆形反应堆核心表盘仪表 |
| `ScatterPlot` | 笛卡尔 X/Y 散点云 |
| `Sparkline` | 紧凑单值趋势迷你线 |
| `SpectrumBars` | 动画竖向频谱 / 能量分布条 |
| `StripChart` | 多通道滚动示波器(医疗监护仪风格) |
| `TreeMap` | 层级 / 扁平比例矩形图 |
| `Oscilloscope` | Braille 画布滚动波形(正弦 / 方波 / 锯齿 / 三角) |
| `StarMap` | 闪烁的确定性星图 |
| `Graph` | 节点 + 边拓扑图(Bresenham 边,Braille 画布) |
| `PieChart` | 实心饼图,切片按比例 + 极角归属,色循环 accent/accent2/ok/warn/alert |
| `Speedometer` | 半圆指针仪表,180° 弧 + 指针按 value 角度(Braille) |
| `LineChart` | 带坐标轴(`│─└`)的折线图,Braille 连线(单 series) |

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
│   ├── widgets/                 # 70 个组件(基础 / 表单 / 指示 / 信息 / 导航 / 反馈 / 特效 / 数据图表)
│   └── audio/                   # 目录(Sound/CATALOG)+ synth + AudioSystem
└── examples/
    ├── dashboard.rs             # 综合科幻仪表盘(全组件 + 音效)
    ├── widget_gallery.rs        # 全组件网格总览
    ├── form_controls.rs         # 表单控件(交互演示)
    ├── hud_effects.rs           # HUD 效果(打字机 / 跑马灯 / 数码时钟)
    ├── indicators.rs            # 指示器 / 容器
    ├── data_viz.rs              # 数据可视化(示波器 / 星图 / 拓扑)
    └── …                        # 其他:agent_console / matrix_rain / button / charts / capture_screenshots
```

- **双路径主题**:直接用 `palette()` 取 `Color`(适于 Canvas 直绘),或用 `stylesheet()` 走 CSS cascade(适于声明式样式)。同源 RGB,不漂移。
- **后端无关渲染**:库通过 ratatui 离屏 `Buffer` 渲染,不做终端 I/O。依赖 `ratatui` + `ratatui-style`,外加 `crossterm`(仅为 `TextInputState::handle_key` 提供按键事件类型;示例用其做终端 I/O)。

---

## 🗺️ 路线图

- [x] 八大主题 + 70 个组件(基础 / 表单 / 指示 / 信息 / 导航 / 反馈 / 特效 / 数据图表)
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
