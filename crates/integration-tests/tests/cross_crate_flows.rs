//! Integration tests spanning combinations of crates that no existing
//! test -- crate-local or otherwise -- already exercises together. See
//! `src/lib.rs` for why this crate exists and what it deliberately does
//! *not* try to prove (the LLM-driven tool-call extraction that still
//! needs a working `llm-runtime`/Candle).

use common::PermissionLevel;
use permission_manager::{AlwaysApproveForTestingOnly, PermissionManager, SecurityRules};
use plugin_manager::{
    Plugin, PluginManager, PluginManifest, PluginMetadata, PluginPermissionPolicy,
};
use std::sync::Arc;
use tool_framework::{
    ResultStatus, Tool, ToolArgs, ToolCategory, ToolOutcome, ToolRegistry, ToolRouter,
};

fn minimal_manifest(id: &str) -> PluginManifest {
    PluginManifest::from_toml(&format!(
        "id = {id:?}\nname = \"Test\"\nversion = \"1.0.0\"\nauthor = \"test\"\ndescription = \"test\"\n\
         license = \"MIT\"\nminimum_sdk_version = \"0.1.0\"\nsupported_platforms = [\"linux\"]\npermissions = []\n"
    ))
    .unwrap()
}

fn activated_plugin_manager(id: &str, instance: Box<dyn Plugin>) -> PluginManager {
    let mut manager = PluginManager::new();
    manager.register(minimal_manifest(id), instance).unwrap();
    manager
        .validate(id, &PluginPermissionPolicy::new())
        .unwrap();
    manager.activate(id).unwrap();
    manager
}

struct EchoPlugin;
impl Plugin for EchoPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: "test.echo-plugin".into(),
            name: "Echo Plugin".into(),
            version: "1.0.0".into(),
        }
    }
    fn initialize(&mut self) -> common::AppResult<()> {
        Ok(())
    }
    fn shutdown(&mut self) -> common::AppResult<()> {
        Ok(())
    }
    fn register_tools(&self) -> Vec<Arc<dyn Tool>> {
        vec![Arc::new(tool_framework::stubs::EchoTool)]
    }
}

/// A tool whose `execute()` panics -- standing in for a genuinely broken
/// or malicious plugin tool, the same role `SlowTool` (tool-framework's
/// own tests) plays for timeouts.
struct PanickingTool;
impl Tool for PanickingTool {
    fn name(&self) -> &str {
        "panicking-plugin-tool"
    }
    fn description(&self) -> &str {
        "Always panics. Proves the router isolates a plugin tool's panic the same way it would a built-in tool's."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(5)
    }
    fn input_schema(&self) -> tool_framework::InputSchema {
        tool_framework::InputSchema::default()
    }
    fn execute(&self, _args: &ToolArgs) -> ToolOutcome {
        panic!("deliberate panic from a plugin-contributed tool");
    }
}

struct PanickingPlugin;
impl Plugin for PanickingPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: "test.panicking-plugin".into(),
            name: "Panicking Plugin".into(),
            version: "1.0.0".into(),
        }
    }
    fn initialize(&mut self) -> common::AppResult<()> {
        Ok(())
    }
    fn shutdown(&mut self) -> common::AppResult<()> {
        Ok(())
    }
    fn register_tools(&self) -> Vec<Arc<dyn Tool>> {
        vec![Arc::new(PanickingTool)]
    }
}

fn router_from(tools: Vec<Arc<dyn Tool>>) -> ToolRouter {
    let mut registry = ToolRegistry::new();
    for tool in tools {
        registry.register(tool).unwrap();
    }
    ToolRouter::new(registry, PermissionLevel::Safe)
}

/// `plugin_manager::PluginManager::collect_tools` already proves it can
/// gather a `Vec<Arc<dyn Tool>>` from active plugins (that crate's own
/// tests). What's new here: pushing that output the rest of the way,
/// into a real `ToolRegistry` and executing it through a real
/// `ToolRouter`, exactly the path the `Plugin` trait's own doc comment
/// promises ("identical validation, permission checks, and timeout
/// enforcement ... No separate, weaker path") -- checked, not just
/// asserted in a comment.
#[tokio::test]
async fn a_plugin_contributed_tool_runs_through_the_real_router() {
    let manager = activated_plugin_manager("test.echo-plugin", Box::new(EchoPlugin));
    let router = router_from(manager.collect_tools());

    let mut args = ToolArgs::new();
    args.insert("text".into(), serde_json::json!("hello from a plugin"));
    let result = router.execute("echo", args).await;

    assert_eq!(result.status, ResultStatus::Ok);
    assert_eq!(result.message, "echo: hello from a plugin");
}

/// `plugin_manager::PluginManager::activate`'s own tests already prove a
/// panic during a plugin's `initialize()`/`shutdown()` is caught
/// (`catch_unwind`, marks the plugin `Failed`). That's a *different*
/// panic path from a plugin's tool panicking during `execute()`, which
/// runs through `tool_framework::ToolRouter::execute`'s `spawn_blocking`
/// instead -- untested, for a plugin-sourced tool specifically, until
/// now.
#[tokio::test]
async fn a_plugin_tools_panic_during_execution_is_isolated_not_a_crash() {
    let manager = activated_plugin_manager("test.panicking-plugin", Box::new(PanickingPlugin));
    let router = router_from(manager.collect_tools());

    let result = router
        .execute("panicking-plugin-tool", ToolArgs::new())
        .await;

    assert_eq!(result.status, ResultStatus::ExecutionFailed);
    assert!(!result.success);
    // The test process is still running to make this assertion at all --
    // that's the actual proof. The message content is a bonus check.
    assert!(result.message.contains("panicked"));
}

/// `assistant-core` (conversation), `tool-framework` + `permission-manager`
/// (a real, gated tool call), and `memory` (persistence) have never been
/// exercised in the same scenario before -- every phase since 7 has
/// explicitly noted that wiring assistant-core's live message flow to
/// real tool calls needs a working `llm-runtime` (Phase 4, blocked on
/// Candle) to extract concrete tool calls from a model's output. This
/// test does **not** change that -- it does not parse a tool call out of
/// `assistant-core`'s reply. What it does show: the four pieces that
/// *would* need to cooperate once that extraction exists already do,
/// today, at the data level -- a conversation session, a real permission
/// decision with its own audit trail, and a tool execution result can
/// all be produced side by side and persisted/inspected together without
/// any interface friction between the crates.
#[tokio::test]
async fn assistant_core_tool_execution_and_memory_cooperate_in_one_scenario() {
    // 1. A real conversation, independent of any tool call.
    let mut assistant =
        assistant_core::AssistantCore::new(Box::new(assistant_core::EchoResponseGenerator));
    let session_id = assistant.create_session("gemma-3-1b");
    let reply = assistant
        .process_message(session_id, "what's my disk usage?")
        .unwrap();
    assert!(reply.contains("pipeline OK"));

    // 2. A real tool call, gated by a real PermissionManager -- entirely
    // independent of assistant-core, the way it will keep being until
    // Phase 4 unblocks.
    let pm = PermissionManager::in_memory(
        SecurityRules::default(),
        Box::new(AlwaysApproveForTestingOnly),
    )
    .unwrap();
    let mut registry = ToolRegistry::new();
    linux_services::tools::register_all(&mut registry).unwrap();
    let router =
        ToolRouter::new(registry, PermissionLevel::ReadOnly).with_permission_checker(Arc::new(pm));
    let tool_result = router.execute("system_info", ToolArgs::new()).await;
    assert!(tool_result.success, "{}", tool_result.message);

    // 3. Persistence: the conversation session round-trips through a
    // real MemoryStore, the same one a live app would use.
    let memory_store =
        memory::MemoryStore::in_memory(Box::<embeddings::HashingEmbedder>::default()).unwrap();
    let session = assistant.session(session_id).unwrap();
    memory_store.save_session(session).unwrap();

    let sessions = memory_store.list_sessions().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, session_id);
}

/// Phase 23's first cross-crate flow: a real screenshot captured through
/// `screenshot`'s tool, handed as-is to `ocr`'s tool through the same
/// router -- proving the two new crates' output/input actually fit
/// together in practice (a captured PNG is something `ocr`'s `tesseract`
/// wrapper can actually open), not just that their types line up on
/// paper. Needs a real X11 display (Xvfb) the same way `screenshot`'s
/// own crate-level doc comment describes -- see that doc comment for the
/// exact setup this expects.
///
/// `Administrator` as `max_auto_level` (no `PermissionChecker`
/// configured) rather than a real `PermissionManager`: this test isn't
/// about permissions -- `capture_screenshot`'s `Moderate` level is
/// already separately proven in `security_adversarial.rs` and
/// `screenshot/tests/permission_integration.rs` -- so the simple
/// fallback that lets both a `Moderate` and a `ReadOnly` tool through
/// unconfirmed keeps this test focused on what it's actually checking.
#[tokio::test]
async fn a_real_captured_screenshot_flows_into_ocr_through_the_same_router() {
    let Ok(display) = std::env::var("DISPLAY") else {
        panic!(
            "DISPLAY is not set -- this test needs a real (or Xvfb) X11 server; see \
             screenshot's crate-level doc comment for setup"
        );
    };
    assert!(!display.is_empty());

    let mut registry = ToolRegistry::new();
    screenshot::tools::register_all(&mut registry).unwrap();
    ocr::tools::register_all(&mut registry).unwrap();
    let router = ToolRouter::new(registry, PermissionLevel::Administrator);

    let dir = std::env::temp_dir().join("integration-test-screenshot-to-ocr");
    let mut capture_args = ToolArgs::new();
    capture_args.insert(
        "output_dir".into(),
        serde_json::json!(dir.to_str().unwrap()),
    );
    let capture_result = router.execute("capture_screenshot", capture_args).await;
    assert_eq!(
        capture_result.status,
        ResultStatus::Ok,
        "{}",
        capture_result.message
    );
    let path = capture_result.data.unwrap()["path"]
        .as_str()
        .unwrap()
        .to_string();

    let mut ocr_args = ToolArgs::new();
    ocr_args.insert("path".into(), serde_json::json!(path));
    let ocr_result = router.execute("extract_text_from_image", ocr_args).await;
    assert_eq!(
        ocr_result.status,
        ResultStatus::Ok,
        "ocr must be able to open and process a real screenshot capture, even if it \
         recognizes no text in it: {}",
        ocr_result.message
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// Phase 25's first cross-crate flow: three real agents -- `research`,
/// `memory`, `security` -- cooperating through one `Coordinator` in a
/// single realistic scenario (find something, remember a note about it,
/// then check what risk the tools involved actually carry), rather than
/// each being proven only in isolation the way their own crate-local
/// tests already do. `code`/`planner` aren't part of this particular
/// scenario -- not because they're less real, just because a
/// find-then-remember-then-check-risk story doesn't naturally involve
/// them; each has its own solid crate-local coverage already.
#[tokio::test]
async fn research_memory_and_security_agents_cooperate_through_one_coordinator() {
    use agents::coordinator::{Coordinator, DispatchStatus};
    use agents::memory_agent::MemoryAgent;
    use agents::message::{AgentMessage, Priority};
    use agents::research_agent::ResearchAgent;
    use agents::security_agent::SecurityAgent;
    use embeddings::HashingEmbedder;
    use memory::MemoryStore;
    use search::{CommandSearchProvider, FileSearchProvider, MemorySearchProvider, SearchEngine};
    use std::sync::Mutex;

    let dir = std::env::temp_dir().join("cross-crate-multi-agent-flow");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("quarterly-report.txt"),
        "revenue increased due to the new agent architecture",
    )
    .unwrap();

    let memory_store = Arc::new(Mutex::new(
        MemoryStore::in_memory(Box::<HashingEmbedder>::default()).unwrap(),
    ));

    let mut tool_registry = ToolRegistry::new();
    linux_services::tools::register_all(&mut tool_registry).unwrap();
    coding_agent::tools::register_all(
        &mut tool_registry,
        Arc::new(Mutex::new(Box::new(assistant_core::EchoResponseGenerator)
            as Box<dyn assistant_core::ResponseGenerator>)),
    )
    .unwrap();

    let search_engine = Arc::new(SearchEngine::new(
        FileSearchProvider::new(vec![dir.clone()]),
        MemorySearchProvider::new(memory_store.clone()),
        CommandSearchProvider::new(&tool_registry, None).unwrap(),
    ));

    let mut coordinator = Coordinator::new();
    coordinator.register(Arc::new(ResearchAgent::new(search_engine)));
    coordinator.register(Arc::new(MemoryAgent::new(memory_store)));
    coordinator.register(Arc::new(SecurityAgent::new(Arc::new(tool_registry))));

    // Step 1: research finds the real file.
    let research_result = coordinator
        .dispatch(AgentMessage::new(
            "test",
            "research",
            serde_json::json!({"query": "quarterly-report", "limit": 5}),
            Priority::Normal,
        ))
        .await;
    assert_eq!(research_result.status, DispatchStatus::Ok);
    let hits = research_result.reply.unwrap().payload["hits"].clone();
    assert!(
        hits.as_array().unwrap().iter().any(|h| h["kind"] == "file"),
        "{hits:?}"
    );

    // Step 2: remember a note about what research found.
    let remember_result = coordinator
        .dispatch(AgentMessage::new(
            "test",
            "memory",
            serde_json::json!({
                "action": "remember",
                "category": "project",
                "title": "quarterly-report finding",
                "content": "found a quarterly report mentioning the new agent architecture",
                "importance": 4,
            }),
            Priority::Normal,
        ))
        .await;
    assert_eq!(remember_result.status, DispatchStatus::Ok);
    assert!(remember_result.reply.unwrap().payload["id"]
        .as_str()
        .is_some());

    // Step 3: check the real risk of the tools this scenario touched --
    // `read_file` (ReadOnly) and `run_diagnostics` (Destructive, Phase
    // 24's first Destructive-level tool) together.
    let security_result = coordinator
        .dispatch(AgentMessage::new(
            "test",
            "security",
            serde_json::json!({"tool_names": ["read_file", "run_diagnostics"]}),
            Priority::Normal,
        ))
        .await;
    assert_eq!(security_result.status, DispatchStatus::Ok);
    let security_payload = security_result.reply.unwrap().payload;
    assert_eq!(security_payload["highest_level"], "Destructive");
    assert_eq!(security_payload["requires_confirmation_count"], 1);

    std::fs::remove_dir_all(&dir).ok();
}

/// A genuine cross-*phase* flow, not just cross-crate within one phase:
/// `ocr` (Phase 23) extracts real text from a real generated image, that
/// text is written to a file, and `rag` (Phase 26) indexes and finds it
/// through a real `search_knowledge_base` call -- proving a Phase 23
/// capability's output is genuinely consumable as Phase 26 input, not
/// merely type-compatible on paper. Mirrors
/// `a_real_captured_screenshot_flows_into_ocr_through_the_same_router`'s
/// own reasoning for why this kind of test belongs here.
///
/// A real finding surfaced while building this test, worth keeping: the
/// first fixture image (a tight 500x100 canvas) had real tesseract
/// misread its first character and clip its last few -- genuine OCR
/// imprecision at an image's edges, not a bug anywhere in this
/// pipeline. See the fixture's own comment below for the fix and how it
/// was confirmed.
#[tokio::test]
async fn ocr_extracted_text_flows_into_the_knowledge_base_and_is_found_by_search() {
    let dir = std::env::temp_dir().join("cross-crate-ocr-to-rag");
    std::fs::create_dir_all(&dir).unwrap();

    let image_path = dir.join("fixture.png");
    let convert = std::process::Command::new("convert")
        // Wide canvas relative to the text, generous margin on every
        // side: a first attempt at 500x100 with this same text had
        // tesseract misread the leading "Pho" as "9" and clip the
        // trailing "rgy" -- a real OCR accuracy issue at the image's
        // edges, not a bug in this pipeline, confirmed by hand (a
        // 700x150 canvas with more margin reads perfectly) before this
        // fixture's dimensions were fixed accordingly. `ocr`'s own test
        // fixture (a shorter string) never hit this; this one only
        // surfaced it with a longer sentence closer to the edges.
        .args(["-size", "700x150", "xc:white"])
        .args(["-font", "DejaVu-Sans-Bold", "-pointsize", "24"])
        .args(["-fill", "black", "-gravity", "center"])
        .args([
            "-annotate",
            "+0+0",
            "Photosynthesis converts sunlight to energy",
        ])
        .arg(&image_path)
        .output()
        .expect("imagemagick must be installed for this test's fixture");
    assert!(
        convert.status.success(),
        "{}",
        String::from_utf8_lossy(&convert.stderr)
    );

    let mut ocr_registry = ToolRegistry::new();
    ocr::tools::register_all(&mut ocr_registry).unwrap();
    let ocr_router = ToolRouter::new(ocr_registry, common::PermissionLevel::ReadOnly);

    let mut ocr_args = ToolArgs::new();
    ocr_args.insert(
        "path".into(),
        serde_json::json!(image_path.to_str().unwrap()),
    );
    let ocr_result = ocr_router
        .execute("extract_text_from_image", ocr_args)
        .await;
    assert_eq!(
        ocr_result.status,
        tool_framework::ResultStatus::Ok,
        "{}",
        ocr_result.message
    );
    let extracted_text = ocr_result.data.unwrap()["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        extracted_text.to_lowercase().contains("photosynthesis"),
        "{extracted_text:?}"
    );

    let text_path = dir.join("extracted.txt");
    std::fs::write(&text_path, &extracted_text).unwrap();

    let kb = std::sync::Arc::new(std::sync::Mutex::new(
        rag::store::KnowledgeBase::in_memory(Box::new(embeddings::HashingEmbedder::new(64)))
            .unwrap(),
    ));
    let mut rag_registry = ToolRegistry::new();
    rag::tools::register_all(&mut rag_registry, kb).unwrap();
    let rag_router = ToolRouter::new(rag_registry, common::PermissionLevel::Administrator);

    let mut index_args = ToolArgs::new();
    index_args.insert(
        "path".into(),
        serde_json::json!(text_path.to_str().unwrap()),
    );
    let index_result = rag_router.execute("index_document", index_args).await;
    assert_eq!(
        index_result.status,
        tool_framework::ResultStatus::Ok,
        "{}",
        index_result.message
    );

    let mut search_args = ToolArgs::new();
    search_args.insert(
        "query".into(),
        serde_json::json!("photosynthesis sunlight energy"),
    );
    let search_result = rag_router
        .execute("search_knowledge_base", search_args)
        .await;
    assert_eq!(search_result.status, tool_framework::ResultStatus::Ok);
    let hits = search_result.data.unwrap();
    let hits = hits["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert!(hits[0]["content"]
        .as_str()
        .unwrap()
        .to_lowercase()
        .contains("photosynthesis"));

    std::fs::remove_dir_all(&dir).ok();
}
