//! Reading and writing PCM WAV files (Development_plan Phase 12 "Audio
//! Processing"). Pure Rust, no dependencies -- the format is simple
//! enough for uncompressed 16-bit PCM (what every tool in this crate
//! actually produces and consumes) that a small hand-written parser is
//! more honest about what it supports than a general-purpose audio crate
//! would be.

use common::{AppError, AppResult};
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct WavAudio {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<i16>,
}

fn err(message: impl Into<String>) -> AppError {
    AppError::Module {
        module: "voice::wav",
        message: message.into(),
    }
}

/// Scans chunks rather than assuming `fmt ` immediately precedes `data`
/// -- real WAV files (including ones produced by `sox`/`espeak-ng`) can
/// carry other chunks (`LIST`, metadata, ...) in between.
pub fn read_wav(path: &Path) -> AppResult<WavAudio> {
    let bytes = std::fs::read(path).map_err(|e| err(format!("reading {}: {e}", path.display())))?;

    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(err(format!("{} is not a RIFF/WAVE file", path.display())));
    }

    let mut pos = 12;
    let mut fmt: Option<(u16, u16, u32, u16)> = None; // (audio_format, channels, sample_rate, bits_per_sample)
    let mut data: Option<Vec<u8>> = None;

    while pos + 8 <= bytes.len() {
        let chunk_id = &bytes[pos..pos + 4];
        let chunk_size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let chunk_start = pos + 8;
        let chunk_end = (chunk_start + chunk_size).min(bytes.len());
        let chunk = &bytes[chunk_start..chunk_end];

        if chunk_id == b"fmt " && chunk.len() >= 16 {
            fmt = Some((
                u16::from_le_bytes(chunk[0..2].try_into().unwrap()),
                u16::from_le_bytes(chunk[2..4].try_into().unwrap()),
                u32::from_le_bytes(chunk[4..8].try_into().unwrap()),
                u16::from_le_bytes(chunk[14..16].try_into().unwrap()),
            ));
        } else if chunk_id == b"data" {
            data = Some(chunk.to_vec());
        }

        // Chunks are word-aligned: a chunk with an odd size has one
        // padding byte after it that isn't part of `chunk_size`.
        pos = chunk_end + (chunk_size % 2);
    }

    let (audio_format, channels, sample_rate, bits_per_sample) =
        fmt.ok_or_else(|| err(format!("{}: no fmt chunk found", path.display())))?;
    let data = data.ok_or_else(|| err(format!("{}: no data chunk found", path.display())))?;

    if audio_format != 1 {
        return Err(err(format!(
            "{}: only uncompressed PCM is supported (format code {audio_format})",
            path.display()
        )));
    }
    if bits_per_sample != 16 {
        return Err(err(format!(
            "{}: only 16-bit samples are supported ({bits_per_sample}-bit found)",
            path.display()
        )));
    }

    let samples = data
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect();
    Ok(WavAudio {
        sample_rate,
        channels,
        samples,
    })
}

/// Writes 16-bit PCM mono/stereo audio as a standard WAV file.
pub fn write_wav(path: &Path, sample_rate: u32, channels: u16, samples: &[i16]) -> AppResult<()> {
    const BITS_PER_SAMPLE: u16 = 16;
    let byte_rate = sample_rate * channels as u32 * (BITS_PER_SAMPLE as u32 / 8);
    let block_align = channels * (BITS_PER_SAMPLE / 8);
    let data_size = (samples.len() * 2) as u32;
    let riff_size = 36 + data_size;

    let mut bytes = Vec::with_capacity(44 + data_size as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&riff_size.to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&byte_rate.to_le_bytes());
    bytes.extend_from_slice(&block_align.to_le_bytes());
    bytes.extend_from_slice(&BITS_PER_SAMPLE.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_size.to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }

    std::fs::write(path, bytes).map_err(|e| err(format!("writing {}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("voice-wav-test-{label}.wav"))
    }

    #[test]
    fn round_trips_written_samples() {
        let path = temp_path("roundtrip");
        let samples: Vec<i16> = (0..1000).map(|i| (i % 200 - 100) as i16 * 100).collect();
        write_wav(&path, 16000, 1, &samples).unwrap();

        let read_back = read_wav(&path).unwrap();
        assert_eq!(read_back.sample_rate, 16000);
        assert_eq!(read_back.channels, 1);
        assert_eq!(read_back.samples, samples);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn reading_a_missing_file_is_a_clear_error() {
        assert!(read_wav(Path::new("/definitely/not/a/real/file.wav")).is_err());
    }

    #[test]
    fn reading_a_non_wav_file_is_a_clear_error() {
        let path = temp_path("notwav");
        std::fs::write(&path, b"this is not a wav file at all").unwrap();
        assert!(read_wav(&path).is_err());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn an_empty_sample_list_still_writes_a_valid_header() {
        let path = temp_path("empty");
        write_wav(&path, 16000, 1, &[]).unwrap();
        let read_back = read_wav(&path).unwrap();
        assert!(read_back.samples.is_empty());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn stereo_round_trips_correctly() {
        let path = temp_path("stereo");
        let samples: Vec<i16> = vec![100, -100, 200, -200, 300, -300];
        write_wav(&path, 44100, 2, &samples).unwrap();
        let read_back = read_wav(&path).unwrap();
        assert_eq!(read_back.channels, 2);
        assert_eq!(read_back.samples, samples);
        std::fs::remove_file(&path).ok();
    }
}
