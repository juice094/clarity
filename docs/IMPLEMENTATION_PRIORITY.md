# Clarity 实现优先级报告（对比 Kimi CLI）

> 更新日期：2026-04-20
> 状态：已重新评估

---

## 📊 当前状态概览

| 模块 | 状态 | 测试覆盖 | 备注 |
|------|------|----------|------|
| SubagentRunner | ✅ **已完成** | 18 测试通过 | 今日实现 |
| BackgroundTaskManager | ⚠️ 骨架完整 | WorkerPool + TaskStore 已实现 | 高优先级（缺自动调度循环 + TUI/Gateway 集成） |
| PersistentMemoryStore | ✅ 已实现 | `clarity-memory` 集成，TUI 在用 | 高优先级（已完成） |
| MCP 完整支持 | ⚠️ 骨架实现 | - | 中高优先级 |
| 工具系统 | ✅ 8 个工具 | 完整 | 基础完成 |
| 审批系统 | ✅ 三种模式 | 完整 | 生产就绪 |
| Wire 通信 | ✅ clarity-wire | 8 测试 | 生产就绪 |
| Web UI | ❌ 画饼 | - | 低优先级 |

---

## 🎯 重新评估的实现优先级

### P0 - 立即实现（本周）

| 排名 | 功能 | 难度 | 价值 | 理由 |
|------|------|------|------|------|
| **1** | **PersistentMemoryStore 真实实现** | 中 | ⭐⭐⭐⭐⭐ | **当前最大瓶颈**。TUI 记忆不持久化，严重影响用户体验。需集成 clarity-memory。
| **2** | **BackgroundTaskManager** | 高 | ⭐⭐⭐⭐⭐ | 支持后台执行和并行子代理。参考 Kimi CLI `background/manager.py`。

### P1 - 短期实现（本月）

| 排名 | 功能 | 难度 | 价值 | 理由 |
|------|------|------|------|------|
| **3** | **MCP 完整支持** | 中 | ⭐⭐⭐⭐ | 扩展工具生态。Kimi CLI 已验证 MCP 价值，需支持 HTTP/SSE transport。
| **4** | **子代理并行执行** | 中 | ⭐⭐⭐⭐ | 批量任务场景必需。参考 `docs/SUBAGENT_PARALLEL_ANALYSIS.md`。 |
| **5** | **E2E 测试** | 中 | ⭐⭐⭐⭐ | 当前只有单元测试，需要真实 LLM 联调测试。 |

### P2 - 中期实现（2-3 月）

| 排名 | 功能 | 难度 | 价值 | 理由 |
|------|------|------|------|------|
| **6** | **Web UI** | 高 | ⭐⭐⭐ | 基于 Wire 的 WebSocket 通信。可延后，TUI 已足够验证核心功能。 |
| **7** | **更多工具** | 低 | ⭐⭐⭐ | 增加 todo, plan 等工具。参考 Kimi CLI 工具列表。 |
| **8** | **Gateway 渠道** | 中 | ⭐⭐⭐ | Discord/Telegram/Webhook 实测。 |

### P3 - 长期/暂缓

| 排名 | 功能 | 难度 | 价值 | 理由 |
|------|------|------|------|------|
| **9** | **VS Code Extension** | 高 | ⭐⭐ | 需要 ACP 协议支持，非核心需求。 |
| **10** | **WASM 插件** | 高 | ⭐ | 风险过高，生态不成熟。优先用 MCP 替代。 |

---

## 🔍 详细分析

### P0-1: PersistentMemoryStore 真实实现

**状态：✅ 已完成（2026-04-15）**

```
实现结果：
├── `clarity-core/src/memory/mod.rs` 提供真实 PersistentMemoryStore
├── TUI (`clarity-tui/src/main.rs`) 启动时自动加载 `.clarity/memory.db`
├── `clarity-memory` HybridStore（热缓存 + FileStore 冷存储）已集成
├── `tests/integration/tests/memory_persistence.rs` 验证持久化
└── 向后兼容：Agent::with_memory() 接口不变

遗留：文档标记未及时更新，本次同步修正。
```

**参考代码**：
- Kimi CLI: `kimi_cli/soul/context.py`
- Clarity: `clarity-memory/src/backends/hybrid.rs`

### P0-2: BackgroundTaskManager

**为什么现在需要？**

```
场景需求：
├── 子代理并行执行需要任务调度
├── 长时间任务需要后台运行
├── 需要任务持久化和恢复
└── 与前台 Agent 协调

实现要点：
├── TaskStore: 任务序列化存储
├── Worker 进程: 隔离执行环境
├── 状态机: pending → running → completed/failed
├── 通知机制: 任务完成通知
└── 生命周期管理: 取消、超时、重试

预期工作量：1-2 周
```

**参考代码**：
- Kimi CLI: `src/kimi_cli/background/manager.py`
- Kimi CLI: `src/kimi_cli/background/worker.py`

---

## 📈 优先级变化说明

### 已完成的项

| 功能 | 原计划 | 实际 | 说明 |
|------|--------|------|------|
| SubagentRunner | P0 (待实现) | ✅ 已完成 | 今日实现，18 测试通过 |
| Git 上下文 | P1 | ✅ 已实现 | 集成到 SubagentRunner |

### 优先级调整的项

| 功能 | 原优先级 | 新优先级 | 调整原因 |
|------|----------|----------|----------|
| PersistentMemoryStore | P1 | **P0** | 发现是严重瓶颈 |
| BackgroundTaskManager | P0 | **P0** | 与子代理系统配套 |
| MCP 完整支持 | P0 | **P1** | SubagentRunner 已提供替代方案 |
| Web UI | P4 | **P2** | 重要性降低，TUI 足够 |

---

## 🛠️ 具体行动计划

### 本周（P0）

```bash
# Day 1-2: PersistentMemoryStore
- [ ] 创建 crates/clarity-core/src/memory/persistent.rs
- [ ] 集成 HybridStore
- [ ] 替换 placeholder 实现
- [ ] 添加单元测试

# Day 3-7: BackgroundTaskManager  
- [ ] 创建 crates/clarity-core/src/background/mod.rs
- [ ] 实现 TaskStore
- [ ] 实现 Worker 进程管理
- [ ] 集成到 Agent
```

### 本月（P1）

```bash
# Week 2: MCP 完整支持
- [ ] 实现 HTTP transport
- [ ] 实现 SSE transport  
- [ ] 工具动态加载
- [ ] 端到端测试

# Week 3: 子代理并行执行
- [ ] 实现 parallel.rs
- [ ] Semaphore 并发控制
- [ ] 结果聚合器
- [ ] 集成测试

# Week 4: E2E 测试
- [ ] 配置真实 LLM 环境
- [ ] TUI 端到端测试
- [ ] 子代理端到端测试
- [ ] 修复发现的问题
```

---

## 💡 决策建议

### 建议 1: 优先修复记忆系统

**理由**：
- 当前 TUI 每次对话都是"失忆"状态
- 用户无法接受没有记忆的 AI 助手
- 技术债务，越早修复成本越低

**风险**：
- 需要修改 Agent 初始化流程
- 可能影响现有测试

### 建议 2: 后台任务与子代理并行一起实现

**理由**：
- 两者共享 Worker 进程基础设施
- BackgroundTaskManager 为并行执行提供调度能力
- Kimi CLI 中这两个功能紧密耦合

### 建议 3: MCP 可以稍后

**理由**：
- SubagentRunner 已提供工具扩展能力
- 8 个内置工具已覆盖 80% 场景
- MCP 需要更多测试和生态成熟

---

## 📋 关键指标追踪

| 指标 | 当前 | 目标（4 周后） | 目标（3 月后） |
|------|------|----------------|----------------|
| 测试数 | 180+ | 220+ | 300+ |
| 功能完成度 | 60% | 75% | 90% |
| 文档准确率 | 85% | 95% | 100% |
| 生产就绪度 | 🟡 原型 | 🟢 可用 | 🟢 稳定 |

---

## 🎯 结论

**最重要的三件事**：

1. **修复 PersistentMemoryStore**（本周）
   - 这是当前最大的可用性问题
   - 工作量可控（2-3 天）
   - 用户价值极高

2. **实现 BackgroundTaskManager**（本周至下周）
   - 支持后台任务和并行执行
   - 与子代理系统配套
   - 参考 Kimi CLI 成熟实现

3. **端到端实测**（本月）
   - 配置真实 LLM 环境
   - 验证完整用户流程
   - 发现并修复实际问题

**暂缓的事项**：
- Web UI（TUI 足够）
- VS Code Extension（非核心）
- WASM 插件（风险高）

---

*报告生成时间：2026-04-04*  
*状态：已重新评估并更新*
