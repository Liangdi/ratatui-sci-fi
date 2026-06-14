//! rodio-backed playback: an [`AudioSystem`] that turns the [`Sound`] catalog
//! into real audio, **synthesized at runtime** — no asset files, no licensing.
//!
//! Enabled by the `audio` cargo feature.
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

use rodio::{buffer::SamplesBuffer, OutputStream, OutputStreamHandle, Sink, Source};

use super::{synth, Sound};
use super::synth::SAMPLE_RATE;

/// Relative volume of the looping ambient bed (it sits under the UI blips).
const AMBIENT_VOL: f32 = 0.10;

/// Procedural audio engine.
///
/// Owns the cpal output stream for its lifetime. One-shot sounds overlap (the
/// backend mixes them); the ambient bed runs on its own [`Sink`] so its volume
/// and on/off can be controlled independently.
///
/// **Graceful degradation:** [`AudioSystem::init`] returns `None` when no usable
/// output device is available (headless server, missing ALSA/PulseAudio, …) —
/// the application then simply runs silently rather than crashing.
pub struct AudioSystem {
    // Kept alive: dropping the stream stops all audio.
    _stream: OutputStream,
    handle: OutputStreamHandle,
    ambient: Option<Sink>,
    volume: f32,
}

impl AudioSystem {
    /// Open the default output device. Returns `None` if audio is unavailable.
    pub fn init() -> Option<AudioSystem> {
        let (_stream, handle) = OutputStream::try_default().ok()?;
        Some(AudioSystem { _stream, handle, ambient: None, volume: 1.0 })
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
        let Ok(sink) = Sink::try_new(&self.handle) else { return; };
        let bed = SamplesBuffer::<f32>::new(1, SAMPLE_RATE, synth::samples(Sound::AmbientHum));
        sink.append(bed.repeat_infinite());
        sink.set_volume(self.volume * AMBIENT_VOL);
        self.ambient = Some(sink);
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
        let src = SamplesBuffer::<f32>::new(1, SAMPLE_RATE, data).convert_samples();
        let _ = self.handle.play_raw(src);
    }
}
