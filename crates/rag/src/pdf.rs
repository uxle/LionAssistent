//! PDF text extraction (Development_plan Phase 26 "PDF Support"). Wraps
//! `pdf-extract`, pinned `=0.6.5` -- confirmed empirically before
//! committing to it (0.7.x fails to resolve on this sandbox's rustc
//! 1.75, the same `edition2024` class of wall `vision`'s `image` pin
//! and `memory`'s `rusqlite` pin already document; 0.6.5 was then
//! confirmed to correctly extract real text from a real PDF -- see this
//! module's own test).
//!
//! # A real gap, found while building the test fixture
//!
//! Generating a test PDF via ImageMagick's `convert` (already a
//! dependency this workspace trusts for `screenshot`/`ocr`) produces a
//! PDF with **no extractable text layer at all** -- `convert` rasterizes
//! the requested text into a bitmap and embeds that as an image, the
//! same as printing a page and re-scanning it. Both `pdf-extract` and
//! the system's own `pdftotext` (poppler-utils) correctly report empty
//! output against it -- not a bug in either tool, an accurate
//! description of what that PDF actually contains. This module's test
//! fixture is instead a minimal, hand-built, spec-correct PDF with a
//! real text-showing content stream (`BT ... Tj ET`), verified with
//! `qpdf --check` and `pdftotext` independently before being trusted as
//! this crate's own test input -- the same "verify the fixture itself
//! is real before testing against it" discipline `ocr`'s and
//! `screenshot`'s own test fixtures already follow.

use common::{AppError, AppResult};
use std::path::Path;

fn module_err(context: impl std::fmt::Display) -> AppError {
    AppError::Module {
        module: "rag",
        message: context.to_string(),
    }
}

/// Extracts all text from `path`. An empty result (`Ok(String::new())`)
/// is a real, valid outcome for a PDF with no text layer (a scanned
/// image, for instance) -- not an error. OCR over rendered pages (this
/// workspace's own `ocr`/`screenshot` crates, Phase 23) is the answer
/// for that case, not something this function attempts itself.
pub fn extract_pdf_text(path: &Path) -> AppResult<String> {
    let bytes =
        std::fs::read(path).map_err(|e| module_err(format!("reading {}: {e}", path.display())))?;
    pdf_extract::extract_text_from_mem(&bytes)
        .map_err(|e| module_err(format!("extracting text from {}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a minimal, spec-correct single-page PDF with a real text
    /// content stream -- exact byte offsets computed as the file is
    /// assembled, not estimated, so the `xref` table it writes is
    /// actually correct rather than merely plausible-looking.
    fn build_minimal_text_pdf(text: &str) -> Vec<u8> {
        let objects: Vec<Vec<u8>> = vec![
            b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
            b"<< /Type /Page /Parent 2 0 R /Resources << /Font << /F1 4 0 R >> >> \
              /MediaBox [0 0 300 150] /Contents 5 0 R >>"
                .to_vec(),
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
            {
                let stream = format!("BT /F1 24 Tf 20 80 Td ({text}) Tj ET");
                format!(
                    "<< /Length {} >>\nstream\n{stream}\nendstream",
                    stream.len()
                )
                .into_bytes()
            },
        ];

        let mut out: Vec<u8> = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::with_capacity(objects.len());
        for (i, body) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
            out.extend_from_slice(body);
            out.extend_from_slice(b"\nendobj\n");
        }

        let xref_offset = out.len();
        let n = objects.len() + 1;
        out.extend_from_slice(format!("xref\n0 {n}\n").as_bytes());
        out.extend_from_slice(b"0000000000 65535 f \n");
        for off in &offsets {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!("trailer\n<< /Size {n} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF")
                .as_bytes(),
        );
        out
    }

    #[test]
    fn extracts_real_text_from_a_real_spec_correct_pdf() {
        let dir = std::env::temp_dir().join("rag-crate-test-pdf");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("fixture.pdf");
        std::fs::write(&path, build_minimal_text_pdf("Hello RAG World")).unwrap();

        let text = extract_pdf_text(&path).unwrap();
        assert!(text.contains("Hello RAG World"), "got: {text:?}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn nonexistent_file_fails_cleanly() {
        assert!(extract_pdf_text(Path::new("/nonexistent/nope.pdf")).is_err());
    }

    #[test]
    fn a_file_that_isnt_a_pdf_fails_cleanly_not_a_panic() {
        let dir = std::env::temp_dir().join("rag-crate-test-not-pdf");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("not-a-pdf.pdf");
        std::fs::write(&path, b"this is definitely not a pdf file").unwrap();

        assert!(extract_pdf_text(&path).is_err());

        std::fs::remove_dir_all(&dir).ok();
    }
}
