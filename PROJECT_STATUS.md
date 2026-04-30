# Clarity Project Status

> Last updated: 2026-04-28
> Branch: `phase2/protocol-pilot` @ `4c9f4de`
> Test baseline: **568 passed, 0 failed, 4 ignored**
> Clippy: **0 warnings** (`-D warnings`)

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
| 7 | UI 栈迁移 | ✅ Complete | `clarity-egui` 替代 `clarity-tauri` 成为主力 GUI 栈 |
| 8 | egui 硬化 | ✅ Complete | Pretext Phase 1：settings 修复、Mutex 替换、`App::update()` 550→64 行拆分、onboarding 模型下载 |
| 9 | **服务商支持硬化** | ✅ Complete | Provider Schema 化、环境变量注入、Settings 增量保存、API Key 引用语法 `${env:VAR}` |
| 10 | **协议先行解锁** | ✅ Complete | AgentProfile TOML、LlmFactory 冻结、CapabilityRegistry、egui 冒烟测试 |
| 11 | **超越 Kimi CLI** | ✅ Complete | V1 风险清偿 + V2 端到端验证通过 |
| 12 | **egui 功能补齐** | ✅ Complete | 审批弹窗 → Plan 可视化 → Skill UI → Token 显示 |

---

## Verified (Tested / Built Successfully)

| Item | Evidence | Date |
|------|----------|------|
| Workspace lib tests | 568 passed, 4 ignored | 2026-04-28 |
| Clippy zero warnings | `-D warnings` clean | 2026-04-27 |
| Tauri dev build | `cargo tauri dev` starts | 2026-04-26 |
| Tauri release build | `.msi` + `.exe` produced | 2026-04-26 |
| EXE runtime dependency scan | Pure system DLLs + UCRT only | 2026-04-26 |
| EXE launch test | `clarity-tauri.exe` starts (GUI blocking) | 2026-04-26 |
| Frontend npm build | `npm run build` succeeds (75 modules) | 2026-04-26 |
| CI workflow syntax | YAML valid, `working-directory` set | 2026-04-26 |
| egui dev build | `cargo run -p clarity-egui` starts | 2026-04-27 |
| egui clippy | 0 warnings (11 resolved) | 2026-04-27 |

---

## Current Stack Positioning

**主力 GUI 栈**：`clarity-egui`（egui 0.31 + glow backend）
**废弃归档**：`clarity-tauri`（Tauri 2 + React/Vite）— 代码保留，新功能不追加
**活跃后端**：`clarity-core` / `clarity-gateway` / `clarity-memory` / `clarity-wire`
**维护模式**：`clarity-tui` / `clarity-claw` / `clarity-headless`

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

**最大风险**：
1. `clarity-egui` **零单元测试**（0 tests / 0 modules）。
2. **Provider 配置硬编码** — 新服务商需改代码，不支持无代码注册。
3. **API Key 明文落盘** — `gui-settings.json` 直接存储明文密钥。
4. **Settings Save 覆盖全配置** — 存在丢失未修改字段的风险（OpenClaw 教训）。

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

| 问题 | 优先级 | 说明 |
|------|--------|------|
| 色彩系统扁平 | P1 | 单层深灰背景，需语义分层（基底/一级表面/二级表面/悬停） |
| 布局靠线框分割 | P1 | 侧边栏与主区之间用边框而非间距区分 |
| 输入区无工具栏 | P1 | 底部只有发送按钮，缺附件/MCP 工具选择 |
| 消息 Segment 结构 | P2 | 头像+内容+时间戳未组件化 |
| 弹窗无阴影/动画 | P2 | Settings/MCP 弹窗缺出现动画和阴影 |
| 图标不统一 | P3 | 使用 emoji，跨平台显示不一致 |
| 无边框窗口 | P3 | OS 标题栏未自定义 |

---

## Unverified / Untested (Requires Action)

| # | Item | Risk Level | Blocker | Proposed Verification Method |
|---|------|------------|---------|------------------------------|
| U1 | **纯净Windows环境安装** — 在无Rust/Node/WebView2的VM上安装MSI并运行 | 🔴 High | 无本地VM | Windows Sandbox 或 GitHub Actions `windows-latest` runner E2E 测试 |
| U2 | **CI端到端验证** — push tag后GitHub Actions完整构建→签名→Release | 🔴 High | 需push测试tag | Push `v0.2.1-test.1` tag 触发 workflow，验证 artifact 产出 |
| U3 | **代码签名效果** — 自签名证书在Defender/SmartScreen下的实际表现 | 🟡 Medium | 需U2完成 | 下载CI产出的.exe，检查属性→数字签名页 |
| U4 | **自动更新检查** — Tauri updater检测新版本并提示下载 | 🟡 Medium | 需U2完成 | 发布测试tag后，运行旧版本看是否提示更新 |
| U5 | **FTUE实际GUI流程** — OnboardingModal在打包应用中的显示、关闭、设置跳转 | 🟡 Medium | 需U1完成 | 人工在VM中完成首次安装→启动→配置→对话 |
| U6 | **模型下载引导** — 用户从Onboarding到下载.gguf到完成首次对话 | 🟡 Medium | T_KALOSM_REAL阻塞 | 云端Provider作为默认路径，本地模型作为进阶选项 |
| U7 | **WebView2缺失环境** — Win10未预装WebView2时的自动下载行为 | 🟡 Medium | 无Win10 VM | 文档说明；依赖Tauri内置的WebView2引导 |
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

1. **WebView2 Dependency（仅 Tauri）** — Windows 11预装；Windows 10可能需自动下载（Tauri处理）。egui 无此依赖。
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
- ✅ 构建产物已生成（MSI/NSIS）— Tauri 侧
- ✅ egui 侧可 `cargo run` 直接运行，无 WebView2 依赖
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

## Quality Gates (Every Commit)

```bash
cargo test --workspace --lib              # 568 passed, 0 failed, 4 ignored
cargo clippy --workspace --lib --bins --tests -- -D warnings  # 零警告
cargo fmt --all -- --check               # 格式检查
```

## Release Gates (Every Release)

```bash
cargo audit                              # 无高危漏洞
cargo run -p clarity-egui               # 本地运行验证（egui 为主力栈）
# 以上 + U1-U5, U9 验证通过
```
