# Clarity 深度分析与实施规划 v2.0

> 分析日期：2026-04-25  
> 基于理论：Cynefin 框架、约束理论 (TOC)、Martin Fowler 技术债务象限、Basecamp Shape Up  
> 数据验证状态：所有代码指标来自 `main@4da4a4d` 实机运行结果；文档指标来自文件逐行审计。

---

## 一、项目健康度诊断

### 1.1 代码健康度评分：A-（优秀，局部待补强）

| 指标 | 实测值 | 评分 | 说明 |
|------|--------|------|------|
| 单元测试 | 502 passed, 0 failed, 4 ignored | A+ | 全绿通过，4 个 ignored 均为本地模型 E2E（合理） |
| Clippy | 0 warnings (`-D warnings`) | A+ | 整个 workspace 零警告 |
| Security Audit | 0 高危, 1 allowed warning | A | Tauri 上游间接依赖，已标记为允许 |
| Format | 58 文件已清理 | A+ | `cargo fmt --all` 通过 |
| 测试覆盖率 | 未量化 | C | `cargo tarpaulin` 仅输出 XML artifact，未设定覆盖率门槛 |
| 代码规模 | 165 files, ~45,401 lines | B+ | 规模健康，但 `clarity-tauri/src/lib.rs` 中 `AppState` 已出现 bloat 迹象 |

**结论**：代码质量处于高位稳定状态。唯一短板是**覆盖率未设门槛**，无法阻止未来回归。

### 1.2 文档健康度评分：C+（严重漂移）

| 文档 | 问题 | 严重程度 |
|------|------|----------|
| `CHANGELOG.md` | [0.1.2] 日期 2026-04-20 早于 [0.1.1] 的 2026-04-23 和 [0.1.0] 的 2026-04-21；缺少 [0.2.0] 正式条目；[Unreleased] 后无 [0.2.0] 章节 | 🔴 高 |
| `RELEASE_v0.2.0.md` | 声称 498 passed, 3 ignored（实测 502 passed, 4 ignored）；未包含 local-llm 默认启用、离线 fallback 等 9 个 commit 的功能 | 🔴 高 |
| `README.md` | "Zero runtime dependencies" 与 `docs/ROADMAP.md` 中"零依赖发行"P0 项矛盾——后者明确该目标尚未完成 | 🟡 中 |
| `docs/PROJECT_STATUS.md` | "审批系统增强"标记为 🔄 进行中，但 `git log` 显示近期无相关代码提交；与 `docs/ROADMAP.md` 中"⏸️ 未启动"矛盾 | 🟡 中 |
| `AGENTS.md` | 信息准确，CUDA 编译说明完整，Known Issues 实时更新 | 🟢 低 |

**结论**：文档处于**"快速迭代导致的累积漂移"**状态。版本管理相关文档（CHANGELOG、RELEASE）与代码实际状态脱节，若不立即修复将侵蚀用户信任。

### 1.3 流程健康度评分：B（CI 完善，发布流程缺失）

| 指标 | 状态 | 评分 |
|------|------|------|
| CI 矩阵 | check/test/clippy/fmt/audit/coverage × 3 OS | A+ |
| 分支策略 | `subagent/<feature>-YYYY-MM-DD` 隔离 + `--no-ff` merge | A |
| 版本标记 | `v0.2.0` tag 指向 `5de3767`（仅 docs commit），早于 main 9 commits；workspace version = "0.1.0" | 🔴 F |
| 发布流程 | 无自动构建、无 artifact、无签名、无更新机制 | 🔴 F |
| 代码审查 | 子代理验收门槛明确（test + clippy + build） | B+ |

**结论**：CI/CD 的"检查"环节极度完善，但"交付"环节完全空白。项目当前处于**"能完美验证代码，却无法把代码变成用户可安装产品"**的状态。

---

## 二、瓶颈分析（约束理论 TOC）

### 2.1 当前完整价值流

```
[代码提交] ──5min──► [CI验证] ──0min──► [合并到main] ──?──► [版本标记]
                                                             │
                                                             ▼
[用户获得价值] ◄──?── [首次运行成功] ◄──?── [下载安装] ◄── [打包构建]
```

### 2.2 各环节吞吐量估算

| 环节 | 周期时间 | 吞吐量限制 | 瓶颈？ |
|------|----------|------------|--------|
| 代码提交 → CI 通过 | ~5 分钟（rust-cache 命中） | GitHub Actions 并发 | 否 |
| 合并到 main | 即时（单开发者） | 人工决策 | 否 |
| 版本标记 | 数分钟（手动 `git tag`） | 人为易错（v0.2.0 tag 错误） | 潜在 |
| 打包构建 | **未启动**（⏸️） | 无能力 | **是** |
| 发布 artifact | **未启动**（⏸️） | 无能力 | **是** |
| 用户下载 | `cargo install --git` 需 Rust 工具链 + 编译 5-10 分钟 | 用户环境 | **是** |
| 首次运行成功 | 需手动配置 API key 或放置 .gguf + tokenizer.json | 用户认知 | **最大瓶颈** |

### 2.3 瓶颈识别与量化

根据 TOC 的"聚焦五步骤"：

1. **识别约束**：用户从"知道 Clarity"到"成功完成第一次对话"的转化路径过长。
   - 当前路径：安装 Rust → `cargo install --git`（编译 5-10 分钟）→ 配置 API key 或下载 .gguf + tokenizer.json → 放置到 ~/models/ → 运行。
   - 对于非 Rust 开发者，步骤 1-2 是极高门槛。
   - 对于本地 LLM 用户，步骤 3-4 需要了解 HuggingFace、GGUF、tokenizer 等概念。

2. **剥削约束**：在约束未解除前，不应投入资源优化非约束环节。
   - 停止新增"Phase 4 长期功能"（LSP、Mobile、Voice），直至发布基础设施完成。
   - 停止优化 CI 时间（已足够快）。

3. **服从约束**：所有其他环节应配合约束环节的节奏。
   - 文档更新优先服务"首次用户体验"，而非架构细节。
   - Issue 响应优先级：安装/启动问题 > 功能请求。

4. **提升约束**：这是当前唯一应投入工程资源的环节。
   - Bet 1（发布基础设施）：将"编译 5-10 分钟"转化为"下载 1 分钟"。
   - Bet 2（首次用户体验）：将"手动放置 .gguf + tokenizer"转化为"引导下载"。

5. **重复**：若约束解除，重新评估（下一步约束可能是"社区增长"或"竞品迭代"）。

### 2.4 非约束环节清单（当前不应投入资源）

| 环节 | 理由 |
|------|------|
| CI 加速 | 已 5 分钟，边际收益极低 |
| 新增 TUI 功能 | TUI 用户可接受 `cargo install`，不受当前约束影响 |
| Gateway Web IDE 增强 | Phase 3C，与核心约束无关 |
| 代码覆盖率提升 | 重要但不紧急，可延至冷却期 |
| Trait 架构重构 | P0 风险项，但在"无法交付给用户"面前优先级降级 |

---

## 三、技术债务清偿计划

按 Martin Fowler 技术债务象限分类：

### 3.1 鲁莽/有意（Deliberate & Reckless）

| 债务项 | 说明 | 偿还策略 |
|--------|------|----------|
| README "Zero runtime dependencies" | 营销表述与技术现实不符。当前 `cargo install` 仍需 Rust 工具链，Tauri 运行时需要 WebView2。 | **立即修复**（< 15 min）：改为 "`cargo install` produces a fully working binary. Pre-built installers coming soon." |

### 3.2 鲁莽/无意（Inadvertent & Reckless）

| 债务项 | 说明 | 偿还策略 |
|--------|------|----------|
| CHANGELOG 版本顺序错误 | [0.1.2] 日期 2026-04-20 排在 [0.1.1] 2026-04-23 之前，违反时间顺序。 | **立即修复**（< 15 min）：调整日期为 2026-04-24 或重排条目。 |
| RELEASE_v0.2.0.md 数据错误 | 测试基数错误（498/3 vs 502/4）；功能列表遗漏 9 个 commit。 | **立即修复**（< 15 min）：更新为实测数据，补充遗漏功能。 |

### 3.3 谨慎/有意（Deliberate & Prudent）

| 债务项 | 说明 | 偿还策略 |
|--------|------|----------|
| `std::sync::RwLock` in `Agent.inner` | AGENTS.md 明确记录：同步锁用于非 async 上下文（TUI/Gateway），是已知限制。 | **监控，暂不处理**。若未来出现性能问题再评估 `tokio::sync::RwLock` 迁移。 |
| CUDA feature 手动启用 | CUDA Toolkit 是重型外部依赖，编译耗时过长，故不作为默认。 | **保持现状**。已在 AGENTS.md 完整记录环境变量配置。 |

### 3.4 谨慎/无意（Inadvertent & Prudent）

| 债务项 | 说明 | 偿还策略 |
|--------|------|----------|
| workspace version = "0.1.0" | tag v0.2.0 与代码版本不一致，属于发布时的疏忽。 | **立即修复**（< 5 min）：`Cargo.toml` version → "0.2.1"（准备补丁）。 |
| `AppState` bloat | AGENTS.md 记录：`tool_registry` 冗余，`session_manager` 等字段持续膨胀。 | **短期**（本周）：提取 `AppState` 构造器，将非核心字段延迟初始化。 |
| `Agent::run_streaming` vs `run_streaming_with_messages` | 两个入口点存在代码重复。 | **短期**（本周）：提取内部辅助函数，减少重复。 |
| `clarity-core` ↔ `clarity-gateway` 耦合 | `AgentController` / `Op` 枚举因 Gateway 需求持续扩展。 | **中期**（本月）：设计 `ChatDriver` trait（AGENTS.md 已建议），隔离 Gateway 特定需求。 |

### 3.5 清偿时间表

| 时间框 | 任务 | 预计工时 | 风险 |
|--------|------|----------|------|
| **立即执行**（< 1 小时） | 修复 CHANGELOG 顺序、RELEASE 数据、workspace version、README 表述 | 30 min | 无 |
| **短期**（本周） | `AppState` 构造器重构、`Agent` streaming 入口去重 | 4-6 h | 低（有测试保护） |
| **中期**（本月） | `ChatDriver` trait 设计（非完整实现） | 8-12 h | 中（涉及多 crate 边界） |

---

## 四、Shape Up 周期设计

Shape Up 核心原则：6 周构建周期 + 2 周冷却期。每个 Bet 必须有"范围、非范围、里程碑、验收标准"。

### Bet 1: 发布基础设施（6 周）

**目标**：让用户能在 3 分钟内从 GitHub Release 下载并运行 Clarity，无需 Rust 工具链。

**范围**：
- 修复版本管理不一致（workspace version, CHANGELOG, tag 策略）
- Tauri Windows 打包（`.msi` 安装版 + `.exe` 便携版）
- GitHub Actions Release workflow（tag push 触发自动构建 + artifact 上传）
- Windows 代码签名流程（自签名证书，实现签名步骤）
- 最小 viable 自动更新（启动时检查 GitHub latest release，显示更新提示）

**非范围**（明确排除，防止范围蔓延）：
- macOS / Linux 打包（延期至下一周期）
- 微软商店 / Homebrew / apt 发布
- 完整自动更新下载+安装（Tauri updater 需签名服务器，超出 6 周能力）
- 嵌入式模型（保持用户自行下载，不增加包体积）

**里程碑**：
| 周次 | 交付物 | 验收标准 |
|------|--------|----------|
| Week 1 | 版本管理修复 + v0.2.1 补丁 | `Cargo.toml` version 与 tag 一致；CHANGELOG 顺序正确 |
| Week 2 | Tauri 打包配置验证 | 本地 `cargo tauri build` 成功产出 `.msi` 和 `.exe` |
| Week 3 | CI Release workflow | `git tag v0.3.0-alpha.1` 后 CI 自动构建并上传 artifact |
| Week 4 | 代码签名集成 | `.exe` 带有自签名证书；Windows SmartScreen 仍可能拦截，但签名流程已跑通 |
| Week 5 | 自动更新检查 | 启动时调用 GitHub API，检测到新版本时前端显示提示 banner |
| Week 6 | E2E 验证 | 从全新 Windows 虚拟机下载 → 安装 → 运行 → 首次对话，总时长 < 3 分钟 |

**验收标准**：
1. 任意 Windows 用户可在 3 分钟内从 GitHub Release 下载并运行。
2. `cargo test --workspace --lib` 仍全绿（502 passed）。
3. `cargo clippy` 仍零警告。
4. 启动时若存在新版本，UI 显示非阻塞提示（非强制更新）。

**风险与缓解**：
| 风险 | 缓解 |
|------|------|
| Tauri Windows 打包因 MSVC/WebView2 失败 | Week 2 前在纯净 Windows VM 预演；CI 使用 `windows-latest` |
| 自签名证书被 Windows Defender 严重拦截 | 文档中明确说明"点击更多信息 → 仍要运行"；后续周期评估购买证书 |
| 前端构建与 Rust 构建版本不一致 | CI 中强制 `npm run build` 先于 `cargo tauri build` |

### Bet 2: 首次用户体验（6 周）

**目标**：消除"用户下载后无法完成首次对话"的流失点。

**范围**：
- 启动状态机设计（有模型/无模型 × 有网/无网）
- 模型下载引导流程（推荐模型、下载来源、进度显示、错误处理）
- Settings Panel 增强（模型下载按钮、模型管理、路径浏览）
- 错误状态设计（prewarm 失败、模型损坏、tokenizer 缺失的具体提示）

**非范围**：
- 模型自动静默下载（尊重用户带宽和存储，必须显式确认）
- 模型量化/转换（只引导下载预量化 GGUF）
- HuggingFace 登录/Token 管理
- 云端 provider 的 API key 自动获取

**里程碑**：
| 周次 | 交付物 | 验收标准 |
|------|--------|----------|
| Week 1 | 启动状态机 Rust 实现 | `AppState` 新增 `FtueState`；IPC 暴露 `get_ftue_state` |
| Week 2 | 前端 Onboarding 组件 | 无模型时显示全屏引导（选择云端 provider 或下载本地模型） |
| Week 3 | 模型下载 Rust 命令 | `download_model(url, path)` 命令，带进度事件 `download:progress` |
| Week 4 | 前端下载进度 UI | 进度条 + 速度显示 + 取消按钮 |
| Week 5 | Settings Panel 模型管理 | 已下载模型列表 + 删除按钮 + "打开目录"按钮 |
| Week 6 | E2E 场景验证 | 4 种启动场景均通过人工测试 |

**验收标准**：
1. **无模型 + 无网**：显示"离线模式需要本地模型"，提供"打开模型目录"按钮。
2. **无模型 + 有网**：显示 Onboarding，提供"使用云端 Provider"和"下载本地模型"两选项。
3. **有模型 + 任意网络**：直接进入聊天界面，后台 prewarm。
4. **模型损坏/tokenizer 缺失**：具体错误提示（非"Failed to load local model"），附修复建议链接。
5. 所有错误状态可通过 Settings Panel 修复，无需手动编辑配置文件。

**风险与缓解**：
| 风险 | 缓解 |
|------|------|
| 模型下载 HTTP 超时/断点续传复杂 | 先用简单 HTTP 下载（Tauri `fetch`）；大模型建议用户使用迅雷等工具手动下载 |
| HuggingFace 镜像不稳定 | 推荐 `hf-mirror.com` 作为主源，官方作为 fallback |
| 推荐模型列表过时 | 前端硬编码列表，每次版本发布时更新；不实现动态查询 |

### 冷却期（2 周）任务清单

- [ ] 代码审查：重点审查 Bet 2 中新增的 `unsafe` 等价操作（HTTP 下载、文件系统写入）
- [ ] 重构：`AppState` 构造器提取（技术债务）
- [ ] 文档：`AGENTS.md` 更新打包命令；`docs/ARCHITECTURE.md` 更新 FTUE 状态机
- [ ] 竞品监控：检查 ZeroClaw、5ire 近 8 周 release note，评估差异化是否仍然成立
- [ ] 下一周期规划：基于本周期实际 velocity 调整下一周期范围

---

## 五、版本管理策略

### 5.1 v0.2.0 tag 问题处理

**当前状态**：
- `v0.2.0` tag → `5de3767`（仅 docs commit，"Release v0.2.0" 文档）
- `main` → `4da4a4d`，领先 tag 9 commits（含 local-llm 默认、离线 fallback、58 文件 fmt）
- workspace `Cargo.toml` → `version = "0.1.0"`

**选项对比**：

| 选项 | 操作 | 优势 | 劣势 | 建议 |
|------|------|------|------|------|
| A | 强制移动 tag (`git tag -f`) | 简单 | 重写公开历史，破坏已有引用 | ❌ 不推荐 |
| B | 发布 v0.2.1 补丁 | 符合 SemVer，不破坏历史 | v0.2.0 tag 仍指向错误 commit | ✅ **推荐** |
| C | 删除 v0.2.0，直接发 v0.3.0 | 彻底解决问题 | v0.2.x 系列空缺，用户困惑 | ⚠️ 激进 |

**决策**：采用 **选项 B + 文档说明**。
1. 立即在 `main` 打 `v0.2.1` 补丁（修正 workspace version + CHANGELOG + RELEASE 数据）。
2. `v0.2.0` tag 保留但废弃，在 Release 页面标注"v0.2.0 为文档标记，请使用 v0.2.1"。
3. 完成 Bet 1 后发布 `v0.3.0`（首个带安装包的正式版本）。

### 5.2 未来版本号管理流程

- **SemVer 严格规则**：
  - MAJOR：破坏性 API 变更（Tauri command 签名变更、配置格式不兼容）
  - MINOR：新功能（发布基础设施、FTUE、新 provider）
  - PATCH：bugfix、文档修正、安全修复
- **版本同步检查清单**（打 tag 前必须执行）：
  1. `Cargo.toml` workspace version 与目标 tag 一致
  2. `CHANGELOG.md` 已添加该版本条目，日期正确
  3. `RELEASE_vX.Y.Z.md` 已创建，测试数据已更新
  4. `cargo test --workspace --lib` 全绿
  5. `cargo clippy` 零警告
- **自动化工具**：
  - 暂不引入 `release-plz`（增加项目复杂度，与单开发者工作流匹配度低）。
  - 采用手动检查清单 + GitHub Actions Release workflow 半自动模式。

### 5.3 CHANGELOG 维护规范（Keep a Changelog 合规性检查）

当前偏离点：
1. ❌ 版本日期未按时间顺序排列（0.1.2 — 2026-04-20 在 0.1.1 — 2026-04-23 之前）。
2. ❌ [Unreleased] 后缺少 [0.2.0] 章节（已交付功能未归档）。
3. ❌ 日期格式不统一（应统一为 `YYYY-MM-DD`）。
4. ✅ 使用 [Added]/[Changed]/[Fixed]/[Security] 分类。
5. ✅ 声明基于 Keep a Changelog 和 SemVer。

修复动作：
- 将 [0.1.2] 日期修正为 `2026-04-24`（或合理日期），确保时间顺序。
- 将 [Unreleased] 中已交付的 local-llm 相关内容移至新建的 [0.2.0] — 2026-04-25 章节。
- 未来每次 PR/合并前，强制要求更新 CHANGELOG（主会话执行合并前检查）。

---

## 六、Tauri 打包架构设计

### 6.1 Windows 打包方案

| 格式 | 优先级 | 理由 | 工具 |
|------|--------|------|------|
| `.msi` | P0 | 标准 Windows 安装体验，支持开始菜单/卸载 | Tauri built-in (`cargo tauri build`) |
| `.exe` (portable) | P0 | 无需安装，直接运行，适合高级用户 | Tauri built-in |
| 微软商店 | P2 | 需要商业账号 + 审核周期，当前无必要 | 延期 |

**依赖处理**：
- WebView2 Runtime：Windows 11 已预装；Windows 10 用户 Tauri installer 可自动下载安装。
- MSVC Runtime：静态链接或随安装包分发。

### 6.2 macOS 打包方案

| 格式 | 优先级 | 理由 |
|------|--------|------|
| `.dmg` | P1 | 标准 macOS 分发格式 |
| 苹果商店 | P3 | 需要 Apple Developer ID + 沙箱适配 |

**延期理由**：juice094 当前开发环境为 Windows，macOS 构建需 GitHub Actions 或物理 Mac。建议在 Bet 1 完成后，利用冷却期在 CI 中验证 macOS 构建，但不作为 v0.3.0 阻塞项。

### 6.3 Linux 打包方案

| 格式 | 优先级 | 理由 |
|------|--------|------|
| `.AppImage` | P2 | 最兼容，无需依赖系统库 |
| `.deb` | P2 | Debian/Ubuntu 用户友好 |
| Flatpak/Snap | P3 | 分发渠道复杂，维护成本高 |

**延期理由**：Linux 目标用户更可能使用 `cargo install`（已具备 Rust 工具链）；Tauri Linux 打包依赖系统库（libwebkit2gtk），CI 构建复杂。优先级低于 Windows。

### 6.4 代码签名策略

| 平台 | 方案 | 成本 | 建议 |
|------|------|------|------|
| Windows | 自签名证书 | 免费 | **先用此方案**。PowerShell `New-SelfSignedCertificate` 生成，CI 中签名。用户首次运行需点击"仍要运行"。 |
| Windows | 标准代码签名证书 | $200-700/年 | v0.5.0+ 评估购买，消除 SmartScreen 警告 |
| Windows | EV 代码签名证书 | $500-1000/年 | 长期目标，当前不需要 |
| macOS | Apple Developer ID | $99/年 | macOS 打包时必需，延期至该阶段 |

### 6.5 自动更新策略

**决策**：**"简单检查"**（不做完整自动更新）。

| 策略 | 说明 | 选择 |
|------|------|------|
| 不做 | 用户自行关注 Release | ❌ 不利于留存 |
| 简单检查 | 启动时查询 GitHub API，有新版本时显示提示 banner，用户手动下载 | ✅ **当前选择** |
| 完整方案 | Tauri updater 自动下载+安装，需签名+更新服务器 | ❌ 复杂度超出现阶段 |

**实现细节**：
- Rust command：`check_update() -> Option<String>`（最新版本号或 null）
- 前端：Settings Panel 底部显示当前版本；若有更新，显示"新版本可用"链接到 Release 页面。
- 频率：每次启动检查一次，结果缓存 24 小时。

### 6.6 GitHub Actions Release workflow 设计

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  release-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Build frontend
        working-directory: crates/clarity-tauri/frontend
        run: npm ci && npm run build
      - name: Build Tauri
        run: cargo tauri build
      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: clarity-windows
          path: |
            target/release/bundle/msi/*.msi
            target/release/bundle/nsis/*.exe
```

---

## 七、首次用户体验 (FTUE) 流程设计

### 7.1 启动时状态机

```
[App Launch]
    │
    ▼
[Check local models exist?]
    ├─ Yes ──────────────────────────┐
    │                                ▼
    │                    [Check network?]
    │                        ├─ Yes ──► [Prewarm preferred provider]
    │                        └─ No  ──► [Prewarm local provider]
    │                                │
    │                                ▼
    │                           [Main Chat UI]
    │
    └─ No ───────────────────────────┐
                                     ▼
                         [Check network?]
                             ├─ Yes ──► [Show Onboarding]
                             │              ├─ Option A: Select cloud provider
                             │              └─ Option B: Download local model
                             └─ No  ──► [Show Error State]
                                            "Offline mode requires a local model.
                                             Please connect to the internet to download,
                                             or place a .gguf file in ~/models/."
```

**状态枚举**（Rust）：
```rust
pub enum FtueState {
    Ready,           // 有模型，正常启动
    Onboarding,      // 无模型，有网
    OfflineBlocked,  // 无模型，无网
    PrewarmFailed(String), // 有模型但加载失败
}
```

### 7.2 模型下载引导流程

**推荐模型**（基于用户硬件自动建议）：

| 用户硬件 | 推荐模型 | 大小 | 来源 |
|----------|----------|------|------|
| 无 GPU / <4GB 显存 | DeepSeek-R1-Distill-Qwen-1.5B-Q4_K_M.gguf | ~1GB | HuggingFace |
| 4-8GB 显存 | Qwen2.5-7B-Instruct.Q4_K_M.gguf | ~4GB | HuggingFace |
| >8GB 显存 | Qwen2.5-14B-Instruct.Q4_K_M.gguf | ~8GB | HuggingFace |

**下载流程**：
1. 用户选择推荐模型（或粘贴自定义 URL）。
2. 前端显示确认对话框（模型名称、大小、预计下载时间）。
3. 用户确认后，调用 `download_model(url, filename)`。
4. Rust 后台使用 `reqwest` 下载，通过 Tauri event `download:progress` 报告进度。
5. 下载完成后，自动扫描 `~/models/`，刷新模型列表。
6. 若下载失败，显示具体错误（网络超时/磁盘满/校验失败），提供重试按钮。

**错误处理**：
- 网络超时：自动重试 3 次，之后提示"网络不稳定，建议手动下载"。
- 磁盘空间不足：下载前检查可用空间（模型大小 × 1.5），不足时提前拒绝。
- 校验：暂不做 SHA256 校验（依赖外部数据源不稳定），仅检查文件大小。

### 7.3 Settings Panel 增强点

新增功能：
1. **"下载模型"按钮**：打开模型选择对话框（见 7.2）。
2. **已下载模型列表**：显示文件名、大小、路径，附带"删除"按钮（需二次确认）。
3. **"打开模型目录"按钮**：调用系统文件管理器打开 `~/models/`。
4. **模型路径浏览选择器**：替代当前 `readOnly` input，允许用户通过系统对话框选择 `.gguf` 文件。

### 7.4 错误状态设计

| 错误场景 | 前端显示 | 修复引导 |
|----------|----------|----------|
| prewarm 失败 | 顶部红色 banner："启动配置错误：{具体原因}" | "打开 Settings" 按钮 |
| 模型文件损坏 | 弹窗："模型文件无法加载（可能损坏）" | "重新下载" 或 "选择其他模型" |
| tokenizer 缺失 | 弹窗："缺少 tokenizer.json" | "下载 tokenizer.json 并放置到模型同级目录" + 打开目录按钮 |
| tokenizer 损坏（<1KB） | 弹窗："tokenizer.json 似乎已损坏" | 同上 |
| 网络 provider 失败 | 顶部黄色 banner："连接失败，已切换至离线模式" | "检查 API Key" 或 "切换至本地模型" |
| 离线恢复 | 顶部绿色 banner："网络已恢复，正在切换回云端 provider" | 自动，无需操作 |

---

## 八、风险矩阵与监控指标

### 8.1 完整风险矩阵

| 风险项 | 概率 | 影响 | 等级 | 理论依据 | 缓解措施 |
|--------|------|------|------|----------|----------|
| Tauri Windows 打包因 MSVC/WebView2 依赖失败 | 中 | 高 | 🔴 高 | TOC：约束环节失败将阻断整个价值流 | CI 预演 + 本地纯净 VM 测试 |
| 自签名证书导致 Windows SmartScreen 严重拦截用户 | 高 | 中 | 🟡 中 | Cynefin Complicated：已知问题，多方案可选 | 文档教育用户 + 后续购买证书 |
| 模型下载引导涉及 HuggingFace 模型协议法律风险 | 低 | 中 | 🟡 中 | 风险报告 1.1 | UI 明确标注各模型协议，Clarity 不分发模型 |
| 竞品（ZeroClaw/5ire）快速迭代覆盖"本地优先"差异化 | 中 | 高 | 🔴 高 | 风险报告 4.2 | 专注差异化不追赶；Shape Up 周期保护聚焦 |
| 单开发者 burnout | 中 | 极高 | 🔴 高 | TOC：人力资源是终极约束 | 严格执行 6+2 Shape Up 节奏；冷却期不开发 |
| CUDA 编译在目标用户机器上失败 | 低 | 低 | 🟢 低 | AGENTS.md 已知限制 | CUDA 为可选 feature，默认 CPU |
| GitHub Actions 分钟数耗尽（免费额度 2000 min/月） | 低 | 中 | 🟡 中 | — | Tauri 构建缓存优化；必要时购买 Team 计划 |
| ROADMAP 风险对冲触发（30 天 Star < 50） | 中 | 高 | 🔴 高 | ROADMAP 4.2 | 提前准备冻结方案，资源回拨至 devbase |

### 8.2 关键监控指标

| 指标 | 当前基线 | 目标 | 测量方式 |
|------|----------|------|----------|
| CI 通过率 | 100% | 维持 100% | GitHub Actions dashboard |
| 发布频率 | 无固定节奏 | 每 6-8 周一个版本 | GitHub Releases 页面 |
| 用户上手时间（下载→首次对话） | 15-30 分钟（需 Rust + 手动配置） | < 5 分钟 | 人工计时（新 Windows VM） |
| GitHub Stars | — | v0.3.0 后 30 天 ≥ 50 | GitHub API |
| Issues + PRs | — | v0.3.0 后 30 天 ≥ 3 | GitHub API |
| `cargo test` 耗时 | ~5 min | < 8 min | CI 日志 |
| 安装包体积 | — | < 50 MB（不含模型） | artifact 属性 |

### 8.3 触发条件（何时调整计划）

| 条件 | 动作 |
|------|------|
| CI 失败率 > 10%（连续 3 次失败） | 暂停功能开发，修复 CI；若 48 小时内无法恢复，回滚至 last known good commit |
| 连续 2 个版本未在 8 周内发布 | 下一周期范围收缩 50%，仅保留核心功能 |
| v0.3.0 发布后 30 天 Star < 20 | 执行 ROADMAP 风险对冲：冻结阶段二，资源回拨至 devbase |
| 竞品发布同等"本地优先"功能 | 召开 2 小时紧急评估：差异化是否仍然成立；若否，调整定位 |
| 单开发者连续工作 > 10 天无休息 | 强制进入冷却期，无论当前周期进度 |

---

## 九、立即执行清单

### 9.1 无需用户确认（技术债务修复）

| # | 任务 | 文件 | 变更内容 | 预计工时 | 依赖 |
|---|------|------|----------|----------|------|
| 1 | 修正 workspace version | `Cargo.toml` | `version = "0.2.1"` | 2 min | 无 |
| 2 | 修复 CHANGELOG 版本顺序 | `CHANGELOG.md` | [0.1.2] 日期改为 `2026-04-24`；新建 [0.2.0] — `2026-04-25` 并迁移 [Unreleased] 已交付内容 | 15 min | 无 |
| 3 | 修正 RELEASE_v0.2.0.md 数据 | `docs/RELEASE_v0.2.0.md` | 测试数改为 502/4；补充 local-llm 默认启用、离线 fallback 等功能 | 10 min | 无 |
| 4 | 修正 README 表述 | `README.md` | "Zero runtime dependencies" → "`cargo install` produces a fully working binary. Pre-built installers coming soon." | 5 min | 无 |
| 5 | 修正 PROJECT_STATUS 审批系统状态 | `docs/PROJECT_STATUS.md` | "🔄 进行中" → "⏸️ 未启动（设计完成，代码未动工）" | 2 min | 无 |

**总计**：< 1 小时，可一次性 commit：`chore: fix documentation drift and version sync for v0.2.1`。

### 9.2 需要用户确认的决策点

| # | 决策 | 选项 | 建议 | 阻塞任务 |
|---|------|------|------|----------|
| A | v0.2.0 tag 处理 | 1) 保留并废弃 2) 强制移动 3) 删除 | **选项 1**：保留并废弃，发 v0.2.1 补丁 | 所有发布相关任务 |
| B | Windows 代码签名 | 1) 自签名（免费，有警告）2) 购买标准证书 | **选项 1**：先用自签名，v0.5.0+ 评估购买 | Bet 1 Week 4 |
| C | 默认推荐模型 | 1) 1.5B（快但弱）2) 7B（平衡） | **选项 2**：默认推荐 7B，但检测显存 < 4GB 时建议 1.5B | Bet 2 Week 3 |
| D | 是否引入 `release-plz` | 1) 手动 2) 半自动 `release-plz` | **选项 1**：手动，单开发者场景下自动化工具收益低 | 长期版本管理 |

### 9.3 任务依赖图

```
技术债务修复 ──► v0.2.1 tag ──► Bet 1: 发布基础设施 ──► Bet 2: 首次用户体验
    │                                                │
    └─► 可并行：短期技术债务（AppState 重构）◄─────────┘
```

---

## 附录：理论基础引用

| 理论 | 应用位置 | 核心观点 |
|------|----------|----------|
| **Cynefin 框架** | 一、八 | 项目处于 **Complicated Domain**（有序但需专家分析）。已知问题（文档漂移、版本不一致）可通过专家诊断解决；无需实验性探索。决策模式：**Sense-Analyze-Respond**。 |
| **约束理论 (TOC)** | 二 | 价值流最大约束是"用户上手时间"。非约束环节（CI、功能开发）已足够快，不应再优化。资源应全部投向约束环节（打包 + FTUE）。 |
| **Martin Fowler 技术债务象限** | 三 | 将债务按"谨慎/鲁莽 × 有意/无意"分类，优先偿还"鲁莽"类（文档夸大、版本错误），因为它们直接损害项目信誉。 |
| **Shape Up (Basecamp)** | 四 | 6 周固定周期 + 2 周冷却。每个 Bet 必须有"范围、非范围、里程碑、验收标准"。非范围是防止范围蔓延的硬性边界。冷却期用于重构和规划，保护开发者不 burnout。 |
| **风险矩阵** | 八 | 概率 × 影响 = 风险等级。高概率高影响项（打包失败、竞品覆盖、burnout）需主动缓解；低概率低影响项（CUDA 编译失败）可接受。 |

---

*本规划基于可验证数据编制。所有"立即执行"任务可在当前会话内完成，无需外部依赖。*
