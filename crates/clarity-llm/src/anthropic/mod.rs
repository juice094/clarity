//! Anthropic Messages API adapter.
//!
//! This module translates Anthropic-formatted requests/responses to and from
//! the `clarity_contract::LlmProvider` interface. It lets any provider in the
//! Clarity ecosystem be exposed behind an Anthropic-compatible facade.

pub mod adapter;
pub mod prompt;
pub mod tools;
pub mod types;

pub use adapter::AnthropicAdapter;
pub use prompt::{build_prompt, content_to_text, strip_tool_xml};
pub use tools::convert_tools;
pub use types::{
    AnthropicBlock, AnthropicContent, AnthropicMessage, AnthropicRequest, AnthropicResponse,
    AnthropicTool, ResponseBlock, Usage,
};

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_contract::{LlmResponse, Message};
    use serde_json::Value;
    use std::sync::Arc;

    struct FakeProvider {
        response: String,
    }

    #[async_trait::async_trait]
    impl clarity_contract::LlmProvider for FakeProvider {
        async fn complete(
            &self,
            _messages: &[Message],
            _tools: &Value,
        ) -> Result<LlmResponse, clarity_contract::AgentError> {
            Ok(LlmResponse {
                content: self.response.clone(),
                tool_calls: vec![],
                is_complete: true,
            })
        }

        fn stream(
            &self,
            _messages: &[Message],
            _tools: &Value,
        ) -> Result<
            tokio::sync::mpsc::Receiver<
                Result<clarity_contract::StreamDelta, clarity_contract::AgentError>,
            >,
            clarity_contract::AgentError,
        > {
            let (tx, rx) = tokio::sync::mpsc::channel(1);
            let _ = tx.try_send(Ok(clarity_contract::StreamDelta::content("")));
            Ok(rx)
        }

        fn set_prompt_cache_key(&self, _key: &str) {}
    }

    #[test]
    fn convert_tools_maps_input_schema_to_parameters() {
        let tools = vec![AnthropicTool {
            name: "test".into(),
            description: "desc".into(),
            input_schema: serde_json::json!({"type": "object", "properties": {"x": {"type": "string"}}}),
        }];
        let result = convert_tools(&tools);
        let arr = result.as_array().unwrap();
        assert_eq!(arr[0]["type"], "function");
        assert_eq!(arr[0]["function"]["name"], "test");
    }

    #[test]
    fn strip_tool_xml_removes_tool_blocks() {
        let input = "Before\n<tool name=\"x\">\n<arg key=\"y\">z</arg>\n</tool>\nAfter";
        let result = strip_tool_xml(input);
        assert!(result.contains("Before"));
        assert!(result.contains("After"));
        assert!(!result.contains("<tool"));
    }

    #[test]
    fn build_prompt_includes_system_and_messages() {
        let msgs = vec![AnthropicMessage {
            role: "user".into(),
            content: AnthropicContent::Text("Hello".into()),
        }];
        let prompt = build_prompt(&msgs, &Some("You are helpful.".to_string()));
        assert!(prompt.contains("System: You are helpful."));
        assert!(prompt.contains("User: Hello"));
    }

    #[test]
    fn content_to_text_handles_tool_blocks() {
        let content = AnthropicContent::Blocks(vec![
            AnthropicBlock::Text {
                text: "Let me check".into(),
            },
            AnthropicBlock::ToolUse {
                id: "toolu_1".into(),
                name: "sh".into(),
                input: serde_json::json!({"command": "ls"}),
            },
        ]);
        let text = content_to_text(&content);
        assert!(text.contains("Let me check"));
        assert!(text.contains("[Tool Call: sh"));
        assert!(text.contains("toolu_1"));
    }

    #[tokio::test]
    async fn adapter_returns_anthropic_response() {
        let provider = Arc::new(FakeProvider {
            response: "Hello from fake provider".to_string(),
        });
        let adapter = AnthropicAdapter::new(provider);

        let request = AnthropicRequest {
            model: Some("claude-test".to_string()),
            max_tokens: 1024,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: AnthropicContent::Text("Hi".to_string()),
            }],
            tools: vec![],
            stream: false,
            system: None,
        };

        let response = adapter.complete(request).await.unwrap();
        assert_eq!(response.role, "assistant");
        assert_eq!(response.response_type, "message");
        assert_eq!(response.model, "claude-test");
        assert_eq!(response.stop_reason, "end_turn");
        assert_eq!(response.content.len(), 1);
        assert!(matches!(
            &response.content[0],
            ResponseBlock::Text { text } if text == "Hello from fake provider"
        ));
    }

    #[tokio::test]
    async fn adapter_parses_xml_tool_calls() {
        let provider = Arc::new(FakeProvider {
            response: r#"I'll run that for you.
<tool name="sh">
<arg key="command">ls</arg>
</tool>"#
                .to_string(),
        });
        let adapter = AnthropicAdapter::new(provider);

        let request = AnthropicRequest {
            model: None,
            max_tokens: 1024,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: AnthropicContent::Text("List files".to_string()),
            }],
            tools: vec![AnthropicTool {
                name: "sh".to_string(),
                description: "shell".to_string(),
                input_schema: serde_json::json!({"type": "object"}),
            }],
            stream: false,
            system: None,
        };

        let response = adapter.complete(request).await.unwrap();
        assert_eq!(response.stop_reason, "tool_use");
        assert_eq!(response.content.len(), 2);
        assert!(matches!(&response.content[0], ResponseBlock::Text { text } if !text.is_empty()));
        assert!(matches!(
            &response.content[1],
            ResponseBlock::ToolUse { name, .. } if name == "sh"
        ));
    }
}
