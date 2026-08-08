//! Standalone, manually-run demonstration of [`sandbox::FilesystemSandbox`].
//!
//! **Not part of `cargo test`.** `restrict_current_process` is
//! process-wide and irreversible -- once called, the *entire remaining
//! process* can only see the paths it was given, permanently. Running
//! that from inside a shared test binary (where Rust's default runner
//! executes many tests as threads in one process) would risk breaking
//! every test that runs afterward, in that binary, for the rest of that
//! process's life. A `cargo example` is its own separate process, so
//! it's the safe place to actually exercise this call.
//!
//! Run with: `cargo run -p sandbox --example filesystem_sandbox_demo`
//!
//! On a real Linux desktop with Landlock actually enforced, this prints
//! `Enforced` and the blocked read fails. In the sandbox this workspace
//! was developed in, it honestly prints `NotEnforced`, and the "blocked"
//! read still succeeds -- see `src/filesystem.rs`'s module doc comment.

use sandbox::{EnforcementStatus, FilesystemSandbox};
use std::fs;

fn main() {
    let allowed_dir = std::env::temp_dir().join("landlock-demo-allowed");
    let blocked_dir = std::env::temp_dir().join("landlock-demo-blocked");
    fs::create_dir_all(&allowed_dir).unwrap();
    fs::create_dir_all(&blocked_dir).unwrap();
    fs::write(allowed_dir.join("ok.txt"), "this should stay readable").unwrap();
    fs::write(
        blocked_dir.join("secret.txt"),
        "this should become unreadable",
    )
    .unwrap();

    println!(
        "Restricting this process to read-only access under {}...",
        allowed_dir.display()
    );

    let status = FilesystemSandbox::new()
        .allow_read(&allowed_dir)
        .restrict_current_process()
        .expect("restrict_current_process failed outright (not just unenforced)");

    match status {
        EnforcementStatus::Enforced => println!("Kernel confirmed: rules are ENFORCED."),
        EnforcementStatus::NotEnforced => {
            println!("Kernel reports: rules are NOT enforced in this environment.")
        }
    }

    let allowed_read = fs::read_to_string(allowed_dir.join("ok.txt"));
    let blocked_read = fs::read_to_string(blocked_dir.join("secret.txt"));

    println!("Read from allowed dir: {:?}", allowed_read.is_ok());
    println!("Read from blocked dir: {:?}", blocked_read.is_ok());

    match status {
        EnforcementStatus::Enforced if allowed_read.is_ok() && blocked_read.is_err() => {
            println!("Behaves exactly as EnforcementStatus::Enforced promises.");
        }
        EnforcementStatus::NotEnforced if blocked_read.is_ok() => {
            println!("Behaves exactly as EnforcementStatus::NotEnforced promises: not actually protected here.");
        }
        _ => println!("Unexpected combination -- worth investigating before trusting this module."),
    }
}
