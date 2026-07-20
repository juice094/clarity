---
title: Clarity 模块解构与专业工程路线对照
category: Planning
date: 2026-06-26
tags: [research, modules, roadmap, architecture, worktree]
---

# Clarity 模块解构与专业工程路线对照

> 本文档将 Clarity 项目自身工作树与 `roadmap.sh`（`nilbuild/developer-roadmap`）中的专业工程路线进行对照，评估各模块的解构情况，并给出模块级后续优化方向。
>
> 用途：作为项目研究与开发的导航图，便于在简历、面试、技术分享或迭代规划时快速定位每个模块的「当前位置」与「下一步」。
>
> 数据来源：
> - 本地仓库：`C:/Users/22414/dev/clarity`
> - 参考路线：`C:/Users/22414/dev/developer-roadmap`（roadmap.sh）
> - 生成脚本：`scripts/arch_health.py`、`scripts/test_runner.py`
> - 实机验证：2026-06-26
>
> **时效注（2026-07-20）**：本文事实层为 2026-06-26 快照，以下已过期：
> - crate 基线「22 活跃 + 1 归档」已失效：现为 24 个 clarity crate + 6 个 syncthing crate + 1 集成测试 crate；`clarity-slint` 也已归档至 `.archive/`。
> - 文中列出的 `clarity-openclaw` 已删除（并入 `clarity-claw`）；缺少后增的 `clarity-knowledge` / `clarity-ui` / `clarity-shell` / `clarity-apps` / `clarity-chrome` / `clarity-anthropic-proxy`。
> - 最新 crate 拓扑以 `AGENTS.md` §3 与 `docs/ARCHITECTURE.md` 为准。方法论对照部分仍有效。

---

## 1. 参考的专业工程路线

从 `nilbuild/developer-roadmap` 中选取与 Clarity 最相关的 10 条路线：

| 路线 | 核心关注点 | 对 Clarity 的指导意义 |
|------|-----------|---------------------|
| **Backend Developer** | 语言、DB、API、缓存、消息队列、CI/CD、12-factor | Rust 后端运行时设计；SQLite 选型；REST/WebSocket API；CI 流水线 |
| **System Design** | 性能/可扩展/可用性、CAP、缓存、异步、通信协议 | Gateway 双端口；本地优先架构；异步任务队列；WebSocket/SSE 流式 |
| **Software Architect** | 分层、DDD、CQRS、Actor、安全、API 设计 | crate 拓扑；Contract-First；事件总线；安全模型 |
| **Software Design and Architecture** | SOLID、YAGNI、耦合/内聚、设计模式 | `clarity-contract` 拆分；前端解耦；Ponytail lazy-senior-dev 原则 |
| **DevOps** | CI/CD、容器、监控、IaC、发布 | GitHub Actions 12-job；release 流水线；telemetry |
| **Rust Developer** | ownership、crate、tokio、Axum、密码学 | workspace 组织；async runtime；Gateway Axum；secrets 加密 |
| **AI Engineer** | LLM、RAG、向量检索、提示工程、部署 | provider 抽象；混合记忆；本地 GGUF；prompt 安全 |
| **AI Agents** ⭐ | Agent loop、工具调用、记忆、ReAct/Plan、MCP、多 Agent | 直接映射 Clarity 核心能力 |
| **QA Engineer** | 测试分层、自动化、CI 集成、性能/安全测试 | lib/bin/doc/integration 四层；手动 QA；测试策略 |
| **Cyber Security** | 安全编码、TLS、密码学、威胁模型、OWASP | `rustls`；`enc2:`；路径/MCP 校验；`SECURITY.md` |

---

## 2. Clarity 完整工作树（Worktree）

### 2.1 项目根目录

```text
C:/Users/22414/dev/clarity/
├── .cargo/                  # cargo 配置、增量编译、Slint 快捷命令
├── .clarity/                # 本地运行时数据（sessions、tasks）
├── .github/workflows/       # CI / Release（12-job）
├── crates/                  # 23 个 crate 目录（22 活跃 + 1 归档）
├── docs/                    # 架构、开发、安全、规划文档
├── examples/                # 独立示例脚本
├── mobile/                  # Android 原生工程（未来 iOS）
├── scripts/                 # verify.ps1、Python 测试/健康脚本
├── skills/                  # Agent 技能模板
├── tests/integration/       # 集成测试 crate
├── third_party/             # syncthing-rust（openclaw BEP 传输）
├── Cargo.toml              # workspace 配置
├── AGENTS.md               # Agent 运行上下文与代码风格
└── README.md / README.en.md / README.zh.md
```

### 2.2 Crate 分层拓扑

```text
[Presentation Layer — 入口前端]
  ├─ clarity-egui          (bin+lib)  桌面 GUI（主入口）
  ├─ clarity-tui           (bin+lib)  终端 UI
  ├─ clarity-gateway       (bin+lib)  Web IDE / HTTP + WebSocket
  ├─ clarity-claw          (bin+lib)  系统托盘节点
  ├─ clarity-headless      (bin)      无头 CLI / CI
  └─ clarity-mobile-core   (bin+lib)  移动端 UniFFI FFI 核心

[Agent Kernel — 运行时核心]
  └─ clarity-core          (lib)      ReAct/Plan、Approval、Skill、MCP、Thread 生命周期

[Infrastructure / Capability — 能力 crate]
  ├─ clarity-memory        (lib)      SQLite + BM25 + 向量搜索
  ├─ clarity-llm           (lib)      Provider 抽象 + Candle GGUF 本地推理
  ├─ clarity-tools         (lib)      内置工具库
  ├─ clarity-mcp           (lib)      MCP 客户端（stdio/SSE/HTTP/WS）
  ├─ clarity-channels      (lib)      外部通道抽象（WeChat iLink / Webhook）
  ├─ clarity-secrets       (lib)      加密 Secret 存储（enc2:）
  ├─ clarity-openclaw      (lib)      OpenClaw/KimiClaw Gateway WebSocket 客户端
  ├─ clarity-telemetry     (lib)      统一遥测（当前由 gateway 使用）
  ├─ clarity-subagents     (lib)      子代理执行器（消费 core）
  ├─ clarity-thread-store  (lib)      Thread 持久化抽象
  └─ clarity-rollout       (lib)      JSONL rollout 持久化

[Contract / Protocol — 零业务逻辑的契约层]
  ├─ clarity-contract      (lib)      共享 trait / 类型（零内部依赖）
  └─ clarity-wire          (lib)      UI ↔ Agent SPMC 事件总线

[Experimental / Utility]
  ├─ clarity-slint         (bin+lib)  实验性 Slint GUI（依赖 contract + wire）
  ├─ clarity-anthropic-proxy (bin)    Anthropic Messages API → DeepSeek 代理
  └─ clarity-tauri         (bin)      已归档，被 workspace 排除
```

### 2.3 规模基线（2026-06-26）

| 指标 | 数值 |
|------|------|
| Workspace members | 24（23 crates + tests/integration） |
| 活跃 crate | 22（+ 1 归档 clarity-tauri） |
| Rust 源文件 | 693 |
| 非空 Rust 行数 | 176,114 |
| lib tests | 1554 passed / 0 failed / 12 ignored |
| bin tests | 275 passed / 0 failed / 2 ignored |
| doc tests | 34 passed / 0 failed / 3 ignored |
| integration tests | 26 passed / 0 failed |
| Clippy warnings | 0 |

---

## 3. 模块级解构评估

### 3.1 评估维度说明

对每个 crate 从 5 个维度打分（✅ 健康 / ⚠️ 需关注 / ❌ 明显问题）：

| 维度 | 含义 |
|------|------|
| **边界清晰度** | 职责是否单一，依赖是否可控 |
| **文档完整性** | README + AGENTS 是否齐全 |
| **测试覆盖** | 是否有测试，是否能独立验证 |
| **专业路线对齐** | 与 roadmap.sh 中对应能力的匹配度 |
| **演进空间** | 是否已有明确的下一步优化方向 |

### 3.2 逐模块评估

#### Contract / Protocol 层

| Crate | 边界 | 文档 | 测试 | 路线对齐 | 演进空间 | 备注 |
|-------|------|------|------|---------|---------|------|
| `clarity-contract` | ✅ | ✅ | ✅ | ✅ | ✅ | 零内部依赖，理想基础层 |
| `clarity-wire` | ✅ | ✅ | ✅ | ✅ | ✅ | SPMC 事件总线，跨前端解耦 |

**优化方向**：
- `clarity-wire` 未来若支持跨进程/网络前端，可考虑增加序列化稳定性测试与版本协商。
- `clarity-contract` 新增 trait 时需同步更新所有实现者；建议维护一个 `BREAKING_CHANGES.md` checklist。

#### Infrastructure / Capability 层

| Crate | 边界 | 文档 | 测试 | 路线对齐 | 演进空间 | 备注 |
|-------|------|------|------|---------|---------|------|
| `clarity-memory` | ✅ | ✅ | ✅ | ✅ | ⚠️ | BM25+向量，hermes feature 依赖外部仓库 |
| `clarity-llm` | ✅ | ✅ | ✅ | ✅ | ✅ | Provider 抽象 + Candle GGUF |
| `clarity-tools` | ✅ | ✅ | ✅ | ✅ | ✅ | 内置工具库，沙箱校验 |
| `clarity-mcp` | ✅ | ✅ | ✅ | ✅ | ✅ | MCP 四传输，clean boundary |
| `clarity-channels` | ⚠️ | ✅ | ✅ | ⚠️ | ❌ | 仅 WeChat iLink 实现，Discord/Slack/Telegram 禁用 |
| `clarity-secrets` | ✅ | ✅ | ✅ | ✅ | ✅ | 小但职责明确 |
| `clarity-openclaw` | ✅ | ✅ | ✅ | ✅ | ✅ | 协议 dialect 检测已落地 |
| `clarity-telemetry` | ✅ | ✅ | ✅ | ✅ | ✅ | 仅依赖 contract，健康 |
| `clarity-subagents` | ⚠️ | ✅ | ✅ | ✅ | ⚠️ | 依赖 core，随 core 重量增长 |
| `clarity-thread-store` | ✅ | ✅ | ✅ | ✅ | ✅ | 2-dep clean crate |
| `clarity-rollout` | ✅ | ✅ | ✅ | ✅ | ✅ | 小而聚焦 |

**优化方向**：
- `clarity-channels`：若多通道非核心，建议拆分为「通道抽象 + 各通道实现」或明确仅保留 WeChat iLink/Webhook，其余归档。
- `clarity-memory`：hermes feature 需外部仓库，建议提供 feature-off 的完整测试路径；可考虑向量索引独立为可选后端。
- `clarity-subagents`：监控随 `clarity-core` 膨胀带来的编译耦合；未来若出现独立调度语义，可进一步拆分。

#### Agent Kernel

| Crate | 边界 | 文档 | 测试 | 路线对齐 | 演进空间 | 备注 |
|-------|------|------|------|---------|---------|------|
| `clarity-core` | ⚠️ | ✅ | ✅ | ✅ | ❌ | 最大 crate，~38k LOC，142 个文件 |

**优化方向**：
- `clarity-core` 是首要拆分对象。潜在子域：
  - `clarity-agent-loop`：ReAct / Plan / streaming
  - `clarity-approval`：审批规则引擎
  - `clarity-skills`：Skill 加载与发现
  - `clarity-session`：Session/Thread 生命周期（部分已迁移到 thread-store）
  - `clarity-background`：后台任务管理
- 拆分原则：每个新 crate 必须被 ≥2 调用方需要，且不能破坏 `clarity-core` 不依赖前端/网络的不变量。

#### Presentation 层

| Crate | 边界 | 文档 | 测试 | 路线对齐 | 演进空间 | 备注 |
|-------|------|------|------|---------|---------|------|
| `clarity-egui` | ⚠️ | ✅ | ✅ | ✅ | ⚠️ | ~30k LOC，144 个文件，面板需控制行数 |
| `clarity-tui` | ✅ | ✅ | ✅ | ✅ | ✅ | 适中 |
| `clarity-gateway` | ⚠️ | ✅ | ✅ | ✅ | ❌ | 12 个内部依赖，耦合最高 |
| `clarity-claw` | ✅ | ✅ | ✅ | ✅ | ✅ | 边界清晰 |
| `clarity-headless` | ✅ | ✅ | ✅ | ✅ | ✅ | 精简 |
| `clarity-mobile-core` | ✅ | ✅ | ✅ | ✅ | ⚠️ | FFI 核心已落地，完整 UI 待续 |

**优化方向**：
- `clarity-egui`：严格执行面板 <300 行规则；引入 `egui_kittest` snapshot 测试；将业务逻辑进一步下沉到纯函数。
- `clarity-gateway`：12 个内部依赖中，部分可能通过 `clarity-wire` + feature flag 解耦；建议按 API 域（session/task/parallel/admin）拆分子模块。
- `clarity-mobile-core`：继续完善 Kotlin/Swift 示例工程；补齐移动端 FFI 集成测试。

#### Experimental / Utility

| Crate | 边界 | 文档 | 测试 | 路线对齐 | 演进空间 | 备注 |
|-------|------|------|------|---------|---------|------|
| `clarity-slint` | ✅ | ✅ | ✅ | ⚠️ | ⚠️ | 实验性，不参与 CI |
| `clarity-anthropic-proxy` | ✅ | ✅ | ✅ | ✅ | ⚠️ | 协议转换已下沉到 `clarity-llm::anthropic` |
| `clarity-tauri` | — | ✅ | — | ❌ | — | 已归档 |

**优化方向**：
- `clarity-anthropic-proxy`：协议转换模块已下沉到 `clarity-llm::anthropic`；当前 crate 为薄壳二进制。
- `clarity-slint`：明确是「技术储备」还是「即将废弃」；若前者，增加 CI 中的可选构建 job。
- `clarity-tauri`：保持归档状态，禁止修改；目录保留仅供历史参考。

---

## 4. 专业路线对照矩阵

### 4.1 AI Agents 路线 vs Clarity

`roadmap.sh/ai-agents` 的核心能力与 Clarity 映射：

| AI Agents 能力 | Clarity 实现 | 文件/模块 |
|---------------|-------------|----------|
| Agent loop（感知→推理→工具调用→观察） | ReAct/Plan loop | `crates/clarity-core/src/agent/` |
| Tool invocation | `Tool` trait + MCP + 内置工具 | `crates/clarity-tools/`、`crates/clarity-mcp/` |
| Agent memory（短期/长期/语义/情景） | SQLite + BM25 + 向量 + rollout | `crates/clarity-memory/`、`crates/clarity-rollout/` |
| ReAct / Plan | `AgentController` / `Op` enum | `crates/clarity-core/src/agent/` |
| MCP | MCP 客户端四传输 | `crates/clarity-mcp/src/` |
| Multi-agent | `SubAgentManager` / `AgentPool` | `crates/clarity-subagents/src/` |
| 安全/红队 | Approval 四层 + MCP 命令校验 | `crates/clarity-core/src/approval/` |
| 本地部署 | Candle GGUF + 单二进制 | `crates/clarity-llm/src/kalosm.rs` |

**结论**：Clarity 在 AI Agents 路线上覆盖度很高，已落地核心能力；主要差距在「评估（evaluation）」与「红队测试」的系统化。

### 4.2 System Design 路线 vs Clarity

| System Design 主题 | Clarity 现状 | 评估 |
|-------------------|-------------|------|
| Latency vs throughput | Pretext 测量已优化 egui 渲染延迟 | ✅ |
| Availability vs consistency | 本地单实例，一致性优先 | ✅ |
| Caching | 无独立缓存层，依赖 SQLite + 向量索引 | ⚠️ 可引入内存缓存 |
| Async / message queues | tokio + `clarity-wire` SPMC | ✅ |
| Load balancing | 不适用（本地运行时） | ✅ 符合定位 |
| Horizontal scaling | 不适用 | ✅ 符合定位 |
| Databases | SQLite + 向量索引 | ✅ 本地优先 |
| Communication | HTTP/WebSocket/SSE | ✅ |

**结论**：Clarity 作为本地运行时，主动放弃了分布式扩展，符合 Hard Veto；缓存层是未来可补充点。

### 4.3 QA Engineer 路线 vs Clarity

| QA 主题 | Clarity 现状 | 评估 |
|--------|-------------|------|
| 测试分层 | lib/bin/doc/integration 四层 | ✅ |
| 自动化测试 | cargo + Python 脚本 | ✅ |
| CI/CD 集成 | GitHub Actions 12-job | ✅ |
| UI 自动化 | 手动 QA | ❌ 待引入 egui_kittest |
| API 契约测试 | 部分集成测试 | ⚠️ Gateway 契约待补 |
| 性能/负载测试 | Pretext 基准 | ⚠️ 可扩展 |
| 安全测试 | cargo audit + 静态校验 | ⚠️ 可引入 fuzz |

**结论**：测试基础扎实，UI/API E2E 是下一层。

### 4.4 Cyber Security 路线 vs Clarity

| 安全主题 | Clarity 现状 | 评估 |
|---------|-------------|------|
| 安全编码 | `unsafe_code = "deny"`；仅 1 处白名单 | ✅ |
| TLS | rustls，openssl 已移除 | ✅ |
| 密码学 | ChaCha20-Poly1305 | ✅ |
| 输入校验 | `sanitize_path`、`validate_mcp_command` | ✅ |
| Secret 管理 | `enc2:` 加密 | ✅ |
| 威胁模型 | `docs/security/THREAT_MODEL.md` | ✅ |
| 漏洞响应 | `SECURITY.md` | ✅ |

**结论**：安全模型完整，可继续增加 fuzz 与依赖审计自动化。

---

## 5. 模块级后续优化方向

### 5.1 高优先级（影响大、风险可控）

| 模块 | 优化项 | 预期收益 | 参考路线 |
|------|--------|---------|---------|
| `clarity-core` | 逐步拆分：agent-loop / approval / skills / background | 降低编译耦合、提升可测试性、便于多人协作 | Software Architect / AI Agents |
| `clarity-gateway` | 按 API 域拆分 handler；引入 API 契约测试 | 降低 12 个内部依赖的维护成本 | Backend / System Design |
| `clarity-egui` | 引入 `egui_kittest` snapshot；面板函数控制在 300 行内 | 防止 UI 回归、降低维护难度 | QA / Software Design |
| `clarity-channels` | 明确多通道战略：保留抽象 + Webhook，其余归档或独立仓库 | 减少误导与维护负担 | Backend / AI Agents |

### 5.2 中优先级（增强能力）

| 模块 | 优化项 | 预期收益 | 参考路线 |
|------|--------|---------|---------|
| `clarity-memory` | 向量后端插件化；补 hermes-off 完整测试 | 提升可移植性 | AI Engineer / System Design |
| `clarity-llm` | Provider failover 混沌测试；本地推理性能基准 | 提升可靠性 | AI Engineer / QA |
| `clarity-mcp` | MCP Server 实现 + 协议一致性测试 | 从 Client 扩展到 Server | AI Agents |
| `clarity-subagents` | 独立调度语义；团队协议标准化 | 支撑更复杂 Multi-Agent | AI Agents |
| `clarity-mobile-core` | Android/iOS 示例工程 + FFI 集成测试 | 推进移动端落地 | Rust Developer |

### 5.3 低优先级（技术债与完善）

| 模块 | 优化项 | 预期收益 | 参考路线 |
|------|--------|---------|---------|
| `clarity-anthropic-proxy` | 协议转换已下沉到 `clarity-llm::anthropic`；保持薄壳二进制 | 减少重复、统一抽象 | Backend |
| `clarity-slint` | 明确实验定位；可选 CI job | 避免技术债累积 | Software Design |
| `clarity-telemetry` | 增加 GreptimeDB 后端 CI 测试 | 提升遥测稳定性 | DevOps |
| 全仓库 | 引入 `cargo llvm-cov` 覆盖率门禁 | 量化未覆盖区域 | QA |

---

## 6. 研究与开发导航

### 6.1 如果你想深入研究某个方向

| 研究方向 | 必读模块 | 必读文档 |
|---------|---------|---------|
| Agent 内核 | `clarity-core/src/agent/` | `AGENTS.md` §5.2、`docs/ARCHITECTURE.md` |
| 工具生态 | `clarity-tools/`、`clarity-mcp/` | `docs/architecture/protocol-layer.md` |
| 记忆与 RAG | `clarity-memory/` | `docs/ARCHITECTURE.md` 记忆相关 |
| 本地 LLM | `clarity-llm/` | `docs/development/provider-config.md` |
| 多前端架构 | `clarity-wire/`、`clarity-egui/`、`clarity-tui/` | `AGENTS.md` §3 |
| 安全模型 | `clarity-secrets/`、`clarity-core/src/approval/` | `SECURITY.md`、`docs/security/THREAT_MODEL.md` |
| 移动端 | `clarity-mobile-core/`、`mobile/` | `docs/mobile-architecture.md` |
| 协议与网格 | `clarity-openclaw/`、`clarity-claw/` | `docs/architecture/claw-protocol.md` |

### 6.2 如果你想做代码贡献

1. **入门**：从 `clarity-wire` 或 `clarity-rollout` 开始，边界清晰、代码量小。
2. **进阶**：参与 `clarity-memory` 的测试增强或 `clarity-llm` 的 provider 扩展。
3. **架构级**：参与 `clarity-core` 的域拆分设计，需先写 RFC 并通过架构审查。
4. **UI 级**：为 `clarity-egui` 补充 `egui_kittest` snapshot 或 design_system 组件。

### 6.3 如果你想准备面试/简历

- 重点讲 `clarity-core` 的 Agent loop、`clarity-memory` 的混合检索、`clarity-llm` 的本地推理、Pretext 三栏布局优化。
- 用数据说话：22 crate / 176k LOC / 1889+ tests / 0 clippy warning。
- 完整素材见 `docs/development/resume-assets.md`。

---

## 7. 附录

### 7.1 参考路线本地路径

- Backend Developer：`C:/Users/22414/dev/developer-roadmap/src/data/roadmaps/backend/`
- System Design：`C:/Users/22414/dev/developer-roadmap/src/data/roadmaps/system-design/`
- Software Architect：`C:/Users/22414/dev/developer-roadmap/src/data/roadmaps/software-architect/`
- AI Agents：`C:/Users/22414/dev/developer-roadmap/src/data/roadmaps/ai-agents/`
- QA Engineer：`C:/Users/22414/dev/developer-roadmap/src/data/roadmaps/qa/`
- Cyber Security：`C:/Users/22414/dev/developer-roadmap/src/data/roadmaps/cyber-security/`

### 7.2 自动化脚本

```powershell
# 生成架构健康报告
python scripts/arch_health.py --json target/arch-health-current.json

# 生成分层测试报告
python scripts/test_runner.py --json target/test-report.json

# 环境健康检查
python scripts/doctor.py
```

### 7.3 历史基线

| 日期 | Rust files | LOC | lib tests | bin tests | integration |
|------|-----------|-----|----------|----------|-------------|
| 2026-06-26 | 693 | 176,114 | 1554 | 275 | 26 |

---

*本文档应与 `docs/okf/clarity-worktree/`、`docs/testing/TEST_STRATEGY.md`、`docs/development/ARCHITECTURE_HEALTH.md` 配合使用。*
