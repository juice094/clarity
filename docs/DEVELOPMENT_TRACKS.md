# 开发轨道快速参考

> 并行推进实施指南

---

## 🎯 三条轨道

```
┌─────────────────────────────────────────────────────────────────┐
│ Track A: 记忆系统 (PersistentMemoryStore)                       │
│ 目标: 让 TUI 拥有持久化记忆                                      │
│ 输入: clarity-memory crate                                      │
│ 输出: 替换 placeholder 的真实实现                               │
└─────────────────────────────────────────────────────────────────┘
                            ↕️ 独立
┌─────────────────────────────────────────────────────────────────┐
│ Track B: 后台任务 (BackgroundTaskManager)                       │
│ 目标: 支持后台执行和任务调度                                     │
│ 输入: 无（新建）                                                │
│ 输出: TaskManager trait + Worker 进程                           │
└─────────────────────────────────────────────────────────────────┘
                            ↕️ B → C 依赖
┌─────────────────────────────────────────────────────────────────┐
│ Track C: 并行执行 (SubagentParallel)                            │
│ 目标: 支持多个子代理并行执行                                     │
│ 输入: TaskManager (Track B)                                     │
│ 输出: ParallelExecutor + SubagentBatch API                      │
└─────────────────────────────────────────────────────────────────┘
```

---

## 📅 四周冲刺计划

### Week 1: 基础并行

| 天 | Track A | Track B | Track C |
|----|---------|---------|---------|
| 1 | HybridStore 集成 | Task 定义 | API 设计 |
| 2 | 缓存层 | TaskStore | trait 定义 |
| 3 | 搜索功能 | TaskManager trait | 文档 |
| 4 | 单元测试 | 状态机测试 | Mock 实现 |
| 5 | **交付**: Core Store | **交付**: Core Manager | **交付**: API Spec |

### Week 2: 核心实现

| 天 | Track A | Track B | Track C |
|----|---------|---------|---------|
| 1 | 缓存优化 | Worker 框架 | 等待 B |
| 2 | 错误处理 | 进程通信 | 聚合器设计 |
| 3 | 性能测试 | Wire File | 文档完善 |
| 4 | 边界处理 | Worker 测试 | 准备集成 |
| 5 | **交付**: Store v1 | **交付**: Worker v1 | **交付**: Design Final |

### Week 3: 集成

| 天 | Track A | Track B | Track C |
|----|---------|---------|---------|
| 1 | 替换 placeholder | Manager 完整 | 基于 B 实现 |
| 2 | Agent 集成 | Worker 集成 | 并发控制 |
| 3 | TUI 集成 | 修复问题 | 聚合器实现 |
| 4 | 集成测试 | Manager 测试 | Executor 测试 |
| 5 | **交付**: Integrated | **交付**: Tested | **交付**: Working |

### Week 4: 测试和发布

| 天 | 全员 |
|----|------|
| 1 | 跨轨道集成测试 |
| 2 | E2E 测试（真实 LLM） |
| 3 | 性能优化 |
| 4 | 完整测试套件 |
| 5 | **发布** 🎉 |

---

## 🔗 关键接口（提前约定）

### Track A 输出

```rust
// crates/clarity-core/src/memory/persistent.rs

#[async_trait]
pub trait PersistentMemoryStore: MemoryStore + Send + Sync {
    async fn init(&self) -> Result<()>;
    async fn save(&self, memory: Memory) -> Result<()>;
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Memory>>;
}

// 工厂函数
pub async fn create_persistent_store(
    db_path: impl AsRef<Path>
) -> Result<impl PersistentMemoryStore>;
```

### Track B 输出

```rust
// crates/clarity-core/src/background/mod.rs

pub struct TaskSpec { /* ... */ }
pub struct TaskResult { /* ... */ }
pub type TaskId = String;

#[async_trait]
pub trait TaskManager: Send + Sync {
    async fn create(&self, spec: TaskSpec) -> Result<TaskId>;
    async fn status(&self, id: TaskId) -> Result<TaskStatus>;
    async fn wait(&self, id: TaskId) -> Result<TaskResult>;
    async fn cancel(&self, id: TaskId) -> Result<()>;
}

// 工厂函数
pub async fn create_task_manager(
    store_path: impl AsRef<Path>
) -> Result<impl TaskManager>;
```

### Track C 输入（来自 B）

```rust
// crates/clarity-core/src/subagents/parallel.rs

pub struct ParallelExecutor<T: TaskManager> {
    task_manager: T,
    // ...
}

impl<T: TaskManager> ParallelExecutor<T> {
    pub async fn run_parallel(
        &self,
        specs: Vec<RunSpec>,
    ) -> ParallelResult {
        // 使用 self.task_manager.create(...)
    }
}
```

---

## ⚠️ 依赖和阻塞

### 阻塞关系

```
Track C (并行执行) ──depends──> Track B (TaskManager)
                                     │
Track A (记忆) <────独立────> ───────┘
```

### 缓解策略

| 阻塞 | 缓解措施 |
|------|----------|
| C 等待 B | Week 1 定义 TaskManager trait，C 用 Mock 开发 |
| B 延迟 | C 在 Week 2 做设计和文档，Week 3 开始实现 |
| 接口变更 | 每日站会同步，trait 定义后冻结 3 天 |

---

## 📊 每日站会模板

```markdown
## 日期: 2026-04-XX

### Track A (记忆)
- 昨天: xxx
- 今天: xxx
- 阻塞: xxx

### Track B (后台)
- 昨天: xxx
- 今天: xxx
- 阻塞: xxx
- 接口变更: xxx (通知 Track C)

### Track C (并行)
- 昨天: xxx
- 今天: xxx
- 阻塞: xxx
- 需要 B 的: xxx
```

---

## ✅ 检查点

### Week 1 结束检查

- [ ] Track A: `cargo test memory::persistent` 通过
- [ ] Track B: `cargo test background` 通过
- [ ] Track C: API 文档完成，Mock 可运行

### Week 2 结束检查

- [ ] Track A: 性能测试通过 (< 100ms 查询)
- [ ] Track B: Worker 进程可启动和通信
- [ ] Track C: 等待 Week 3

### Week 3 结束检查

- [ ] Track A: TUI 集成完成，记忆可持久化
- [ ] Track B: TaskManager 完整可用
- [ ] Track C: ParallelExecutor 基于 TaskManager 运行

### Week 4 结束检查

- [ ] 集成测试: 3 个 track 一起工作
- [ ] E2E 测试: 真实 LLM 场景通过
- [ ] 文档: 每个 track 有使用文档

---

## 🚀 快速开始

### Track A 开发者

```bash
# 1. 创建分支
git checkout -b feature/persistent-memory

# 2. 创建文件
touch crates/clarity-core/src/memory/persistent.rs

# 3. 实现 HybridStore 集成
# 4. 运行测试
cargo test -p clarity-core memory::persistent
```

### Track B 开发者

```bash
# 1. 创建分支
git checkout -b feature/background-tasks

# 2. 创建模块
mkdir -p crates/clarity-core/src/background
touch crates/clarity-core/src/background/mod.rs
touch crates/clarity-core/src/background/worker.rs

# 3. 定义 TaskManager trait
# 4. 运行测试
cargo test -p clarity-core background
```

### Track C 开发者

```bash
# 1. 创建分支
git checkout -b feature/subagent-parallel

# 2. 等待 Track B 的 TaskManager trait
# 3. 先设计 API 和文档
# 4. 用 Mock 实现并行逻辑
```

---

*生成时间：2026-04-04*
*下次同步：每日 10:00*
