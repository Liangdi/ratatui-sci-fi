//! rodio-backed playback: an [`AudioSystem`] that turns the [`Sound`] catalog
//! into real audio, **synthesized at runtime** — no asset files, no licensing.
//!
//! Enabled by the `audio` cargo feature.
//!
//! # rodio 0.22 API
//!
//! rodio 0.22 replaced the old `OutputStream` / `Sink` / `play_raw` API with a
//! device-sink + mixer + player model. The mapping this module uses:
//!
//! - `OutputStream::try_default()` → `DeviceSinkBuilder::from_default_device()?.open_stream()`,
//!   yielding a [`MixerDeviceSink`] (kept alive to sustain audio) plus a
//!   [`Mixer`] handle sounds are submitted to.
//! - the looping ambient bed runs on a long-lived [`Player`]
//!   (`Player::connect_new(mixer)` + `append(repeat_infinite())`), replacing the
//!   old dedicated `Sink`.
//! - one-shot sounds each get a short-lived [`Player`] that is
//!   [`detach`](Player::detach)ed so it plays out after the `Player` value drops.
//!
//! # Example
//!
//! ```no_run
//! # #[cfg(feature = "audio")] {
//! use ratatui_sci_fi::audio::{AudioSystem, Sound};
//!
//! // `None` on a headless box — the app then runs silently, never crashes.
//! if let Some(mut audio) = AudioSystem::init() {
//!     audio.start_ambient();
//!     audio.play(Sound::UiConfirm);
//! }
//! # }
//! ```

use std::num::NonZero;

use rodio::{
    buffer::SamplesBuffer,
    source::Source,
    DeviceSinkBuilder, MixerDeviceSink, Player,
};

use super::{synth, Sound};
use super::synth::SAMPLE_RATE;

/// Relative volume of the looping ambient bed (it sits under the UI blips).
const AMBIENT_VOL: f32 = 0.10;

/// Mono channel count as a `NonZero<u16>` (rodio 0.22's `ChannelCount`).
fn one_channel() -> NonZero<u16> {
    NonZero::new(1).expect("1 is non-zero")
}

/// The synth's sample rate as a `NonZero<u32>` (rodio 0.22's `SampleRate`).
fn sample_rate() -> NonZero<u32> {
    NonZero::new(SAMPLE_RATE).expect("SAMPLE_RATE is non-zero")
}

/// Procedural audio engine.
///
/// Owns the cpal output stream (via the [`MixerDeviceSink`]) for its lifetime.
/// One-shot sounds overlap (the backend mixes them); the ambient bed runs on its
/// own [`Player`] so its volume and on/off can be controlled independently.
///
/// **Graceful degradation:** [`AudioSystem::init`] returns `None` when no usable
/// output device is available (headless server, missing ALSA/PulseAudio, …) —
/// the application then simply runs silently rather than crashing.
pub struct AudioSystem {
    // Kept alive: dropping the sink stops all audio.
    _sink: MixerDeviceSink,
    ambient: Option<Player>,
    volume: f32,
}

impl AudioSystem {
    /// Open the default output device. Returns `None` if audio is unavailable.
    pub fn init() -> Option<AudioSystem> {
        // rodio 0.22: build a device sink from the default device and open the
        // mixing stream. Both steps can fail on a headless box → degrade to None.
        let sink = DeviceSinkBuilder::from_default_device().ok()?.open_stream().ok()?;
        Some(AudioSystem { _sink: sink, ambient: None, volume: 1.0 })
    }

    /// Set the master volume (`0.0..=1.0`). Affects one-shots and the ambient bed.
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        if let Some(sink) = &self.ambient {
            sink.set_volume(self.volume * AMBIENT_VOL);
        }
    }

    /// Start (or keep running) the looping ambient bed.
    pub fn start_ambient(&mut self) {
        if self.ambient.is_some() {
            return;
        }
        let player = Player::connect_new(self._sink.mixer());
        let bed = SamplesBuffer::new(one_channel(), sample_rate(), synth::samples(Sound::AmbientHum));
        player.append(bed.repeat_infinite());
        player.set_volume(self.volume * AMBIENT_VOL);
        // Keep the player alive on `self` so the loop keeps playing.
        self.ambient = Some(player);
    }

    /// Stop the ambient bed if it is running.
    pub fn stop_ambient(&mut self) {
        if let Some(sink) = self.ambient.take() {
            sink.stop();
        }
    }

    /// Play a one-shot sound. Best-effort: playback errors are silently ignored.
    ///
    /// [`Sound::AmbientHum`] is a no-op here — use [`Self::start_ambient`].
    pub fn play(&self, sound: Sound) {
        if matches!(sound, Sound::AmbientHum) {
            return;
        }
        let mut data = synth::samples(sound);
        if (self.volume - 1.0).abs() > f32::EPSILON {
            for s in &mut data {
                *s *= self.volume;
            }
        }
        // Each one-shot gets its own short-lived player on the shared mixer.
        // `detach` lets it play out after this `Player` value is dropped at the
        // end of the statement.
        let player = Player::connect_new(self._sink.mixer());
        player.append(SamplesBuffer::new(one_channel(), sample_rate(), data));
        player.set_volume(self.volume);
        player.detach();
    }
}
