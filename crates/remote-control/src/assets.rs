//! Static assets for the mobile companion PWA (Development_plan
//! Phase 28's "Android App" bullet -- see `lib.rs` for why this is a
//! PWA rather than a native APK).
//!
//! Embedded into the compiled binary via `include_bytes!` rather than
//! read from disk at runtime: a `remote-control::server::serve` caller
//! (eventually `app`) shouldn't have to know or care where this
//! crate's source tree happens to sit relative to the current working
//! directory, the same reasoning nothing else in this workspace reads
//! its own source files back at runtime either.

pub fn get(name: &str) -> &'static [u8] {
    match name {
        "index.html" => include_bytes!("../assets/index.html"),
        "app.js" => include_bytes!("../assets/app.js"),
        "style.css" => include_bytes!("../assets/style.css"),
        "manifest.json" => include_bytes!("../assets/manifest.json"),
        _ => b"",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_asset_server_rs_routes_to_is_non_empty() {
        for name in ["index.html", "app.js", "style.css", "manifest.json"] {
            assert!(!get(name).is_empty(), "{name} embedded as empty");
        }
    }

    #[test]
    fn unknown_asset_name_returns_empty_rather_than_panicking() {
        assert_eq!(get("does-not-exist.txt"), b"");
    }

    #[test]
    fn manifest_is_valid_json() {
        let bytes = get("manifest.json");
        let parsed: serde_json::Value =
            serde_json::from_slice(bytes).expect("manifest.json must be valid JSON");
        assert!(parsed.get("name").is_some());
        assert!(parsed.get("start_url").is_some());
    }

    #[test]
    fn index_html_references_app_js_and_manifest() {
        let html = std::str::from_utf8(get("index.html")).unwrap();
        assert!(html.contains("app.js"));
        assert!(html.contains("manifest.json"));
    }
}
