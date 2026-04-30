# Agent OS 愿景备忘录

> 性质：用户原始需求 + 架构映射分析。非已执行决策，非 Roadmap 承诺。
> 用途：在上下文压缩后，新会话可快速恢复对"多窗口 Agent 操作系统"需求的完整认知。
> 创建日期：2026-04-30
> 关联项目：clarity

---

## 一、用户原始需求（8 条）

1. **持久化高人格窗口** — 用于 Claw 存储高人格化的生活情感（当前为 KimiClaw 云服务器）
2. **记忆空间窗口** — 用于管理知识库与项目认知，作为观察者或分析者（当前为 KimiWeb）
3. **专项项目经理窗口** — 若干个能对专项项目进行长线开发的专项项目经理（当前为 KimiCLI 多窗口会话）
4. **窗口间不等价权限 + 层级认知** — 下层能认知到高层存在，但权限不对等
5. **窗口发展下线子代理** — 每个窗口有 spawn 子 Agent 进行代工的能力
6. **不同安全组不同工具** — 安全组隔离，工具可用性按组分配
7. **指定会话子代理的服务商模型** — 每个子 Agent 可独立指定 provider / model
8. **上层→下层信息注入 + 平层公告板** — 层级信息流动 + 同层共享信息交流

---

## 二、需求 → Clarity 现状映射

| 需求 | 对应模块 | 现状 | 差距 |
|------|---------|------|------|
| 1. 持久化高人格窗口 | `clarity-egui` + `clarity-memory` | 单窗口，session 内存切换，进程退出丢失 | **大**：需独立窗口级 Agent 实例 + 磁盘持久化 |
| 2. 记忆空间窗口（观察者） | `clarity-memory` + `devbase` MCP | 存储层已有（SQLite + BM25 + CosineIndex） | **中**：缺独立观察者人格绑定和专用 UI |
| 3. 专项项目经理 | `subagents` + `background` + `plan` | 后端能力齐全（SubAgent spawn、Background Task、Plan 工具） | **中**：缺独立窗口和长线任务持久化 |
| 4. 不等价权限 + 层级认知 | `subagents::token` | 有沙箱边界（`verify_sandbox_escape`） | **中**：缺显式 parent/child 层级权限协议 |
| 5. 发展下线子代理 | `subagents::builder` + `parallel` | `build_subagent()`、`SubAgentBatch` 已存在 | **小**：缺"窗口内一键 spawn"的 UI 入口 |
| 6. 不同安全组不同工具 | `capability` + `registry` | `CapabilityRegistry`、`Token::verify_allowed_tool` 已存在 | **小**：缺安全组到窗口的动态绑定 |
| 7. 指定子代理服务商模型 | `llm::model_registry` | `build_provider_from_registry_with_key()` Sprint 9 已解锁 | **小**：后端已有，需暴露到 spawn 配置 |
| 8. 上层注入 + 平层公告板 | `clarity-wire` | `Wire` 单 session 内广播，不跨 session | **大**：缺跨窗口 IPC / 共享状态总线 |

---

## 三、核心缺失：三个 Architectural Holes

### Hole 1 — 进程模型：从「单窗口多标签」到「多窗口多进程」

当前 `clarity-egui` 用 `eframe::run_native` 跑一个 `App` 实例，所有 session 是 `HashMap<String, Session>` 内部切换。

目标架构需要：
- 每个窗口是独立 OS 进程（或至少独立 `eframe` 实例），有自己的 `Agent` + `AppState`
- 一个轻量**窗口管理器**负责 spawn / 监控 / 回收窗口
- 窗口崩溃不影响其他窗口

### Hole 2 — 跨窗口通信：Wire 的边界突破

`clarity-wire` 目前只在一个 `Agent` ↔ `egui App` 之间流转。需要：
- **层级注入总线**：parent Agent 向下广播 `SystemPrompt` 片段、记忆摘要、高层目标
- **平层公告板**：同层窗口的只读共享空间（类似 MQTT topic）
- **不等价查询**：child 可读 parent 公开状态；parent 可读写 child 全部状态

### Hole 3 — 人格持久化：从 Session 到 Soul

当前 session 切换 = 人格切换（通过 `SystemPrompt` 重建）。"格雷常驻窗口"需要：
- 独立的持久化状态文件（非 `gui-settings.json`）
- 启动时自动恢复上一个 soul 状态
- 与 `clarity-memory` 编译器深度绑定，不同 soul 的记忆编译隔离

---

## 四、架构路径对比

| 路径 | 模型 | 优点 | 缺点 | 适合阶段 |
|------|------|------|------|---------|
| **A. 单进程多窗口** | 同一进程多个 `eframe` 窗口，共享 core | 快速验证，不改进程模型 | 一崩全崩；窗口数受限 | 概念验证 |
| **B. 主进程 + 渲染进程** | `clarity-gateway` 为主进程，窗口为 thin client | 崩溃隔离；跨窗口通信天然 | 需重写 egui 为 client；延迟增加 | 中期演进 |
| **C. 微内核 + 独立进程** | 每个窗口独立 OS 进程，`clarity-hub` 仲裁 | 真正隔离；可混用技术栈 | 等于重写半个操作系统 | 长期愿景 |

---

## 五、轻量起步想象（路径 A 最小可行结构）

```
clarity-hub (主线程)
  ├── Window 0: 「格雷」常驻
  │     Agent: soul="格雷", tools=[], model=local-gguf
  │     持久化: ~/.clarity/souls/grey.state
  │
  ├── Window 1: 「观察者」知识库
  │     Agent: soul="observer", tools=[file_read, web_search, memory_query]
  │     持久化: ~/.clarity/souls/observer.state
  │
  ├── Window 2: 「专项 A」项目经理
  │     Agent: soul="pm-rust", tools=[file_write, shell, git, subagent_spawn]
  │     子代理: Window 2 内 spawn SubAgent，用 deepseek 模型
  │     持久化: ~/.clarity/souls/pm-rust.state
  │
  └── SharedBoard: 平层公告板（内存 HashMap + 定时落盘）
```

**上层→下层注入**：`clarity-hub` 在每个 child `Agent::run()` 前，自动把 parent 的 `executive_summary` 注入到 `SystemPrompt` 的 `context` 组件。

**平层公告板**：`SharedBoard` 暴露为 `tools::shared_board_write` / `tools::shared_board_read`，所有 Agent 可用，带 TTL 和权限过滤。

---

## 六、能力孤岛声明

> ⚠️ 本文件基于会话当时的上下文快照生成。Clarity 项目存在大量能力孤岛（子系统已实现但未被主流程激活、模块间接口未打通、文档与代码不同步）。
>
> 任何基于本文档的架构决策，必须在执行前重新验证对应模块的**实际代码状态**，不可仅凭本文档描述直接推进。
>
> 建议验证路径：
> 1. `cargo test --workspace --lib` 确认基线
> 2. 直接阅读目标 crate 的 `src/lib.rs` 和最新测试
> 3. 检查 `docs/ai-protocol.md` 获取最新架构决策

---

*本文件由 AI 会话生成，人类开发者可直接编辑。重大架构变更需同步更新映射表。*
