# clarity-core Sprint Plan — 2026-04-17

## 目标
为 agri-paper 的 Domain-as-Config 范式提供 clarity-core 侧的支撑：Personality 模板变量、DomainPersonaConfig 解析、kalosm 本地 Provider 骨架。

## 任务清单（按优先级）

### P0 — 今晚完成（2026-04-17）

1. **PersonalityConfig 扩展 `template_variables`**
   - 文件：`crates/clarity-core/src/personality/types.rs`
   - 动作：在 `PersonalityConfig` 中增加 `pub template_variables: Option<HashMap<String, String>>`
   - 动作：更新 `Default`、`with_*` builder 方法
   - 动作：更新 `PersonalityLoader::fill_variables()` 遍历 `template_variables`
   - 动作：补充单元测试（默认空、注入自定义变量、覆盖硬编码变量）

2. **DomainPersonaConfig 结构体与解析测试**
   - 文件：`crates/clarity-core/src/personality/domain.rs`（新建）
   - 动作：定义 `DomainPersonaConfig` 结构体，包含：
     - `persona: BasePersona`（agent_name, user_name, yuan_type, locale, template_variables）
     - `tools: Option<Vec<DomainToolSchema>>`
     - `system_prompt: Option<SystemPromptConfig>`（template 字段）
   - 动作：定义 `DomainToolSchema`（name, description, parameters: HashMap<String, String>）
   - 动作：定义 `SystemPromptConfig`（template: String）
   - 动作：提供 `parse_domain_persona(path: impl AsRef<Path>) -> Result<DomainPersonaConfig, _>`
   - 动作：在 `personality/mod.rs` 中 re-export `DomainPersonaConfig`
   - 动作：写单元测试，以 agri-paper 会议中提供的 `agri_expert.toml` 样例为 golden sample 做解析验证

3. **kalosm Provider 骨架文件**
   - 文件：`crates/clarity-core/src/llm/kalosm.rs`（新建）
   - 动作：定义 `KalosmProvider` 结构体（空字段或 `model_path: PathBuf`）
   - 动作：实现 `LlmProvider` trait 的空壳（`complete` 返回 mock response，`stream` 返回单条 mock delta，`set_prompt_cache_key` 空实现）
   - 动作：在 `llm/mod.rs` 中 `pub mod kalosm;` 并 re-export `KalosmProvider`
   - 动作：确保 `cargo check --workspace` 通过（kalosm 真实依赖暂不加，用 feature gate 或纯空壳）

### P1 — 下周一前完成（2026-04-21）

4. **AgentController 增加 `Op::ReloadPersonality`**
   - 文件：`crates/clarity-core/src/agent/ops.rs`、`controller.rs`
   - 动作：新增 `Op::ReloadPersonality(PersonalityConfig)` 变体
   - 动作：在 `AgentController::run()` 中处理该操作，调用 `agent.reload_personality()`
   - 动作：在 `Agent` 上增加 `reload_personality()` 方法（替换内部 personality，不重启 loop）
   - 动作：补充 controller 测试

5. **kalosm Provider 真实实现（等待 agri-paper 7B 数据）**
   - 前置条件：agri-paper 提供 kalosm 7B 模型加载代码 + benchmark 数据
   - 动作：将空壳 `KalosmProvider::complete()` 替换为真实 `kalosm::Model` 调用
   - 动作：评估 `stream()` 实现（kalosm 原生流式 vs 轮询模拟）
   - 动作：在 `LlmFactory::auto()` 中增加本地模型检测路径（`~/.clarity/models/`）

## 设计约束

- `clarity-core` **不引入 `notify` crate**，热重载触发由 TUI/Gateway 负责。
- `kalosm` 相关依赖未来用 Cargo feature `kalosm = ["dep:kalosm"]` 隔离，但本周骨架阶段先不加真实依赖，确保默认构建零增重。
- 所有改动需 backward compatible：`PersonalityConfig` 的新字段必须有合理 default，现有测试不得破坏。

## 阻塞点

| 阻塞项 | 负责方 | 预期解除 |
|--------|--------|----------|
| agri-paper 7B 模型数据 | agri-paper | 2026-04-20（本周末） |
| agri_expert.toml 样例文件 | agri-paper | 已提供（在 agri-paper 仓库 configs/） |

## 验收标准

- `cargo test --workspace` 全绿
- `cargo check --workspace` 零错误
- `DomainPersonaConfig` 能成功解析 agri-paper 提供的 `agri_expert.toml` 样例
- `KalosmProvider` 空壳编译通过
- `PersonalityLoader::fill_variables()` 支持运行时自定义变量注入

---
*Plan created by clarity-core agent (Kimi Code CLI) on 2026-04-17*
