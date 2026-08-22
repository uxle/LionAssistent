//! Local-network remote control and live notifications for a paired
//! mobile companion (Development_plan Phase 28 "Mobile Companion":
//! "Android App", "Remote Control", "Notifications").
//!
//! ## "Android App": a PWA, not a native APK, and why that's not a
//! ## downgrade in this sandbox
//!
//! Confirmed empirically before deciding anything, not assumed: this
//! sandbox cannot build a native Android app. `rustc --print target-list`
//! does list `aarch64-linux-android` and friends, but no target's
//! standard library is actually installed (`rustc --print sysroot`
//! shows only `x86_64-unknown-linux-gnu` present), `rustup` -- the only
//! way to fetch one -- isn't available, and Google's own distribution
//! servers are outside this sandbox's allowed egress list, confirmed
//! directly (`curl dl.google.com` returns `403`, not a timeout or DNS
//! failure). A real cross-compile attempt against
//! `--target aarch64-linux-android` was run and failed exactly as that
//! predicts (`can't find crate for std`). This is the same category of
//! wall Phase 20 already hit and handled the same way for Flatpak
//! (network wall) and Snap (no tooling): name it precisely, don't
//! pretend around it, and build what's genuinely real instead.
//!
//! What's real instead: [`assets`] serves a small, dependency-free
//! Progressive Web App -- plain HTML/CSS/JS, no bundler, no npm, no
//! framework -- that pairs with this assistant, lists and runs tools,
//! and shows live notifications. A PWA installs to a phone's home
//! screen (`manifest.json`'s `display: standalone`) on both Android
//! and iOS with no app-store distribution or platform SDK at all, and
//! every line of it is real, checked-in, and tested (see [`assets`]'s
//! own test module) rather than a spec description of an APK this
//! sandbox cannot produce. It is a genuinely different artifact from a
//! native app -- no background execution when the tab isn't open, no
//! platform-specific APIs -- and that trade-off is named here, not
//! discovered later by someone expecting an `.apk` in `packaging/`.
//!
//! ## "Notifications": live-while-connected, not cloud push
//!
//! See [`notify`]'s module docs for why this phase's notifications are
//! SSE-while-connected rather than OS-level push-while-closed: the
//! latter needs a cloud relay (FCM/APNs) that PRD SS3's own
//! non-goal #3 rules out for this assistant.
//!
//! ## "Remote Control": the same permission pipeline, a different door
//!
//! See [`pairing`]'s module docs for the pairing threat model and
//! [`server`]'s for how `/tools/execute` reaches `ToolRouter::execute`
//! -- the same validate/permission-check/timeout pipeline any local
//! caller already goes through, so a paired remote device is granted
//! no more trust than a local one, only a different transport to reach
//! the identical gate.
//!
//! ## Deliberately not built this phase
//!
//! - **A model-callable "pair a new device" tool.** Pairing is a
//!   security boundary a *human* crosses deliberately, not a decision
//!   an LLM should ever be able to make on the model's own initiative
//!   -- unlike `mcp`'s "no connect tool" gap (a structural limitation
//!   of `Tool::execute`'s signature), this one is a deliberate design
//!   stance: even if the plumbing were trivial, this would still not
//!   be a tool.
//! - **TLS.** This phase's server is plain HTTP, intended for
//!   loopback-only by default (`config::RemoteControlConfig`) or a
//!   trusted LAN if a user explicitly opts in. A real
//!   internet-exposed deployment would need it; that's a materially
//!   different threat model than "a phone on the same home WiFi",
//!   which is the one this phase actually targets (see `pairing`'s
//!   module docs).
//! - **Wiring into `app`.** `app` does not yet depend on
//!   `tool-framework` itself, or on any other tool-providing crate
//!   (`search`, `rag`, `linux-services`, `mcp`, ...) -- this phase
//!   doesn't reach ahead of that.

pub mod assets;
pub mod notify;
pub mod pairing;
pub mod server;

pub use notify::NotificationRelay;
pub use pairing::{Device, DeviceStore, PairingCode, PairingError, PairingManager, RateLimiter};
pub use server::{serve, SharedState};
