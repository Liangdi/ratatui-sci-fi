//! Audio catalog — **sound-effect definitions only** (no playback yet).
//!
//! The playback engine (likely `rodio`, behind an `audio` cargo feature) lands
//! in a later phase. For now this module fixes the catalog: a stable set of
//! sound-effect ids, their asset filenames, descriptions, and intended
//! triggers — so widget authors can already call `audio::play(Sound::UiTick)`
//! against a future `AudioSystem` without re-deciding names.
//!
//! # Asset plan
//!
//! Asset filenames are relative to `assets/audio/`. The intended path is to
//! **synthesize** these effects at runtime (square/sine/noise sources with
//! fast decay — no shipped `.wav` files, no licensing burden). If raw samples
//! are ever shipped, they must be CC0/asset-license-clean.
//!
//! Runtime synthesis is in [`synth`] (no deps, always available); rodio
//! playback is in [`system`] behind the `audio` feature.

pub mod synth;

#[cfg(feature = "audio")]
pub mod system;

#[cfg(feature = "audio")]
pub use system::AudioSystem;

/// A catalogued sound effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoundSpec {
    /// Stable id, also used as the `Sound` variant name in kebab... err, snake.
    pub id: &'static str,
    /// Asset filename under `assets/audio/`.
    pub filename: &'static str,
    /// Human description of the sonic character.
    pub description: &'static str,
    /// When it should fire (UI binding).
    pub trigger: &'static str,
    /// `true` for ambient loops; `false` for one-shots.
    pub looped: bool,
}

/// The full sound-effect catalog.
///
/// Grouped: ambient loops → UI feedback → alerts.
pub const CATALOG: &[SoundSpec] = &[
    // ── Ambient (looping) ───────────────────────────────────────────────
    SoundSpec {
        id: "ambient_hum",
        filename: "ambient_hum.wav",
        description: "低频电流/风扇持续底噪,模拟飞船控制台长期开机",
        trigger: "进入主界面时循环",
        looped: true,
    },
    SoundSpec {
        id: "radar_echo",
        filename: "radar_echo.wav",
        description: "雷达每转一圈的低沉回音「嗵——」",
        trigger: "雷达扫描线完成一周",
        looped: false,
    },
    // ── UI feedback (one-shot) ──────────────────────────────────────────
    SoundSpec {
        id: "ui_tick",
        filename: "ui_tick.wav",
        description: "光标在选项间移动的短促清脆电子音",
        trigger: "Up/Down 切换菜单",
        looped: false,
    },
    SoundSpec {
        id: "keyboard_clack",
        filename: "keyboard_clack.wav",
        description: "复古机械键盘磁性哒哒声",
        trigger: "文本输入逐字符",
        looped: false,
    },
    SoundSpec {
        id: "ui_confirm",
        filename: "ui_confirm.wav",
        description: "类似《星际迷航》电脑的确认合成音",
        trigger: "Enter / 按钮确认",
        looped: false,
    },
    // ── Alerts (one-shot) ───────────────────────────────────────────────
    SoundSpec {
        id: "alert_siren",
        filename: "alert_siren.wav",
        description: "持续低频脉冲警报",
        trigger: "Error 日志 / 警告弹窗",
        looped: false,
    },
];

/// Type-safe handle to a catalogued sound effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    AmbientHum,
    RadarEcho,
    UiTick,
    KeyboardClack,
    UiConfirm,
    AlertSiren,
}

impl Sound {
    /// The catalog spec for this sound.
    pub const fn spec(self) -> &'static SoundSpec {
        match self {
            Sound::AmbientHum => &CATALOG[0],
            Sound::RadarEcho => &CATALOG[1],
            Sound::UiTick => &CATALOG[2],
            Sound::KeyboardClack => &CATALOG[3],
            Sound::UiConfirm => &CATALOG[4],
            Sound::AlertSiren => &CATALOG[5],
        }
    }

    /// Asset filename under `assets/audio/`.
    pub fn filename(self) -> &'static str {
        self.spec().filename
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_complete_and_unique() {
        use std::collections::HashSet;
        let mut ids = HashSet::new();
        let mut files = HashSet::new();
        for s in CATALOG {
            assert!(ids.insert(s.id), "duplicate id {}", s.id);
            assert!(files.insert(s.filename), "duplicate filename {}", s.filename);
        }
        // Every Sound variant must map to a catalog entry.
        assert_eq!(Sound::AmbientHum.spec().id, "ambient_hum");
        assert_eq!(Sound::AlertSiren.spec().id, "alert_siren");
        assert!(Sound::AmbientHum.filename().ends_with(".wav"));
    }
}
