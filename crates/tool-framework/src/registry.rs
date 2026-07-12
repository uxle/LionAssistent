//! Tool registration (TDS SS2 "Tool Framework"; Apis SS7 "Tool Lifecycle:
//! Register -> Validate -> Execute -> Return Result -> Log").

use crate::Tool;
use common::{AppError, AppResult};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("tools", &self.list())
            .finish()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a tool. Refuses to silently replace an existing tool
    /// with the same name -- shadowing e.g. a `delete_file` tool without
    /// anyone noticing is exactly the kind of silent behavior PRD SS6.6
    /// rules out ("the assistant must refuse unsafe or ambiguous...
    /// actions"); use `unregister` first if replacement is intentional.
    pub fn register(&mut self, tool: Arc<dyn Tool>) -> AppResult<()> {
        let name = tool.name().to_string();
        if self.tools.contains_key(&name) {
            return Err(AppError::Module {
                module: "tool-framework",
                message: format!("a tool named {name:?} is already registered"),
            });
        }
        self.tools.insert(name, tool);
        Ok(())
    }

    pub fn unregister(&mut self, name: &str) -> bool {
        self.tools.remove(name).is_some()
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// Tool names, sorted for stable, predictable output.
    pub fn list(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.tools.keys().map(|s| s.as_str()).collect();
        names.sort_unstable();
        names
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stubs::EchoTool;

    #[test]
    fn registers_and_retrieves_a_tool() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(EchoTool)).unwrap();
        assert!(registry.get("echo").is_some());
        assert_eq!(registry.list(), vec!["echo"]);
    }

    #[test]
    fn duplicate_registration_is_an_error() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(EchoTool)).unwrap();
        assert!(registry.register(Arc::new(EchoTool)).is_err());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn unregister_removes_the_tool() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(EchoTool)).unwrap();
        assert!(registry.unregister("echo"));
        assert!(registry.is_empty());
    }

    #[test]
    fn unregistering_an_unknown_tool_returns_false() {
        assert!(!ToolRegistry::new().unregister("nonexistent"));
    }

    #[test]
    fn get_on_unknown_tool_returns_none() {
        assert!(ToolRegistry::new().get("nonexistent").is_none());
    }

    #[test]
    fn list_is_sorted() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(crate::stubs::SlowTool)).unwrap();
        registry.register(Arc::new(EchoTool)).unwrap();
        assert_eq!(registry.list(), vec!["echo", "slow-stub"]);
    }
}
