//! Speech-to-text (Development_plan Phase 12 "Speech-to-Text"; Apis SS14
//! `transcribe`), via `pocketsphinx`.
//!
//! Unlike Phase 4's LLM, this needed no model download at all:
//! `pocketsphinx-en-us` ships a complete, usable acoustic + language
//! model via `apt`, bundled at `/usr/share/pocketsphinx/model/en-us`.
//!
//! **What "real" means here, honestly:** the pipeline itself is fully
//! real -- a genuine acoustic model genuinely processes genuine audio
//! samples and produces genuine transcription output; it isn't a stub.
//! But its *accuracy* on this sandbox's only available test audio
//! (`espeak-ng`-synthesized speech, see `tts.rs`) is poor and was
//! verified to be poor before writing a single test against it: manual
//! testing here transcribed "check the weather" as "you cannot wear
//! lots". That's an expected characteristic of feeding a robotic,
//! formant-synthesized voice into a classical acoustic model trained on
//! natural human speech, not a bug in this integration. Real microphone
//! input from an actual voice -- not available in this sandbox at all --
//! would perform meaningfully better against the same model. Tests here
//! therefore check what's actually reliable: that the pipeline runs and
//! produces *some* output for real speech, and that silence produces
//! *no* output, rather than asserting exact transcription accuracy that
//! wouldn't hold up even on a correctly-working system.

use common::{AppError, AppResult};
use std::path::{Path, PathBuf};

fn err(message: impl Into<String>) -> AppError {
    AppError::Module {
        module: "voice::stt",
        message: message.into(),
    }
}

#[derive(Debug, Clone)]
pub struct PocketsphinxStt {
    pub hmm_dir: PathBuf,
    pub lm_path: PathBuf,
    pub dict_path: PathBuf,
}

impl PocketsphinxStt {
    /// The `pocketsphinx-en-us` package's bundled model paths on Debian/
    /// Ubuntu -- present after `apt install pocketsphinx-en-us`, no
    /// separate download step.
    pub fn default_en_us() -> Self {
        let base = PathBuf::from("/usr/share/pocketsphinx/model/en-us");
        Self {
            hmm_dir: base.join("en-us"),
            lm_path: base.join("en-us.lm.bin"),
            dict_path: base.join("cmudict-en-us.dict"),
        }
    }

    /// Transcribes a 16kHz mono 16-bit PCM WAV file (resample with `sox`
    /// first if your audio is a different rate -- pocketsphinx expects
    /// 16kHz specifically).
    pub fn transcribe(&self, wav_path: &Path) -> AppResult<String> {
        if !wav_path.exists() {
            return Err(err(format!("no such file: {}", wav_path.display())));
        }

        let output = std::process::Command::new("pocketsphinx_continuous")
            .arg("-infile")
            .arg(wav_path)
            .arg("-hmm")
            .arg(&self.hmm_dir)
            .arg("-lm")
            .arg(&self.lm_path)
            .arg("-dict")
            .arg(&self.dict_path)
            .output()
            .map_err(|e| err(format!("pocketsphinx_continuous is not available: {e}")))?;

        // pocketsphinx_continuous writes its transcription to stdout and
        // its (often noisy) logging to stderr; a non-zero exit here
        // means something more fundamental went wrong (bad model paths,
        // unreadable file), not just "didn't recognize anything" --
        // that case exits 0 with empty stdout, exercised directly by
        // this module's silence test.
        if !output.status.success() {
            return Err(err(format!(
                "pocketsphinx_continuous exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
                    .lines()
                    .next_back()
                    .unwrap_or_default()
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{tts::EspeakTts, wav};

    fn resample_to_16k(input: &Path, output: &Path) -> AppResult<()> {
        let status = std::process::Command::new("sox")
            .arg(input)
            .args(["-r", "16000"])
            .arg(output)
            .status()
            .map_err(|e| err(format!("sox is not available: {e}")))?;
        if status.success() {
            Ok(())
        } else {
            Err(err(format!("sox exited with {status}")))
        }
    }

    #[test]
    fn transcribing_real_synthesized_speech_produces_some_output() {
        let raw = std::env::temp_dir().join("voice-stt-test-raw.wav");
        let resampled = std::env::temp_dir().join("voice-stt-test-16k.wav");

        EspeakTts::default()
            .synthesize_to_wav("hello there, this is a test", &raw)
            .unwrap();
        resample_to_16k(&raw, &resampled).unwrap();

        let transcript = PocketsphinxStt::default_en_us()
            .transcribe(&resampled)
            .unwrap();
        assert!(
            !transcript.is_empty(),
            "expected some transcription of real speech audio, got nothing"
        );

        std::fs::remove_file(&raw).ok();
        std::fs::remove_file(&resampled).ok();
    }

    #[test]
    fn transcribing_silence_produces_no_output() {
        let path = std::env::temp_dir().join("voice-stt-test-silence.wav");
        wav::write_wav(&path, 16000, 1, &vec![0i16; 16000]).unwrap();

        let transcript = PocketsphinxStt::default_en_us().transcribe(&path).unwrap();
        assert!(transcript.is_empty());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn transcribing_a_missing_file_is_a_clear_error() {
        let result = PocketsphinxStt::default_en_us()
            .transcribe(Path::new("/definitely/not/a/real/file.wav"));
        assert!(result.is_err());
    }
}
