# Clarity Project Status

> Last updated: 2026-05-11
> Branch: `main`
> Test baseline: **826 passed lib + 116 passed bin = 942 (workspace), 0 failed, 7 ignored**
> Clippy: **0 warnings** (`-D warnings`)
> Active ADR: **ADR-006 Protocol Layer Convergence** — Phase A/B/C complete; Phase D pending
> Active Sprint: **S3.1/S3.2 complete**（settings 审计 + 集中提交点）
>
> ⚠️ **并行会话协调（2026-05-11）**：前端设计相关调整正在**其他会话**进行中。
> 本会话已暂停 `crates/clarity-egui/src/` 下的代码改动。S3.3-5 待协调后启动。

---

## Release Chain Status (Shape Up Cycle)

| Sprint | Goal | Status | Key Deliverable |
|--------|------|--------|-----------------|
| 1 | Documentation止血 + 版本对齐 | ✅ Complete | CHANGELOG顺序修正、版本同步、README表述修正 |
| 2 | 单二进制打包验证 | ✅ Complete | MSI (`Clarity_0.2.1_x64_en-US.msi`, 8.5MB) + NSIS (`Clarity_0.2.1_x64-setup.exe`, 5.9MB) |
| 3 | CI闭环 | ✅ Complete | `.github/workflows/release.yml` `working-directory` 修复 |
| 4 | FTUE闭环 | ✅ Complete | `SettingsPanel` 保存后自动触发 `reload_llm` |
| 5 | 冷却验证 | ✅ Complete | 测试524 passed, Clippy零警告 |
| 6 | 可用性急救 | ✅ Complete | GUI API key 输入框 + `LlmFactory::create_with_key` — Clarity 真正可用 |
| 7 | UI 栈迁移 | ✅ Complete | `clarity-egui` 替代 `clarity-tauri` 成为 GUI 模式前端（Hybrid UI 的 GUI 腿） |
| 8 | egui 硬化 | ✅ Complete | Pretext Phase 1：settings 修复、Mutex 替换、`App::update()` 550→64 行拆分、onboarding 模型下载 |
| 9 | **服务商支持硬化** | ✅ Complete | Provider Schema 化、环境变量注入、Settings 增量保存、API Key 引用语法 `${env:VAR}` |
| 10 | **协议先行解锁** | ✅ Complete | AgentProfile TOML、LlmFactory 冻结、CapabilityRegistry、egui 冒烟测试 |
| 11 | **超越 Kimi CLI** | ✅ Complete | V1 风险清偿 + V2 端到端验证通过 |
| 12 | **egui 功能补齐** | ✅ Complete | 审批弹窗 → Plan 可视化 → Skill UI → Token 显示 |
| 13 | **安全止血 + 审批一致性** | ✅ Complete | A: 熔断/脱敏/Prompt边界；B: 超时/竞态/身份分层 |
| 13.5 | **UX Hardening** | ✅ Complete | Multiline + Draft Persistence + Steer + Smart Approval |
| Phase C | **架构解耦** | ✅ Complete | `ensure_llm` 三层解耦 + `list_pending` trait化 |
| 14 | **egui 设计系统硬化** | ✅ Complete | 配色深蓝灰+铜色、overlay阴影token、间距规范化、i18n、自绘标题栏 |
| 15 | **OAuth Provider 架构重构** | ✅ Complete | 5 Phase 泛化：KimiCode→OAuthLlm/OAuthTokenManager/OAuthDeviceFlowClient，新增 provider 零代码 |
| 16 | **契约层扩展 (Phase 0)** | ✅ Complete | `LlmProvider`/`Tool`/`AgentError`/`CapabilityToken` 提取到 `clarity-contract`，新增联邦原语 |
| 17 | **安全硬化 (P0 双项)** | ✅ Complete | Log 层 credential 脱敏 (`RedactingWriter`) + LLM prompt injection 防御 (`<tool_result>` XML 边界符) |
| 17.5 | **egui 全局快捷键 MVP** | ✅ Complete | `shortcuts` 模块集中化：Ctrl+N/Enter/K/Shift+P/Period/Shift+T；Enter 发送增加焦点守卫 |
| 18–20 | **架构解耦 + 工具止血 + UI 零边框** | ✅ Complete | `TurnContext` / `AgentLoop` trait / `ToolPayloadAdapter`；`list_cron`/`task_create`/`ChannelSendTool` 注册；Thinking Log 持久化；零边框清理 |
| 35 | **子代理预算可视化 + 会话快照** | ✅ Complete | `SubagentStore` 磁盘持久化 + JSON 导出/导入 + `rfd` 对话框 |
| 36 | **Cron/Team UI + 子代理状态** | ✅ Complete | Cron 调度/Team 协调面板本地 mock；`SubagentStore` JSON save/load |
| 36.5 | **UI 指示器迁移 + 死代码清理** | ✅ Complete | Agent/Gateway 状态点迁移至标题栏；dead code 清理；rapid-Enter debounce 修复 |
| 36.6 | **Cron 侧边栏迁移 + Markdown 表格** | ✅ Complete | Cron 从独立 SidePanel 迁入 sidebar 可折叠 section；自研 `RenderBlock::Table` |
| 38-C | **CI Pipeline Hardening** | ✅ Complete | 7-job CI 全绿；跨平台 lint 差异修复；`libxdo` 链接修复 |
| 39 | **Runtime Stability + Backlog** | ✅ Complete | `TaskStore` panic 消除；TODO/FIXME 清理；`ParallelExecutor` cancel token |
| 40 | **Runtime Robustness + Integration Tests** | ✅ Complete | parking_lot 迁移（~154 锁 unwrap 消除）；MCP end-to-end 集成测试；Dependabot 跟进 |
| 41 | **UI 审计修复与视觉精调** | ✅ Complete | CJK 子集字体 297KB；错误气泡增强；侧边栏信息架构重构；Phosphor 图标系统 |
| 43+ | **Protocol Convergence + Engineering Discipline** | 🟢 A/B/C Complete | `docs/CODE-CHANGE-PRINCIPLES.md` 七条原则强制生效；ADR-006 Phase A/B/C 完成 (-792 行死代码，含 Gen-2 协议+view 通道；新 clippy `-D warnings` 通过；测试基线 955 → 932) |
| S4-α | **Widget Extraction POC** | ✅ Complete | `widgets/provider_row.rs` 抽取自 `provider_tab.rs:62-118`；2 处 painter.rect_filled 替换为 Frame::fill+stroke；5 个单元测试覆盖；H1/H2/H3 三个方法论假设全部成立 |
| S4-β | **Widget Extraction Replication** | ✅ Complete | `widgets/theme_card.rs` 抽取自 `interface_tab.rs:203-256`；painter.rect_filled + 2 处 painter.rect_stroke 完全消除；5 个单元测试；**panel-level allocate+Sense::click 反模式清零 (4→0)** |
| S3.1 | **Settings Truth Audit** | ✅ Complete | 发现"3 源"实际是 **2 真相 + 1 镜像**；TRUTH A/B 不自动同步是 3 个真实 bug 根因；修订 S3 路线图（4d 而非 5d） |
| S3.2 | **Settings Commit Centralization** | ✅ Complete | 4 处重复 sync 代码 → 3 个 helper（`commit_settings`/`apply_approval_mode_to_runtime`/`trigger_llm_reload`）；纯重构零行为变更 |
| Anthropic | **Anthropic Managed Agents 架构映射** | ✅ Complete | 基于 Kimi share 对话深入对比；ADR-008 Draft 起草；架构定位文档新增 §五-A 章节；累计 +638 行架构决议（mapping + ADR + positioning） |

---

## Verified (Tested / Built Successfully)

| Item | Evidence | Date |
|------|----------|------|
| Anthropic Managed Agents 架构映射 | `docs/notes/2026-05-11-anthropic-managed-agents-mapping.md` + ADR-008 Draft | 2026-05-11 |
| Workspace lib + bin tests (post S3.2) | 826 lib + 116 bin = 942 passed, 0 failed, 7 ignored | 2026-05-11 |
| S3.1 settings truth audit | `docs/notes/2026-05-11-S3-settings-truth-audit.md`（修订诊断方向） | 2026-05-11 |
| S3.2 commit centralization | 4 处重复 sync → 3 helpers; 纯重构 | 2026-05-11 |
| Hybrid UI 认知修正 | `docs/notes/...-retrospective.md §9` + `PROJECT_STATUS.md §Current Stack Positioning` | 2026-05-11 |
| Workspace lib + bin tests (post S4-β) | 826 lib + 116 bin = 942 passed, 0 failed, 7 ignored | 2026-05-11 |
| S4-β panel-level 反模式清零 | allocate+Sense::click 在 panels/components: 0 | 2026-05-11 |
| Workspace lib + bin tests (post S4-α) | 826 lib + 111 bin = 937 passed, 0 failed, 7 ignored | 2026-05-11 |
| S4-α widget extraction | provider_row.rs +5 tests passing; 2 painter calls eliminated | 2026-05-11 |
| Workspace lib + bin tests (post C.2) | 826 lib + 106 bin = 932 passed, 0 failed, 7 ignored | 2026-05-11 |
| Strict clippy (post C.2) | `cargo clippy --workspace --lib --bins --tests -- -D warnings` PASS | 2026-05-11 |
| ADR-006 Phase A | `#[deprecated]` markers added; zero functional change | 2026-05-11 |
| ADR-006 Phase B | EventBus / sync_to_wire producer paths removed | 2026-05-11 |
| ADR-006 Phase C.1 | Gen-2 protocol (Event/EventMsg/EventBus) deleted (-481 行) | 2026-05-11 |
| ADR-006 Phase C.2 | Wire view channel + 下游死订阅者 deleted (-309 行) | 2026-05-11 |
| Workspace lib tests | 849 passed, 0 failed, 7 ignored | 2026-05-10 |
| Workspace lib tests | 551 passed (clarity-core), 6 ignored | 2026-05-06 |
| Workspace lib tests | 577 passed, 6 ignored | 2026-04-30 |
| Clippy zero warnings | `-D warnings` clean | 2026-04-30 |
| CI workflow syntax | YAML valid, `working-directory` set | 2026-04-26 |
| egui dev build | `cargo run -p clarity-egui` starts | 2026-05-01 |
| egui clippy | 1 warning (Locale::label unused) | 2026-05-01 |
| Custom titlebar | `with_decorations(false)` + drag region + min/max/close | 2026-05-01 |
| i18n framework | `i18n.rs` + `t()` + EN/中文 toggle in sidebar | 2026-05-01 |
| Theme overhaul | Deep navy + copper, 13 new tokens | 2026-05-01 |
| Spacing normalization | 50+ `add_space(N)` → `theme.space_*` | 2026-05-01 |
| Release binary build | `cargo build --release` success, 5 binaries | 2026-05-08 |
| Log credential redaction | `clarity_core::logging::RedactingWriter` + tracing subscriber integration | 2026-05-08 |
| Prompt injection defense | `<tool_result>` XML delimiter + system prompt hardening | 2026-05-08 |
| Global shortcuts MVP | `Ctrl+N` new session, `Ctrl+Enter` send, `Ctrl+K` focus input, `Ctrl+Shift+P` palette, `Ctrl+.` toggle skill, `Ctrl+Shift+T` toggle team | 2026-05-08 |
| Release perf (headless startup) | ~52ms (`--help`) | 2026-05-08 |
| Release perf (gateway startup) | ~1.1s (port 18790 ready) | 2026-05-08 |
| Release perf (egui memory) | peak 141 MB / avg 140 MB | 2026-05-08 |
| Release perf (gateway memory) | peak 21 MB / avg 21 MB | 2026-05-08 |
| Task panel output viewing | `Output` button for terminal tasks + async `get_result_opt()` + result modal | 2026-05-08 |
| Subagent output viewing | `Output` button for completed subagents + live `output_lines` modal | 2026-05-08 |

### Archived — Legacy `clarity-tauri` Stack (2026-04-26)

`clarity-tauri` 已于 Sprint 7 归档并移出仓库。以下记录保留仅作历史追溯，不再维护或重复验证。

| Item | Evidence | Date |
|------|----------|------|
| Tauri dev build | `cargo tauri dev` starts | 2026-04-26 |
| Tauri release build | `.msi` + `.exe` produced | 2026-04-26 |
| EXE runtime dependency scan (Tauri) | Pure system DLLs + UCRT only | 2026-04-26 |
| EXE launch test (Tauri) | `clarity-tauri.exe` starts (GUI blocking) | 2026-04-26 |
| Frontend npm build (Tauri) | `npm run build` succeeds (75 modules) | 2026-04-26 |

---

## Current Stack Positioning

**Hybrid UI 架构（2026-05-11 修正认知）**：Clarity 是典型的"GUI × TUI 混血"应用，
两种前端**同等一线**，共享后端，前端多态：

| 前端 | 形态 | 适用场景 |
|------|------|---------|
| `clarity-egui` | GUI mode（egui 0.31 + glow backend） | 本地工作站：鼠标、多栏、富文本、虚拟列表、CJK 字体 |
| `clarity-tui` | TUI mode（ratatui + crossterm） | 远程/SSH/轻量：键盘流、低占用、Shell 生态无缝衔接 |

**共享底座**：`clarity-core` / `clarity-gateway` / `clarity-memory` / `clarity-wire` / `clarity-contract`

**辅助栈**：`clarity-claw`（tray monitor） / `clarity-headless`（CLI / CI 脚本场景）

**已归档**：`clarity-tauri`（Tauri 2 + React/Vite，Sprint 7 后停止追加新功能）

> **重要纠正**：本节早期版本曾把 tui 框架为"维护模式 / secondary"，违反了 Hybrid UI
> 的设计本质。tui 与 egui 是同一产品在不同环境下的两种"皮肤"——不是主备关系。
> 后端统一、前端多态是核心架构原则。任何只服务于单一前端的协议层都应当被审视
> （如 ADR-006 §1.3 校正发现 `ViewCommand` 同时被 tui+egui+gateway 真实消费）。

### 前后端功能 Parity（关键差距）

后端 `clarity-core` 功能极为完整（Agent 循环、20+ 工具、审批系统、Plan 模式、并行子代理、后台任务、MCP 三协议、Skill 系统、记忆系统、本地 LLM）。

前端 `clarity-egui` 在**聊天体验**（流式、虚拟列表、美观气泡、文件预览、CJK 字体、消息队列）上已超越 Tauri，但在 **core 功能暴露**上存在显著缺口：

| 功能 | core | egui | 说明 |
|------|:----:|:----:|------|
| Agent 运行/流式 | ✅ | ✅ | — |
| 工具调用可视化 | ✅ | ✅ | Running/Done 状态气泡 |
| Compaction Banner | ✅ | ✅ | 压缩状态提示条 |
| **审批交互 UI** | ✅ | ✅ | DiffPopup 模态弹窗 + 键盘快捷键 + 交互拦截 |
| **Plan 步骤可视化** | ✅ | ✅ | 实时状态图标 ⏳/▶️/✅/❌ + 步骤间取消检查 |
| **子代理/并行执行** | ✅ | ❌ | 无多 Agent 进度面板 |
| 后台任务面板 | ✅ | 只读 | 无创建/取消/Cron 配置操作 |
| **技能系统 UI** | ✅ | ✅ | 浮动面板 + ON/OFF 切换 + 元数据展示 + 刷新按钮 |
| **Token 用量显示** | ✅ | ✅ | Session 累计格式化（千位分隔符）+ Sidebar 底部摘要 |
| **模型下载 GUI** | ✅ | ✅ | onboarding 首次启动引导 + HF 直链下载 + 进度条 |
| **日志/Console 面板** | — | ❌ | Tauri 曾实现，egui 无 |
| LSP 集成 | ✅ | ❌ | Tauri 曾实现，egui 无 |
| MCP 配置面板 | ✅ | ✅ | 服务器列表、启用/禁用、保存 |

**最大风险**（已缓解项划线删除）：
1. ~~`clarity-egui` 零单元测试~~ — 已部分缓解：`app_state`/`settings`/`theme`/`profile_overlay`/`llm_policy` 纯逻辑测试 32+，UI 渲染测试仍为缺口。
2. **Provider 配置硬编码** — 新服务商需改代码，不支持无代码注册。
3. ~~API Key 明文落盘~~ — 已缓解：支持 `${env:VAR}` 环境变量引用语法，密钥可不落盘。
4. ~~Settings Save 覆盖全配置~~ — 已缓解：增量 merge 保存，只写入变更字段。

**Sprint 13 进行中修复项**：
- ✅ A1: Agent 工具调用失败后无限重试 → 三级错误分类 + 不可恢复错误立即停止
- ✅ A2: 错误消息泄露绝对路径 → `ToolError::sanitize_paths()` 脱敏层
- ✅ A3: System Prompt 泄露内部信息 → Git/Cargo.toml 移出 Prompt + 路径 `<external>` 脱敏 + 身份规则硬化
- ✅ B1: `InMemoryApprovalRuntime` 内存状态丢失 → `wait_for_response` 内置 300s 超时，超时后自动 Cancel
- ✅ B2: 审批 request_id 不一致 → 并发 resolve 竞态测试 + 不存在请求校验测试
- ✅ B3: Agent 身份混乱 → 对外统一 Clarity / `AgentInner.provider_label` 保留原始模型名用于 tracing

---

## Active Issues（已确认，待修复）

### A1. Settings 模型配置体验缺陷（P1）

**诊断结论**：`model` 字段持久化机制本身完好（serde 序列化/反序列化无遗漏）。问题集中在**UI 交互层**和**边缘容错**：

1. **UI 无模型下拉列表**：`get_available_models()` 返回的 provider→model 映射在 `render_settings_panel` 中被完全忽略，Model 字段是自由文本 `TextEdit`。用户需手动输入模型 ID（如 `kimi-k2-07132k`），易出错且无自动校验。
2. **Provider/Model 不联动**：切换 provider 时 model 文本不会自动更新，可能导致 provider 与 model 不匹配（如 kimi provider + gpt-4o model）。
3. **`load()` 静默吞错**：`gui-settings.json` 损坏时直接回退 `default_with_env()`，无任何日志，用户感知为"配置丢失"。
4. **`default_with_env()` model 环境变量互斥缺失**：同时设置 `KIMI_API_KEY` + `OPENAI_MODEL` 会导致 provider=kimi 但 model=openai 型号的不匹配状态。
5. **`ensure_llm` 无网络 fallback**：与 `clarity-tauri` 侧不同，egui 的 `ensure_llm` 在网络不可用时不会自动回退到 local provider。

**修复路径**：
- 短期：将 Model `TextEdit` 改为基于 `get_available_models` 的 `ComboBox`，联动 provider 切换。
- 中期：补齐 `load()` 错误日志 + 损坏文件备份；修复 `default_with_env()` 互斥逻辑；补齐 `ensure_llm` 网络 fallback。
- 长期：借鉴 Kimi CLI 的 settings 分层设计（环境变量 → 配置文件 → 交互式选择），见代办项 T_KIMICLI_REF。

### A2. egui 交互型功能缺口（P2）— ✅ Sprint 12 已修复

**状态**: 2026-04-28 Sprint 12 完成。审批弹窗、Plan 可视化、Skill UI、Token 用量显示已全部补齐。
**剩余缺口**: 子代理进度面板、后台任务创建/取消 UI、日志/Console 面板。

### A3. Provider 配置架构缺陷（P0）

**来源**：OpenHanako / OpenClaw 服务商持久化调研（2026-04-27）。

| 缺陷 | 当前状态 | 目标状态 |
|------|---------|---------|
| Provider 硬编码枚举 | egui `ComboBox` 写死 5 个 provider | Schema 化配置（TOML/JSON），无代码注册新 Provider |
| 单模型选择 | 一个 Model 下拉框 | chat / utility / utilityLarge 角色分工 |
| 全局单一配置 | 所有 Agent 共享 `GuiSettings` | 全局默认值 + Agent 级覆盖 |
| API Key 明文存储 | `gui-settings.json` 明文 `String` | 支持 `${env:KIMI_API_KEY}` 语法，避免密钥落盘 |
| Settings Save 覆盖 | 整个 `GuiSettings` 序列化覆盖 | 增量保存，只写入变更字段 |

**修复路径**：
- 短期：环境变量注入语法 + Settings 增量保存
- 中期：Provider Schema 化（baseURL / authType / modelListEndpoint / compatibility）
- 长期：Agent 级 Provider 覆盖 + 多模型角色分工

### A4. UI/UX 零碎问题（P2）

**来源**：egui GUI 美化审计（2026-04-27）。

| 问题 | 优先级 | 说明 | 状态 |
|------|--------|------|------|
| 色彩系统扁平 | P1 | 单层深灰背景，需语义分层 | ✅ Overlay 5级 + shadow 4级 + 语义表面色3种（Phase 1/3）|
| 布局靠线框分割 | P1 | 侧边栏与主区之间用边框而非间距区分 | ⏸️ |
| 输入区无工具栏 | P1 | 底部只有发送按钮，缺附件/MCP 工具选择 | ⏸️ |
| 间距硬编码 | P1 | 50+处 `add_space(N)` 未使用 token | ✅ 全部替换为 `theme.space_*`（Phase 2）|
| 消息 Segment 结构 | P2 | 头像+内容+时间戳未组件化 | ⏸️ |
| 弹窗无阴影/动画 | P2 | Settings/MCP 弹窗缺出现动画和阴影 | ✅ shadow token + toast fade-in（Phase 3/4）|
| 图标不统一 | P3 | 使用 emoji，跨平台显示不一致 | ⏸️ |
| 无边框窗口 | P3 | OS 标题栏未自定义 | ✅ 自绘标题栏 + drag region + 按钮 |
| i18n 支持 | P1 | 无中英文切换 | ✅ `i18n.rs` + 侧边栏切换 |
| 字体/代码可读性 | P1 | 字号偏小、代码块无区分 | ✅ 14→15px 正文、code_block_bg、行高22px |
| 配色长期疲劳 | P1 | 纯黑+亮紫 | ✅ 深蓝灰+铜色、降低蓝光 |

---

## Unverified / Untested (Requires Action)

| # | Item | Risk Level | Blocker | Proposed Verification Method |
|---|------|------------|---------|------------------------------|
| U1 | **纯净Windows环境安装** — 在无Rust/Node/WebView2的VM上安装MSI并运行 | 🔴 High | 无本地VM | Windows Sandbox 或 GitHub Actions `windows-latest` runner E2E 测试 |
| U2 | **CI端到端验证** — push tag后GitHub Actions完整构建→签名→Release | 🔴 High | 需push测试tag | Push `v0.2.1-test.1` tag 触发 workflow，验证 artifact 产出 |
| U3 | **代码签名效果** — 自签名证书在Defender/SmartScreen下的实际表现 | 🟡 Medium | 需U2完成 | 下载CI产出的.exe，检查属性→数字签名页 |
| U4 | ~~**自动更新检查** — Tauri updater检测新版本~~ | 🗄️ Archived | `clarity-tauri` 已归档（Sprint 7） | 不再验证。egui 无内置自动更新；未来如需可通过 GitHub Release API 自行实现 |
| U5 | **FTUE实际GUI流程** — OnboardingModal在打包应用中的显示、关闭、设置跳转 | 🟡 Medium | 需U1完成 | 人工在VM中完成首次安装→启动→配置→对话 |
| U6 | **模型下载引导** — 用户从Onboarding到下载.gguf到完成首次对话 | 🟡 Medium | T_KALOSM_REAL阻塞 | 云端Provider作为默认路径，本地模型作为进阶选项 |
| U7 | ~~**WebView2缺失环境** — Win10未预装WebView2时的自动下载~~ | 🗄️ Archived | `clarity-tauri` 已归档（Sprint 7） | 不再验证。egui 基于 glow 原生 OpenGL，无 WebView2 依赖 |
| U8 | **NSIS便携版运行** — `.exe` 直接运行（非安装） | 🟢 Low | 无 | 双击验证即可 |
| U9 | **egui 纯净环境运行** — 无 WebView2 依赖的 egui 单二进制在裸机运行 | 🟡 Medium | 无 | egui 无 WebView2 依赖，但需验证字体/渲染在裸机表现 |

---

## External Blockers

| ID | Item | Status | Impact | Mitigation |
|----|------|--------|--------|------------|
| T_KALOSM_REAL | agri-paper 7B模型数据未到达 | 🔴 持续阻塞 | 本地模型首次体验路径不完整 | 云端Provider（OpenAI/Anthropic）作为默认首次体验路径 |
| T_KIMICLI_REF | 借鉴 Kimi CLI 的 settings/模型选择交互设计 | ⏸️ 冻结代办 | settings 体验优化参考 | 不推进实现，仅作为设计参考归档于 `docs/plans/2026-04-27-egui-pretext-health-plan.md` 冻结项 |

---

## Known Limitations (Documented, Not Blockers)

1. ~~**WebView2 Dependency（仅 legacy Tauri）**~~ — `clarity-tauri` 已归档（Sprint 7）。egui 基于 glow 原生 OpenGL，无 WebView2 依赖。
2. **CUDA Optional** — `cuda` feature flag控制，非必需
3. **Self-Signed Certificate** — 无商业证书，SmartScreen可能拦截，文档已说明
4. **Discord/Telegram Channels** — 因CVE禁用，Slack可用
5. **cargo audit warnings** — 11个unmaintained（上游依赖），已标记为允许
6. ~~egui 审批交互缺失~~ — ✅ Sprint 12 已修复。Interactive/Plan/Yolo 三模式均已在 egui 可用。
7. **egui 零测试** — 计划通过 `docs/plans/2026-04-27-egui-pretext-health-plan.md` 分阶段补齐

---

## Decision: Continue Sprint 2-4 Validation vs Switch to Plan B

### Current Assessment

**方案A的核心约束已扩展**：**"任意Windows用户可在3分钟内从GitHub Release下载并运行Clarity（egui 版本）"**。

当前状态：
- ✅ ~~构建产物已生成（MSI/NSIS）— Tauri 侧~~（Sprint 7 已归档，归 egui 侧继续验证）
- ✅ egui 可 `cargo run` 直接运行，无 WebView2 依赖
- ✅ 代码已修复并推送（clippy 零警告）
- ❌ 未在真实/纯净环境中验证安装包
- ❌ CI未实际触发验证
- ❌ 首次用户体验未端到端验证
- 🔄 egui 健康度运维 plan 已产出，待执行

**结论：继续完成方案A的验证工作，同时推进 egui 硬化。**

理由：
1. egui 彻底摆脱 WebView2 依赖，纯净环境验证意义更大
2. 构建产物已产出，验证成本远低于重新开发
3. 剩余未验证项（U1-U5, U9）均可通过 GitHub Actions + 少量人工验证完成
4. Pretext 运维 plan 为 egui 提供了可量化的硬化路径

### Recommended Next Steps

| Priority | Action | Est. Time | Owner |
|----------|--------|-----------|-------|
| P0 | Push test tag `v0.3.0-test.1` 触发CI，验证 workflow 完整跑通 | 10 min + CI time | Human |
| P0 | 人工验证CI产出的 egui binary 在本地运行 | 15 min | Human |
| P1 | 使用Windows Sandbox验证 egui 纯净环境运行 | 30 min | Human |
| P1 | 修复 Settings 模型选择 ComboBox + provider/model 联动 | ½ 天 | Agent |
| P1 | 验证FTUE完整路径：启动→设置OpenAI key→选择模型→首次对话 | 20 min | Human |
| P2 | 启动 Pretext 运维 plan Phase 1（热路径清剿） | 2 周 | Agent |
| P2 | 发布v0.3.1-alpha并开始社区推广 | — | After P0-P1 |

### Abort Criteria (Switch to Plan B)

若以下任一条件触发，重新评估方案：
1. CI workflow 在3次尝试后仍无法产出可用artifact
2. 包体积在UPX压缩后仍 >100MB（当前8.5MB MSI，远低于阈值）
3. egui 在裸机环境中字体渲染/输入法出现不可修复的回归
4. 30天风险对冲窗口到期（GitHub Star < 50 且 Issue+PR < 3）

---

## Phase 0 审计：已知风险与待优化（2026-05-01）

| 风险 | 级别 | 说明 | 缓解/修复状态 |
|------|------|------|-------------|
| `parallel_batches` HashMap 无限增长 | 🔴 | Gateway 每次 `/v1/parallel` 插入一条进度记录，之前无清理机制 | ✅ 已修复 — 每5分钟后台清理非运行中批次 |
| `clarity-tui` `run_parallel` 旧签名 | 🟡 | TUI 调用 `run_parallel(specs, config)` 缺 `progress` 参数 | ✅ 已修复 — 补传 `None` |
| Claw `quick_chat` 每次创建新 Tokio Runtime | 🟢 | 同步线程内无法复用主 Runtime，每次 HTTP 调用新建 Runtime | ⏸️ 低风险 — HTTP 请求完成即释放，偶发调用可接受 |
| Contract `Error` 类型迁移暂缓 | 🟡 | `ToolError::sanitize_paths()` 依赖 `dirs`，无法直接迁入 contract | ✅ 已解决 — `dirs` 加入 contract 依赖，`AgentError`/`ToolError`/`CapabilityToken` 全量提取 |
| `SubAgentProgress` JSON 反序列化宽容 | 🟢 | 使用 `unwrap_or_default` 静默忽略 Gateway 响应格式变化 | ⏸️ 低风险 — 仅在 API 变更时可能隐藏错误 |
| `clarity-egui settings.rs` 本地格式变更 | 🟢 | 编辑器可能自动格式化 `settings.rs`，引入不存在的 Theme 字段引用 | ✅ 已恢复 — `git checkout` 还原 |

---

## Quality Gates (Every Commit)

```bash
cargo test --workspace --lib              # 602+ passed, 0 failed
cargo clippy -p clarity-contract -p clarity-core -p clarity-gateway -p clarity-claw -p clarity-tui -- -D warnings  # 零警告
cargo fmt --all -- --check               # 格式检查
```

## Release Gates (Every Release)

```bash
cargo audit                              # 无高危漏洞
cargo run -p clarity-egui               # 本地运行验证（Hybrid UI: GUI 模式）
cargo run -p clarity-tui                # 本地运行验证（Hybrid UI: TUI 模式）
# 以上 + U1-U5, U9 验证通过
```
