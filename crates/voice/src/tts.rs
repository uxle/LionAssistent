//! Text-to-speech (Development_plan Phase 12 "Text-to-Speech"; Apis SS14
//! `speak`), via `espeak-ng`.
//!
//! `espeak-ng` is classical formant synthesis, not a neural model -- no
//! download needed (unlike Phase 4's LLM), and it's genuinely available
//! via `apt`. It sounds robotic, not natural, but it's real, fast, and
//! fully offline. [`EspeakTts::synthesize_to_wav`] is the reliable core
//! (writes real audio data, verified in this sandbox);
//! [`EspeakTts::speak`] additionally tries to play it live, which fails
//! gracefully here since this container has no sound server -- the same
//! honest split as `linux_services::audio` (Phase 8).

use common::{AppError, AppResult};
use std::path::Path;

fn err(message: impl Into<String>) -> AppError {
    AppError::Module {
        module: "voice::tts",
        message: message.into(),
    }
}

#[derive(Debug, Clone)]
pub struct EspeakTts {
    /// espeak-ng voice code, e.g. "en", "en-us", "fr".
    pub voice: String,
    pub speed_wpm: u32,
}

impl Default for EspeakTts {
    fn default() -> Self {
        Self {
            voice: "en".to_string(),
            speed_wpm: 150,
        }
    }
}

impl EspeakTts {
    pub fn new(voice: impl Into<String>, speed_wpm: u32) -> Self {
        Self {
            voice: voice.into(),
            speed_wpm,
        }
    }

    /// Synthesizes `text` to a real WAV file at `output_path`.
    pub fn synthesize_to_wav(&self, text: &str, output_path: &Path) -> AppResult<()> {
        let status = std::process::Command::new("espeak-ng")
            .args(["-v", &self.voice, "-s", &self.speed_wpm.to_string(), "-w"])
            .arg(output_path)
            .arg(text)
            .status()
            .map_err(|e| err(format!("espeak-ng is not available: {e}")))?;

        if status.success() {
            Ok(())
        } else {
            Err(err(format!("espeak-ng exited with {status}")))
        }
    }

    /// Synthesizes and immediately tries to play `text` out loud.
    /// Synthesis is real and always attempted; playback needs a sound
    /// server (`aplay`), which this container doesn't have -- fails
    /// gracefully rather than silently doing nothing, same as
    /// `linux_services::audio`.
    pub fn speak(&self, text: &str) -> AppResult<()> {
        let tmp_path = std::env::temp_dir().join(format!(
            "voice-tts-{}-{}.wav",
            std::process::id(),
            fastrand_ish()
        ));
        self.synthesize_to_wav(text, &tmp_path)?;
        let result = play_wav(&tmp_path);
        std::fs::remove_file(&tmp_path).ok();
        result
    }
}

/// A tiny, dependency-free way to get a probably-unique suffix for a
/// temp filename -- not cryptographic, just avoiding collisions between
/// concurrent calls within the same process.
fn fastrand_ish() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

/// Plays a WAV file via `aplay` (ALSA). Fails gracefully without a sound
/// server, the same honest split as `linux_services::audio`'s `pactl`
/// calls.
pub fn play_wav(path: &Path) -> AppResult<()> {
    let output = std::process::Command::new("aplay")
        .arg(path)
        .output()
        .map_err(|e| err(format!("aplay is not available: {e}")))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(err(format!(
            "aplay failed (no audio output device? this sandbox has none): {}",
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wav;

    #[test]
    fn synthesizes_real_audio_data() {
        let path = std::env::temp_dir().join("voice-tts-test-synth.wav");
        EspeakTts::default()
            .synthesize_to_wav("hello world", &path)
            .unwrap();

        let audio = wav::read_wav(&path).unwrap();
        assert!(!audio.samples.is_empty());
        assert!(audio.sample_rate > 0);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn different_text_produces_different_length_audio() {
        let short_path = std::env::temp_dir().join("voice-tts-test-short.wav");
        let long_path = std::env::temp_dir().join("voice-tts-test-long.wav");
        let tts = EspeakTts::default();
        tts.synthesize_to_wav("hi", &short_path).unwrap();
        tts.synthesize_to_wav("this is a much longer sentence to synthesize", &long_path)
            .unwrap();

        let short_audio = wav::read_wav(&short_path).unwrap();
        let long_audio = wav::read_wav(&long_path).unwrap();
        assert!(long_audio.samples.len() > short_audio.samples.len());

        std::fs::remove_file(&short_path).ok();
        std::fs::remove_file(&long_path).ok();
    }

    #[test]
    fn playback_fails_clearly_without_a_sound_server() {
        let path = std::env::temp_dir().join("voice-tts-test-playback.wav");
        EspeakTts::default()
            .synthesize_to_wav("test", &path)
            .unwrap();
        assert!(play_wav(&path).is_err());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn speak_still_reports_the_playback_error_after_synthesizing() {
        // Synthesis succeeds (real), only playback fails (no sound
        // server here) -- `speak` should surface that failure, not swallow it.
        assert!(EspeakTts::default().speak("test").is_err());
    }
}
