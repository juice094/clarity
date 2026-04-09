src_path = r'C:\Users\22414\Desktop\clarity\llm_mod_head.rs'
dst_path = r'C:\Users\22414\Desktop\clarity\crates\clarity-core\src\llm\mod.rs'

with open(src_path, 'r', encoding='utf-16') as f:
    text = f.read()
text = text.replace('\r\n', '\n')

# 1. Add imports
old = 'use std::env;'
new = 'use std::env;\nuse std::sync::OnceLock;\nuse std::time::Duration;'
assert old in text, 'import old not found'
text = text.replace(old, new, 1)

# 2. Add shared client before OpenAI types comment
old = '// ==================== OpenAI Compatible API Types ===================='
new = '''static SHARED_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

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

// ==================== OpenAI Compatible API Types ===================='''
assert old in text, 'comment old not found'
text = text.replace(old, new, 1)

# 3. Add prompt_cache_key to ChatCompletionRequest
old = '''#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    stream: bool,
}'''
new = '''#[derive(Debug, Serialize)]
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
}'''
assert old in text, 'ChatCompletionRequest old not found'
text = text.replace(old, new, 1)

# 4. Add prompt_cache_key to OpenAiCompatibleLlm struct
old = '''pub struct OpenAiCompatibleLlm {
    api_key: String,
    base_url: String,
    model: String,
    client: reqwest::Client,
}'''
new = '''pub struct OpenAiCompatibleLlm {
    api_key: String,
    base_url: String,
    model: String,
    client: reqwest::Client,
    prompt_cache_key: Option<String>,
}'''
assert old in text, 'OpenAiCompatibleLlm struct old not found'
text = text.replace(old, new, 1)

# 5. Update OpenAiCompatibleLlm::new
old = '''    pub fn new(
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
    }'''
new = '''    pub fn new(
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
    }'''
assert old in text, 'OpenAiCompatibleLlm::new old not found'
text = text.replace(old, new, 1)

# 6. Add set_prompt_cache_key inherent method
old = '''        Ok(Self::new(api_key, base_url, model))
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleLlm {'''
new = '''        Ok(Self::new(api_key, base_url, model))
    }

    pub fn set_prompt_cache_key(&mut self, key: impl Into<String>) {
        self.prompt_cache_key = Some(key.into());
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleLlm {'''
assert old in text, 'from_env old not found'
text = text.replace(old, new, 1)

# 7. Update complete request body
old = '''        let request_body = ChatCompletionRequest {
            model: self.model.clone(),
            messages: api_messages,
            temperature: None,
            max_tokens: None,
            stream: false,
        };'''
new = '''        let request_body = ChatCompletionRequest {
            model: self.model.clone(),
            messages: api_messages,
            temperature: None,
            max_tokens: None,
            stream: false,
            prompt_cache_key: self.prompt_cache_key.clone(),
        };'''
assert old in text, 'complete request body old not found'
text = text.replace(old, new, 1)

# 8. Update stream request body
old = '''        let request_body = ChatCompletionRequest {
            model: self.model.clone(),
            messages: api_messages,
            temperature: None,
            max_tokens: None,
            stream: true,
        };'''
new = '''        let request_body = ChatCompletionRequest {
            model: self.model.clone(),
            messages: api_messages,
            temperature: None,
            max_tokens: None,
            stream: true,
            prompt_cache_key: self.prompt_cache_key.clone(),
        };'''
assert old in text, 'stream request body old not found'
text = text.replace(old, new, 1)

# 9. Add set_prompt_cache_key to OpenAiCompatibleLlm trait impl
old = '''        Ok(rx)
    }
}

/// Kimi (Moonshot) LLM Provider'''
new = '''        Ok(rx)
    }

    fn set_prompt_cache_key(&mut self, key: &str) {
        self.set_prompt_cache_key(key);
    }
}

/// Kimi (Moonshot) LLM Provider'''
assert old in text, 'OpenAiCompatibleLlm impl end old not found'
text = text.replace(old, new, 1)

# 10. Add set_prompt_cache_key to KimiLlm trait impl
old = '''#[async_trait]
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
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String, AgentError>>, AgentError> {
        self.inner.stream(messages, tools)
    }
}

/// Anthropic LLM Provider'''
new = '''#[async_trait]
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
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String, AgentError>>, AgentError> {
        self.inner.stream(messages, tools)
    }

    fn set_prompt_cache_key(&mut self, key: &str) {
        self.inner.set_prompt_cache_key(key);
    }
}

/// Anthropic LLM Provider'''
assert old in text, 'KimiLlm impl old not found'
text = text.replace(old, new, 1)

# 11. Append tests
test_module = '''

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_completion_request_serialization_with_cache_key() {
        let request = ChatCompletionRequest {
            model: "test-model".into(),
            messages: vec![ApiMessage {
                role: "user".into(),
                content: Some("hello".into()),
                tool_calls: None,
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
                content: Some("hello".into()),
                tool_calls: None,
            }],
            temperature: None,
            max_tokens: None,
            stream: false,
            prompt_cache_key: None,
        };
        let json = serde_json::to_value(&request).unwrap();
        assert!(json.get("prompt_cache_key").is_none());
    }
}
'''
text = text.rstrip() + test_module

with open(dst_path, 'w', encoding='utf-8') as f:
    f.write(text)

print('Done')
