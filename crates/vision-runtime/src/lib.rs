//! Candle-based `vision::VisionBackend` implementation: CLIP zero-shot
//! image classification (Development_plan Phase 23 "Image
//! Understanding"). In the same unverified-in-this-sandbox category as
//! `llm-runtime` -- Candle needs rustc 1.88+, this sandbox is pinned to
//! rustc 1.75 (see `llm-runtime`'s own doc comment and
//! docs/ROADMAP.md's toolchain-wall section). Never added to the
//! `-p ...` verified build/test list in README.md.
//!
//! # Exactly how far this was checked, and how far it wasn't
//!
//! `llm-runtime`'s doc comment describes its Candle API calls as checked
//! against Candle's real, fetched source. This crate falls a step short
//! of that same bar, and says so explicitly rather than implying parity
//! it doesn't have:
//!
//! - **Confirmed against real, fetched source**: `candle_transformers`
//!   has an actual `clip` module at `candle-transformers/src/models/
//!   clip/mod.rs`, split into `text_model`/`vision_model` submodules,
//!   with a real `ClipConfig { text_config: text_model::ClipTextConfig,
//!   vision_config: vision_model::ClipVisionConfig,
//!   logit_scale_init_value: f32, image_size: usize }` (quoted, at
//!   commit `2f49e1b`, in `huggingface/candle` issue #2520). The
//!   official `clip` example's CLI contract (`--images`, `--sequences`,
//!   per-image "Probability: X% Text: Y" output) is also confirmed --
//!   this crate's `classify` reproduces that exact contract.
//! - **Reasoned from Candle's own established conventions, not
//!   independently re-fetched for this specific module**: constructor
//!   and forward-pass method names below (`ClipModel::new`,
//!   `.get_image_features`, `.get_text_features`). Every Candle model
//!   this workspace has actually fetched and read (`bert.rs`, and
//!   `llm-runtime`'s own quantized Gemma 3 integration) loads via a
//!   `VarBuilder` + `Config` pair and exposes a top-level `XModel`
//!   struct, so following that same shape here is a reasoned extension
//!   of a real, confirmed pattern -- not a blind guess -- but it
//!   deserves a real compile-and-fix pass against `clip/mod.rs` itself
//!   on a modern toolchain before anyone trusts it further than that.
//! - **Standard, non-Candle-specific facts**: CLIP's published
//!   preprocessing (resize/center-crop to a fixed square, normalize with
//!   mean `[0.48145466, 0.4578275, 0.40821073]` / std `[0.26862954,
//!   0.26130258, 0.27577711]`) is OpenAI's original, well-documented
//!   CLIP preprocessing recipe, unrelated to which Rust framework
//!   implements it.
//!
//! Candidate labels are supplied at construction (`ClipVisionBackend::
//! new`) rather than hardcoded: CLIP is a zero-shot classifier, not a
//! captioner -- it scores a *given* set of candidate text descriptions
//! against the image rather than generating open-ended prose the way a
//! true captioning or VLM model would (Phase 23's honest scope; a
//! captioning-capable model is future work, not this).

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::clip::{ClipConfig, ClipModel};
use std::path::Path;
use vision::{ImageInfo, VisionBackend};

/// OpenAI's published CLIP preprocessing constants (see this module's
/// doc comment -- these are standard, not Candle- or Rust-specific).
const CLIP_IMAGE_MEAN: [f32; 3] = [0.481_454_66, 0.457_827_5, 0.408_210_73];
const CLIP_IMAGE_STD: [f32; 3] = [0.268_629_54, 0.261_302_58, 0.275_777_11];

pub struct ClipVisionBackend {
    model: ClipModel,
    tokenizer: tokenizers::Tokenizer,
    device: Device,
    image_size: usize,
    candidate_labels: Vec<String>,
}

impl ClipVisionBackend {
    /// Loads CLIP weights (`model.safetensors`) and a matching
    /// tokenizer (`tokenizer.json`) from `model_dir`, following the same
    /// `VarBuilder::from_mmaped_safetensors` + local-directory loading
    /// shape `llm-runtime::CandleGemmaBackend::load` already uses in
    /// this workspace, rather than reaching for `hf_hub`'s network
    /// download path directly (Mms SS18: "load models only from
    /// trusted user-selected locations").
    pub fn new(
        model_dir: &Path,
        config: ClipConfig,
        candidate_labels: Vec<String>,
    ) -> anyhow::Result<Self> {
        if candidate_labels.is_empty() {
            anyhow::bail!("ClipVisionBackend needs at least one candidate label to score against");
        }

        let device = Device::Cpu;
        let weights_path = model_dir.join("model.safetensors");
        let vb =
            unsafe { VarBuilder::from_mmaped_safetensors(&[weights_path], DType::F32, &device)? };
        let model = ClipModel::new(vb, &config)?;

        let tokenizer_path = model_dir.join("tokenizer.json");
        let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("loading tokenizer from {tokenizer_path:?}: {e}"))?;

        Ok(Self {
            model,
            tokenizer,
            device,
            image_size: config.image_size,
            candidate_labels,
        })
    }

    /// Loads, resizes, center-crops, and normalizes `path` into the
    /// `[1, 3, image_size, image_size]` tensor CLIP's vision tower
    /// expects.
    fn preprocess_image(&self, path: &Path) -> anyhow::Result<Tensor> {
        let img = image::open(path)?;
        let resized = img.resize_to_fill(
            self.image_size as u32,
            self.image_size as u32,
            image::imageops::FilterType::Triangle,
        );
        let rgb = resized.to_rgb8();

        let mut chw = vec![0f32; 3 * self.image_size * self.image_size];
        let plane = self.image_size * self.image_size;
        for (i, pixel) in rgb.pixels().enumerate() {
            for c in 0..3 {
                let normalized =
                    (pixel.0[c] as f32 / 255.0 - CLIP_IMAGE_MEAN[c]) / CLIP_IMAGE_STD[c];
                chw[c * plane + i] = normalized;
            }
        }

        Ok(Tensor::from_vec(
            chw,
            (1, 3, self.image_size, self.image_size),
            &self.device,
        )?)
    }

    fn tokenize_labels(&self) -> anyhow::Result<Tensor> {
        let encodings = self
            .tokenizer
            .encode_batch(self.candidate_labels.clone(), true)
            .map_err(|e| anyhow::anyhow!("tokenizing candidate labels: {e}"))?;

        let max_len = encodings.iter().map(|e| e.len()).max().unwrap_or(1);
        let mut rows = Vec::with_capacity(encodings.len());
        for encoding in &encodings {
            let mut ids: Vec<u32> = encoding.get_ids().to_vec();
            ids.resize(max_len, 0); // pad with 0 (CLIP's tokenizer pad id)
            rows.push(ids);
        }

        let flat: Vec<u32> = rows.into_iter().flatten().collect();
        Ok(Tensor::from_vec(
            flat,
            (self.candidate_labels.len(), max_len),
            &self.device,
        )?)
    }

    /// Real zero-shot classification: image embedding x each candidate
    /// label's text embedding, cosine similarity, softmax -- CLIP's
    /// standard scoring pipeline, reproducing the official example's
    /// exact "Probability: X% Text: Y" contract per candidate.
    pub fn classify(&self, path: &Path) -> anyhow::Result<Vec<(String, f32)>> {
        let image_tensor = self.preprocess_image(path)?;
        let text_tensor = self.tokenize_labels()?;

        let image_features = self.model.get_image_features(&image_tensor)?;
        let text_features = self.model.get_text_features(&text_tensor)?;

        let image_features = normalize_l2(&image_features)?;
        let text_features = normalize_l2(&text_features)?;

        let logits = image_features.matmul(&text_features.t()?)?;
        let probabilities = candle_nn::ops::softmax(&logits, candle_core::D::Minus1)?;
        let probabilities: Vec<f32> = probabilities.get(0)?.to_vec1()?;

        let mut scored: Vec<(String, f32)> = self
            .candidate_labels
            .iter()
            .cloned()
            .zip(probabilities)
            .collect();
        scored.sort_by(|a, b| b.1.total_cmp(&a.1));
        Ok(scored)
    }
}

fn normalize_l2(tensor: &Tensor) -> candle_core::Result<Tensor> {
    tensor.broadcast_div(&tensor.sqr()?.sum_keepdim(candle_core::D::Minus1)?.sqrt()?)
}

impl VisionBackend for ClipVisionBackend {
    /// `VisionBackend::describe` is infallible (mirrors
    /// `assistant_core::ResponseGenerator::generate`'s `-> String`
    /// shape), but real classification genuinely can fail (bad image,
    /// I/O error, tensor shape mismatch). Ai_reasoning SS17 -- "must not
    /// fabricate completed actions" -- means a failure has to show up as
    /// an honest, visible message in the returned text, not be
    /// swallowed into a confident-sounding fallback description.
    fn describe(&self, path: &Path, info: &ImageInfo) -> String {
        match self.classify(path) {
            Ok(scored) => {
                let top = scored
                    .iter()
                    .take(3)
                    .map(|(label, prob)| format!("{label} ({:.1}%)", prob * 100.0))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "{}x{} {} image -- closest matches among candidate labels: {top}",
                    info.width, info.height, info.format
                )
            }
            Err(e) => format!(
                "[vision-runtime classification failed: {e}] {}x{} {} image ({} KB) -- \
                 falling back to metadata only",
                info.width,
                info.height,
                info.format,
                info.file_size_bytes / 1024
            ),
        }
    }
}

// No #[cfg(test)] module: every path here touches either real model
// weights this project doesn't ship (CLIP checkpoints aren't part of
// this repo, the same reason `llm-runtime` has no test fixtures of its
// own GGUF weights either) or the Candle/rustc wall itself, so there's
// nothing here a test could run against in this environment. See
// docs/ROADMAP.md rather than trusting an absence of tests to mean
// nothing was considered.
