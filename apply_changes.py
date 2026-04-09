import re

def replace_if(text, old, new):
    if old in text:
        return text.replace(old, new, 1)
    return text

# ===================== llm/mod.rs =====================
llm_path = r'C:\Users\22414\Desktop\clarity\crates\clarity-core\src\llm\mod.rs'
with open(llm_path, 'r', encoding='utf-8') as f:
    llm = f.read()
llm = llm.replace('\r\n', '\n')

llm = replace_if(llm, 'use std::env;', 'use std::env;\nuse std::sync::OnceLock;\nuse std::time::Duration;')

llm = replace_if(llm, '// ==================== OpenAI Compatible API Types ====================',
'''static SHARED_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn shared_http_client() -> reqwest::Client {
    SHARED_HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(10))
            .pool_max_idle_per_host(10)
            .build()
            .expect("failed to build reqwest client")
    }).clone()
}

// ==================== OpenAI Compatible API Types ====================''')

llm = replace_if(llm,
'''#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    stream: bool,
}''',
'''#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_cache_key: Option<String>,
}''')

llm = replace_if(llm,
'''pub struct OpenAiCompatibleLlm {
    api_key: String,
    base_url: String,
    model: String,
    client: reqwest::Client,
}''',
'''pub struct OpenAiCompatibleLlm {
    api_key: String,
    base_url: String,
    model: String,
    client: reqwest::Client,
    prompt_cache_key: Option<String>,
}''')

llm = replace_if(llm,
'''    pub fn new(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: base_url.into(),
            model: model.into(),
            client: reqwest::Client::new(),
        }
    }''',
'''    pub fn new(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: base_url.into(),
            model: model.into(),
            client: shared_http_client(),
            prompt_cache_key: None,
        }
    }''')

llm = replace_if(llm,
'''        Ok(Self::new(api_key, base_url, model))
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleLlm {''',
'''        Ok(Self::new(api_key, base_url, model))
    }

    pub fn set_prompt_cache_key(&mut self, key: impl Into<String>) {
        self.prompt_cache_key = Some(key.into());
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleLlm {''')

llm = replace_if(llm,
'''        let request_body = ChatCompletionRequest {
            model: self.model.clone(),
            messages: api_messages,
            temperature: None,
            max_tokens: None,
            stream: false,
        };''',
'''        let request_body = ChatCompletionRequest {
            model: self.model.clone(),
            messages: api_messages,
            temperature: None,
            max_tokens: None,
            stream: false,
            prompt_cache_key: self.prompt_cache_key.clone(),
        };''')

llm = replace_if(llm,
'''        let request_body = ChatCompletionRequest {
            model: self.model.clone(),
            messages: api_messages,
            temperature: None,
            max_tokens: None,
            stream: true,
        };''',
'''        let request_body = ChatCompletionRequest {
            model: self.model.clone(),
            messages: api_messages,
            temperature: None,
            max_tokens: None,
            stream: true,
            prompt_cache_key: self.prompt_cache_key.clone(),
        };''')

llm = replace_if(llm,
'''        Ok(rx)
    }
}

/// Kimi (Moonshot) LLM Provider''',
'''        Ok(rx)
    }

    fn set_prompt_cache_key(&mut self, key: &str) {
        self.set_prompt_cache_key(key);
    }
}

/// Kimi (Moonshot) LLM Provider''')

# KimiLlm
llm = replace_if(llm,
'''#[async_trait]
impl LlmProvider for KimiLlm {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        self.inner.complete(messages, tools).await
    }

    fn stream(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
        self.inner.stream(messages, tools)
    }
}

/// Anthropic LLM Provider''',
'''#[async_trait]
impl LlmProvider for KimiLlm {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        self.inner.complete(messages, tools).await
    }

    fn stream(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
        self.inner.stream(messages, tools)
    }

    fn set_prompt_cache_key(&mut self, key: &str) {
        self.inner.set_prompt_cache_key(key);
    }
}

/// Anthropic LLM Provider''')

# AnthropicLlm
llm = replace_if(llm,
'''        Ok(rx)
    }
}

/// Factory for creating LLM providers''',
'''        Ok(rx)
    }

    fn set_prompt_cache_key(&mut self, _key: &str) {}
}

/// Factory for creating LLM providers''')

# Append tests if not present
our_tests = '''

    #[test]
    fn test_chat_completion_request_serialization_with_cache_key() {
        let request = ChatCompletionRequest {
            model: "test-model".into(),
            messages: vec![ApiMessage {
                role: "user".into(),
                content: "hello".into(),
            }],
            temperature: None,
            max_tokens: None,
            stream: false,
            prompt_cache_key: Some("cache-key-123".into()),
        };
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json.get("model").unwrap(), "test-model");
        assert_eq!(json.get("prompt_cache_key").unwrap(), "cache-key-123");
    }

    #[test]
    fn test_chat_completion_request_serialization_without_cache_key() {
        let request = ChatCompletionRequest {
            model: "test-model".into(),
            messages: vec![ApiMessage {
                role: "user".into(),
                content: "hello".into(),
            }],
            temperature: None,
            max_tokens: None,
            stream: false,
            prompt_cache_key: None,
        };
        let json = serde_json::to_value(&request).unwrap();
        assert!(json.get("prompt_cache_key").is_none());
    }
'''
if 'test_chat_completion_request_serialization_with_cache_key' not in llm:
    llm = llm.rstrip()
    if llm.endswith('}'):
        llm = llm[:-1] + our_tests + '}\n'
    else:
        llm = llm + our_tests

with open(llm_path, 'w', encoding='utf-8') as f:
    f.write(llm)
print('llm/mod.rs done')

# ===================== agent/mod.rs =====================
agent_path = r'C:\Users\22414\Desktop\clarity\crates\clarity-core\src\agent\mod.rs'
with open(agent_path, 'r', encoding='utf-8') as f:
    agent = f.read()
agent = agent.replace('\r\n', '\n')

agent = replace_if(agent, 'use crate::error::{AgentError, ToolError};',
                   'use crate::error::{AgentError, ToolError};\nuse crate::llm::StreamDelta;')

agent = replace_if(agent,
'''    fn stream(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError>;
}''',
'''    fn stream(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError>;

    /// Set a prompt cache key for providers that support prompt caching.
    fn set_prompt_cache_key(&mut self, _key: &str) {}
}''')

agent = replace_if(agent,
'''            // Get LLM response
            let response = llm.complete(&messages, &tools).await?;''',
'''            // Get LLM response with timeout
            let response = tokio::time::timeout(
                tokio::time::Duration::from_secs(45),
                llm.complete(&messages, &tools),
            )
            .await
            .map_err(|_| AgentError::Llm("LLM request timed out after 45s".into()))??;''')

agent = replace_if(agent,
'''                // Try streaming first, fall back to complete() if not supported
                match llm.stream(&messages, &tools) {
                    Ok(mut stream_rx) => {
                        while let Some(chunk_result) = stream_rx.recv().await {
                            match chunk_result {
                                Ok(chunk) => {
                                    final_response.push_str(&chunk);
                                    on_chunk(&chunk);
                                    // Send streaming chunk via wire
                                    self.send_wire_message(WireMessage::ContentPart {
                                        text: chunk.clone(),
                                    });
                                }
                                Err(e) => return Err(e),
                            }
                        }
                    }
                    Err(_) => {
                        // Streaming not supported, use complete() and simulate streaming
                        let content = response.content.clone();
                        // Stream character by character for smooth display
                        for c in content.chars() {
                            let chunk = c.to_string();
                            final_response.push_str(&chunk);
                            on_chunk(&chunk);
                            // Send streaming chunk via wire
                            self.send_wire_message(WireMessage::ContentPart {
                                text: chunk.clone(),
                            });
                            // Small delay for visual effect (optional, can remove)
                            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                        }
                    }
                }''',
'''                // Try streaming first, fall back to complete() if not supported
                match tokio::time::timeout(
                    tokio::time::Duration::from_secs(45),
                    async { llm.stream(&messages, &tools) },
                )
                .await
                {
                    Ok(Ok(mut stream_rx)) => {
                        while let Some(chunk_result) = stream_rx.recv().await {
                            match chunk_result {
                                Ok(delta) => {
                                    if let Some(text) = delta.content {
                                        final_response.push_str(&text);
                                        on_chunk(&text);
                                        // Send streaming chunk via wire
                                        self.send_wire_message(WireMessage::ContentPart {
                                            text: text.clone(),
                                        });
                                    }
                                }
                                Err(e) => return Err(e),
                            }
                        }
                    }
                    Ok(Err(_)) => {}
                    Err(_) => {
                        // Streaming timed out or not supported, use complete() and simulate streaming
                        let content = response.content.clone();
                        // Stream character by character for smooth display
                        for c in content.chars() {
                            let chunk = c.to_string();
                            final_response.push_str(&chunk);
                            on_chunk(&chunk);
                            // Send streaming chunk via wire
                            self.send_wire_message(WireMessage::ContentPart {
                                text: chunk.clone(),
                            });
                            // Small delay for visual effect (optional, can remove)
                            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                        }
                    }
                }''')

with open(agent_path, 'w', encoding='utf-8') as f:
    f.write(agent)
print('agent/mod.rs done')
