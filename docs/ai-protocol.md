# AI 关键决策记录 · Clarity

> 本文件记录跨 AI 会话的关键架构决策、状态锚点和 Hard Veto 边界。
> 用途：在上下文压缩后，新会话可快速恢复项目认知。
> 协议版本：V3.1-EP-O

---

## 一、当前会话锚点

**最后更新**：2026-04-27
**当前分支**：`main` @ `39a7308`（ahead of origin/main by 1）
**架构模式**：CLI（单轮/短轮次）
**定位声明**：Clarity 是集群协作原语的单机验证运行时（非本地聊天工具）。
**会话状态**：clarity-egui clippy warnings 清零（11 处修复）；Pretext 健康度审查完成，运维 plan 已写入 `docs/plans/2026-04-27-egui-pretext-health-plan.md`；working tree 待提交

---

## 二、Hard Veto（不可逾越边界）

| 约束 | 状态 | 说明 |
|------|------|------|
| 本地 LLM 优先 | ✅ 生效 | `LocalGgufProvider` 已验证；`ensure_llm` 自动 fallback |
| 禁止数据外泄 | ✅ 生效 | API key 仅存储本地；云端 Provider 由用户显式选择 |
| 禁止 Docker | ✅ 生效 | 无容器化依赖 |
| 禁止 RAG(Qdrant) | ✅ 生效 | `clarity-memory` 使用 SQLite + BM25 + CosineIndex |
| 禁止 Electron | ✅ 生效 | Tauri 2 已替代 |
| 项目广度 ≤ 5 核心工具 | ⚠️ 接近边界 | 当前 6 crates + Tauri GUI + Gateway + TUI，已达上限，新增功能需裁减 |
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
      ↑              ↑
      └────── clarity-tauri ──────┘
      ↑
clarity-memory
      ↑
clarity-wire
```

- `clarity-tui` / `clarity-headless` / `clarity-claw` 均依赖 `clarity-core`
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

暂无 P0/P1 级活跃 bug。

### 5.2 长期冻结（约束解除前不投入）

- T_APPROVAL V2（AI 分类器）
- 快捷键系统
- Mobile 适配
- Plugin SDK / Sandbox
- Vim 集成

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
| egui Pretext 健康度运维硬化（三阶段） | 6 周 | Plan 已产出，见 `docs/plans/2026-04-27-egui-pretext-health-plan.md` |
| Phase A：WebSocket MCP + Gateway↔BTM | 2 周 | 用户确认启动 |
| Release v0.3.1（质量硬化） | ½ 天 | 无 |
| 单机跨窗口协作架构设计 | 2-3 天 | 需 plan 模式 |

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
