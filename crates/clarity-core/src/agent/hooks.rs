use crate::error::AgentError;
use crate::types::ToolCall;
use serde_json::Value;

/// Result of a hook invocation.
pub enum HookResult {
    /// Continue with the operation.
    Continue,
    /// Cancel the operation with the given error.
    Cancel(AgentError),
    /// Replace the arguments/content with the provided value.
    Replace(Value),
}

/// Trait for lifecycle hooks.
#[async_trait::async_trait]
pub trait AgentHook: Send + Sync {
    /// Called before a tool is executed. Can modify or cancel the call.
    async fn before_tool_call(&self, tool_call: &mut ToolCall) -> HookResult;

    /// Called after a tool is executed. Can inspect or modify the result.
    async fn after_tool_call(&self, tool_call: &ToolCall, result: &mut Value);

    /// Called before LLM inference. Can inspect or modify messages.
    async fn on_llm_input(&self, messages: &mut Vec<clarity_llm::api::Message>);
}

/// Registry of hooks, applied in registration order.
#[derive(Default)]
pub struct HookRegistry {
    hooks: Vec<Box<dyn AgentHook>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, hook: Box<dyn AgentHook>) {
        self.hooks.push(hook);
    }

    pub async fn before_tool_call(&self, tool_call: &mut ToolCall) -> HookResult {
        for hook in &self.hooks {
            match hook.before_tool_call(tool_call).await {
                HookResult::Cancel(e) => return HookResult::Cancel(e),
                HookResult::Replace(v) => {
                    // Parse v as new arguments
                    if let Ok(args) = serde_json::from_value::<ToolCall>(v) {
                        *tool_call = args;
                    }
                }
                HookResult::Continue => {}
            }
        }
        HookResult::Continue
    }

    pub async fn after_tool_call(&self, tool_call: &ToolCall, result: &mut Value) {
        for hook in &self.hooks {
            hook.after_tool_call(tool_call, result).await;
        }
    }

    pub async fn on_llm_input(&self, messages: &mut Vec<clarity_llm::api::Message>) {
        for hook in &self.hooks {
            hook.on_llm_input(messages).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_llm::api::Message;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    struct CancelHook;

    #[async_trait::async_trait]
    impl AgentHook for CancelHook {
        async fn before_tool_call(&self, _tool_call: &mut ToolCall) -> HookResult {
            HookResult::Cancel(AgentError::ToolExecutionFailed(
                "test".to_string(),
                "cancelled by hook".to_string(),
            ))
        }

        async fn after_tool_call(&self, _tool_call: &ToolCall, _result: &mut Value) {}

        async fn on_llm_input(&self, _messages: &mut Vec<Message>) {}
    }

    struct ReplaceHook;

    #[async_trait::async_trait]
    impl AgentHook for ReplaceHook {
        async fn before_tool_call(&self, tool_call: &mut ToolCall) -> HookResult {
            let mut new_tc = tool_call.clone();
            new_tc.function.arguments = r#"{"replaced":true}"#.to_string();
            HookResult::Replace(serde_json::to_value(new_tc).unwrap())
        }

        async fn after_tool_call(&self, _tool_call: &ToolCall, _result: &mut Value) {}

        async fn on_llm_input(&self, _messages: &mut Vec<Message>) {}
    }

    struct InspectHook {
        inspected: Arc<AtomicBool>,
    }

    #[async_trait::async_trait]
    impl AgentHook for InspectHook {
        async fn before_tool_call(&self, _tool_call: &mut ToolCall) -> HookResult {
            HookResult::Continue
        }

        async fn after_tool_call(&self, _tool_call: &ToolCall, result: &mut Value) {
            if result.get("ok").is_some() {
                self.inspected.store(true, Ordering::SeqCst);
            }
        }

        async fn on_llm_input(&self, _messages: &mut Vec<Message>) {}
    }

    #[tokio::test]
    async fn test_before_tool_call_cancel() {
        let mut registry = HookRegistry::new();
        registry.register(Box::new(CancelHook));

        let mut tc = ToolCall {
            id: "1".to_string(),
            call_type: "function".to_string(),
            function: crate::types::FunctionCall {
                name: "test_tool".to_string(),
                arguments: r#"{}"#.to_string(),
            },
        };

        let result = registry.before_tool_call(&mut tc).await;
        assert!(
            matches!(
                result,
                HookResult::Cancel(AgentError::ToolExecutionFailed(_, _))
            ),
            "expected Cancel result"
        );
    }

    #[tokio::test]
    async fn test_before_tool_call_replace() {
        let mut registry = HookRegistry::new();
        registry.register(Box::new(ReplaceHook));

        let mut tc = ToolCall {
            id: "1".to_string(),
            call_type: "function".to_string(),
            function: crate::types::FunctionCall {
                name: "test_tool".to_string(),
                arguments: r#"{"original":true}"#.to_string(),
            },
        };

        let result = registry.before_tool_call(&mut tc).await;
        assert!(
            matches!(result, HookResult::Continue),
            "expected Continue result"
        );
        assert_eq!(tc.function.arguments, r#"{"replaced":true}"#);
    }

    #[tokio::test]
    async fn test_after_tool_call_inspect() {
        let inspected = Arc::new(AtomicBool::new(false));
        let mut registry = HookRegistry::new();
        registry.register(Box::new(InspectHook {
            inspected: inspected.clone(),
        }));

        let tc = ToolCall {
            id: "1".to_string(),
            call_type: "function".to_string(),
            function: crate::types::FunctionCall {
                name: "test_tool".to_string(),
                arguments: r#"{}"#.to_string(),
            },
        };

        let mut result = serde_json::json!({"ok": true});
        registry.after_tool_call(&tc, &mut result).await;
        assert!(
            inspected.load(Ordering::SeqCst),
            "hook should have inspected the result"
        );
    }
}
