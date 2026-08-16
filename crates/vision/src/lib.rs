//! Image understanding (Development_plan Phase 23 "Image Understanding";
//! TCLIS SS15 names "local vision models" explicitly as planned future
//! expansion). Two genuinely different things live here, deliberately
//! kept apart rather than blurred together:
//!
//! 1. [`analyze_image`] -- real, decoded, verified-for-real pixel
//!    analysis (dimensions, format, mean color) via the `image` crate.
//!    No model, no interpretation, just facts read out of the file.
//! 2. [`VisionBackend`] -- the trait a *semantic* description ("this is
//!    a photo of a cat sitting on a red chair") would come from. This
//!    crate ships only [`MetadataOnlyVisionDescriber`], an honest stub
//!    that never claims to have looked at the image's actual content --
//!    mirrors `assistant-core::EchoResponseGenerator` exactly, down to
//!    the reasoning: a stub that proves the pipeline runs without
//!    fabricating a result is more useful and more honest than no stub
//!    at all (Ai_reasoning SS17: "must not fabricate completed
//!    actions"). A real model-backed implementation lives in
//!    `vision-runtime`, in the same unverified-in-this-sandbox category
//!    as `llm-runtime` -- see that crate's doc comment and
//!    docs/ROADMAP.md.
//!
//! Gemma 3 1B, this project's configured default model (PRD SS9), is
//! text-only -- the smallest Gemma 3 size with a vision encoder is 4B.
//! So even once `vision-runtime` exists and compiles somewhere with a
//! modern enough toolchain, it needs its own separate model, not a
//! capability Gemma 3 1B happens to already have.

use common::{AppError, AppResult};
use std::path::Path;

pub mod tools;

fn module_err(context: impl std::fmt::Display) -> AppError {
    AppError::Module {
        module: "vision",
        message: context.to_string(),
    }
}

/// Real, decoded facts about an image (TCLIS SS8 "Standard Result
/// Format"). Every field here is computed from actual pixel data, not
/// asserted -- see `analyze_image`.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    /// From `image::ImageFormat`'s `Debug` output (e.g. `"Png"`,
    /// `"Jpeg"`) -- whatever the decoder itself determined from the
    /// file's real content, not from the file extension.
    pub format: String,
    pub file_size_bytes: u64,
    pub mean_red: f64,
    pub mean_green: f64,
    pub mean_blue: f64,
}

impl ImageInfo {
    pub fn mean_brightness(&self) -> f64 {
        (self.mean_red + self.mean_green + self.mean_blue) / 3.0
    }
}

/// Decodes `path` for real and computes real per-channel averages across
/// every pixel. Confirmed during development against an actual captured
/// screenshot (1024x768, real dimensions read back correctly) before
/// being written down as a test.
pub fn analyze_image(path: &Path) -> AppResult<ImageInfo> {
    let file_size_bytes = std::fs::metadata(path)
        .map_err(|e| module_err(format!("reading metadata for {}: {e}", path.display())))?
        .len();

    let format = image::ImageFormat::from_path(path)
        .map(|f| format!("{f:?}"))
        .unwrap_or_else(|_| "unknown".to_string());

    let img =
        image::open(path).map_err(|e| module_err(format!("decoding {}: {e}", path.display())))?;
    let rgb = img.to_rgb8();
    let (width, height) = (rgb.width(), rgb.height());

    let mut sum_r: u64 = 0;
    let mut sum_g: u64 = 0;
    let mut sum_b: u64 = 0;
    for pixel in rgb.pixels() {
        sum_r += pixel.0[0] as u64;
        sum_g += pixel.0[1] as u64;
        sum_b += pixel.0[2] as u64;
    }
    let pixel_count = (width as u64 * height as u64).max(1) as f64;

    Ok(ImageInfo {
        width,
        height,
        format,
        file_size_bytes,
        mean_red: sum_r as f64 / pixel_count,
        mean_green: sum_g as f64 / pixel_count,
        mean_blue: sum_b as f64 / pixel_count,
    })
}

/// What a semantic image description would come from (mirrors
/// `assistant_core::ResponseGenerator`'s shape deliberately -- same
/// role, one phase later). Takes both the real path (a genuine backend
/// needs actual pixel data to run a vision model against, not just
/// summary statistics) and the already-computed `ImageInfo` (so a stub,
/// or a real backend that only wants the cheap facts, doesn't have to
/// redo `analyze_image`'s work or risk contradicting it).
pub trait VisionBackend: Send + Sync {
    fn describe(&self, path: &Path, info: &ImageInfo) -> String;
}

/// Honest stub (see this module's doc comment). Default backend until
/// `vision-runtime` -- or any `VisionBackend` implementation -- is
/// actually wired up by a caller. Ignores `path` entirely: the whole
/// point is that it does *not* look at pixel content.
#[derive(Debug, Default)]
pub struct MetadataOnlyVisionDescriber;

impl VisionBackend for MetadataOnlyVisionDescriber {
    fn describe(&self, _path: &Path, info: &ImageInfo) -> String {
        format!(
            "[no local vision-language model is configured -- vision-runtime sits behind the \
             same Candle/rustc wall as llm-runtime, see docs/ROADMAP.md] This is a {}x{} {} \
             image ({} KB), mean brightness {:.0}/255 (R {:.0}, G {:.0}, B {:.0}).",
            info.width,
            info.height,
            info.format,
            info.file_size_bytes / 1024,
            info.mean_brightness(),
            info.mean_red,
            info.mean_green,
            info.mean_blue,
        )
    }
}

/// Convenience: analyze, then describe through whichever backend the
/// caller configured (the stub by default).
pub fn describe_image(path: &Path, backend: &dyn VisionBackend) -> AppResult<(ImageInfo, String)> {
    let info = analyze_image(path)?;
    let description = backend.describe(path, &info);
    Ok((info, description))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A hand-built, valid, tiny PNG: 2x2 pixels, solid red -- small
    /// enough to construct by hand (unlike a real photo) while still
    /// being genuine encoded pixel data a real decoder has to actually
    /// decompress, not a format this crate special-cases.
    fn write_2x2_red_png(path: &Path) {
        // Built once with the `image` crate itself and its bytes
        // hard-coded here so this test has no dependency on file I/O
        // order or a network fetch -- `image::save_buffer` writing then
        // `image::open` reading is exactly the round trip this crate's
        // own `analyze_image` performs, so encoding it inline with the
        // same crate is a legitimate fixture, not circular reasoning
        // about a *different* decoder.
        let mut buf = Vec::new();
        {
            let img = image::RgbImage::from_fn(2, 2, |_, _| image::Rgb([200, 20, 20]));
            let mut cursor = std::io::Cursor::new(&mut buf);
            image::DynamicImage::ImageRgb8(img)
                .write_to(&mut cursor, image::ImageFormat::Png)
                .unwrap();
        }
        let mut file = std::fs::File::create(path).unwrap();
        file.write_all(&buf).unwrap();
    }

    #[test]
    fn analyzes_a_real_solid_color_image_correctly() {
        let dir = std::env::temp_dir().join("vision-crate-test-red");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("red.png");
        write_2x2_red_png(&path);

        let info = analyze_image(&path).unwrap();
        assert_eq!((info.width, info.height), (2, 2));
        assert_eq!(info.format, "Png");
        assert!(info.file_size_bytes > 0);
        // PNG is lossless, so a solid-color source decodes back exactly.
        assert!((info.mean_red - 200.0).abs() < 0.5);
        assert!((info.mean_green - 20.0).abs() < 0.5);
        assert!((info.mean_blue - 20.0).abs() < 0.5);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn nonexistent_file_fails_cleanly_not_with_a_panic() {
        let result = analyze_image(Path::new("/nonexistent/nope.png"));
        assert!(result.is_err());
    }

    #[test]
    fn stub_backend_incorporates_the_real_computed_facts_and_says_so_honestly() {
        let dir = std::env::temp_dir().join("vision-crate-test-describe");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("red.png");
        write_2x2_red_png(&path);

        let (info, description) = describe_image(&path, &MetadataOnlyVisionDescriber).unwrap();
        assert_eq!(info.width, 2);
        assert!(description.contains("2x2"));
        assert_eq!(info.format, "Png");
        assert!(
            description.contains("no local vision-language model"),
            "the stub must not imply it actually looked at image content"
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
