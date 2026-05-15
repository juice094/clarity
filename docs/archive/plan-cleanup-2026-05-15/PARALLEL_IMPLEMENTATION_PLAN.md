# 并行推进实施计划

> 规划日期：2026-04-04
> 目标：并行推进 PersistentMemoryStore、BackgroundTaskManager 和子代理并行执行

---

## 🎯 并行推进架构

```
Week 1-2 (并行阶段 1)
├── Track A: PersistentMemoryStore (内存系统)
│   └── 负责人: Developer A
│   └── 产出: 可持久化的记忆存储
│
├── Track B: BackgroundTaskManager Core (任务核心)
│   └── 负责人: Developer B  
│   └── 产出: 任务定义、存储、序列化
│
└── Track C: Subagent Parallel API (并行接口)
    └── 负责人: Developer C
    └── 产出: 并行执行 API 设计

Week 3-4 (并行阶段 2)
├── Track A: Memory-Integration (记忆集成)
│   └── 集成到 Agent
│   └── TUI 集成测试
│
├── Track B: Background Worker (后台 Worker)
│   └── Worker 进程实现
│   └── 进程间通信
│
└── Track C: Parallel Executor (并行执行器)
    └── 基于 TaskManager 实现并行
    └── 结果聚合器

Week 5 (集成测试)
├── 端到端集成测试
├── 性能基准测试
└── 文档更新
```

---

## 📋 模块依赖分析

### 依赖矩阵

| 模块 | 依赖 | 被依赖 | 可以并行？ |
|------|------|--------|-----------|
| PersistentMemoryStore | clarity-memory | Agent, TUI | ✅ 独立 |
| BackgroundTaskManager Core | 无 | Worker, Parallel | ✅ 独立 |
| Task Worker | TaskManager Core | Parallel | ⚠️ 依赖 Track B |
| Subagent Parallel API | Runner | Parallel Executor | ✅ 接口先行 |
| Parallel Executor | TaskManager | 无 | ⚠️ 依赖 Track B |

### 接口契约（提前定义）

```rust
// ==================== Track A 输出接口 ====================

/// PersistentMemoryStore 接口
#[async_trait]
pub trait PersistentMemoryStore: MemoryStore {
    /// 初始化存储
    async fn init(&self) -> Result<()>;
    
    /// 保存记忆（自动持久化）
    async fn save(&self, memory: Memory) -> Result<()>;
    
    /// 搜索记忆
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Memory>>;
}

// ==================== Track B 输出接口 ====================

/// 任务定义
#[derive(Serialize, Deserialize)]
pub struct BackgroundTask {
    pub id: TaskId,
    pub spec: TaskSpec,
    pub status: TaskStatus,
    pub created_at: u64,
    pub updated_at: u64,
}

/// TaskManager 接口
#[async_trait]
pub trait TaskManager {
    /// 创建任务
    async fn create(&self, spec: TaskSpec) -> Result<TaskId>;
    
    /// 获取任务状态
    async fn status(&self, id: TaskId) -> Result<TaskStatus>;
    
    /// 取消任务
    async fn cancel(&self, id: TaskId) -> Result<()>;
    
    /// 等待任务完成
    async fn wait(&self, id: TaskId) -> Result<TaskResult>;
}

// ==================== Track C 依赖接口 ====================

/// 并行执行需要 TaskManager
pub struct ParallelExecutor<T: TaskManager> {
    task_manager: T,
    max_concurrency: usize,
}
```

---

## 🚀 Track A: PersistentMemoryStore

### 阶段 1: 核心实现 (Week 1)

```rust
// crates/clarity-core/src/memory/persistent.rs

use clarity_memory::{HybridStore, FileBackend, SqliteBackend};

pub struct PersistentMemoryStoreImpl {
    inner: HybridStore,
    cache: Arc<RwLock<HashMap<String, Memory>>>,
}

#[async_trait]
impl PersistentMemoryStore for PersistentMemoryStoreImpl {
    async fn init(&self) -> Result<()> {
        // 初始化目录和数据库
    }
    
    async fn save(&self, memory: Memory) -> Result<()> {
        // 保存到缓存 + 持久化
    }
    
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Memory>> {
        // 搜索：内存缓存 + 数据库
    }
}
```

**里程碑**:
- [ ] Day 1: HybridStore 集成
- [ ] Day 2: 缓存层实现
- [ ] Day 3: 搜索功能
- [ ] Day 4: 单元测试 (10+ 测试)
- [ ] Day 5: 代码审查

### 阶段 2: 集成 (Week 3)

```rust
// crates/clarity-core/src/memory/mod.rs

// 替换 placeholder 实现
pub use persistent::PersistentMemoryStoreImpl as PersistentMemoryStore;

// Agent 集成
impl Agent {
    pub fn with_persistent_memory(self, store: PersistentMemoryStore) -> Self {
        // 设置真实存储
    }
}
```

**里程碑**:
- [ ] Day 1: 替换 placeholder
- [ ] Day 2: Agent 集成
- [ ] Day 3: TUI 集成
- [ ] Day 4-5: 集成测试

---

## 🚀 Track B: BackgroundTaskManager

### 阶段 1: 核心定义 (Week 1)

```rust
// crates/clarity-core/src/background/mod.rs

/// 任务定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    pub name: String,
    pub description: String,
    pub agent_type: String,
    pub prompt: String,
    pub max_iterations: Option<usize>,
    pub timeout_secs: Option<u64>,
}

/// 任务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// 任务结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub status: TaskStatus,
    pub output: String,
    pub elapsed_ms: u64,
    pub steps: usize,
}

/// 任务管理器 trait（提前定义，供 Track C 使用）
#[async_trait]
pub trait TaskManager: Send + Sync {
    async fn create(&self, spec: TaskSpec) -> Result<TaskId>;
    async fn status(&self, id: TaskId) -> Result<TaskStatus>;
    async fn cancel(&self, id: TaskId) -> Result<()>;
    async fn wait(&self, id: TaskId) -> Result<TaskResult>;
    async fn list(&self) -> Result<Vec<TaskInfo>>;
}
```

**里程碑**:
- [ ] Day 1: Task 定义和序列化
- [ ] Day 2: TaskStore 实现（文件存储）
- [ ] Day 3: TaskManager trait
- [ ] Day 4: 单元测试（状态机测试）
- [ ] Day 5: 代码审查

### 阶段 2: Worker 实现 (Week 2-3)

```rust
// crates/clarity-core/src/background/worker.rs

/// 后台 Worker
pub struct BackgroundWorker {
    task_id: TaskId,
    runtime: WorkerRuntime,
}

impl BackgroundWorker {
    /// 启动 Worker 进程
    pub async fn spawn(task_id: TaskId) -> Result<WorkerHandle> {
        // 使用 tokio::process 启动子进程
    }
    
    /// 执行任务
    pub async fn run(&mut self) -> TaskResult {
        // 加载任务
        // 创建 Agent
        // 执行子代理
        // 保存结果
    }
}
```

**里程碑**:
- [ ] Week 2 Day 1-2: Worker 进程框架
- [ ] Week 2 Day 3-4: 进程间通信（Wire File）
- [ ] Week 2 Day 5: Worker 测试
- [ ] Week 3 Day 1-2: TaskManager 完整实现
- [ ] Week 3 Day 3-5: 集成测试

---

## 🚀 Track C: Subagent Parallel Execution

### 阶段 1: API 设计 (Week 1)

```rust
// crates/clarity-core/src/subagents/parallel.rs

/// 并行配置
#[derive(Debug, Clone)]
pub struct ParallelConfig {
    pub max_concurrency: usize,
    pub timeout_secs: Option<u64>,
    pub cancel_on_error: bool,
}

/// 并行执行结果
#[derive(Debug, Clone)]
pub struct ParallelResult {
    pub results: Vec<SubagentResult>,
    pub failures: Vec<(String, SubagentError)>,
    pub total_elapsed_ms: u64,
}

/// 并行执行 trait（基于 TaskManager）
#[async_trait]
pub trait ParallelExecutor {
    async fn run_parallel(
        &self,
        specs: Vec<RunSpec>,
        config: ParallelConfig,
    ) -> ParallelResult;
}
```

**里程碑**:
- [ ] Day 1: API 设计文档
- [ ] Day 2: trait 定义
- [ ] Day 3: 结果聚合器设计
- [ ] Day 4: 与 TaskManager 接口对接
- [ ] Day 5: 代码审查

### 阶段 2: 实现 (Week 3-4)

```rust
// crates/clarity-core/src/subagents/parallel.rs

use crate::background::TaskManager;

pub struct SubagentParallelExecutor<T: TaskManager> {
    task_manager: T,
    runner: SubagentRunner,
}

impl<T: TaskManager> ParallelExecutor for SubagentParallelExecutor<T> {
    async fn run_parallel(
        &self,
        specs: Vec<RunSpec>,
        config: ParallelConfig,
    ) -> ParallelResult {
        // 使用 TaskManager 创建后台任务
        // 并发控制通过 TaskManager 实现
        // 收集结果
    }
}

/// 高级 API
pub struct SubagentBatch {
    specs: Vec<RunSpec>,
    config: ParallelConfig,
}

impl SubagentBatch {
    pub async fn execute(self) -> ParallelResult {
        // 批量执行
    }
}
```

**里程碑**:
- [ ] Week 3 Day 1-3: 基于 TaskManager 的实现
- [ ] Week 3 Day 4-5: 结果聚合器
- [ ] Week 4 Day 1-3: SubagentBatch API
- [ ] Week 4 Day 4-5: 集成测试

---

## 📅 详细时间线

### Week 1: 并行开发启动

```
周一
├── Track A: HybridStore 调研和集成
├── Track B: Task 定义和序列化设计
└── Track C: 并行 API 接口设计

周二
├── Track A: 缓存层实现
├── Track B: TaskStore 文件存储实现
└── Track C: trait 定义和文档

周三
├── Track A: 搜索功能实现
├── Track B: TaskManager trait 定义
└── Track C: 与 Track B 接口对接

周四
├── Track A: 单元测试
├── Track B: 状态机单元测试
└── Track C: Mock 实现和测试

周五
├── Track A: 代码审查
├── Track B: 代码审查
└── Track C: 代码审查
```

### Week 2: 核心实现

```
周一
├── Track A: 缓存优化
├── Track B: Worker 进程框架
└── Track C: 等待 TaskManager 实现

周二
├── Track A: 错误处理优化
├── Track B: 进程间通信设计
└── Track C: 结果聚合器设计

周三
├── Track A: 性能测试
├── Track B: Wire File 通信实现
└── Track C: 文档完善

周四
├── Track A: 边界情况处理
├── Track B: Worker 测试
└── Track C: 准备集成

周五
├── Track A: Week 2 交付
├── Track B: Worker 基础完成
└── Track C: 等待 Week 3
```

### Week 3: 集成阶段

```
周一
├── Track A: 替换 placeholder 实现
├── Track B: TaskManager 完整实现
└── Track C: 基于 TaskManager 的实现

周二
├── Track A: Agent 集成
├── Track B: Worker 集成测试
└── Track C: 并发控制实现

周三
├── Track A: TUI 集成
├── Track B: 修复 Worker 问题
└── Track C: 结果聚合器实现

周四
├── Track A: 集成测试
├── Track B: TaskManager 测试
└── Track C: ParallelExecutor 测试

周五
├── Track A: 文档和示例
├── Track B: 文档和示例
└── Track C: SubagentBatch API
```

### Week 4: 完善和测试

```
周一
├── 跨 Track 集成测试
├── 修复集成问题
└── 性能基准测试

周二
├── E2E 测试（真实 LLM）
├── 问题修复
└── 文档更新

周三
├── 性能优化
├── 边界情况处理
└── 代码审查

周四
├── 完整测试套件
├── CI/CD 集成
└── 发布准备

周五
├── 最终审查
├── 文档发布
└── 庆祝 🎉
```

---

## 🔄 同步机制

### 每日站会（15 分钟）

```
时间: 每天上午 10:00
参与: Track A, B, C 负责人
内容:
  1. 昨天完成什么？
  2. 今天计划做什么？
  3. 有什么阻塞？
```

### 接口变更通知

```
当 Track B 的接口变更时：
1. 立即通知 Track C
2. 更新接口文档
3. 提供迁移指南
4. 保持向后兼容（如果可能）
```

### 代码审查轮值

```
周一: A 审 B, B 审 C, C 审 A
周二: B 审 A, C 审 B, A 审 C
周三: ...
```

---

## 🛡️ 风险缓解

### 风险 1: Track B 延迟影响 Track C

**缓解措施**:
- Week 1 提前定义 TaskManager trait
- Track C 使用 Mock 实现进行开发
- Week 2 结束前必须完成 TaskManager 基础

### 风险 2: 接口不匹配

**缓解措施**:
- 每日站会同步接口变更
- 使用 trait 定义作为契约
- 提供接口适配器模式

### 风险 3: 资源冲突（测试/环境）

**缓解措施**:
- 使用独立测试数据库
- Mock 外部依赖（LLM）
- 本地开发环境隔离

---

## 📊 成功指标

| 指标 | 目标 | 验证方式 |
|------|------|----------|
| 代码覆盖率 | >80% | `cargo tarpaulin` |
| 测试通过 | 100% | `cargo test` |
| 文档完整 | 100% | 每个 public API 有文档 |
| 性能基准 | 定义基准 | 对比单线程/并行 |
| 集成测试 | 通过 | E2E 测试套件 |

---

## 📝 交付清单

### Track A 交付物
- [ ] `crates/clarity-core/src/memory/persistent.rs`
- [ ] 单元测试（15+ 测试）
- [ ] 集成测试
- [ ] 使用文档

### Track B 交付物
- [ ] `crates/clarity-core/src/background/mod.rs`
- [ ] `crates/clarity-core/src/background/worker.rs`
- [ ] 单元测试（20+ 测试）
- [ ] Worker 进程实现
- [ ] 使用文档

### Track C 交付物
- [ ] `crates/clarity-core/src/subagents/parallel.rs`
- [ ] 单元测试（10+ 测试）
- [ ] 并行执行示例
- [ ] 使用文档

### 集成交付物
- [ ] 端到端测试套件
- [ ] 性能基准报告
- [ ] 更新后的 README
- [ ] 更新后的 PROJECT_REPORT

---

*规划生成时间：2026-04-04*
*预计完成时间：4 周后（2026-05-02）*
