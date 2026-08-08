//! Wires update-checking up as real [`tool_framework::Tool`]
//! implementations. Both are narrow on purpose:
//!
//! - Neither takes a URL from the caller. The manifest URL is fixed at
//!   construction time (by whatever composes the tool registry --
//!   `app`, in the normal case), never something an LLM's tool call can
//!   redirect. A general "fetch any URL" capability is a materially
//!   different, broader security surface than this (see `network`'s
//!   crate-level docs); these two tools deliberately don't become that.
//! - `check_for_updates` is `ReadOnly`; `download_update` is `Moderate`
//!   (TCLIS SS6 Level 2 -- it writes a file) and so requires
//!   confirmation under this workspace's default `SecurityRules`
//!   (`auto_approve_at_or_below: Safe`), same as any other tool that
//!   creates a file on disk.
//! - Neither is `ToolCategory::Network` -- that's already Phase 8's
//!   category for *local* network status (Wi-Fi, interfaces; TCLIS SS4).
//!   `PackageManagement` is the closer semantic match: TCLIS's own text
//!   lists "Update packages" under that category, and checking/fetching
//!   an application update is the same kind of operation one level up.

use crate::download::download_update;
use crate::manifest::check_for_update;
use network::HttpClient;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tool_framework::{InputSchema, PermissionLevel, Tool, ToolArgs, ToolCategory, ToolOutcome};

fn err_outcome(e: impl std::fmt::Display) -> ToolOutcome {
    ToolOutcome {
        success: false,
        message: e.to_string(),
        data: None,
    }
}

pub struct CheckForUpdatesTool {
    pub client: Arc<HttpClient>,
    pub manifest_url: String,
    pub current_version: String,
}

impl Tool for CheckForUpdatesTool {
    fn name(&self) -> &str {
        "check_for_updates"
    }
    fn description(&self) -> &str {
        "Checks whether a newer version of the assistant is available."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::PackageManagement
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(30)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::new(vec![])
    }
    fn execute(&self, _args: &ToolArgs) -> ToolOutcome {
        match check_for_update(&self.client, &self.manifest_url, &self.current_version) {
            Ok(Some(manifest)) => ToolOutcome {
                success: true,
                message: format!(
                    "update available: {} -> {}",
                    self.current_version, manifest.version
                ),
                data: Some(serde_json::json!({
                    "update_available": true,
                    "current_version": self.current_version,
                    "latest_version": manifest.version,
                    "notes": manifest.notes,
                    "download_url": manifest.download_url,
                })),
            },
            Ok(None) => ToolOutcome {
                success: true,
                message: format!("already up to date (version {})", self.current_version),
                data: Some(serde_json::json!({
                    "update_available": false,
                    "current_version": self.current_version,
                })),
            },
            Err(e) => err_outcome(e),
        }
    }
}

pub struct DownloadUpdateTool {
    pub client: Arc<HttpClient>,
    pub manifest_url: String,
    pub current_version: String,
    pub dest_dir: PathBuf,
}

impl Tool for DownloadUpdateTool {
    fn name(&self) -> &str {
        "download_update"
    }
    fn description(&self) -> &str {
        "Downloads the latest available update, if any, to disk. Does not install or apply it."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::PackageManagement
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Moderate
    }
    fn timeout(&self) -> Duration {
        // Real release artifacts are much bigger than anything else this
        // workspace's tools transfer -- a generous budget relative to
        // the 10-20s other tools use.
        Duration::from_secs(300)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::new(vec![])
    }
    fn execute(&self, _args: &ToolArgs) -> ToolOutcome {
        let manifest =
            match check_for_update(&self.client, &self.manifest_url, &self.current_version) {
                Ok(Some(manifest)) => manifest,
                Ok(None) => {
                    return ToolOutcome {
                        success: true,
                        message: format!(
                            "already up to date (version {}); nothing downloaded",
                            self.current_version
                        ),
                        data: None,
                    }
                }
                Err(e) => return err_outcome(e),
            };

        match download_update(&self.client, &manifest, &self.dest_dir) {
            Ok(path) => ToolOutcome {
                success: true,
                message: format!(
                    "downloaded version {} to {}",
                    manifest.version,
                    path.display()
                ),
                data: Some(serde_json::json!({
                    "version": manifest.version,
                    "path": path.display().to_string(),
                })),
            },
            Err(e) => err_outcome(e),
        }
    }
}

/// Registers both tools, for `app` to call once at startup alongside
/// `linux_services::tools::register_all` and `search::tools::register_all`
/// -- not done in this phase, for the same reason those two also
/// weren't (`app` still can't be compile-verified in this sandbox;
/// docs/ROADMAP.md).
pub fn register_all(
    registry: &mut tool_framework::ToolRegistry,
    client: Arc<HttpClient>,
    manifest_url: impl Into<String>,
    current_version: impl Into<String>,
    dest_dir: PathBuf,
) -> common::AppResult<()> {
    let manifest_url = manifest_url.into();
    let current_version = current_version.into();
    registry.register(Arc::new(CheckForUpdatesTool {
        client: client.clone(),
        manifest_url: manifest_url.clone(),
        current_version: current_version.clone(),
    }))?;
    registry.register(Arc::new(DownloadUpdateTool {
        client,
        manifest_url,
        current_version,
        dest_dir,
    }))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use network::testing::MockServer;
    use tool_framework::{ToolRegistry, ToolRouter};

    fn manifest_json(version: &str, download_url: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "version": version,
            "download_url": download_url,
            "notes": "release notes",
        }))
        .unwrap()
    }

    fn router(manifest_url: String, dest_dir: PathBuf) -> ToolRouter {
        let mut registry = ToolRegistry::new();
        register_all(
            &mut registry,
            Arc::new(HttpClient::new().unwrap()),
            manifest_url,
            "1.0.0",
            dest_dir,
        )
        .unwrap();
        ToolRouter::new(registry, PermissionLevel::Moderate)
    }

    #[tokio::test]
    async fn check_for_updates_reports_an_available_update() {
        let server = MockServer::respond_once(
            200,
            "application/json",
            manifest_json("2.0.0", "http://x/y.tar.gz"),
        );
        let dir = std::env::temp_dir().join("updater-tools-test-check");

        let result = router(server.base_url.clone(), dir.clone())
            .execute("check_for_updates", ToolArgs::new())
            .await;
        server.join();

        assert!(result.success, "{}", result.message);
        assert_eq!(result.data.unwrap()["latest_version"], "2.0.0");
    }

    #[tokio::test]
    async fn check_for_updates_reports_up_to_date() {
        let server = MockServer::respond_once(
            200,
            "application/json",
            manifest_json("1.0.0", "http://x/y.tar.gz"),
        );
        let dir = std::env::temp_dir().join("updater-tools-test-uptodate");

        let result = router(server.base_url.clone(), dir.clone())
            .execute("check_for_updates", ToolArgs::new())
            .await;
        server.join();

        assert!(result.success, "{}", result.message);
        assert_eq!(result.data.unwrap()["update_available"], false);
    }

    #[tokio::test]
    async fn download_update_writes_the_file_when_available() {
        let dir = std::env::temp_dir().join("updater-tools-test-download");
        std::fs::remove_dir_all(&dir).ok();

        // Two requests happen inside one `execute` call: the manifest
        // check, then the artifact download -- each needs its own mock
        // server (MockServer serves exactly one request).
        let artifact_server =
            MockServer::respond_once(200, "application/gzip", b"archive bytes".to_vec());
        let download_url = format!("{}/ai-assistant-2.0.0.tar.gz", artifact_server.base_url);
        let manifest_server = MockServer::respond_once(
            200,
            "application/json",
            manifest_json("2.0.0", &download_url),
        );

        let result = router(manifest_server.base_url.clone(), dir.clone())
            .execute("download_update", ToolArgs::new())
            .await;
        manifest_server.join();
        artifact_server.join();

        assert!(result.success, "{}", result.message);
        assert!(dir.join("ai-assistant-2.0.0.tar.gz").exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn download_update_is_a_no_op_when_already_current() {
        let dir = std::env::temp_dir().join("updater-tools-test-noop");
        std::fs::remove_dir_all(&dir).ok();

        let server = MockServer::respond_once(
            200,
            "application/json",
            manifest_json("1.0.0", "http://x/y.tar.gz"),
        );
        let result = router(server.base_url.clone(), dir.clone())
            .execute("download_update", ToolArgs::new())
            .await;
        server.join();

        assert!(result.success, "{}", result.message);
        assert!(!dir.exists(), "nothing should have been written");
    }

    #[tokio::test]
    async fn both_tools_are_registered() {
        let dir = std::env::temp_dir().join("updater-tools-test-registered");
        assert_eq!(
            router("http://127.0.0.1:1".to_string(), dir)
                .registry()
                .list(),
            vec!["check_for_updates", "download_update"]
        );
    }
}
