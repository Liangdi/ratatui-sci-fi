//! Procedural (asset-free) synthesis for the catalogued sound effects.
//!
//! Every function returns a **mono** `Vec<f32>` at [`SAMPLE_RATE`]. This module
//! has **no dependencies**, so the sound design is unit-testable and reusable
//! without the `audio` feature. Playback (rodio) lives in [`super::system`].
//!
//! Design: each effect is a short waveform shaped by an exponential-decay
//! envelope — the classic analog-UI sound palette (blips, clacks, sweeps,
//! sirens, hums). The ambient bed is built from integer-cycle sines so it
//! loops seamlessly when the player wraps it with `repeat_infinite`.

use std::f32::consts::PI;

use super::Sound;

/// Synthesis sample rate (Hz). Matches the rate handed to rodio at playback.
pub const SAMPLE_RATE: u32 = 44_100;

/// Number of samples spanning `seconds`.
fn dur(seconds: f32) -> usize {
    (seconds * SAMPLE_RATE as f32).round() as usize
}

/// Exponential-decay envelope: 1.0 at t=0, falling with time-constant `tau`
/// seconds, with a short linear `attack` (seconds) to avoid an opening click.
fn decay_env(t: f32, tau: f32, attack: f32) -> f32 {
    let a = (t / attack).min(1.0);
    a * (-t / tau).exp()
}

/// A tiny deterministic LCG for noise — seeded so synthesis is reproducible
/// (matters for tests and seamless loops).
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    /// Uniform noise in roughly `[-1, 1]`, using the LCG's high bits.
    fn next_f32(&mut self) -> f32 {
        // Numerical Recipes LCG constants (hex for clean 4-digit grouping).
        self.0 = self
            .0
            .wrapping_mul(0x5851_F42D_4C95_7F2D)
            .wrapping_add(0x1405_7B7E_F767_814F);
        (((self.0 >> 33) as u32) as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// Render `sound`'s sample buffer.
pub fn samples(sound: Sound) -> Vec<f32> {
    match sound {
        Sound::UiTick => ui_tick(),
        Sound::UiConfirm => ui_confirm(),
        Sound::KeyboardClack => keyboard_clack(),
        Sound::RadarEcho => radar_echo(),
        Sound::AmbientHum => ambient_hum(),
        Sound::AlertSiren => alert_siren(),
    }
}

/// Short high blip — "cursor moved".
pub fn ui_tick() -> Vec<f32> {
    const DUR_S: f32 = 0.03;
    const FREQ: f32 = 1_200.0;
    const VOL: f32 = 0.35;
    let dt = 1.0 / SAMPLE_RATE as f32;
    (0..dur(DUR_S))
        .map(|i| {
            let t = i as f32 * dt;
            (2.0 * PI * FREQ * t).sin() * decay_env(t, 0.010, 0.002) * VOL
        })
        .collect()
}

/// Rising chirp — "confirmed". Phase is accumulated from the instantaneous
/// (linearly swept) frequency for a clean glide with no discontinuities.
pub fn ui_confirm() -> Vec<f32> {
    const DUR_S: f32 = 0.15;
    const F0: f32 = 520.0;
    const F1: f32 = 1_500.0;
    const VOL: f32 = 0.35;
    let n = dur(DUR_S);
    let dt = 1.0 / SAMPLE_RATE as f32;
    let mut out = Vec::with_capacity(n);
    let mut phase = 0.0f32;
    for i in 0..n {
        let frac = i as f32 / n as f32;
        let f = F0 + (F1 - F0) * frac;
        phase += 2.0 * PI * f * dt;
        let t = i as f32 * dt;
        out.push(phase.sin() * decay_env(t, 0.080, 0.004) * VOL);
    }
    out
}

/// Mechanical key clack — a noise burst with a low-frequency body, very sharp decay.
pub fn keyboard_clack() -> Vec<f32> {
    const DUR_S: f32 = 0.025;
    const VOL: f32 = 0.45;
    let n = dur(DUR_S);
    let dt = 1.0 / SAMPLE_RATE as f32;
    let mut rng = Lcg::new(0xC1AC_C1AC);
    (0..n)
        .map(|i| {
            let t = i as f32 * dt;
            let noise = rng.next_f32();
            let body = (2.0 * PI * 180.0 * t).sin() * 0.3;
            ((noise + body) * decay_env(t, 0.004, 0.001) * VOL).clamp(-1.0, 1.0)
        })
        .collect()
}

/// Low reverberant "嗵" — the radar sweep echo.
pub fn radar_echo() -> Vec<f32> {
    const DUR_S: f32 = 0.28;
    const FREQ: f32 = 110.0;
    const VOL: f32 = 0.5;
    let dt = 1.0 / SAMPLE_RATE as f32;
    (0..dur(DUR_S))
        .map(|i| {
            let t = i as f32 * dt;
            (2.0 * PI * FREQ * t).sin() * decay_env(t, 0.12, 0.005) * VOL
        })
        .collect()
}

/// Looping ambient bed — low-frequency hum. Built from 60/120/90 Hz sines over
/// exactly 2.0 s, i.e. 120/240/180 integer cycles, so it wraps seamlessly.
pub fn ambient_hum() -> Vec<f32> {
    const DUR_S: f32 = 2.0;
    const VOL: f32 = 0.12;
    let dt = 1.0 / SAMPLE_RATE as f32;
    (0..dur(DUR_S))
        .map(|i| {
            let t = i as f32 * dt;
            let s = (2.0 * PI * 60.0 * t).sin() * 0.6
                + (2.0 * PI * 120.0 * t).sin() * 0.25
                + (2.0 * PI * 90.0 * t).sin() * 0.15;
            s * VOL
        })
        .collect()
}

/// Two-tone siren — alternating square waves (the alert).
pub fn alert_siren() -> Vec<f32> {
    const DUR_S: f32 = 1.0;
    const F_LO: f32 = 700.0;
    const F_HI: f32 = 950.0;
    const VOL: f32 = 0.35;
    const SWITCH: f32 = 0.25; // seconds per tone
    let dt = 1.0 / SAMPLE_RATE as f32;
    (0..dur(DUR_S))
        .map(|i| {
            let t = i as f32 * dt;
            let f = if ((t / SWITCH) as u64).is_multiple_of(2) { F_LO } else { F_HI };
            let square = (2.0 * PI * f * t).sin().signum();
            // Quick attack, then hold — the siren stays loud across its window.
            let env = (t / 0.01).min(1.0);
            square * env * VOL
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Sound; 6] = [
        Sound::UiTick,
        Sound::UiConfirm,
        Sound::KeyboardClack,
        Sound::RadarEcho,
        Sound::AmbientHum,
        Sound::AlertSiren,
    ];

    #[test]
    fn all_sounds_nonempty_finite_and_bounded() {
        for s in ALL {
            let buf = samples(s);
            assert!(!buf.is_empty(), "{s:?}: empty buffer");
            assert!(
                buf.iter().all(|&v| v.is_finite() && (-1.0..=1.0).contains(&v)),
                "{s:?}: sample out of [-1,1]",
            );
            assert!(buf.iter().any(|&v| v.abs() > 1e-4), "{s:?}: effectively silent");
        }
    }

    #[test]
    fn durations_match() {
        assert_eq!(ui_tick().len(), dur(0.03));
        assert_eq!(ui_confirm().len(), dur(0.15));
        assert_eq!(keyboard_clack().len(), dur(0.025));
        assert_eq!(radar_echo().len(), dur(0.28));
        assert_eq!(ambient_hum().len(), dur(2.0));
        assert_eq!(alert_siren().len(), dur(1.0));
    }

    #[test]
    fn synthesis_is_deterministic() {
        assert_eq!(ui_tick(), ui_tick());
        // The clack uses a seeded LCG, so it must reproduce exactly.
        assert_eq!(keyboard_clack(), keyboard_clack());
    }

    #[test]
    fn ambient_is_periodic_at_loop_boundary() {
        // Integer-cycle sines ⇒ the sample *after* the loop equals sample[0].
        let a = ambient_hum();
        let dt = 1.0 / SAMPLE_RATE as f32;
        let t_next = a.len() as f32 * dt;
        let expected = ((2.0 * PI * 60.0 * t_next).sin() * 0.6
            + (2.0 * PI * 120.0 * t_next).sin() * 0.25
            + (2.0 * PI * 90.0 * t_next).sin() * 0.15)
            * 0.12;
        assert!((a[0] - expected).abs() < 1e-5, "ambient not seamless at loop point");
    }
}
