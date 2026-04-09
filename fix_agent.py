import re

path = 'crates/clarity-core/src/agent/mod.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

# 1. Add StreamDelta import
content = content.replace(
    'use crate::error::{AgentError, ToolError};\n\nuse crate::memory::{Memory, MemoryStore, MemoryTicker};',
    'use crate::error::{AgentError, ToolError};\nuse crate::llm::StreamDelta;\nuse crate::memory::{Memory, MemoryStore, MemoryTicker};'
)

# 2. Update LlmProvider trait stream signature
content = content.replace(
    ') -> Result<tokio::sync::mpsc::Receiver<Result<String, AgentError>>, AgentError>;\n\n    /// Set a prompt cache key for providers that support prompt caching.',
    ') -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError>;\n\n    /// Set a prompt cache key for providers that support prompt caching.'
)

# 3. Update MockLlm::stream
content = content.replace(
    '''    fn stream(
        &self,
        _messages: &[Message],
        _tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String, AgentError>>, AgentError> {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tokio::spawn(async move {
            let _ = tx.send(Ok("This is a mock response".to_string())).await;
        });
        Ok(rx)
    }''',
    '''    fn stream(
        &self,
        _messages: &[Message],
        _tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tokio::spawn(async move {
            let _ = tx.send(Ok(StreamDelta {
                content: Some("This is a mock response".to_string()),
                tool_calls: vec![],
            })).await;
        });
        Ok(rx)
    }'''
)

# 4. Update run_streaming loop
old_streaming = '''                Ok(Ok(mut stream_rx)) => {
                    // Notify UI that generation has started
                    self.send_wire_message(WireMessage::ContentPart {
                        text: String::new(),
                    });

                    while let Some(chunk_result) = stream_rx.recv().await {
                        match chunk_result {
                            Ok(chunk) => {
                                final_response.push_str(&chunk);
                                on_chunk(&chunk);
                                self.send_wire_message(WireMessage::ContentPart {
                                    text: chunk,
                                });
                            }
                            Err(e) => return Err(e),
                        }
                    }

                    info!("Agent loop completed after {} iterations (streaming)", iteration + 1);
                    completed = true;
                    break;
                }'''

new_streaming = '''                Ok(Ok(mut stream_rx)) => {
                    // Notify UI that generation has started
                    self.send_wire_message(WireMessage::ContentPart {
                        text: String::new(),
                    });

                    let mut stream_tool_calls: Vec<ToolCall> = Vec::new();
                    while let Some(delta_result) = stream_rx.recv().await {
                        match delta_result {
                            Ok(delta) => {
                                if let Some(content) = delta.content {
                                    if !content.is_empty() {
                                        final_response.push_str(&content);
                                        on_chunk(&content);
                                        self.send_wire_message(WireMessage::ContentPart {
                                            text: content,
                                        });
                                    }
                                }
                                stream_tool_calls.extend(delta.tool_calls);
                            }
                            Err(e) => return Err(e),
                        }
                    }

                    // If streaming produced tool calls, enter tool-execution round
                    if !stream_tool_calls.is_empty() {
                        messages.push(Message {
                            role: MessageRole::Assistant,
                            content: final_response.clone(),
                            tool_calls: Some(stream_tool_calls.clone()),
                            tool_call_id: None,
                        });
                        self.process_tool_calls(&stream_tool_calls, &mut messages).await;
                        continue;
                    }

                    info!("Agent loop completed after {} iterations (streaming)", iteration + 1);
                    completed = true;
                    break;
                }'''

content = content.replace(old_streaming, new_streaming)

# 5. Update test mocks - CountingMockLlm
content = content.replace(
    '''            fn stream(
                &self,
                _messages: &[Message],
                _tools: &Value,
            ) -> Result<tokio::sync::mpsc::Receiver<Result<String, AgentError>>, AgentError> {
                let (tx, rx) = tokio::sync::mpsc::channel(1);
                tokio::spawn(async move {
                    let _ = tx.send(Ok("This is a mock response".to_string())).await;
                });
                Ok(rx)
            }
        }

        let registry = ToolRegistry::new();''',
    '''            fn stream(
                &self,
                _messages: &[Message],
                _tools: &Value,
            ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
                let (tx, rx) = tokio::sync::mpsc::channel(1);
                tokio::spawn(async move {
                    let _ = tx.send(Ok(StreamDelta {
                        content: Some("This is a mock response".to_string()),
                        tool_calls: vec![],
                    })).await;
                });
                Ok(rx)
            }
        }

        let registry = ToolRegistry::new();'''
)

# 6. Update test mocks - MockLlmWithToolCall (first occurrence)
content = content.replace(
    '''            fn stream(
                &self,
                _messages: &[Message],
                _tools: &Value,
            ) -> Result<tokio::sync::mpsc::Receiver<Result<String, AgentError>>, AgentError> {
                let (tx, rx) = tokio::sync::mpsc::channel(1);
                tokio::spawn(async move {
                    let _ = tx.send(Ok("Mock response".to_string())).await;
                });
                Ok(rx)
            }
        }

        // 创建注册表并注册一个 Mock 工具''',
    '''            fn stream(
                &self,
                _messages: &[Message],
                _tools: &Value,
            ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
                let (tx, rx) = tokio::sync::mpsc::channel(1);
                tokio::spawn(async move {
                    let _ = tx.send(Ok(StreamDelta {
                        content: Some("Mock response".to_string()),
                        tool_calls: vec![],
                    })).await;
                });
                Ok(rx)
            }
        }

        // 创建注册表并注册一个 Mock 工具'''
)

# 7. Update test mocks - MockLlmWithToolCall (second occurrence)
# Split around the unique context to target the second one specifically
parts = content.split('let registry = ToolRegistry::new();\n        let approval_rt = Arc::new(InMemoryApprovalRuntime::new());')
if len(parts) == 2:
    first = parts[0]
    second = parts[1]
    second = second.replace(
        '''            fn stream(
                &self,
                _messages: &[Message],
                _tools: &Value,
            ) -> Result<tokio::sync::mpsc::Receiver<Result<String, AgentError>>, AgentError> {
                let (tx, rx) = tokio::sync::mpsc::channel(1);
                tokio::spawn(async move {
                    let _ = tx.send(Ok("Mock response".to_string())).await;
                });
                Ok(rx)
            }
        }

        let registry = ToolRegistry::new();''',
        '''            fn stream(
                &self,
                _messages: &[Message],
                _tools: &Value,
            ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
                let (tx, rx) = tokio::sync::mpsc::channel(1);
                tokio::spawn(async move {
                    let _ = tx.send(Ok(StreamDelta {
                        content: Some("Mock response".to_string()),
                        tool_calls: vec![],
                    })).await;
                });
                Ok(rx)
            }
        }

        let registry = ToolRegistry::new();'''
    )
    content = first + 'let registry = ToolRegistry::new();\n        let approval_rt = Arc::new(InMemoryApprovalRuntime::new());' + second

with open(path, 'w', encoding='utf-8') as f:
    f.write(content)

print('Done')
