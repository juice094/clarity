---
title: LLM 提供商统一编排架构设计
category: Document
date: 2026-05-16
tags: [document, llm]
---

# LLM 提供商统一编排架构设计

## 现状问题

1. **重复包装**：DeepSeek、智谱、通义等都使用 OpenAI 兼容 API，却需要单独的文件
2. **配置分散**：每个提供商硬编码自己的环境变量读取逻辑
3. **缺乏注册表**：无法动态切换提供商，必须代码级别修改

## 目标架构：Provider Registry 模式

```rust
// 统一提供商描述（数据驱动）
pub struct ProviderDefinition {
    pub name: String,
    pub base_url: String,
    pub api_key_env: String,
    pub default_model: String,
    pub protocol: Protocol,  // OpenAI, Anthropic, Custom
    pub headers: HashMap<String, String>, // 额外请求头
}

// 注册表（类似 ToolRegistry）
pub struct LlmProviderRegistry {
    providers: HashMap<String, ProviderDefinition>,
}

impl LlmProviderRegistry {
    // 内置提供商定义
    pub fn with_builtin_providers() -> Self {
        let mut registry = Self::new();
        
        registry.register(ProviderDefinition {
            name: "deepseek".into(),
            base_url: "https://api.deepseek.com/v1".into(),
            api_key_env: "DEEPSEEK_API_KEY".into(),
            default_model: "deepseek-chat".into(),
            protocol: Protocol::OpenAI,
            headers: default(),
        });
        
        registry.register(ProviderDefinition {
            name: "kimi".into(),
            base_url: "https://api.moonshot.cn/v1".into(),
            api_key_env: "KIMI_API_KEY".into(),
            default_model: "moonshot-v1-8k".into(),
            protocol: Protocol::OpenAI,
            headers: default(),
        });
        
        registry.register(ProviderDefinition {
            name: "dashscope".into(),  // 通义千问
            base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".into(),
            api_key_env: "DASHSCOPE_API_KEY".into(),
            default_model: "qwen-max".into(),
            protocol: Protocol::OpenAI,
            headers: default(),
        });
        
        registry.register(ProviderDefinition {
            name: "zhipu".into(),  // 智谱
            base_url: "https://open.bigmodel.cn/api/paas/v4".into(),
            api_key_env: "ZHIPU_API_KEY".into(),
            default_model: "glm-4".into(),
            protocol: Protocol::OpenAI,
            headers: default(),
        });
        
        // Anthropic 协议的特殊处理
        registry.register(ProviderDefinition {
            name: "kimi-coding".into(),
            base_url: "https://api.kimi.com/coding/".into(),
            api_key_env: "ANTHROPIC_AUTH_TOKEN".into(),
            default_model: "kimi-for-coding".into(),
            protocol: Protocol::Anthropic,
            headers: [("anthropic-version".into(), "2023-06-01".into())].into(),
        });
        
        registry
    }
    
    // 根据配置创建实际的 LlmProvider
    pub fn create_provider(&self, name: &str, config: &Config) -> Result<Box<dyn LlmProvider>, Error> {
        let def = self.providers.get(name).ok_or(Error::ProviderNotFound)?;
        
        match def.protocol {
            Protocol::OpenAI => Ok(Box::new(OpenAiCompatibleLlm::new(
                config.api_key.clone(),
                def.base_url.clone(),
                config.model.clone(),
            ))),
            Protocol::Anthropic => Ok(Box::new(KimiLlm::new(
                config.api_key.clone(),
                def.base_url.clone(),
                config.model.clone(),
            ))),
        }
    }
}
```

## 配置驱动示例

```toml
# .clarity.toml
[providers.deepseek]
api_key = "${DEEPSEEK_API_KEY}"
model = "deepseek-chat"

[providers.kimi]
api_key = "${KIMI_API_KEY}"
model = "moonshot-v1-8k"

[providers.local]
api_key = "ollama"
base_url = "http://localhost:11434/v1"
model = "llama3.2"

# 运行时切换
[profile.coding]
provider = "kimi-coding"
model = "kimi-for-coding"

[profile.reasoning]
provider = "deepseek"
model = "deepseek-reasoner"
```

## 优势

1. **添加新提供商 = 添加配置项**，无需新建文件
2. **运行时切换**：通过配置切换提供商，无需重新编译
3. **统一错误处理**：注册表层面处理 API key 缺失、网络错误等
4. **用户自定义**：用户可以在配置中定义私有 API 端点

## 与 Nanobot 对比

| 特性 | Nanobot | Clarity 当前 | Clarity 目标 |
|------|---------|-------------|-------------|
| 新增提供商成本 | 1 个文件 ~100 行 | 1 个文件 ~100 行 | **1 行配置** |
| 运行时切换 | ✅ | ❌ | ✅ |
| 用户自定义端点 | ✅ | ❌ | ✅ |
| 协议支持 | 20+ 独立实现 | 2 个通用实现 | **2 个通用 + 配置驱动** |

## 实施建议

**Phase 1（本会话）**：保持当前实现，先完成渠道系统
**Phase 2（下一迭代）**：
1. 创建 `LlmProviderRegistry`
2. 将现有提供商迁移为 `ProviderDefinition`
3. 支持配置驱动

这样可以：**20+ 提供商 = 2 个实现 + 20 行配置** vs Nanobot 的 20+ 文件
