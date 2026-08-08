//! `network` -- HTTP client, downloads, and the "online capabilities"
//! Development_plan Phase 14 asks for.
//!
//! One synchronous [`HttpClient`] (see `client`'s doc comment on why
//! synchronous, matching every other stateful crate in this workspace)
//! provides `GET`/`POST`+JSON/streaming download, backed by `ureq` and
//! the OS's own certificate trust store rather than a compiled-in root
//! list -- see [`client`]'s doc comment for why that distinction turned
//! out to matter in practice, not just in theory.
//!
//! This crate is deliberately *not* exposed as an assistant tool on its
//! own. TCLIS SS4's tool categories are all about local system control;
//! a general "fetch any URL" capability reachable from natural language
//! is a materially different, broader security surface (PRD SS3's "no
//! cloud dependency for the core assistant," Archd SS3 "secure by
//! default") that nothing in this project's specs actually calls for.
//! What *is* built on top: the `updater` crate uses this client for one
//! narrow, concrete, low-risk purpose (checking/downloading application
//! updates from one pre-configured URL, never one an LLM supplies) --
//! see that crate's docs. "Optional cloud features" (Development_plan
//! Phase 14, PRD SS3's non-goal notwithstanding -- it says no *required*
//! cloud dependency, not that opt-in ones are off the table forever) get
//! their enabling infrastructure from this crate today; a concrete
//! feature built on it is future work, per MMS SS19's own "optional
//! cloud model providers (user-configurable)" being listed there as
//! exactly that -- future, not this phase.

pub mod client;
pub mod testing;

pub use client::{DownloadOutcome, HttpClient, HttpResponse};
