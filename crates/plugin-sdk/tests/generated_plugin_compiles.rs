//! Proves `generator::generate_plugin`'s output is a genuinely
//! compiling, genuinely passing plugin crate -- not just
//! template text that looks plausible. Runs the real, installed
//! `cargo build` and `cargo test` against real generated files on
//! disk, the same standard `coding-agent`'s own scratch-crate tests
//! already set (`diagnostics.rs`'s `run_cargo_diagnostics` tests) for
//! "a tool that produces Cargo output must be proven against a real
//! `cargo` invocation."
//!
//! Generates into a real, temporary directory directly under this
//! workspace's own `crates/` -- not `/tmp` -- specifically so the
//! generated `Cargo.toml`'s relative path dependencies (`../common`,
//! `../tool-framework`, `../plugin-manager`) resolve exactly the way
//! they would for a real plugin author following this same layout.
//! The generated crate's own `[workspace]` table (see
//! `templates/Cargo.toml.template`) keeps it fully independent of the
//! main workspace's `Cargo.lock` and `target/` despite living inside
//! its directory tree -- this test never touches either.

use plugin_sdk::{generate_plugin, PluginSpec};
use std::path::PathBuf;
use std::process::Command;

/// A directory under this workspace's real `crates/`, cleaned up
/// (including its own nested `target/`) when dropped -- guarantees
/// cleanup even if an assertion partway through this test panics,
/// learning from this project's own earlier lesson about leftover
/// `target/` directories exhausting disk space during verification.
struct GeneratedPluginGuard(PathBuf);

impl Drop for GeneratedPluginGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn workspace_crates_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR is .../ai-assistant/crates/plugin-sdk; its
    // parent is .../ai-assistant/crates -- exactly where a real plugin
    // is meant to live alongside common/tool-framework/plugin-manager.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("plugin-sdk is always under crates/")
        .to_path_buf()
}

/// A `target/` directory shared across every test in this file via
/// `CARGO_TARGET_DIR`, deliberately outside any individual generated
/// plugin's own directory so it survives each plugin's own cleanup --
/// and deliberately *not* removed by this file at all, matching the
/// same reasoning `~/.cargo/registry`'s own cache is never wiped
/// automatically: a build cache's entire purpose is to persist across
/// runs. (`.gitignore` excludes `crates/_generated_*/` so this and any
/// per-test directory a hard crash left behind never end up
/// committed.) Each generated plugin is still its own independent `[workspace]`
/// (see `templates/Cargo.toml.template`'s doc comment) with its own
/// `Cargo.lock`, but since every one of them depends on the exact same
/// pinned versions of common/tool-framework/plugin-manager and their
/// transitive dependencies, Cargo's own build cache (keyed by crate,
/// version, features, and profile -- not by which top-level workspace
/// asked for it) reuses already-compiled dependency artifacts across
/// them. This cuts this file's total real-`cargo`-invocation time
/// substantially without changing what's actually being proven: two
/// separate, real `cargo build` runs against two separately generated
/// plugins, sharing a dependency cache the exact same way two
/// independent real-world plugin authors' machines would, if they'd
/// both already built the same dependency versions once before.
fn shared_target_dir() -> PathBuf {
    workspace_crates_dir().join("_generated_plugin_test_shared_target")
}

#[test]
fn a_generated_plugin_genuinely_compiles_and_passes_its_own_tests() {
    let target_dir =
        workspace_crates_dir().join(format!("_generated_plugin_test_{}", std::process::id()));
    // In case a previous run of this test crashed hard enough to skip
    // its own Drop cleanup.
    std::fs::remove_dir_all(&target_dir).ok();
    let _guard = GeneratedPluginGuard(target_dir.clone());

    let mut spec = PluginSpec::new(
        "generated-proof-plugin",
        "Proves plugin_sdk's generator produces real, compiling code",
        "plugin-sdk integration test",
    )
    .unwrap();
    spec.tool_name = "prove_it_works".to_string();
    spec.tool_struct_name = "ProveItWorksTool".to_string();

    generate_plugin(&spec, &target_dir).expect("generation itself must succeed");

    assert!(target_dir.join("Cargo.toml").is_file());
    assert!(target_dir.join("manifest.toml").is_file());
    assert!(target_dir.join("src/lib.rs").is_file());

    // The real, standards-correct proof: an actual `cargo build`
    // against the actual generated Cargo.toml, exactly as a plugin
    // author would run it themselves.
    let build = Command::new("cargo")
        .arg("build")
        .arg("--manifest-path")
        .arg(target_dir.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", shared_target_dir())
        .output()
        .expect("cargo must be on PATH -- every other test in this workspace already assumes this");

    assert!(
        build.status.success(),
        "generated plugin failed to build:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    // And the generated tests (the ones templates/lib.rs.template
    // itself writes) must actually pass, not merely exist as text.
    let test = Command::new("cargo")
        .arg("test")
        .arg("--manifest-path")
        .arg(target_dir.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", shared_target_dir())
        .output()
        .expect("cargo must be on PATH");

    let test_stdout = String::from_utf8_lossy(&test.stdout);
    assert!(
        test.status.success(),
        "generated plugin's own tests failed:\nstdout: {test_stdout}\nstderr: {}",
        String::from_utf8_lossy(&test.stderr)
    );
    assert!(
        test_stdout.contains("test result: ok"),
        "expected at least one passing test in generated output, got:\n{test_stdout}"
    );
    assert!(
        test_stdout.contains("plugin_reports_the_declared_metadata")
            || test_stdout.contains("2 passed"),
        "expected the generator's own two template tests to run:\n{test_stdout}"
    );
}

#[test]
fn a_generated_plugin_with_a_custom_permission_level_still_compiles() {
    let target_dir = workspace_crates_dir().join(format!(
        "_generated_plugin_test_perm_{}",
        std::process::id()
    ));
    std::fs::remove_dir_all(&target_dir).ok();
    let _guard = GeneratedPluginGuard(target_dir.clone());

    let mut spec = PluginSpec::new(
        "generated-perm-plugin",
        "Proves a non-default permission level still compiles",
        "plugin-sdk integration test",
    )
    .unwrap();
    spec.permission_level = tool_framework::PermissionLevel::Moderate;

    generate_plugin(&spec, &target_dir).unwrap();

    let build = Command::new("cargo")
        .arg("build")
        .arg("--manifest-path")
        .arg(target_dir.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", shared_target_dir())
        .output()
        .expect("cargo must be on PATH");

    assert!(
        build.status.success(),
        "generated plugin with PermissionLevel::Moderate failed to build:\nstderr: {}",
        String::from_utf8_lossy(&build.stderr)
    );
}
