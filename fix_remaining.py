import re

# ===================== agent/mod.rs =====================
agent_path = r'C:\Users\22414\Desktop\clarity\crates\clarity-core\src\agent\mod.rs'
with open(agent_path, 'r', encoding='utf-8') as f:
    agent = f.read()
agent = agent.replace('\r\n', '\n')

# Remove duplicate import (non-adjacent)
agent = agent.replace('use crate::llm::StreamDelta;\n', '', 1)

# Change set_prompt_cache_key to have default body
old = '''    /// Set a prompt cache key for provider-side cache routing.
    fn set_prompt_cache_key(&mut self, key: &str);
}'''
new = '''    /// Set a prompt cache key for provider-side cache routing.
    fn set_prompt_cache_key(&mut self, _key: &str) {}
}'''
if old in agent:
    agent = agent.replace(old, new, 1)

with open(agent_path, 'w', encoding='utf-8') as f:
    f.write(agent)
print('agent/mod.rs fixed')

# ===================== deepseek.rs =====================
deepseek_path = r'C:\Users\22414\Desktop\clarity\crates\clarity-core\src\llm\deepseek.rs'
with open(deepseek_path, 'r', encoding='utf-8') as f:
    ds = f.read()
ds = ds.replace('\r\n', '\n')

# Remove the inherent method (it will be covered by default trait method)
old = '''    pub fn set_prompt_cache_key(&mut self, key: &str) {
        self.inner.set_prompt_cache_key(key);
    }
}

#[async_trait]
impl LlmProvider for DeepSeekProvider {'''
new = '''}

#[async_trait]
impl LlmProvider for DeepSeekProvider {'''
if old in ds:
    ds = ds.replace(old, new, 1)

# Add set_prompt_cache_key inside the trait impl
old = '''    fn stream(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<crate::llm::StreamDelta, AgentError>>, AgentError> {
        self.inner.stream(messages, tools)
    }
}

/// Available DeepSeek models'''
new = '''    fn stream(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<crate::llm::StreamDelta, AgentError>>, AgentError> {
        self.inner.stream(messages, tools)
    }

    fn set_prompt_cache_key(&mut self, key: &str) {
        self.inner.set_prompt_cache_key(key);
    }
}

/// Available DeepSeek models'''
if old in ds:
    ds = ds.replace(old, new, 1)

with open(deepseek_path, 'w', encoding='utf-8') as f:
    f.write(ds)
print('deepseek.rs fixed')

# ===================== llm/mod.rs =====================
llm_path = r'C:\Users\22414\Desktop\clarity\crates\clarity-core\src\llm\mod.rs'
with open(llm_path, 'r', encoding='utf-8') as f:
    llm = f.read()
llm = llm.replace('\r\n', '\n')

# Fix let rx -> let mut rx in test
llm = llm.replace('let rx = llm.stream(&[], &serde_json::json!({})).unwrap();',
                  'let mut rx = llm.stream(&[], &serde_json::json!({})).unwrap();', 1)

with open(llm_path, 'w', encoding='utf-8') as f:
    f.write(llm)
print('llm/mod.rs fixed')
