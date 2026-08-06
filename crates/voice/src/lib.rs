//! `voice` -- text-to-speech, speech-to-text, voice activity detection,
//! and WAV I/O (Development_plan Phase 12; Apis SS14 Voice API).
//!
//! Uses classical (non-neural) tools throughout, deliberately: `espeak-ng`
//! for TTS and `pocketsphinx` for STT both ship complete, usable models
//! via `apt` -- unlike Phase 4's LLM, nothing here needed a model
//! download from a host this sandbox can't reach. The tradeoff is
//! quality: formant-synthesized speech sounds robotic, and a classical
//! acoustic model recognizes real human speech far better than it
//! recognizes synthetic speech (this sandbox's only available test
//! audio). Every module's doc comment is specific about what's been
//! verified to actually work versus what's mechanically real but not
//! accuracy-tested.
//!
//! No live microphone or speaker exists in this sandbox (the same
//! constraint `linux_services::audio` documented in Phase 8): playback
//! and any notion of live "start listening"/"stop listening" fail
//! gracefully here and would work on a real desktop with real audio
//! hardware. File-based synthesis (`tts::EspeakTts::synthesize_to_wav`)
//! and file-based transcription (`stt::PocketsphinxStt::transcribe`)
//! don't have that limitation at all -- they're fully real and fully
//! tested here.

pub mod stt;
pub mod tts;
pub mod vad;
pub mod wav;

pub use stt::PocketsphinxStt;
pub use tts::EspeakTts;
pub use vad::VoiceActivityDetector;
pub use wav::{read_wav, write_wav, WavAudio};
