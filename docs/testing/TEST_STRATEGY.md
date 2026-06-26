---
title: Clarity Test Strategy
category: Testing
date: 2026-06-25
tags: [testing, strategy, automation, qa]
---

# Clarity 自动化测试策略

> 本文档定义 Clarity 的测试目标、分层模型、模块覆盖矩阵与演进路线。它指导 `scripts/test_runner.py` 的扩展方向，也是 QA/CI/开发者共享的测试设计基准。
>
> 快速入口：[`TESTER_GUIDE.md`](./TESTER_GUIDE.md)  
> 运行基线：[`AGENTS.md`](../../AGENTS.md) §6  
> 架构健康：[`docs/development/ARCHITECTURE_HEALTH.md`](../../docs/development/ARCHITECTURE_HEALTH.md)

---

## 1. 测试哲学

Clarity 是**本地优先、多入口、单内核**的 Rust AI 运行时。测试的核心目标不是「覆盖每一行代码」，而是验证以下三类不变量：

1. **契约不变量**：`clarity-contract` 中的 trait / 消息协议跨 crate 一致。
2. **协作不变量**：`clarity-core` 与 `clarity-memory`/`clarity-llm`/`clarity-mcp` 等能力 crate 能正确组合。
3. **入口不变量**：每个前端/入口（egui/tui/gateway/headless/claw/mobile-core）都能独立构建、启动并消费同一内核。

因此测试按 **「目标 × 模块」** 两个维度组织，而非单纯按模块堆叠单元测试。

---

## 2. 测试目标维度

| 目标 | 关注点 | 典型失败模式 |
|------|--------|-------------|
| **契约/接口正确性** | trait 行为、序列化格式、协议状态机 | 改了 `WireMessage` 但只同步了 egui，TUI/Gateway 崩溃 |
| **单元逻辑** | 纯函数、算法、状态机 | BM25 排序错误、provider failover 不触发 |
| **集成协作** | 多 crate 组合的数据流与控制流 | session 持久化后回放丢失工具结果 |
| **二进制/入口行为** | bin target 启动、参数、退出码、日志 | `clarity-headless --output json` 输出非法 JSON |
| **UI/交互** | 渲染、布局、事件路由、快捷键 | 右 rail 展开时输入框被覆盖 |
| **端到端工作流** | 完整用户场景 | 本地模型加载 → 发送 prompt → 工具调用 → 结果展示的完整链路 |

---

## 3. 测试分层金字塔

```text
           △ 少量 E2E / 手动 QA
          ╱ ╲    工作流、UI、设备配对、发布验收
         ╱   ╲
        ╱ 中量  ╲   二进制测试 + 集成测试 + API 契约测试
       ╱  入口    ╲   验证每个 bin 与跨 crate 组合
      ╱─────────────╲
     ╱    大量单元     ╲   模块内算法/状态机/trait 实现
    ╱   + 文档测试      ╲   + rustdoc 示例
   ╱──────────────────────╲
```

### 3.1 单元测试（Unit）

- **范围**：单个 crate 内部模块，不依赖外部进程/网络/文件系统（或依赖被 mock）。
- **命令**：`cargo test --workspace --lib --exclude clarity-slint`
- **当前数量**：1554 passed / 0 failed
- **负责内容**：
  - `clarity-memory`：BM25 评分、chunking、向量检索
  - `clarity-llm`：provider 选择、failover 逻辑、消息格式化
  - `clarity-core`：`AgentController` 状态机、Approval 规则、Skill 解析
  - `clarity-tools`：路径校验、命令注入拦截

### 3.2 二进制测试（Bin）

- **范围**：每个 `[[bin]]` 或 lib-with-bin 的 `main.rs` 中 `#[cfg(test)]` 逻辑。
- **命令**：`cargo test --workspace --bins --exclude clarity-slint -- --test-threads=2`
- **当前数量**：275 passed / 0 failed
- **负责内容**：
  - 参数解析（`clap`）
  - `build_provider()` 工厂
  - `protocol_renderer.rs` 在 TUI/egui 中的渲染适配
  - 启动路径中的错误处理

### 3.3 文档测试（Doc）

- **范围**：`///` 示例代码。
- **命令**：`cargo test --workspace --doc --exclude clarity-slint -- --test-threads=2`
- **当前数量**：34 passed / 0 failed
- **负责内容**：保证公开 API 的示例代码可编译、可运行。

### 3.4 集成测试（Integration）

- **范围**：跨 crate 的真实组合，通常使用临时目录/内存数据库/mock 服务器。
- **命令**：`cargo test -p clarity-integration-tests --lib`
- **当前数量**：26 passed / 0 failed
- **负责内容**：
  - `adaptive_loop`
  - `session_v2_migration`
  - `telemetry_end_to_end`
  - `thread_api`

### 3.5 E2E / 手动 QA

- **范围**：完整用户场景，可能涉及真实模型、真实浏览器、真实托盘图标。
- **当前方式**：`docs/testing/TESTER_GUIDE.md` §5 手动验收清单。
- **演进目标**：将高频场景自动化为契约测试或 UI 快照测试。

---

## 4. 模块 × 目标覆盖矩阵

| 模块 | 契约 | 单元 | 集成 | 二进制 | UI/API | E2E |
|------|------|------|------|--------|--------|-----|
| `clarity-contract` | ✅ trait 测试 | — | — | — | — | — |
| `clarity-wire` | ✅ 序列化 | ✅ SPMC | ✅ 跨前端 | — | — | — |
| `clarity-core` | ✅ `Op`/`AgentController` | ✅ 状态机 | ✅ 集成 | — | — | — |
| `clarity-memory` | — | ✅ BM25/向量 | ✅ 端到端检索 | — | — | — |
| `clarity-llm` | ✅ provider 契约 | ✅ failover | ✅ 本地推理链路 | — | — | — |
| `clarity-mcp` | ✅ 命令校验 | ✅ transport | ✅ 真实 MCP server | — | — | — |
| `clarity-tools` | ✅ `Tool` 实现 | ✅ 沙箱 | ✅ 工具链 | — | — | — |
| `clarity-egui` | ✅ 协议渲染 | ✅ widgets | ❌ | ✅ bin | **手动** | 手动 |
| `clarity-tui` | ✅ 协议渲染 | ✅ 弹窗 | ❌ | ✅ bin | **手动** | 手动 |
| `clarity-gateway` | ✅ API/WS 序列化 | ✅ handlers | ✅ 集成 | ✅ bin | **契约待补** | 手动 |
| `clarity-claw` | — | ✅ dialect | ❌ | ✅ bin | **手动** | 手动 |
| `clarity-headless` | — | ✅ CLI args | ✅ 管道输入 | ✅ bin | — | 手动 |
| `clarity-mobile-core` | ✅ FFI 契约 | ✅ UniFFI | ❌ | — | — | 手动 |
| `clarity-channels` | ✅ 消息格式 | ✅ WeChat iLink | ❌ | — | — | 手动 |

> 注：❌ 表示当前缺失或薄弱；**粗体** 表示计划重点补强。

---

## 5. 当前已验证的基线

| 层级 | 命令 | 基线（2026-06-25） |
|------|------|-------------------|
| lib | `cargo test --workspace --lib --exclude clarity-slint` | 1554 / 0 / 0 |
| bins | `cargo test --workspace --bins --exclude clarity-slint -- --test-threads=2` | 275 / 0 / 2 |
| doc | `cargo test --workspace --doc --exclude clarity-slint -- --test-threads=2` | 34 / 0 / 3 |
| integration | `cargo test -p clarity-integration-tests --lib` | 26 / 0 / 0 |
| clippy | `cargo clippy ... -D warnings` | 0 warning |
| fmt | `cargo fmt --all -- --check` | 0 diff |

---

## 6. 缺失项与演进路线

### 高优先级

| 缺失项 | 价值 | 推荐方案 |
|--------|------|---------|
| **egui UI snapshot / kittest** | 防止 Pretext 布局、MessageBubble、右 rail 卡片回归 | 引入 `egui_kittest`，对关键面板做 snapshot；已写入 `ARCHITECTURE_HEALTH.md` 改进方向 |
| **Gateway HTTP/WebSocket API 契约测试** | 保证 Admin UI 和外部客户端不会 broken | 用 `reqwest` + `tokio-tungstenite` 写启动-请求-断言测试 |
| **本地 LLM 推理基准** | 防止模型加载/推理性能退化 | 固定 `.gguf` + 固定 prompt，断言 tok/s 与输出稳定性 |

### 中优先级

| 缺失项 | 价值 | 推荐方案 |
|--------|------|---------|
| Provider failover 混沌测试 | 验证 `ReliableProvider` 在多个 provider 失败时正确切换 | mock 3 个 provider，依次失败 |
| Claw 设备配对流程测试 | 保证发现 → 审批 → 保存 token 链路可用 | mock OpenClaw Gateway WebSocket |
| Session 迁移回归测试 | 防止 V1→V2 迁移破坏历史数据 | 已部分覆盖，可扩展到更多历史样本 |

### 低优先级

| 缺失项 | 价值 | 推荐方案 |
|--------|------|---------|
| 跨平台构建测试 | Windows/Linux/macOS 构建一致性 | CI matrix 已覆盖，本地可周期性验证 |
| 覆盖率门禁 | 量化未覆盖区域 | `cargo llvm-cov` 集成到 CI |

---

## 7. `test_runner.py` 的定位与扩展

### 当前定位

`scripts/test_runner.py` 负责按 **层级** 跑默认矩阵：

```text
lib → bins → doc → integration
```

输出 Markdown/JSON 报告，适合 QA 和 CI 使用。

### 未来扩展方向

| 扩展 | 命令示例 | 复杂度 |
|------|---------|--------|
| 按 crate 筛选 | `python scripts/test_runner.py --crate clarity-memory` | 低 |
| 按目标维度筛选 | `python scripts/test_runner.py --dimension integration` | 低 |
| 自动识别变更 crate | `python scripts/test_runner.py --changed-only` | 中（需 git diff 解析） |
| 覆盖率联动 | `python scripts/test_runner.py --coverage` | 中（依赖 `cargo llvm-cov`） |
| 失败重试 | `python scripts/test_runner.py --retry 2` | 低 |
| 性能基准集成 | `python scripts/test_runner.py --bench pretext` | 中 |

---

## 8. 与 CI 的映射

`.github/workflows/ci.yml` 当前 12-job，本地最简等价验证：

```powershell
python scripts/doctor.py
python scripts/test_runner.py
cargo fmt --all -- --check
```

建议 CI 中将 `test_runner.py` 作为统一的测试报告生成器，替代分散的 `cargo test` job，便于统一输出格式和失败定位。

---

## 9. 相关文件

- [`TESTER_GUIDE.md`](./TESTER_GUIDE.md) — QA 操作手册
- [`AGENTS.md`](../../AGENTS.md) §6 — 测试基线
- [`ARCHITECTURE_HEALTH.md`](../../docs/development/ARCHITECTURE_HEALTH.md) — 架构健康迭代
- [`scripts/test_runner.py`](../../scripts/test_runner.py) — 测试编排脚本
- [`scripts/doctor.py`](../../scripts/doctor.py) — 环境检查脚本
- [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml) — CI 流水线
