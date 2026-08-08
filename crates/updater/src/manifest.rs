//! "Updates" (Development_plan Phase 14): compares the running
//! application's version against a small JSON manifest fetched from a
//! pre-configured URL. This is original design work for this project --
//! nothing in the PRD/Apis/TDS docs specifies a manifest wire format, so
//! this is deliberately the smallest shape that answers "is there
//! something newer, and where do I get it":
//!
//! ```json
//! {
//!   "version": "1.3.0",
//!   "download_url": "https://example.invalid/releases/ai-assistant-1.3.0.tar.gz",
//!   "notes": "Bug fixes and performance improvements."
//! }
//! ```

use common::{AppError, AppResult};
use network::HttpClient;
use semver::Version;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct UpdateManifest {
    pub version: String,
    pub download_url: String,
    #[serde(default)]
    pub notes: String,
}

/// Fetches `manifest_url` and compares it against `current_version`
/// (semver strings, e.g. `"1.2.0"` -- `CARGO_PKG_VERSION` of whatever
/// binary is asking). Returns `Ok(Some(manifest))` only if the manifest
/// really does describe something newer; `Ok(None)` covers both "already
/// current" and "no manifest published yet" (a 404) -- neither is an
/// error, both just mean "nothing to report." Genuine problems (an
/// unreachable server, a malformed manifest, an unparseable version on
/// either side) are `Err`.
pub fn check_for_update(
    client: &HttpClient,
    manifest_url: &str,
    current_version: &str,
) -> AppResult<Option<UpdateManifest>> {
    let current = Version::parse(current_version).map_err(|e| {
        AppError::Config(format!("invalid current version {current_version:?}: {e}"))
    })?;

    let response = client.get(manifest_url)?;
    if response.status == 404 {
        return Ok(None);
    }
    if !response.is_success() {
        return Err(AppError::Module {
            module: "updater",
            message: format!(
                "update manifest at {manifest_url} returned status {}",
                response.status
            ),
        });
    }

    let manifest: UpdateManifest = serde_json::from_str(&response.body)
        .map_err(|e| AppError::Serialization(format!("parsing update manifest: {e}")))?;

    let remote = Version::parse(&manifest.version).map_err(|e| {
        AppError::Serialization(format!(
            "update manifest has an invalid version {:?}: {e}",
            manifest.version
        ))
    })?;

    Ok((remote > current).then_some(manifest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use network::testing::MockServer;

    fn manifest_json(version: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "version": version,
            "download_url": "https://example.invalid/releases/ai-assistant.tar.gz",
            "notes": "test notes",
        }))
        .unwrap()
    }

    #[test]
    fn reports_an_update_when_the_manifest_is_newer() {
        let server = MockServer::respond_once(200, "application/json", manifest_json("2.0.0"));
        let client = HttpClient::new().unwrap();
        let result = check_for_update(&client, &server.base_url, "1.0.0").unwrap();
        server.join();

        let manifest = result.expect("expected an update to be reported");
        assert_eq!(manifest.version, "2.0.0");
    }

    #[test]
    fn reports_no_update_when_current() {
        let server = MockServer::respond_once(200, "application/json", manifest_json("1.0.0"));
        let client = HttpClient::new().unwrap();
        let result = check_for_update(&client, &server.base_url, "1.0.0").unwrap();
        server.join();

        assert!(result.is_none());
    }

    #[test]
    fn reports_no_update_when_the_manifest_is_actually_older() {
        // A rollback / mirror lag scenario -- never report a "downgrade"
        // as an update.
        let server = MockServer::respond_once(200, "application/json", manifest_json("0.9.0"));
        let client = HttpClient::new().unwrap();
        let result = check_for_update(&client, &server.base_url, "1.0.0").unwrap();
        server.join();

        assert!(result.is_none());
    }

    #[test]
    fn numeric_version_comparison_is_not_lexicographic() {
        // A lexicographic (string) compare would think "1.9.0" > "1.10.0".
        let server = MockServer::respond_once(200, "application/json", manifest_json("1.10.0"));
        let client = HttpClient::new().unwrap();
        let result = check_for_update(&client, &server.base_url, "1.9.0").unwrap();
        server.join();

        assert!(result.is_some());
    }

    #[test]
    fn a_missing_manifest_is_not_an_error() {
        let server = MockServer::respond_once(404, "text/plain", b"not found".to_vec());
        let client = HttpClient::new().unwrap();
        let result = check_for_update(&client, &server.base_url, "1.0.0").unwrap();
        server.join();

        assert!(result.is_none());
    }

    #[test]
    fn a_server_error_is_an_error() {
        let server = MockServer::respond_once(500, "text/plain", b"oops".to_vec());
        let client = HttpClient::new().unwrap();
        let result = check_for_update(&client, &server.base_url, "1.0.0");
        server.join();

        assert!(result.is_err());
    }

    #[test]
    fn a_malformed_manifest_is_an_error() {
        let server = MockServer::respond_once(200, "application/json", b"not json at all".to_vec());
        let client = HttpClient::new().unwrap();
        let result = check_for_update(&client, &server.base_url, "1.0.0");
        server.join();

        assert!(result.is_err());
    }

    #[test]
    fn an_invalid_current_version_is_an_error() {
        let client = HttpClient::new().unwrap();
        let result = check_for_update(&client, "http://127.0.0.1:1", "not-a-version");
        assert!(result.is_err());
    }
}
