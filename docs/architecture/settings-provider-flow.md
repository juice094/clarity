# Settings → Provider 数据流

> 状态：已完成 Settings 单源化迁移（S3.3 → S3.4）。
> 相关文件：
> - `crates/clarity-egui/src/app_state.rs`
> - `crates/clarity-egui/src/llm_loader.rs`
> - `crates/clarity-egui/src/panels/settings/provider_tab.rs`
> - `crates/clarity-egui/src/provider.rs`
> - `crates/clarity-llm/src/runtime.rs`

## 核心原则

**一个概念只有一个写入点。**

- 用户可见的 settings 唯一写入点是 `settings_store.settings_edit` → `commit_settings()` → 磁盘 + `AppState.cached_settings`。
- provider 定义的唯一写入点是 `ProviderRegistry`（磁盘 `~/.config/clarity/providers/*.toml` + 内置默认值）。
- LLM provider 实例化不再依赖任何全局可变缓存；每次 `ensure_llm` 都根据当前 `cached_settings` + `ProviderRegistry` 重新派生 `RuntimeProviderConfig`。

## 数据流

```text
Settings UI ──settings_edit──┐
                             ▼
Provider Apply ──► auto_save_settings() ──► commit_settings() ──► disk + cached_settings
                                                          │
ensure_llm ◄──────────── cached_settings + profile overlay
    │
    ├─ ProviderRegistry (provider def) ──► RuntimeProviderConfig ──► build_provider()
    ├─ chat-only provider (deepseek-device) ──► ProviderDefinition::to_deepseek_device_provider()
    └─ local provider ──► LocalGgufProvider
```

## 关键类型与职责

| 类型/模块 | 职责 |
|-----------|------|
| `GuiSettings` | 用户级前端设置（provider、model、api_key、profile 等）。 |
| `ProviderRegistry` | 合并内置 + 自定义 provider 定义；提供 `base_url`、`api_format`、密钥引用。 |
| `ApiFormat::runtime_api_format()` | 将前端序列化格式映射为 `clarity_llm::runtime::build_provider` 可识别的 snake_case 协议名。 |
| `llm_loader::runtime_config_from_definition()` | 从 `ProviderDefinition` + `GuiSettings` 派生 `RuntimeProviderConfig`。 |
| `llm_loader::load_llm()` | 按 `ProviderSelection` 加载 LLM；优先尝试 registry-derived config，失败则回退到 `ModelRegistry` / `LlmFactory`。 |
| `clarity_llm::runtime::build_provider()` | 唯一实例化入口：从显式 `RuntimeProviderConfig` 创建 `Box<dyn LlmProvider>`。 |
| `LlmBinding` | 记录当前已绑定的 `(provider, model, local_model_path)`，用于避免不必要的重复加载。 |

## 写入点清单

| 写入点 | 写入内容 | 是否合法 | 说明 |
|--------|----------|----------|------|
| `provider_tab.rs` 点选 provider | `settings_store.settings_edit.provider/model` | ✅ | 只改编辑态，随后 `auto_save_settings()` 落盘。 |
| `provider_tab.rs` Apply | `settings_store.settings_edit` → 磁盘 + `cached_settings` | ✅ | 不再写任何全局运行时缓存。 |
| `provider_tab.rs` 测试连接/刷新模型 | 临时构造 `RuntimeProviderConfig` | ✅ | 仅用于单次 API 调用，不持久化。 |
| `app_logic.rs::commit_settings()` | 磁盘 + `cached_settings` | ✅ | 唯一持久化入口。 |
| `app_state.rs::ensure_llm()` | 读取 `cached_settings` + `ProviderRegistry` | ✅ | 只读，不写入任何配置状态。 |
| ~~`clarity_llm::runtime::ACTIVE_CONFIG`~~ | ~~全局 `Mutex<Option<RuntimeProviderConfig>>`~~ | ❌ 已移除 | 旧 S3.3 缓存，已删除。 |

## 切换与重载语义

- **同 provider + 同 model**：`binding_matches` 命中，直接复用当前 LLM，不重新构建。
- **provider 或 model 变化**：`binding_matches` 未命中，`ensure_llm` 重新派生 config 并构建 provider。
- **profile 切换**：`apply_profile_overlay` 在 `ensure_llm` 调用时动态覆盖 `settings` 副本；若覆盖后的 provider/model 与当前 binding 不同，则自动重载。
- **Apply 后立即生效**：`provider_tab.rs` Apply 后调用 `auto_save_settings()`；下次用户发送消息或调用 `ensure_llm` 时即使用新配置。惰性重载，不强制刷新。

## 历史状态

- S3.3：`provider_tab.rs` Apply 时构造 `RuntimeProviderConfig` 并写入 `clarity_llm::runtime::ACTIVE_CONFIG`；`ensure_llm` 优先读该全局缓存。
- S3.4：移除 `ACTIVE_CONFIG` 及相关函数，所有 provider 实例化统一从 `cached_settings` + `ProviderRegistry` 派生。
