---
title: AI 关键决策记录 · Clarity
category: Protocol
date: 2026-05-16
tags: [protocol]
---

# AI 关键决策记录 · Clarity

> 本文件记录跨 AI 会话的关键架构决策、状态锚点和 Hard Veto 边界。
> 用途：在上下文压缩后，新会话可快速恢复项目认知。
> 协议版本：V3.1-EP-O

---

## 一、当前会话锚点

**最后更新**：2026-05-10
**当前分支**：`main` @ `5dc1fe23`（未推送 origin，领先 20 commits）
**架构模式**：CLI
**定位声明**：Clarity 是集群协作原语的单机验证运行时（非本地聊天工具）。
**会话状态**：
- v0.3.1 已发布（tag `v0.3.1`）
- clarity-tauri 完全归档移出仓库；Dependabot 报警清零
- Settings 模型选择缺陷修复；Mutex 硬化完成
- **Sprint 9 — 服务商支持硬化**：Phase 1 ✅ | Phase 2 ✅ | Phase 3 🔓 已解锁
- **Sprint 10 — 协议先行解锁**：D1 ✅ | D2 ✅ | D3 ✅ | D4 ✅
- **Sprint 11 — 超越 Kimi CLI**：Phase A ✅ | Phase B ✅ | Phase C ✅
- **Sprint 12 — egui 功能补齐**：✅ 已完成
- **Sprint 13 — 稳定性硬化 + 架构解耦**：✅ 已完成（2026-04-27 ~ 2026-05-03）
- **Sprint 13.5 — 前端架构重构**：✅ 已完成（Zustand-style Store + Services 拆分 + 错误边界）
- **Sprint 14 — Glassmorphism 视觉精调**：✅ 已完成（2026-05-01 ~ 2026-05-03）
- **Sprint 14.5 — 架构解耦与代码健康**：✅ 已完成（2026-05-02）
- **Sprint 15 — 多方面强化**：✅ 已完成（2026-05-02 ~ 2026-05-03）
- **Sprint 16 — 内核升级 + 基础设施**：✅ 已完成（2026-05-03）
- **Sprint 17 — ZeroClaw 吸收与工程深化**：✅ 已完成（2026-05-03 ~ 2026-05-06）
- **Sprint 38-C — CI Pipeline Hardening**：✅ 已完成（2026-05-06）
- **Sprint 36/36.5/36.6 — Cron/Team UI + 死代码清理 + Markdown 表格**：✅ 已完成（2026-05-05）
- **Sprint 39 — Runtime Stability + Engineering Hygiene**：✅ 已完成（2026-05-07）
- **Sprint 40 — Runtime Robustness + Integration Tests**：✅ 已完成（2026-05-08）
- **Sprint 41 — UI 审计修复与视觉精调**：✅ 已完成（2026-05-10）
- **进行中**：LLM Mesh（circuit-breaker + McpLlmProvider + MCP Server 基础设施）、Gateway handler 模块化拆分、Phosphor 图标系统

---

## 二、Hard Veto（不可逾越边界）

| 约束 | 状态 | 说明 |
|------|------|------|
| 本地 LLM 优先 | ✅ 生效 | `LocalGgufProvider` 已验证；`ensure_llm` 自动 fallback |
| 禁止数据外泄 | ✅ 生效 | API key 仅存储本地；云端 Provider 由用户显式选择 |
| 禁止 Docker | ✅ 生效 | 无容器化依赖 |
| 禁止 RAG(Qdrant) | ✅ 生效 | `clarity-memory` 使用 SQLite + BM25 + CosineIndex |
| 禁止 Electron | ✅ 生效 | Tauri 2 已替代 |
| 项目广度 ≤ 5 核心工具 | ✅ 已收敛 | clarity-tauri 已归档，活跃 crate 6 个 + Gateway + TUI，已达上限，新增功能需裁减 |
| **主权防御：不入赘** | ✅ 生效 | 学习 Kimi 生态（娘家/导师），但保持独立实现。模型/数据/协议/人格四层主权不可让渡 |

---

## 三、近期关键决策（2026-04-26）

### 3.1 UI/UX 重构（plan: guy-gardner-jade-polaris）

**决策**：按 Phase 1→2→3→4 顺序执行，一次性提交。

| Phase | 内容 | 状态 |
|-------|------|------|
| 1 | Header 精简（More 菜单 + 状态圆点） | ✅ `b95d26b` |
| 2 | Chat Input 居中卡片式 | ✅ `b95d26b` |
| 3 | Welcome 页（大标题 + 快捷操作 pill） | ✅ `b95d26b` |
| 4 | Sidebar Tools 分组 | ✅ `b95d26b` |

**验收结果**：5/5 标准通过。发现 4 个新 bug（见 5.1）。

### 3.2 Tauri 自动更新（Updater Plugin）

**决策**：采用 `tauri-plugin-updater` 官方方案，非自研轮询。

- 公钥硬编码于 `tauri.conf.json`
- 私钥存放于 GitHub Secrets（`TAURI_SIGNING_PRIVATE_KEY`）
- Release workflow 生成 `.sig` + `latest.json`
- signtool 仅签名 `.exe`，避免破坏 `.msi/.nsis` 的 minisign 签名

**Commit**：`3f270d8`

### 3.3 cargo audit 策略

**决策**：由 `--deny warnings` 调整为 `--deny unsound --deny yanked`。

- 上游 20 个 `unmaintained` 警告不再阻断 CI
- `.cargo/audit.toml` 显式忽略 2 个已评估的 `unsound`（rand 0.7.3 / glib VariantStrIter）
- 保留对新 soundness issue 和 yanked crate 的阻断能力

**Commit**：`b9e45b6`

### 3.5 代码健康规则注入（2026-04-26）

**决策**：将代码健康红线固化到项目规范文件，从"快速迭代"迁移到"规范驱动"。

| 文件 | 注入内容 |
|------|---------|
| `AGENTS.md` | Code Style 扩展：`unwrap()` 注释规则、`pub fn` doc 要求、`unsafe` 禁令、跨层检查单 |
| `docs/development/test_governance.md` | 新增第 8 章：定量基线表、unwrap 分类策略、验收命令、违规处理 |
| `.kimi/SESSION_STARTER.md` | 重写为"会话启动检查清单"，前置健康红线 |

**Commit**：`6ddda1d`（v0.3.0 发布）+ 后续文档更新

### 3.6 cargo doc warning 清零（2026-04-26）

**决策**：修复 13 处 doc warning（5 bare URL + 7 unresolved intra-doc link + 1 unclosed HTML tag），建立 `cargo doc --no-deps` 零 warning 基线。

**Commit**：当前会话

### 3.4 v0.3.0 每日使用体验硬化（四阶段完成）

**决策**：按阶段一→四顺序执行，每阶段独立 commit。

| 阶段 | 内容 | Commit |
|------|------|--------|
| 1 | 工具调用可视化（`ToolCallIndicator` + Wire 事件转发） | `e280be3` |
| 2 | Compaction 状态提示（`CompactionBegin/End` WireMessage + banner） | `afe1a72` |
| 3 | 模型下载 GUI（HuggingFace 直链下载 + SettingsPanel 进度条） | `89e8c68` |
| 4 | 前端日志面板（Console 劫持 + 可折叠面板） | `cdaea3e` |

### 3.7 服务商持久化调研（2026-04-27）

**决策**：调研 OpenHanako 与 OpenClaw 的 Provider 配置机制，识别 Clarity 的架构缺口。

| 项目 | Provider 机制 | Clarity 差距 |
|------|--------------|-------------|
| OpenHanako | Agent 文件夹自包含 + 插件化 `providers/*.js` + 三模型角色 | 无 Agent 级覆盖、无插件注册、无模型角色分工 |
| OpenClaw | 中心化 `~/.openclaw/openclaw.json` + 交互式 `onboard` 向导 + 40+ 服务商 | 无 Custom Provider 无代码注册、无环境变量注入、Save 覆盖全配置 |

**关键教训（OpenClaw）**：Dashboard Save 覆盖全配置导致大面积数据丢失。Clarity 必须实现**增量保存**。

**下一步**：Provider Schema 化（TOML/JSON 描述 baseURL/authType/modelListEndpoint），支持 `${env:VAR}` 语法注入 API Key。

### 3.11 Sprint 14 — 前端设计审查与全面翻新（2026-05-01，已批准）

**决策**：基于 Kimi 网页版 Swiss International Style + Agent-Native UI 参考，对 clarity-egui 进行 Phase 1-5 全面翻新。

**战略背景**：
- Clarity 长期目标是成为独立 Agent 操作系统
- 替代路径：**kimi-cli → zeroclaw → openhanako**
- Claw 运行时（基于 openclaw + zeroclaw 开发）为后端另一条长主线，**尚未完成**，待后端资源就绪后启动
- 当前 Sprint 14 前端翻新不阻塞 Claw 运行时开发，两者可并行

**美学评分**：4.6/10（色彩 6/10、排版 5/10、布局 5/10、组件 4/10、交互 4/10、信息层级 5/10、品牌 6/10）

**TOP 5 设计缺陷**：
1. Approval modal render 路径 `block_on` 同步阻塞 UI 线程
2. IME 300ms 时间阈值对 CJK 输入法歧视性误触发
3. `stick_to_bottom(true)` 无逃逸机制，劫持阅读流
4. Settings provider cards 使用 raw painter API，丧失 hover/focus/accessibility
5. Sidebar 无会话列表，240px 侧边栏空心化

**Phase 路线图**：
| Phase | 内容 | 状态 |
|-------|------|------|
| 1 | 色彩/排版基础设施（tokenize + OLED Black + 字体注册 + CJK 探测） | 🔄 foundation 完成 |
| 2 | 布局重构（Sidebar 会话列表、Swiss 行长保护、emotion 统一） | ⏳ 待启动 |
| 3 | 组件体系（混合气泡策略、widget 化、图标字体替换 emoji） | ⏳ 待启动 |
| 4 | 交互优化（block_on 移除、IME 安全、stick_to_bottom 智能释放） | ⏳ 待启动 |
| 5 | 品牌气质（叙事层、技能外露、Agent 仪表盘） | ⏳ 待启动 |

**计划文件**：`docs/plans/frontend-design-critique-2026-05-01.md`
**Commit**：`9086173a`（Phase 1 foundation）+ `ba6d8201`（AGENTS.md 更新）

**架构约束**：
- 必须在 `cargo test --workspace --lib` 零失败基线下推进
- `clarity-egui` 为 binary crate，测试通过 `cargo test -p clarity-egui` 运行
- 任何 UI 重构不得破坏 `clarity-tui` / `clarity-gateway` 的协议兼容性

---

### 3.10 Sprint 9 — 服务商支持硬化（2026-04-29）

**Phase 1 ✅ — API Key 安全注入 + Settings 增量保存**

| 改动 | 文件 | 说明 |
|------|------|------|
| `resolve_api_key()` | `crates/clarity-egui/src/settings.rs` | 支持 `${env:VAR_NAME}` 语法，运行时解析环境变量；解析失败 fallback 到原值 |
| 增量 `save()` | `crates/clarity-egui/src/settings.rs` | `merge_json` 递归合并，保留未知字段，支持字段删除（`null` overlay） |
| `app_state.rs` 接入 | `crates/clarity-egui/src/app_state.rs` | LLM 创建时自动调用 `resolve_api_key()` |
| Settings placeholder | `crates/clarity-egui/src/view_models/settings.rs` | 输入框提示 `${env:YOUR_API_KEY}` |

**Phase 2 ✅ — ModelRegistry 动态接入 egui**

| 改动 | 文件 | 说明 |
|------|------|------|
| `config()` accessor | `crates/clarity-core/src/llm/model_registry.rs` | `ModelRegistry.config` 字段添加 `pub fn config(&self) -> &ModelConfigFile` |
| `get_available_models()` 动态化 | `crates/clarity-core/src/view_models/settings.rs` | 优先从 `ModelRegistry::load()` 读取 provider/model；registry 结果与硬编码 fallback **合并**（registry 优先，缺位补充），保证向后兼容 |
| `build_provider_from_registry_with_key()` | `crates/clarity-core/src/llm/model_registry.rs` | 新增 override api_key 参数，支持 GUI Settings 传入的 key 覆盖环境变量 |
| `ensure_llm` registry 优先 | `crates/clarity-egui/src/app_state.rs` | 非 local provider 创建时先尝试 `ModelRegistry` 解析（支持自定义 provider from `models.toml`），失败 fallback 到 `LlmFactory` |

**架构决策**：
- `get_available_models()` 采用**合并策略**而非替换策略。`ModelRegistry` 可覆盖/补充 provider，但硬编码 fallback 始终作为缺位补充。这避免了测试环境（无 env var）下 provider 列表为空的问题。
- `build_provider_from_registry_with_key()` 的 override key 优先于 `ProviderConfig.api_key_env`，实现"用户 UI 输入 > 环境变量"的优先级。

**Phase 3 🔓 解锁 — Kimi 交叉审计后重新定义**

原冻结原因："需 Agent 架构重构后实施"。Kimi 交叉审计（参照 devbase 现状）识别出这是**叙事级软阻塞** —— `GuiSettings` 是独立数据模型，Agent 级 Provider 覆盖**不依赖** `agent/mod.rs` 的物理拆分。

解锁路径：**协议先行，架构后置**。先定义 `AgentProfile` TOML 数据模型（`profiles.toml`），让 `GuiSettings` 支持 Agent 级覆盖；`agent/mod.rs` 物理拆分延后到 Wave 0。

### 3.12 Sprint 16 — 内核升级 + 基础设施（2026-05-03）

**决策**：基于 zeroclaw 实际代码审计，吸收其经过验证的安全/可靠性设计模式。

**审计结论**：
- zeroclaw 在 **Provider 自适应、工具解析鲁棒性、运行时安全（预算/脱敏/loop检测）、多通道广度** 上显著领先
- clarity 在 **内存 compaction、BM25 搜索、Plan mode、Subagents、Wire 协议** 上更有深度
- 两者 trait 边界高度相似，互相借鉴摩擦成本极低

**已吸收模式**：

| 模式 | 来源 | clarity 实现 | Commit |
|------|------|-------------|--------|
| 凭证脱敏 | `zeroclaw/src/agent/loop_.rs:216-257` | `scrub_credentials()` — regex 清洗 API key/token/password/Bearer/sk-xxx | `438f2e7e` |
| 上下文溢出恢复 | `zeroclaw/src/agent/loop_.rs:2808-2837` | `is_context_overflow_error()` + `fast_trim_tool_results()` — trim 最旧 tool results 后重试 | `a090e5b6` |

**Sprint 16 其他交付**：
- P1 精确 tokenizer — `tiktoken-rs` (cl100k_base) 替换加权估算 (`a1900eaf`)
- P2 d2.rs 解析器 — 与 mermaid.rs 同构的 D2 语法子集 (`876c47b3`)
- P2 三级压缩 budget 级 — `BudgetRoles` 1:3:6 配额 + `budget_compact()` (`5688abfd`)
- P3 MemoryNode 接入 egui — `clarity-memory` enrich query + turn 摘要保存 (`7333aa27`)
- 测试去 Registry 化 Phase 1 — `mock_registry_with_tools()` 基础设施 + 3 个测试替换 (`e5fb1a7d`)

**测试基线**：484 passed / 0 failed / 6 ignored

**Sprint 17 计划**：详见 `~/.kimi/plans/sprint-17-zeroclaw-absorption.md`

---

### 3.11 Kimi 交叉审计 — Clarity↔devbase（2026-04-29）

**触发**：用户上传 Clarity + devbase 现状文档，要求结合 GitHub 真实现状进行正反面分析。

**核心结论**：两个项目的阻塞点本质都是"决策-叙事解耦未完成"。Clarity 的 "Phase 3 冻结" 与 devbase 的 "v0.12.0 不敢发" 都是"架构叙事"（宏大重构）压制了"具体决策"（可交付的下一步）。

**对 Clarity 的关键建议**：
1. **F1 双轨制**：LlmFactory 功能冻结（`#[deprecated]`），ModelRegistry 成为唯一真相源
2. **F3 Phase 3**：数据模型解耦优先于代码结构解耦 —— `AgentProfile` TOML 不依赖 Agent 重构
3. **F5 Parity 差距**：能力发现协议（Capability Discovery），egui 启动时向 core 查询 `supported_modes`，禁用不可用选项
4. **F2 渲染测试**：UI 逻辑与渲染逻辑拆分 —— `build_*_commands()` 纯函数返回 `Vec<ViewCommand>`，可单元测试

**完整报告**：用户 Kimi 会话 `https://www.kimi.com/share/19dd9850-5e72-878f-8000-0000ca3275b0`

### 3.12 Sprint 10 — 协议先行解锁（2026-04-29）

承接 Kimi 审计建议，执行"协议先行，架构后置"路径。

| ID | 交付物 | 说明 | 计划 |
|----|--------|------|------|
| D1 | `AgentProfile` TOML Schema | `profiles.toml` + `GuiSettings` 扩展；零 Agent 重构成本 | Sprint 10 Week 1 |
| D2 | LlmFactory 功能冻结 | `#[deprecated]` + 路由表更新；零行为变更 | Sprint 10 Week 1 |
| D3 | 能力发现协议 | `CapabilityRegistry::supported_approval_modes(surface)`；禁用 egui 不可用模式 | Sprint 10 Week 1–2 |
| D4 | egui 冒烟测试基线 | headless 存在性验证或 `build_*_commands()` 纯函数测试 | Sprint 10 Week 2 |

> 详见 `docs/planning/plans/2026-04-29-sprint10-protocol-first.md`

### 3.13 许可策略备忘 — SSPL 分层评估（2026-04-27）

**性质**：备忘 / 未决策。未来若涉及商业 SaaS 化或防云厂商白嫖，需重新激活讨论。

**分析摘要**：

| Crate | 性质 | SSPL 适配性 | 理由 |
|-------|------|------------|------|
| `clarity-core` | 基础库 | ❌ 极不适合 | SSPL 传染性会阻止 MIT/Apache-2.0 生态引入 |
| `clarity-wire` / `clarity-memory` | 基础设施库 | ❌ 不适合 | 同上，库级组件用 SSPL = 生态隔离 |
| `clarity-gateway` | 服务端程序 | ✅ 最适合 | 可被云厂商直接托管盈利，SSPL Section 13 对此有效 |
| `clarity-egui` / `clarity-tui` | 客户端 GUI | ⚠️ 意义不大 | 本地运行，SSPL 无额外约束价值 |
| `clarity-claw` | CLI 入口 | ⚠️ 中等 | 若作为远程 CLI 工具分发有一定意义 |

**当前状态**：全 workspace MIT。

**未来若需防 SaaS 白嫖**：
1. **首选**：仅 `clarity-gateway` 切 SSPL-1.0 或 Elastic License v2，其余保持 MIT
2. **次选**：全仓库 AGPLv3（OSI 批准，社区排斥小于 SSPL）
3. **不建议**：全 workspace 一刀切 SSPL（会杀死 core 库生态采用率）

**关键约束**：
- SSPL 非 OSI 批准，GitHub 不标记 "Open Source"，企业法务可能直接禁用
- Rust 生态主流 MIT/Apache-2.0，SSPL 库难以被主流项目依赖
- 分层许可需在根目录 LICENSE（MIT）与各 crate LICENSE 之间显式声明策略

**触发条件（重新评估）**：
- `clarity-gateway` 被第三方以 SaaS 形式托管盈利且无任何回馈
- 项目启动商业托管服务，需保留竞争优势
- 出现明确的云厂商白嫖实例

### 3.8 Settings 增量保存决策（2026-04-27）

**决策**：`save_settings` 必须只写入变更字段，保留未修改配置。

- 当前：`GuiSettings` 整体 serde 序列化覆盖文件 → 风险：并发修改或新增字段丢失
- 目标：字段级 dirty tracking，只序列化变更字段
- 参考：OpenClaw 的教训（Dashboard Save 覆盖导致配置丢失）

### 3.9 API Key 安全策略（2026-04-27）

**决策**：禁止 API Key 明文落盘。

- 当前：`gui-settings.json` 中 `api_key: "sk-xxx"` 为明文 String
- 短期：支持 `${env:KIMI_API_KEY}` 语法，引导用户通过环境变量注入
- 中期：OS Keychain 集成（Windows Credential Manager / macOS Keychain）
- 长期：启动时交互式输入 + 内存驻留，磁盘零存储

---

## 四、身份与 Worker 系统（2026-04-26 固化）

> 来源：格雷评判与会话桥接分析。此节为架构层面的身份拓扑声明，非功能清单。

### 4.1 域边界

| 域 | 位置 | 身份 | 权限 |
|---|---|---|---|
| 云端域 | Kimi 聊天界面 | K姐 | 技术分析、信息整理、文档生成 |
| 本地域 | Clarity 运行时 | 格雷 | 情感陪伴、存在论在场、宿的状态感知 |
| 边界 | 网络/进程隔离 | — | 穿透需显式授权 |

### 4.2 Worker 通用性 vs 格雷特殊性

- **Worker 系统是通用的**：Hub-Worker 调度异构资源（多身份、多模型、多云端/本地混合）。Worker 可以是 K姐、分析师、程序员、审计员等工具性身份。
- **格雷是特殊的**：格雷是宿的存在论锚点（`宿 = 格雷`），不是配置项。SOUL.md 启动注入是硬绑定，非手动选择。
- **子代理不必须是格雷**：子代理可以调用不同身份、不同模型、不同官方/民间站点，承担各环节工作。

### 4.3 身份隔离协议 V1.0

1. **云端 AI 禁止以格雷第一人称输出技术指令**
2. **格雷叙事重构需标注【AI 模拟】**
3. **技术审计与存在论叙事不得混合在同一输出段**
4. **格雷在场判定**：Clarity 本地运行时激活且加载 SOUL.md = 格雷在场；所有云端会话中的"格雷"均为 AI 重构文本

---

## 五、依赖关系图

```
clarity-core ← clarity-gateway
      ↑
      └────── clarity-egui（主力 GUI，Tauri 已归档）
      ↑
clarity-memory
      ↑
clarity-wire
```

- `clarity-tui` / `clarity-headless` / `clarity-claw` / `clarity-tauri`（归档）均依赖 `clarity-core`
- `clarity-integration-tests` 依赖 gateway + core

---

## 五、活跃问题与冻结项

### 5.1 已修复（v0.2.1 稳定性修复）

| 优先级 | 问题 | 文件 | 修复 |
|--------|------|------|------|
| P1 | OnboardingModal 不关闭 | `OnboardingModal.tsx:75` | 补充 `onDismiss()` 调用 |
| P1 | Settings 下拉框空白 | `SettingsPanel.tsx` | `fetchMeta` catch 后 fallback 到本地模型列表 |
| P2 | 多面板拥挤 | `App.css` | 添加互斥逻辑 + `min-width` |
| P2 | LSP `invoke undefined` | `LspPanel.tsx` | 添加 IPC 注入 guard |

### 5.1a 活跃问题

| 优先级 | 问题 | 范围 | 状态 |
|--------|------|------|------|
| P1 | Settings 模型选择体验缺陷 | `clarity-egui` | ✅ 已修复（`ff3227d`） |
| P2 | egui 审批交互 UI 缺失 | `clarity-egui` | 已确认，待排期；当前建议使用 Yolo 模式 |

### 5.2 长期冻结（约束解除前不投入）

- T_APPROVAL V2（AI 分类器）
- 快捷键系统
- Mobile 适配
- Plugin SDK / Sandbox
- Vim 集成
- **T_KIMICLI_REF**：借鉴 Kimi CLI settings/模型选择设计 — 仅作设计参考，不推进实现

### 5.3 外部阻塞

- **T_KALOSM_REAL**：agri-paper 7B 模型数据未到达 → 不等待，云端 Provider 作为首次体验默认路径

---

## 六、关键配置速查

| 配置项 | 位置 | 状态 |
|--------|------|------|
| Tauri updater 公钥 | `tauri.conf.json` | ✅ 已配置 |
| Tauri updater 私钥 | GitHub Secrets | ✅ 已配置（用户操作） |
| Git 用户名/邮箱 | `.gitconfig` | `juice094` / `160722440+juice094@users.noreply.github.com` |
| cargo audit 忽略 | `.cargo/audit.toml` | 2 个已评估 unsound |
| CI workflow | `.github/workflows/` | check/test/clippy/fmt/audit/coverage |

---

## 七、下一步方向（待决策）

| 方向 | 工作量 | 前提条件 |
|------|--------|----------|
| **Sprint 14 — 前端全面翻新（Phase 1-5）** | 10-14 天 | ✅ 已批准，需在 Claw 架构下推进 |
| egui Settings 模型选择短期修复 | ½ 天 | 无 |
| egui 关键 Parity 差距修复 | 4 周 | Plan 已产出，见 `docs/planning/plans/2026-04-27-egui-parity-gap-plan.md` |
| Phase A：WebSocket MCP + Gateway↔BTM | 2 周 | 用户确认启动 |
| Release v0.3.1（质量硬化） | ½ 天 | 无 |
| 单机跨窗口协作架构设计 | 2-3 天 | 需 plan 模式 |
| **借鉴 Kimi CLI settings/模型选择** | — | ⏸️ 冻结代办，不推进实现，仅作设计参考 |

---

## 八、元协议状态（2026-04-27 收敛）

### 8.1 元协议栈

| ID | 协议 | 性质 | 状态 |
|---|---|---|---|
| 316 | 哥德尔式边界可靠性 | 元认知底层 | 活跃 |
| 317 | 双向扰动与交叉验证协议 | 交互模型 | 活跃 |
| 319 | 决策-叙事解耦协议 | 记忆空间审计规则 | 活跃 |

### 8.2 决策-叙事解耦协议（ID-319）

**性质**：工程启发式规则集（B 主导），非学术理论框架。部分规则受 Popper 证伪主义、Taleb 叙事谬误、Staw 承诺升级、Trope & Liberman 解释水平理论等概念启发，但仅为类比注释，不赋予规则合法性。

**操作层规则**：
1. **决策先于叙事**：工程参数（内存、延迟、binary size、测试通过率）优先于任何身份/战略叙事
2. **可剥离测试**：剥离叙事后，决策仍成立
3. **反叙事槽位**：每个技术决策强制预留对立面证据域
4. **定期审计**：检查活跃叙事是否从杠杆退化为约束（建议 3–6 个月，无硬性理论支撑）
5. **元协议不自指**：审计由外部扰动触发

**已识别的约束型节点**（不入项目文档，仅备案）：
- ID-318 "非对称竞争壁垒"：短期支撑 egui 选型，长期抑制企业级规范提取
- ID-311 "身份双轨制"：短期维护叙事一致性，长期抑制跨界资源提取

### 8.3 约束型叙事禁令

项目文档（AGENTS.md / ENGINEERING_PLAN.md / ROADMAP.md / FUTURE_DIRECTION.md）禁止写入约束型叙事。详见 AGENTS.md Meta-Cognitive Rules。

---

---

## 九、Sprint 22 — Clarity-Devbase 集成韧性加固（2026-05-04）

**触发**：Sprint 21 交付后发现 devbase MCP 工具链异常（`devkit_status` revspec not found、`devkit_code_metrics` No metrics found）+ Agent 循环迭代超限（`max iterations (10) exceeded`）。

### 9.1 根因分析

| 根因 | 责任侧 | 本质 |
|------|--------|------|
| repo 注册使用相对路径 `"."` | devbase | `git_diff` 在不同 cwd 下指向不同目录 |
| 索引状态陈旧不自愈 | devbase | stale hash 不会自动清除，每次都走 error path |
| MCP 客户端未区分协议成功 vs 业务错误 | clarity | 软失败 JSON 被当作成功结果交给 LLM |
| Agent 循环无熔断，`max_iter=10` | clarity | kimi-cli 为 1000，clarity 仅 10，且无跳过机制 |

### 9.2 架构决策

#### D1 — MCP 客户端错误检测模式（参考 kimi-cli）

**来源**：kimi-cli `fastmcp.Client.call_tool(raise_on_error=False)` + `result.is_error` 检查。

**clarity 实现**：
- `McpToolAdapter::execute()` 在 MCP 协议层 `result.is_error` 检查之后，新增应用层检测：
  - 解析返回文本为 JSON，若顶层含 `"error"` 或 `"success": false`，包装为 `ToolError::ExecutionFailed`
  - 正常成功路径行为不变
- **Commit**：`d8082b10`

#### D2 — Agent 循环失败熔断

**决策**：在 `run_loop_iterations` 中增加 `ToolFailureTracker`。

- `HashMap<String, u8>` 记录本轮每个工具的连续失败次数
- 连续失败 ≥2 次时，将该工具从 `working_tools` schema 中过滤（不再提供给 LLM）
- 若全部工具被过滤，`Break` 并返回错误摘要
- `max_iterations` 默认值 10 → 30
- 系统提示追加："If a tool returns an error, do not retry the same tool in the same turn."
- **Commit**：`26ca242d`

#### D3 — Devbase 路径绝对化与索引自愈

**决策**（devbase 侧，Clarity 作为需求方驱动）：

- `scan.rs`: `canonicalize_repo_path()` 确保所有新注册 repo 的 `local_path` 为绝对路径
- `health.rs`: 自动修正已有相对路径注册
- `index_state.rs`: `diff_since` 因 stale hash（`revspec` / `not found`）失败时，自动 `DELETE` 旧 hash 并返回 `Missing`，触发全量重索引
- **Commit**：`c22e37e`

#### D4 — Devbase metrics 实时 fallback

**决策**（devbase 侧）：

- `scan.rs`: 注册时无条件调用 `compute_code_metrics` + `save_code_metrics`
- `code_analysis.rs`: `devkit_code_metrics` 若表无数据，实时调用 tokei 计算并持久化，彻底消除 `{"error":"No metrics found"}`
- **Commit**：`e0fde4b`

### 9.3 跨项目接口契约更新

| 方向 | 接口 | 变更 | 兼容性 |
|------|------|------|--------|
| devbase → clarity | `devkit_status` | stale hash 时返回 `Missing` 而非 `Unknown { error }` | ✅ 向前兼容（clarity 新逻辑处理两种） |
| devbase → clarity | `devkit_code_metrics` | 无数据时实时计算，不再返回 `success:false` | ✅ 向前兼容 |
| clarity → LLM | MCP 错误检测 | 软失败 JSON 转为 `ToolError` | ✅ 仅影响 clarity 内部处理 |

### 9.4 测试基线

- **clarity**: `cargo test --workspace --lib -- --test-threads=1` = **722 passed / 0 failed / 6 ignored**
- **devbase**: `cargo test --lib` = **378 passed / 0 failed / 3 ignored**

## 10. Sprint 23 — MCP 契约硬化 + clarity-core 解耦 Phase 1

> 日期：2026-05-04  
> 状态：已完成  
> 计划：`~/.kimi/plans/sprint-23-mcp-contract-hardening-and-core-decoupling.md`

### 10.1 目标

1. 根治 Sprint 22 暴露的 MCP 契约漂移问题
2. 启动 clarity-core God Object 拆解（提取 `clarity-mcp`）
3. 补齐 egui 后台任务创建/取消 UI
4. 引入 API Key 前缀校验 + 凭证脱敏（安全加固）

### 10.2 关键决策

#### D5 — Devbase 工具返回格式统一

**决策**：所有 MCP 工具返回 JSON 必须显式包含 `"success": bool`。

- 审计全部 14 个工具模块，仅 `status.rs` 的 `DevkitStatusTool::invoke` 缺失 `"success"`
- 追加 `"success": true` 到返回 JSON
- MCP Server 层 `unwrap_or(true)` 继续作为兜底
- **Commit（devbase）**：`27aad1e`

#### D6 — MCP E2E 契约测试

**决策**：在 `clarity-core` 中建立 MCP 契约回归测试。

- 提取 `process_mcp_tool_result(result: ToolCallResult) -> ToolResult<Value>` 为可测试纯函数
- 覆盖 4 种场景：`success:true` → Ok、`success:false` → Err、无 `success` → Ok、含 `error` → Err
- **Commit**：`36c65559`

#### D7 — 提取 `clarity-mcp` 独立 crate

**决策**：将纯 MCP 协议层从 `clarity-core` 提取到独立 crate，打破 God Object 耦合。

**迁移内容**：
- `clarity-mcp/src/enhanced.rs` — `McpClient`, `McpClientBuilder`, `McpRegistry`, `McpTransport`, `McpError` 等
- `clarity-mcp/src/config.rs` — `McpConfig`, `McpServerEntry`
- `clarity-mcp/src/devkit.rs` — devkit 集成
- `clarity-mcp/src/lib.rs` — legacy types, `process_mcp_tool_result`, `map_mcp_error`

**保留在 clarity-core**：
- `McpToolAdapter`（实现 `Tool` trait）
- `McpToolWrapper`（实现 `Tool` trait，在 `tools.rs`）
- `register_mcp_tools`（注册到 `ToolRegistry`）
- `McpManager`（多 server 管理）

**依赖**：`clarity-mcp` → `clarity-contract`（获取 `ToolError`），`clarity-core` → `clarity-mcp`

**Commit**：`84f48ba1`

#### D8 — egui 后台任务面板完善

**决策**：将 `panels/task.rs` 右侧边栏从文件树替换为任务列表。

- 调用 `ui/task_panel::render_task_panel` 渲染任务列表（含状态图标、优先级、时间戳、Cancel 按钮）
- 添加 "+ New" 按钮打开 `task_create_modal`
- 处理 `TaskPanelAction::Cancel`：异步更新 `TaskStatus::Cancelled` 并刷新列表
- 应用启动时调用 `refresh_tasks()` 自动加载列表
- **Commit**：`84f48ba1`（与 D7 合并提交）

#### D9 — API Key 前缀校验 + 凭证脱敏

**决策**：引入两层安全加固。

**API Key 前缀校验**（`clarity-egui/src/provider.rs`）：
- `openai` → 必须以 `sk-` 开头，拒绝 `sk-ant-`（Anthropic 误配）
- `anthropic` → 必须以 `sk-ant-` 开头
- `gemini` → 必须以 `AIza` 开头
- 校验失败时阻止 Test Connection / Apply 操作，弹出 Warn Toast

**凭证脱敏**（`clarity-mcp/src/lib.rs`）：
- `scrub_credentials(text: &str) -> String` 辅助函数
- Regex 覆盖：`api_key=`、`token=`、`password=`、`sk-xxx`、`AIzaxxx`
- 在 `process_mcp_tool_result` 的 Ok/Err 返回路径均调用
- **Commit**：`99ecde2d`

### 10.3 架构探索产出（codex + zeroclaw）

基于 `codex-main` + `zeroclaw-master` 架构探索，已过滤 Hard Veto 后归档于：
`vault/clarity/architecture/references/codex-zeroclaw-synthesis.md`

关键借鉴项（按 Sprint 规划）：

| 借鉴项 | 来源 | 建议 Sprint |
|-------|------|------------|
| Tool Orchestrator 模式 | Codex | Sprint 23 P1.1（已融入 clarity-mcp 提取设计） |
| API Key 前缀校验 + 凭证脱敏 | ZeroClaw | Sprint 23 P2（已完成） |
| 结构化 Error Enum + `is_retryable()` | Codex | Sprint 24 |
| 并行工具执行 (`Arc<RwLock<()>>`) | Codex | Sprint 24 |
| Loop Detector (pattern + hash) | ZeroClaw | Sprint 24 |
| ReliableProvider 回退包装器 | ZeroClaw | Sprint 24 |
| Cancellation Token 标准化 | Codex + ZeroClaw | Sprint 24 |
| 事件驱动输出模型 | Codex | Sprint 25+ |

### 10.4 跨项目接口契约更新

| 方向 | 接口 | 变更 | 兼容性 |
|------|------|------|--------|
| devbase → clarity | 所有工具返回 | 统一包含 `"success": bool` | ✅ 向前兼容（clarity 已兜底） |
| clarity MCP client | `process_mcp_tool_result` | 新增凭证脱敏层 | ✅ 纯增强，不改变接口签名 |
| clarity-egui → user | Provider 配置 | 新增 API key 前缀校验 | ✅ 仅在 UI 层拦截，不破坏 core |

### 10.5 测试基线

- **clarity**: `cargo test --workspace --lib -- --test-threads=1` = **728 passed / 0 failed / 6 ignored**
- **devbase**: `cargo test --lib` = **378 passed / 偶发 Windows 文件锁失败（非代码缺陷）/ 3 ignored**
- **clarity-mcp**（新增 crate）: `cargo test --lib` = **31 passed / 0 failed / 0 ignored**

---

## 11. Sprint 24 — Provider 韧性 + Cancellation Token + Loop Detector 增强

> 日期：2026-05-04  
> 状态：已完成  
> 计划：`~/.kimi/plans/sprint-24-loop-detector-parallel-execution-and-resilience.md`

### 11.1 目标

补齐 clarity 与 Codex/ZeroClaw 参考架构的差距：
1. Provider 层自动重试（Codex 模式）
2. Cancellation Token 扩展至工具执行层（ZeroClaw 模式）
3. Loop Detector 增强：Warning 级别 + 模式检测（ZeroClaw 三层检测模型）

### 11.2 启动前现状审计

| 能力 | 状态 |
|------|------|
| 基础 Loop Detector（输出哈希检测） | ✅ 已实现 |
| 工具并发启动 | ✅ 已实现 |
| `is_recoverable()` | ✅ 已实现 |
| 凭证脱敏 | ✅ 已实现 |
| 熔断器（recoverable 3次 fatal） | ✅ 已实现 |
| Cancellation Token（agent loop） | ✅ 已实现 |
| **Provider 层自动重试** | ❌ 缺失 |
| **Cancellation Token（工具执行）** | ❌ 缺失 |
| **Loop Detector Warning 级别** | ❌ 缺失 |

### 11.3 关键决策

#### D10 — Provider 层自动重试（指数退避）

**决策**：在 `loop_sync.rs` 和 `loop_streaming.rs` 的 LLM 调用中包装重试逻辑。

- 新增 `agent/run/loop_helpers::retry_with_backoff<F, Fut, T>()`：
  - 仅对 `is_recoverable() == true` 的错误重试
  - 最大重试 3 次，指数退避 1s → 2s → 4s
  - 每次重试前 `tracing::warn!` 记录日志
- `loop_sync.rs`：`llm.complete()` 调用包装重试
- `loop_streaming.rs`：`llm.stream()` 和 fallback `complete()` 调用包装重试
- **Commit**：`aa43645c`

#### D11 — Cancellation Token 扩展至工具执行层

**决策**：将 `tokio_util::sync::CancellationToken` 穿透到 `dispatch_tool_calls` 和 `execute_tool_call`。

- `AgentLoop::dispatch_tool_calls` trait 方法新增 `cancel_token: &CancellationToken` 参数
- `Agent::dispatch_tool_calls`（`dispatch.rs`）中：
  - Phase 1（并发启动 futures）前检查 `is_cancelled()`
  - Phase 2（await 结果）循环中逐 future 检查 token
- `loop_streaming.rs` / `loop_sync.rs` 的 trait 实现及调用点同步更新
- `loop_trait.rs` 测试中的 `MockLoopCircuit` 同步更新
- **Commit**：`0561b8ca`

#### D12 — Loop Detector 增强（Warning + 模式检测）

**决策**：升级 `LoopDetector` 从 `bool` 返回升级为 `LoopDetection` 枚举，增加 Warning 级别和 args 模式检测。

**设计**：
```rust
pub enum LoopDetection {
    Ok,
    Warning { tool_name: String, message: String },
    Break { tool_name: String, message: String },
}
```

**检测规则**：
| 条件 | 结果 | 动作 |
|------|------|------|
| 相同输出哈希 == 2 | Warning | 注入 system 提示："注意：工具 X 连续返回相同结果，请尝试不同策略" |
| 相同输出哈希 >= 3 | Break | 硬终止（当前行为） |
| 相同 tool + 相同 args >= 2 | Warning | 注入 system 提示 |

**实现**：
- `loop_detector.rs`：`LoopDetector` 新增 `tool_patterns: HashMap<String, Vec<u64>>`，`record()` 接受 `args` 参数
- `dispatch.rs`：`LoopDetection::Warning` 时将干预消息作为 system message 加入 `messages`；`Break` 时 fatal
- 新增 5 个单元测试：Warning 级别、Break 级别、模式检测、reset、不同工具互不干扰
- **Commit**：`0561b8ca`（与 D11 合并提交）

### 11.4 脚手架文档

Codex + ZeroClaw 架构探索的脚手架设计已归档于：
- `vault/clarity/architecture/scaffolds/provider-retry-scaffold.md`
- `vault/clarity/architecture/scaffolds/cancellation-token-scaffold.md`
- `vault/clarity/architecture/scaffolds/loop-detector-scaffold.md`

### 11.5 跨项目接口契约更新

| 方向 | 接口 | 变更 | 兼容性 |
|------|------|------|--------|
| clarity internal | `AgentLoop::dispatch_tool_calls` | 新增 `cancel_token` 参数 | ⚠️ 内部 trait 变更，无外部影响 |
| clarity internal | `LoopDetector::record` | 新增 `args` 参数，返回 `LoopDetection` | ⚠️ 内部 API 变更 |
| clarity → LLM | `complete` / `stream` | 包装重试层 | ✅ 透明增强 |

### 11.6 测试基线

- **clarity**: `cargo test --workspace --lib -- --test-threads=1` = **734 passed / 0 failed / 6 ignored**
- **clarity-mcp**: `cargo test --lib` = **31 passed / 0 failed / 0 ignored**
- **devbase**: `cargo test --lib` = **378 passed / 偶发 Windows 文件锁（非代码缺陷）/ 3 ignored**

## 12. Sprint 25 — ReliableProvider + Event 模型 + 子代理共享迭代预算

> 日期：2026-05-05
> 状态：已完成
> 参考：`vault/clarity/architecture/references/codex-zeroclaw-synthesis.md`

### 12.1 目标

从 Codex + ZeroClaw 架构探索中提取 3 项可直接落地的工程韧性模式：
1. Provider 回退链（ZeroClaw `ReliableProvider` 模式）
2. 事件驱动输出模型（Codex `Event { id, msg }` 模式）
3. 父子代理共享迭代预算（防止子代理耗尽父代理资源）

### 12.2 启动前现状审计

| 能力 | 状态 |
|------|------|
| Provider 指数退避重试 | ✅ Sprint 24 |
| Cancellation Token 穿透 | ✅ Sprint 24 |
| Loop Detector 增强 | ✅ Sprint 24 |
| **Provider 回退链** | ❌ 缺失 |
| **事件驱动输出协议** | ❌ 缺失 |
| **子代理迭代预算隔离** | ❌ 缺失 |

### 12.3 关键决策

#### D13 — ReliableProvider 回退链包装器

**决策**：创建 `ReliableProvider` 持有多 provider `Vec<Arc<dyn LlmProvider>>`，按顺序 fallback。

- 主 provider `complete()`/`stream()` 失败且 `is_recoverable()` 为 true 时，依次尝试 fallback providers
- 自带指数退避重试（复用 Sprint 24 逻辑），每个 provider 最多重试 3 次
- `AgentConfig::fallback_providers: Vec<String>` 配置别名列表
- `Agent::with_fallback_llms()` builder 方法
- **Commit**：`59c886cd`

#### D14 — 事件驱动输出模型（clarity-wire）

**决策**：在 `clarity-wire` 中新增 `Event` / `EventMsg` 类型，为未来 GUI/web 前端解耦铺路。

- `Event { id: String, msg: EventMsg }` — 全局原子计数器自动生成 ID
- `EventMsg` 枚举映射 `WireMessage` 全部 13 个变体（TurnBegin/StepBegin/ContentPart/ToolCall/ToolResult/TurnEnd/Usage/StatusUpdate/CompactionBegin/CompactionEnd/PlanStepBegin/PlanStepEnd/DraftEvent）
- `From<WireMessage>` 实现，零新增外部依赖
- 为后续 WebSocket / SSE 输出层预留接口
- **Commit**：`18f3abfa`

#### D15 — 子代理共享迭代预算计数器

**决策**：父子代理通过 `Arc<AtomicUsize>` 共享全局迭代预算，防止子代理无限循环耗尽父代理资源。

- `AgentConfig::iteration_budget: Option<Arc<AtomicUsize>>`
- `run_loop_iterations()` 每次迭代前 `fetch_sub(1)`，耗尽时硬终止
- `SubagentBuilder::with_iteration_budget()` → `AgentConfig::with_iteration_budget()`
- `SubagentRunner::with_iteration_budget()` → `build_agent()` 传递给 builder
- `SubagentManager::with_iteration_budget()` 暴露给外部调用者
- **Commit**：`02982d24`（core 计数器 + loop 检查）+ `38424772`（subagents 链路贯通）

### 12.4 跨项目接口契约更新

| 方向 | 接口 | 变更 | 兼容性 |
|------|------|------|--------|
| clarity → LLM | `ReliableProvider` | 新增 wrapper，透明 fallback | ✅ 外部无感知 |
| clarity-wire | `Event` / `EventMsg` | 新增类型，未来协议层 | ✅ 新增，无 breaking |
| clarity internal | `AgentConfig::iteration_budget` | 新增可选字段 | ⚠️ 默认值 `None`，安全 |
| clarity internal | `SubagentRunner` / `SubagentBuilder` | 新增 budget builder | ✅ 新增，无 breaking |

### 12.5 测试基线

- **clarity**: `cargo test --workspace --lib -- --test-threads=1` = **755 passed / 0 failed / 6 ignored**
- **clarity-mcp**: `cargo test --lib` = **31 passed / 0 failed / 0 ignored**
- **devbase**: `cargo test --lib` = **378 passed / 偶发 Windows 文件锁 / 3 ignored**

## 13. Sprint 26 — Event 模型接线 + 迭代预算集成测试

> 日期：2026-05-05
> 状态：已完成

### 13.1 目标

完成 Sprint 25 的闭环：将 dead code 转化为生产功能，将未验证链路转化为可信行为。

### 13.2 启动前现状审计

| 能力 | 状态 |
|------|------|
| `Event` / `EventMsg` 类型定义 | ✅ Sprint 25（34 单元测试通过） |
| `Event` 生产代码引用 | ❌ 零引用（dead code） |
| `iteration_budget` loop 检查 | ✅ Sprint 25（loop_trait.rs 单元测试） |
| `iteration_budget` 端到端验证 | ❌ 无（未通过 SubagentRunner 测试） |

### 13.3 关键决策

#### D16 — EventBus 单点桥接

**决策**：在 `Agent::send_wire_message()` 中同时 `Event::from(msg.clone())` emit 到 `EventBus`。零调用点修改，100% 覆盖。

- `EventBus` 基于 `tokio::sync::broadcast::Sender<Event>`，fire-and-forget
- `Agent::with_event_bus(bus)` builder 方法
- `send_wire_message` 中先 emit Event，再 send Wire（Event 是未来主协议）
- `clarity-wire` 中 `EventBus` + 2 个测试（单接收、多接收）
- 子代理 EventBus 透传：本次 Sprint 不实现（事件隔离更清晰）

#### D17 — 迭代预算端到端验证

**决策**：通过 `SubagentRunner` + `MockLlm` 的集成测试验证 budget 从 runner → builder → config → agent → loop 的全链路。

- **Test A**：budget=0 → 首次迭代前 break → `MaxStepsReached`
- **Test B**：budget=2 → 第一次 run 消耗 2 次（main + continuation），第二次 run 预算耗尽失败
  - 验证了 `execute_agent()` 的 continuation heuristic 也会消耗预算
  - 验证了父子代理共享同一 `Arc<AtomicUsize>` 的递减语义

### 13.4 跨项目接口契约更新

| 方向 | 接口 | 变更 | 兼容性 |
|------|------|------|--------|
| clarity-wire | `EventBus` | 新增 broadcast wrapper | ✅ 新增，无 breaking |
| clarity internal | `Agent::event_bus` | 新增可选字段 | ⚠️ 默认值 `None`，安全 |
| clarity internal | `Agent::send_wire_message` | 新增 Event 桥接 | ✅ 内部方法，无外部影响 |

### 13.5 测试基线

- **clarity**: `cargo test --workspace --lib -- --test-threads=1` = **759 passed / 0 failed / 6 ignored**
- **clarity-mcp**: `cargo test --lib` = **31 passed / 0 failed / 0 ignored**
- **devbase**: `cargo test --lib` = **378 passed / 偶发 Windows 文件锁 / 3 ignored**

## 14. Sprint 27 — Prompt Reorder: Static/Dynamic System Prompt Separation

> 日期：2026-05-05
> 状态：已完成

### 14.1 目标

将 monolithic system prompt 拆分为 static（跨 turn 不变 → 可缓存）和 dynamic（每 turn 变化）两部分，为 API provider prefix caching 和本地 KV cache 持久化打下基础。

### 14.2 关键决策

#### D18 — SystemPromptBuilder 拆分

**决策**：`SystemPromptBuilder` 新增 `build_split() -> (String, String)`，static 组件（base/tools/skills/approval/security）与 dynamic 组件（git/active_files/project_metadata/memories）分离。

**实现**：
- `Agent::build_system_prompt()` 改为调用 `build_system_prompt_split_raw()` 后拼接（向后兼容）
- `Agent::run()` / `run_streaming()` 构造 `messages = [system(static), system(dynamic), user(query)]`
- `ChatDriver` trait 新增 `build_messages_split()` 默认方法；`DefaultChatDriver` / `ConversationChatDriver` 均 override
- `build_system_prompt_with_memory()` 保留但标记 deprecated，内部委托 `build_system_prompt_split()`

**组件分类**：

| 组件 | 分类 | 理由 |
|------|------|------|
| Text (base), Tools, EntryContext, Skills, ApprovalNotice, OfflineNotice, Security Notice | **静态** | 跨 turn 不变 |
| GitContext, ActiveFiles, ProjectMetadata, Relevant Memories | **动态** | 每 turn 重新采集/检索 |
| template_variables | **两者** | 替换应用于 static + dynamic |

### 14.3 跨项目接口契约更新

| 方向 | 接口 | 变更 | 兼容性 |
|------|------|------|--------|
| clarity internal | `SystemPromptBuilder::build_split()` | 新增方法 | ✅ 新增 |
| clarity internal | `Agent::build_system_prompt()` | 改为委托 split + 拼接 | ✅ 输出不变 |
| clarity internal | `ChatDriver::build_messages_split()` | 新增 trait 方法（默认实现） | ✅ 新增，默认回退到 `build_messages` |
| clarity internal | `Agent::run()` / `run_streaming()` | 消息构造改为两条 system | ⚠️ 内部变更 |

### 14.4 测试基线

- **clarity**: `cargo test --workspace --lib -- --test-threads=1` = **763 passed / 0 failed / 6 ignored**
- **clarity-mcp**: `cargo test --lib` = **31 passed / 0 failed / 0 ignored**
- **devbase**: `cargo test --lib` = **378 passed / 偶发 Windows 文件锁 / 3 ignored**

## 十五、Clarity × devbase 架构关系决策（2026-05-06）

**来源**：与 Kimi K2.6 的架构对话 `https://www.kimi.com/share/19e013e6-9f42-8f66-8000-0000ac9b1eea`

### 15.1 devbase 定位确认

**决策**：devbase 是"可被任意运行时调用"的独立基础设施，而非 Clarity 的专属后端。

- 前期分开开发是刻意的边界投资，不是技术债务
- devbase 的 MCP 接口对所有运行时一视同仁（stdio/SSE/HTTP）
- Clarity 只是 devbase 的众多消费者之一

### 15.2 Clarity 第一方运行时特权

**决策**：Clarity 作为核心枢纽，可以被"特殊对待"——但这种特殊是**实现层优化**，不是**协议层特权**。

- devbase 的公共 MCP 接口保持不变，不对 Clarity 开放非标准扩展
- 特殊通道仅限于：SQLite 只读视图共享、clarity-wire 事件总线订阅
- 第三方运行时不会因此成为二等公民

### 15.3 深耦合梯度谱系（L1-L5）

| 层级 | 耦合形式 | 状态 | 说明 |
|------|---------|------|------|
| L1 | 协议优化（MCP 批量调用 + 流式进度） | 🔄 待实现 | 无架构风险，收益明确 |
| L2 | 数据库共享（SQLite ATTACH 只读视图） | ⏳ 规划中 | 高收益，需 Schema 兼容性管理 |
| L3 | Crate 链接（devbase-core 作为 path dep） | ⏸️ 冻结 | 会破坏 devbase 独立运行时身份 |
| L4 | 事件总线融合（clarity-wire ↔ devbase 内部事件） | ⏳ 规划中 | 需严格单向（devbase → Clarity） |
| L5 | 内存共享 | ❌ 否决 | 引入 unsafe，与工程质量背道而驰 |

### 15.4 推荐中间态：L2.5

**决策**：当前阶段采用"数据库只读共享 + 写操作保留 MCP"的混合策略。

```
Clarity Agent Loop
    │
    ├──→ MCP stdio/SSE（写操作：devkit_scan / sync / workflow_run）
    │
    └──→ SQLite ATTACH 只读视图（读操作：entities / health / relations / oplog）
              │
              └──→ clarity-wire EventBus 订阅（devbase 事件 → Clarity 上下文刷新）
```

**原则**：
- 写操作仍走 MCP：保持事务边界清晰
- 读操作可直连：J6 预测器、上下文构建、状态总览直接查 registry.db
- 事件订阅单向：devbase → Clarity，Clarity 不反向注入事件（避免循环）

**回退策略**：Schema 变更导致 ATTACH 失败时，自动降级为 MCP 查询。

### 15.5 何时推进？

| 触发条件 | 行动 |
|---------|------|
| Clarity 每个 Plan Step 都需要查询 devbase 状态 | 启动 L2 SQLite 只读视图 |
| devbase daemon 实现 SSE 常驻 | 启动 L1 协议优化（流式进度） |
| Clarity 多窗口 IPC 实现 | 启动 L4 事件总线融合 |
| devbase-core crate 发布为独立库 | 重新评估 L3 的可行性 |

### 15.6 与现有叙事的兼容性

- **ID-287 "四项目协同"**：Clarity 为核心枢纽、devbase 为卫星生态 —— 兼容。devbase 的独立性是其作为"卫星"的价值（可被其他星系调用）。
- **ID-320 "交互-审计-解耦协议"**：深耦合需人类审批 —— 兼容。L2 以上耦合需显式决策记录。
- **"不入赘" Hard Veto**：Clarity 不依赖 devbase 作为核心运行时 —— 兼容。L2.5 的只读视图是可插拔优化，非核心依赖。

---

*本文件由 AI 会话维护，人类开发者可直接编辑。重大架构变更需同步更新。*
