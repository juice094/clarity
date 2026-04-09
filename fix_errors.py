import re

# ===================== llm/mod.rs =====================
llm_path = r'C:\Users\22414\Desktop\clarity\crates\clarity-core\src\llm\mod.rs'
with open(llm_path, 'r', encoding='utf-8') as f:
    llm = f.read()
llm = llm.replace('\r\n', '\n')

# Remove duplicate imports
while 'use std::sync::OnceLock;\nuse std::time::Duration;\nuse std::sync::OnceLock;\nuse std::time::Duration;' in llm:
    llm = llm.replace('use std::sync::OnceLock;\nuse std::time::Duration;\nuse std::sync::OnceLock;\nuse std::time::Duration;',
                      'use std::sync::OnceLock;\nuse std::time::Duration;', 1)

# Remove duplicate shared client block
shared_block = '''static SHARED_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

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
'''
while llm.count(shared_block) > 1:
    idx = llm.rfind(shared_block)
    llm = llm[:idx] + llm[idx+len(shared_block):]

# Fix AnthropicLlm::new: revert client and remove prompt_cache_key
old = '''    pub fn new(
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
    }
}

#[async_trait]
impl LlmProvider for AnthropicLlm {'''
new = '''    pub fn new(
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
    }
}

#[async_trait]
impl LlmProvider for AnthropicLlm {'''
if old in llm:
    llm = llm.replace(old, new, 1)

with open(llm_path, 'w', encoding='utf-8') as f:
    f.write(llm)
print('llm/mod.rs fixed')

# ===================== agent/mod.rs =====================
agent_path = r'C:\Users\22414\Desktop\clarity\crates\clarity-core\src\agent\mod.rs'
with open(agent_path, 'r', encoding='utf-8') as f:
    agent = f.read()
agent = agent.replace('\r\n', '\n')

# Remove duplicate StreamDelta import
while 'use crate::llm::StreamDelta;\nuse crate::llm::StreamDelta;' in agent:
    agent = agent.replace('use crate::llm::StreamDelta;\nuse crate::llm::StreamDelta;', 'use crate::llm::StreamDelta;', 1)

with open(agent_path, 'w', encoding='utf-8') as f:
    f.write(agent)
print('agent/mod.rs fixed')

# ===================== deepseek.rs =====================
deepseek_path = r'C:\Users\22414\Desktop\clarity\crates\clarity-core\src\llm\deepseek.rs'
with open(deepseek_path, 'r', encoding='utf-8') as f:
    ds = f.read()
ds = ds.replace('\r\n', '\n')

old = '''    fn stream(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String, AgentError>>, AgentError> {
        self.inner.stream(messages, tools)
    }'''
new = '''    fn stream(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<crate::llm::StreamDelta, AgentError>>, AgentError> {
        self.inner.stream(messages, tools)
    }'''
if old in ds:
    ds = ds.replace(old, new, 1)

with open(deepseek_path, 'w', encoding='utf-8') as f:
    f.write(ds)
print('deepseek.rs fixed')
