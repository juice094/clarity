# AI 关键决策记录 · Clarity

> 本文件记录跨 AI 会话的关键架构决策、状态锚点和 Hard Veto 边界。
> 用途：在上下文压缩后，新会话可快速恢复项目认知。
> 协议版本：V3.1-EP-O

---

## 一、当前会话锚点

**最后更新**：2026-04-29
**当前分支**：`phase2/protocol-pilot` @ `48006e5`（已推送 origin）
**架构模式**：CLI（单轮/短轮次）
**定位声明**：Clarity 是集群协作原语的单机验证运行时（非本地聊天工具）。
**会话状态**：
- v0.3.1 已发布（tag `v0.3.1`）：model_download + onboarding + unwrap 审计 + egui release CI
- clarity-tauri 完全归档移出仓库；Dependabot 报警清零
- Settings 模型选择缺陷修复；Mutex 硬化完成；App::update() 550→64 行拆分完成
- **新增**：OpenHanako / OpenClaw 服务商持久化调研完成，Provider 配置架构缺陷已识别
- **新增**：egui GUI 美化审计完成，UI/UX 问题清单已建立
- **Sprint 9 — 服务商支持硬化**：Phase 1 ✅ | Phase 2 ✅ | Phase 3 🔓 已解锁
- **Sprint 10 — 协议先行解锁**：D1 ✅ | D2 ✅ | D3 ✅ | D4 ✅
- **Sprint 11 — 超越 Kimi CLI**：Phase A 🔄 (上下文注入) | Phase B 🔄 (编辑精度) | Phase C 🔄 (终端体验)

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
| `docs/test_governance.md` | 新增第 8 章：定量基线表、unwrap 分类策略、验收命令、违规处理 |
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

> 详见 [`docs/plans/2026-04-29-sprint10-protocol-first.md`](./docs/plans/2026-04-29-sprint10-protocol-first.md)

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
| egui Settings 模型选择短期修复 | ½ 天 | 无 |
| egui Pretext 健康度运维硬化（三阶段） | 6 周 | Phase 1 已完成（settings 修复、Mutex 替换、`App::update()` 550→64 行拆分） |
| egui 关键 Parity 差距修复 | 4 周 | Plan 已产出，见 `docs/plans/2026-04-27-egui-parity-gap-plan.md` |
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

*本文件由 AI 会话维护，人类开发者可直接编辑。重大架构变更需同步更新。*
