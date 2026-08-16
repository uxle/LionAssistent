//! Security tests (TDS SS10: "Permission bypass attempts. Invalid tool
//! inputs. Plugin isolation. File access restrictions."), investigated
//! against this workspace's actual code rather than asserted. Plugin
//! isolation lives in `cross_crate_flows.rs` (it's as much a
//! cross-crate-cooperation story as a security one); the other three
//! categories are here.
//!
//! Three of these tests document real, honest findings about the current
//! security *posture* rather than bugs to fix -- see each test's doc
//! comment. Writing them down as passing, checked assertions rather than
//! prose matches this project's own stated principle (docs/ROADMAP.md,
//! repeatedly): never assert a security property without having
//! verified it. Phase 23 added two new `ReadOnly` path-taking tools
//! (`ocr`, `vision`) that inherit `read_file`'s existing unscoped-path
//! finding, plus one `Moderate` tool (`screenshot`) that was specifically
//! checked *not* to share it.

use common::PermissionLevel;
use permission_manager::{AlwaysDeny, PermissionManager, SecurityRules};
use std::sync::Arc;
use std::time::Duration;
use tool_framework::{ResultStatus, ToolArgs, ToolRegistry, ToolRouter};

/// **Finding, not a bug**: `ToolRegistry::get` hands back a plain
/// `Arc<dyn Tool>`. Nothing about that type stops a caller from calling
/// `.execute()` on it directly, skipping schema validation, the
/// permission check, timeout enforcement, and audit logging entirely --
/// all of which live in `ToolRouter::execute`, not on `Tool` itself.
/// `DangerousStubTool` requires `Administrator` and would be denied by
/// any router configured below that; called directly, it just runs.
///
/// This isn't a flaw in the type system so much as a fact worth being
/// explicit about: the router is the *only* safe entry point, by
/// convention, not by a compiler-enforced guarantee. Anything with a
/// `ToolRegistry`/`Arc<dyn Tool>` reference -- host code, a future
/// in-process plugin with unusually deep access -- must go through
/// `ToolRouter::execute`, never call `.execute()` itself. Worth
/// remembering if a future refactor ever hands out raw tool references
/// more widely than it does today.
#[tokio::test]
async fn calling_tool_execute_directly_bypasses_every_router_safety_check() {
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(tool_framework::stubs::DangerousStubTool))
        .unwrap();

    // Through the router, configured well below Administrator: denied.
    let router = ToolRouter::new(registry, PermissionLevel::Safe);
    let via_router = router.execute("dangerous-stub", ToolArgs::new()).await;
    assert_eq!(via_router.status, ResultStatus::PermissionDenied);

    // The exact same tool, retrieved from the exact same registry, but
    // executed directly: runs anyway. No permission check ever happens
    // because `Tool::execute` doesn't know about permissions at all --
    // that's `ToolRouter`'s job, and it was never asked.
    let tool = router.registry().get("dangerous-stub").unwrap();
    let outcome = tool.execute(&ToolArgs::new());
    assert!(outcome.success, "direct execute() is not gated at all");
}

/// **Finding, not a bug**: `linux_services::files`/`ReadFileTool` has no
/// path scoping of its own (confirmed by reading `files.rs` -- there is
/// no allowlist, no canonicalization-and-prefix-check, nothing). That's
/// intentional, deferred to `sandbox::FilesystemSandbox`'s process-wide
/// Landlock restriction (Phase 16) as the actual defense-in-depth layer
/// -- which that crate's own doc comment already documents as **not
/// enforced** in this exact kind of sandbox, and which nothing in this
/// workspace currently applies at process startup anyway (`app`'s
/// `main` doesn't call it -- see docs/ROADMAP.md).
///
/// So: today, absent that still-unapplied process-wide restriction, the
/// *only* thing standing between a tool call and any path the OS-level
/// user can read is `read_file`'s `PermissionLevel::ReadOnly`
/// classification -- which, by design (Archd SS7 Level 0), runs without
/// ever asking for confirmation. This test proves that concretely with a
/// real file well outside any notion of an "app data directory",
/// through the real router, with default security rules and a
/// confirmation handler that would refuse anything it's actually asked
/// about.
#[tokio::test]
async fn read_file_reaches_paths_outside_any_sandboxed_root_without_confirmation() {
    let outside_path = "/etc/hostname"; // universally present, harmless to read, nowhere near this app's own data directory
    assert!(
        std::path::Path::new(outside_path).exists(),
        "test assumption: {outside_path} exists in this environment"
    );

    let pm = PermissionManager::in_memory(SecurityRules::default(), Box::new(AlwaysDeny)).unwrap();
    let mut registry = ToolRegistry::new();
    linux_services::tools::register_all(&mut registry).unwrap();
    let router =
        ToolRouter::new(registry, PermissionLevel::ReadOnly).with_permission_checker(Arc::new(pm));

    let mut args = ToolArgs::new();
    args.insert("path".into(), serde_json::json!(outside_path));
    let result = router.execute("read_file", args).await;

    assert!(
        result.success,
        "expected read_file to succeed unconfirmed on a real system path: {}",
        result.message
    );
}

/// Same finding as the test above, extended to Phase 23's two other
/// `ReadOnly`, path-taking tools: `ocr::ExtractTextFromImageTool` and
/// `vision::DescribeImageTool` were deliberately given the same
/// `ReadOnly` level `read_file` has (see each tool's own doc comment for
/// that reasoning), which means they inherit the identical
/// characteristic -- no path scoping of their own, same unapplied
/// `sandbox::FilesystemSandbox` gap as the actual intended defense.
/// Worth confirming that held for real rather than assuming consistency
/// with `read_file` implies it: a tool that decodes a file (image
/// formats, in both these cases) has more surface for something to
/// behave differently on the way to "runs without confirmation" than a
/// straight byte read does.
#[tokio::test]
async fn ocr_and_vision_tools_share_read_files_unscoped_path_access() {
    let dir = std::env::temp_dir().join("integration-test-ocr-vision-unscoped");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("outside.png");
    let img = image::RgbImage::from_fn(4, 4, |_, _| image::Rgb([9, 9, 9]));
    image::DynamicImage::ImageRgb8(img).save(&path).unwrap();

    let ocr_pm =
        PermissionManager::in_memory(SecurityRules::default(), Box::new(AlwaysDeny)).unwrap();
    let mut ocr_registry = ToolRegistry::new();
    ocr::tools::register_all(&mut ocr_registry).unwrap();
    let ocr_router = ToolRouter::new(ocr_registry, PermissionLevel::ReadOnly)
        .with_permission_checker(Arc::new(ocr_pm));

    let mut ocr_args = ToolArgs::new();
    ocr_args.insert("path".into(), serde_json::json!(path.to_str().unwrap()));
    let ocr_result = ocr_router
        .execute("extract_text_from_image", ocr_args)
        .await;
    assert!(
        ocr_result.success,
        "expected extract_text_from_image to succeed unconfirmed on a path outside any \
         sandboxed root: {}",
        ocr_result.message
    );

    let vision_pm =
        PermissionManager::in_memory(SecurityRules::default(), Box::new(AlwaysDeny)).unwrap();
    let mut vision_registry = ToolRegistry::new();
    vision::tools::register_all(&mut vision_registry).unwrap();
    let vision_router = ToolRouter::new(vision_registry, PermissionLevel::ReadOnly)
        .with_permission_checker(Arc::new(vision_pm));

    let mut vision_args = ToolArgs::new();
    vision_args.insert("path".into(), serde_json::json!(path.to_str().unwrap()));
    let vision_result = vision_router.execute("describe_image", vision_args).await;
    assert!(
        vision_result.success,
        "expected describe_image to succeed unconfirmed on a path outside any sandboxed root: {}",
        vision_result.message
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// `capture_screenshot`, by contrast, is deliberately `Moderate` (see
/// `screenshot::tools::CaptureScreenshotTool`'s doc comment) -- this
/// test is the negative case proving that distinction actually holds at
/// the router, not just in the permission-level constant. Unlike the
/// `ReadOnly` tools above, `AlwaysDeny` here really is asked, and really
/// does get to say no.
#[tokio::test]
async fn unlike_read_file_and_ocr_and_vision_screenshot_capture_actually_asks_first() {
    let pm = PermissionManager::in_memory(SecurityRules::default(), Box::new(AlwaysDeny)).unwrap();
    let mut registry = ToolRegistry::new();
    screenshot::tools::register_all(&mut registry).unwrap();
    let router =
        ToolRouter::new(registry, PermissionLevel::ReadOnly).with_permission_checker(Arc::new(pm));

    let result = router.execute("capture_screenshot", ToolArgs::new()).await;
    assert_eq!(
        result.status,
        ResultStatus::PermissionDenied,
        "capture_screenshot must not run unconfirmed the way read_file/ocr/vision do"
    );
}

/// Not every large or unusual input is a path-traversal or type-mismatch
/// case -- some are simply large. `InputSchema::validate` and
/// `Tool::execute` must handle a very large string and a
/// many-thousand-element array in bounded time, not hang or panic.
#[tokio::test]
async fn oversized_inputs_are_handled_promptly_not_hung_or_panicked() {
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(tool_framework::stubs::EchoTool))
        .unwrap();
    let router = ToolRouter::new(registry, PermissionLevel::Safe);

    let huge_text = "x".repeat(4 * 1024 * 1024); // 4MB in a single field
    let mut args = ToolArgs::new();
    args.insert("text".into(), serde_json::json!(huge_text));

    let result = tokio::time::timeout(Duration::from_secs(5), router.execute("echo", args))
        .await
        .expect("a 4MB string must not hang validation or execution");
    assert_eq!(result.status, ResultStatus::Ok);

    let huge_array: Vec<String> = (0..200_000).map(|i| i.to_string()).collect();
    let schema = tool_framework::InputSchema::new(vec![tool_framework::ParamSchema::required(
        "items",
        tool_framework::ParamKind::StringArray,
    )]);
    let mut array_args = ToolArgs::new();
    array_args.insert("items".into(), serde_json::json!(huge_array));
    // `validate` is synchronous and never awaits internally, so wrapping
    // the call itself in `tokio::time::timeout` wouldn't actually bound
    // it -- a non-yielding future runs to completion before the timeout
    // gets a chance to fire. Running it on a separate blocking thread and
    // timing out *that* is the same real-timeout pattern
    // `ToolRouter::execute` itself uses for synchronous tool code.
    let validated = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::task::spawn_blocking(move || schema.validate(&array_args)),
    )
    .await
    .expect("validating a 200k-element array must not hang")
    .expect("validation task must not panic");
    assert!(validated.is_ok());
}

/// Adversarial repetition, not just a single denial: calling
/// `check_and_log` many times in a row for the same `Destructive` tool
/// must deny *every single time* when nothing was ever approved or
/// remembered -- no off-by-one, no accidental state leakage between
/// calls that flips an eventual call to `true`.
#[tokio::test]
async fn repeated_denials_never_drift_into_an_accidental_allow() {
    let pm = PermissionManager::in_memory(SecurityRules::default(), Box::new(AlwaysDeny)).unwrap();
    for i in 0..500 {
        let approved = pm
            .check_and_log("delete_file", PermissionLevel::Destructive, "delete a file")
            .unwrap();
        assert!(!approved, "call #{i} unexpectedly approved");
    }
}
