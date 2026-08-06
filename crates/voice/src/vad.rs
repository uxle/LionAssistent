//! Voice activity detection (Development_plan Phase 12 "Wake Word",
//! scoped honestly -- see below).
//!
//! This is energy-based VAD: "is there speech-level sound in this
//! audio", a classical, reliable DSP technique with no model to
//! download. It is deliberately **not** true wake-word phrase
//! recognition ("did someone say *this specific phrase*") -- that needs
//! either a trained keyword-spotting model or at least a phoneme
//! matcher, and evaluating one honestly would need the same kind of
//! model file this sandbox can't download (Phase 4's `llm-runtime` has
//! the fuller account of that constraint). VAD is a real, useful,
//! fully-reliable building block for "notice that someone started
//! talking" -- genuinely part of a wake-word pipeline, just not the
//! whole thing.

#[derive(Debug, Clone, Copy)]
pub struct VoiceActivityDetector {
    /// RMS amplitude (0..=32767 for 16-bit audio) above which a frame is
    /// considered speech. Higher = less sensitive to background noise,
    /// but more likely to miss quiet speech.
    pub energy_threshold: f64,
}

impl Default for VoiceActivityDetector {
    fn default() -> Self {
        // A conservative default: well above typical room-tone/hiss
        // noise floors, well below normal speaking volume, for 16-bit
        // PCM samples.
        Self {
            energy_threshold: 500.0,
        }
    }
}

impl VoiceActivityDetector {
    pub fn new(energy_threshold: f64) -> Self {
        Self { energy_threshold }
    }

    fn rms(samples: &[i16]) -> f64 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum_sq: f64 = samples.iter().map(|&s| (s as f64) * (s as f64)).sum();
        (sum_sq / samples.len() as f64).sqrt()
    }

    /// Whether `samples` contains speech-level energy.
    pub fn is_speech(&self, samples: &[i16]) -> bool {
        Self::rms(samples) >= self.energy_threshold
    }

    /// Splits `samples` into fixed-size frames and reports which ones
    /// contain speech-level energy -- the building block for a real-time
    /// "listen until speech starts, then stops" loop over a live audio
    /// stream.
    pub fn detect_frames(&self, samples: &[i16], frame_size: usize) -> Vec<bool> {
        if frame_size == 0 {
            return Vec::new();
        }
        samples
            .chunks(frame_size)
            .map(|frame| self.is_speech(frame))
            .collect()
    }

    /// The sample index where speech-level energy first appears, if any
    /// (useful for trimming leading silence before running STT on a
    /// captured clip).
    pub fn first_speech_frame(&self, samples: &[i16], frame_size: usize) -> Option<usize> {
        self.detect_frames(samples, frame_size)
            .iter()
            .position(|&is_speech| is_speech)
            .map(|i| i * frame_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn silence(n: usize) -> Vec<i16> {
        vec![0; n]
    }

    fn tone(n: usize, amplitude: i16) -> Vec<i16> {
        (0..n)
            .map(|i| {
                let phase = (i as f64) * 0.3;
                (phase.sin() * amplitude as f64) as i16
            })
            .collect()
    }

    #[test]
    fn silence_is_not_speech() {
        let vad = VoiceActivityDetector::default();
        assert!(!vad.is_speech(&silence(1000)));
    }

    #[test]
    fn a_loud_tone_is_speech() {
        let vad = VoiceActivityDetector::default();
        assert!(vad.is_speech(&tone(1000, 20000)));
    }

    #[test]
    fn a_quiet_tone_is_not_speech() {
        let vad = VoiceActivityDetector::default();
        assert!(!vad.is_speech(&tone(1000, 50)));
    }

    #[test]
    fn empty_input_is_not_speech() {
        assert!(!VoiceActivityDetector::default().is_speech(&[]));
    }

    #[test]
    fn detect_frames_finds_the_loud_segment() {
        let vad = VoiceActivityDetector::default();
        let mut samples = silence(500);
        samples.extend(tone(500, 20000));
        samples.extend(silence(500));

        let frames = vad.detect_frames(&samples, 100);
        assert!(frames[0..5].iter().all(|&s| !s)); // first 500 samples = silent
        assert!(frames[5..10].iter().any(|&s| s)); // middle 500 samples = loud
    }

    #[test]
    fn first_speech_frame_locates_the_onset() {
        let vad = VoiceActivityDetector::default();
        let mut samples = silence(300);
        samples.extend(tone(200, 20000));

        let onset = vad.first_speech_frame(&samples, 100).unwrap();
        assert_eq!(onset, 300); // speech starts exactly at sample 300, frame-aligned to 100
    }

    #[test]
    fn first_speech_frame_is_none_for_pure_silence() {
        let vad = VoiceActivityDetector::default();
        assert!(vad.first_speech_frame(&silence(1000), 100).is_none());
    }

    #[test]
    fn threshold_is_configurable() {
        let permissive = VoiceActivityDetector::new(10.0);
        let strict = VoiceActivityDetector::new(30000.0);
        let quiet = tone(1000, 100);
        assert!(permissive.is_speech(&quiet));
        assert!(!strict.is_speech(&quiet));
    }
}
