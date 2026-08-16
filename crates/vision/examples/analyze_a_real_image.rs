//! Generates a small real PNG, analyzes it for real (no mocking), and
//! shows the honest stub `VisionBackend` versus what a real backend's
//! result shape looks like. Run with:
//!
//!     cargo run -p vision --example analyze_a_real_image

use vision::{analyze_image, describe_image, MetadataOnlyVisionDescriber};

fn main() {
    let dir = std::env::temp_dir().join("vision-example");
    std::fs::create_dir_all(&dir).expect("create example output dir");
    let path = dir.join("gradient.png");

    // A real 64x64 gradient, encoded by the same `image` crate
    // `analyze_image` decodes with below -- genuine pixel data, not a
    // fixture stood in for one.
    let img = image::RgbImage::from_fn(64, 64, |x, y| {
        image::Rgb([(x * 4) as u8, (y * 4) as u8, 128])
    });
    image::DynamicImage::ImageRgb8(img)
        .save(&path)
        .expect("write example PNG");

    // `analyze_image` alone: just the real, decoded facts.
    let info = analyze_image(&path).expect("decode the PNG we just wrote");
    println!("dimensions: {}x{}", info.width, info.height);
    println!("format: {}", info.format);
    println!("file size: {} bytes", info.file_size_bytes);
    println!(
        "mean color: R {:.1} G {:.1} B {:.1} (brightness {:.1}/255)",
        info.mean_red,
        info.mean_green,
        info.mean_blue,
        info.mean_brightness()
    );

    // `describe_image`: the same analysis, plus whatever a VisionBackend
    // says about it. This example only ships the honest stub -- a real
    // model-backed description would come from `vision-runtime`'s
    // `ClipVisionBackend`, which needs Candle and real model weights
    // this repo doesn't ship, so it can't run in an example like this
    // one (see vision-runtime's own doc comment).
    let (_, description) =
        describe_image(&path, &MetadataOnlyVisionDescriber).expect("describe the PNG");
    println!("\nMetadataOnlyVisionDescriber says:\n{description}");

    std::fs::remove_dir_all(&dir).ok();
}
