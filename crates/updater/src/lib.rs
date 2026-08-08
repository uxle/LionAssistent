//! `updater` -- version checking and update downloads (Development_plan
//! Phase 14 "Updates" module), built on `network`'s [`network::HttpClient`].
//!
//! [`manifest::check_for_update`] compares the running app's version
//! against a small JSON manifest at a pre-configured URL;
//! [`download::download_update`] fetches what it points at. Neither
//! installs anything -- see [`download`]'s doc comment for why
//! self-replacing the running binary is out of scope here, deliberately,
//! not just unfinished.
//!
//! [`tools`] exposes both as real, narrow `Tool`s (`check_for_updates`,
//! `download_update`) -- neither accepts a URL from a caller; the
//! manifest URL is fixed at construction time by whatever composes the
//! tool registry, never something an LLM's tool call can redirect.

pub mod download;
pub mod manifest;
pub mod tools;

pub use download::download_update;
pub use manifest::{check_for_update, UpdateManifest};
