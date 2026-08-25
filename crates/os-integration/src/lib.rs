//! AI Operating System Integration (Development_plan Phase 30 --
//! the last phase on the roadmap as originally laid out: "Deep Linux
//! Integration", "LionOS Integration", "System-wide Assistant",
//! "Desktop Automation", "Native AI APIs", "Full OS Control Layer").
//!
//! Three of these six bullets are real, tested capability in this
//! crate. The other three needed more than code -- they needed an
//! honest look at what the words themselves actually mean before
//! deciding what, if anything, to build. That look is written out in
//! full below, in the same place the code would otherwise be, because
//! for those three bullets the reasoning *is* the deliverable.
//!
//! ## What's real
//!
//! - [`windows`] -- **Desktop Automation**: real `wmctrl`/`xdotool`
//!   window management, fully tested against a real Xvfb + fluxbox +
//!   xterm stack (confirmed empirically before writing a line of
//!   implementation, not assumed to work because the binaries exist --
//!   `tests/window_management_integration.rs`).
//! - [`systemd`] -- **Deep Linux Integration**: real `systemctl --user`
//!   service management. Verification is real but partial, and said so
//!   precisely rather than glossed over: this sandbox is not booted
//!   with systemd as PID 1, so there is no running session bus for the
//!   happy path to reach -- see that module's own docs for the exact,
//!   confirmed failure text and why this is the same category of gap
//!   Phase 20 named for Flatpak/Snap and Phase 28 named for the
//!   Android SDK, not a new kind of shortcut.
//! - [`dbus_api`] -- **Native AI APIs**: a real D-Bus service exposing
//!   `ToolRouter` to other local applications, tested against a real
//!   session bus this crate's own tests start
//!   (`tests/dbus_api_integration.rs`) -- the standard Linux IPC
//!   mechanism, distinct from `remote-control`'s HTTP server (Phase
//!   28), which exists for a different audience (a phone on the LAN,
//!   with no D-Bus access at all) rather than a replacement for it.
//! - `assets/ai-assistant.service` -- **System-wide Assistant**: a
//!   real systemd user-service unit file for autostart. Not verifiable
//!   end to end here for the same PID-1 reason `systemd`'s own gap
//!   exists, but it's a real, checked-in artifact a user can
//!   `systemctl --user enable` on an actual desktop Linux system, not
//!   a description of one.
//!
//! ## LionOS Integration: not built, because there is nothing to
//! ## integrate with
//!
//! This is not a sandbox limitation, a missing dependency, or a
//! blocked network call -- every other "not built this phase" in this
//! workspace's history has been one of those. LionOS does not exist
//! anywhere as a real, specified, or even prototyped target. Searching
//! this project's own documents confirms it: PRD SS2 goal 7 calls this
//! assistant a foundation "for a **future** custom desktop OS
//! assistant"; Archd SS18 and DIG SS19 both list "**future** LionOS
//! integration" among planned extensions; nowhere in PRD, Archd, TDS,
//! Apis, DBDMS, TCLIS, Ai_reasoning, DIG, UI-UX, Plugins, or Mms is
//! there a LionOS API, IPC mechanism, kernel interface, system call
//! convention, or specification of any kind. There is no codebase, no
//! binary, no protocol, no version number -- nothing this crate could
//! link against, shell out to, or even name precisely the way
//! `systemd`'s own gap names an exact, real error message.
//!
//! Writing "LionOS integration" code under these conditions has
//! exactly two honest shapes, and both are refused here:
//!
//! 1. **Invent a fictional LionOS API and implement against it.** This
//!    would produce code that looks real -- compiles, has tests, has
//!    documentation -- while integrating with nothing that exists. It
//!    is the single clearest violation imaginable of this workspace's
//!    own standing rule, present in every phase before this one: every
//!    capability is confirmed against something real before being
//!    claimed. A fabricated target is the opposite of that, dressed up
//!    to look like the same thing.
//! 2. **Build a generic plugin/extension point and call it "LionOS
//!    readiness."** This would not be a lie in the same way, but it
//!    would be a different kind of overclaim: naming a speculative
//!    extension mechanism after a specific, named future system
//!    implies knowledge of that system's actual shape that this
//!    project does not have. `mcp`, `plugin-sdk`, and this crate's own
//!    `dbus_api` already are the extension points a real LionOS
//!    integration would most likely build on, whenever LionOS is real
//!    enough to integrate with -- they don't need a second, redundant,
//!    speculatively-named layer on top to be "ready."
//!
//! So: nothing under this heading. When LionOS exists as a real
//! target -- with something concrete to read, run, or connect to --
//! integrating with it is new work for a new, not-yet-written roadmap
//! entry, not a line item this phase could have completed by writing
//! more code today.
//!
//! ## "Full OS Control Layer": named, and declined as written
//!
//! Read literally, this bullet asks for unrestricted control over the
//! operating system. That is not an omission to fill in -- it is a
//! direct contradiction of this project's own foundational
//! requirements, present since PRD SS3's very first two non-goals:
//! "No unrestricted autonomous root access" and "No fully general
//! 'run anything' shell control without safety checks." Every phase
//! since Phase 9 has built toward the opposite of a full control
//! layer: a fail-closed `PermissionManager` (Phase 9), five bounded
//! permission levels with Administrator actions off by default (PRD
//! SS6.6, confirmed still enforced over a remote transport as
//! recently as Phase 28's own tests), and a rule -- restated in
//! `mcp`'s, `remote-control`'s, and this crate's own `dbus_api`
//! module docs -- that a new transport reaching this assistant is
//! never a new grant of trust, only a new door to the same gate.
//! Implementing this bullet as written would mean unwinding all of
//! that, in the very last phase, for the sake of completing a
//! checklist item.
//!
//! What this phase does instead, and what "full" honestly means here:
//! the *broadest tool coverage* this project ships -- window
//! management and systemd service control added in this crate, on top
//! of everything Phases 8, 26, and 27 already contributed -- with
//! every single one of those tools still passing through the exact
//! same `ToolRouter::execute` pipeline as the first tool this
//! workspace ever registered. Broader coverage, identical trust model.
//! Not a control layer with no ceiling; a taller building on the same
//! foundation.

pub mod dbus_api;
pub mod systemd;
pub mod tools;
pub mod windows;

pub use tools::register_all;
