//! Downloads the artifact a [`crate::UpdateManifest`] points at.
//!
//! Deliberately stops there. This crate does **not** replace the running
//! binary, extract an archive over the install directory, or restart the
//! application -- self-updating-in-place is a genuinely risky operation
//! (partial writes to a binary that's currently executing, no atomic
//! rename story that works the same across deb/rpm/AppImage/Flatpak/Snap,
//! fighting the very package manager Development_plan Phase 20 already
//! plans to package this app for) that a "Networking" phase has no
//! business deciding unilaterally. Downloading to a clearly-labeled
//! location and telling the user what's there is the honest stopping
//! point; installing it is either a manual step or, more likely, "update
//! via your distro's package manager" once Phase 20 exists -- exactly
//! what `linux_services`'s existing package tools (Phase 8,
//! `ToolCategory::PackageManagement`) are already for.

use crate::manifest::UpdateManifest;
use common::AppResult;
use network::HttpClient;
use std::path::{Path, PathBuf};

/// Downloads `manifest.download_url` into `dest_dir` (created if it
/// doesn't exist), named after the URL's last path segment. Returns the
/// full path written.
pub fn download_update(
    client: &HttpClient,
    manifest: &UpdateManifest,
    dest_dir: &Path,
) -> AppResult<PathBuf> {
    std::fs::create_dir_all(dest_dir)?;

    let filename = manifest
        .download_url
        .rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .unwrap_or("update-download");
    let dest = dest_dir.join(filename);

    client.download(&manifest.download_url, &dest)?;
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use network::testing::MockServer;

    #[test]
    fn downloads_to_a_file_named_after_the_url() {
        let dir = std::env::temp_dir().join("updater-download-test-basic");
        std::fs::remove_dir_all(&dir).ok();

        let server =
            MockServer::respond_once(200, "application/gzip", b"fake archive bytes".to_vec());
        let manifest = UpdateManifest {
            version: "9.9.9".to_string(),
            download_url: format!("{}/ai-assistant-9.9.9.tar.gz", server.base_url),
            notes: String::new(),
        };

        let client = HttpClient::new().unwrap();
        let path = download_update(&client, &manifest, &dir).unwrap();
        server.join();

        assert_eq!(path, dir.join("ai-assistant-9.9.9.tar.gz"));
        assert_eq!(std::fs::read(&path).unwrap(), b"fake archive bytes");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn creates_the_destination_directory_if_missing() {
        let dir = std::env::temp_dir().join("updater-download-test-mkdir");
        std::fs::remove_dir_all(&dir).ok();
        assert!(!dir.exists());

        let server = MockServer::respond_once(200, "application/gzip", b"bytes".to_vec());
        let manifest = UpdateManifest {
            version: "1.0.0".to_string(),
            download_url: format!("{}/file.tar.gz", server.base_url),
            notes: String::new(),
        };

        let client = HttpClient::new().unwrap();
        download_update(&client, &manifest, &dir).unwrap();
        server.join();

        assert!(dir.exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}
