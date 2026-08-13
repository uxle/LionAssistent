//! A minimal multi-turn chat loop, plus deferred backend construction
//! (Development_plan Phase 18 "Lazy Loading"). Run with:
//!
//!     cargo run -p assistant-core --example minimal_chat_loop

use assistant_core::{
    AssistantCore, EchoResponseGenerator, LazyResponseGenerator, ResponseGenerator,
};

fn main() {
    // In a real app, constructing the actual backend (loading model
    // weights off disk) can be slow enough to matter for how fast the
    // window first appears. LazyResponseGenerator defers that until the
    // first real message instead of blocking startup on it -- this
    // factory closure doesn't run at all until `generate()` is first
    // called, below.
    let backend = LazyResponseGenerator::new(|| {
        println!("(backend constructed on first use, not before)");
        Box::new(EchoResponseGenerator) as Box<dyn ResponseGenerator>
    });

    let mut assistant = AssistantCore::new(Box::new(backend));
    let session_id = assistant.create_session("gemma-3-1b");

    println!("session created, backend not yet constructed\n");

    for turn in ["hello!", "what can you help me with?", "thanks"] {
        let reply = assistant.process_message(session_id, turn).unwrap();
        println!("user: {turn}");
        println!("assistant: {reply}\n");
    }

    let session = assistant.session(session_id).unwrap();
    println!(
        "conversation so far: {} messages",
        session.conversation.messages().len()
    );
}
