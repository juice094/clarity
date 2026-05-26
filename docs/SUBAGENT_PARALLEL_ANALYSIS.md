---
title: 子代理并行执行分析
category: Document
date: 2026-05-16
tags: [document, agent]
---

# 子代理并行执行分析

> 分析日期：2026-04-04
> 分析目标：评估子代理并行执行的可行性、场景和实现方案

---

## 1. 当前状态

### 1.1 现有实现（顺序执行）

```rust
// SubagentRunner::run() - 当前为顺序执行
pub async fn run(&self, spec: RunSpec, ...) -> Result<SubagentResult, SubagentError> {
    // 1. 准备实例
    // 2. 构建 Agent
    // 3. 执行代理循环
    // 4. 返回结果
}
```

**特点**：
- 单任务执行
- 阻塞等待结果
- 适合需要严格顺序的复杂任务

### 1.2 已有并行基础设施

```rust
// crates/clarity-core/src/agent/enhanced.rs
pub struct ParallelToolExecutor;

impl ParallelToolExecutor {
    pub async fn execute_parallel<F, Fut, T, E>(
        tool_calls: Vec<ToolCall>,
        executor: F,
    ) -> Vec<(String, Result<T, E>)>
    where
        F: Fn(&ToolCall) -> Fut + Clone + Send + Sync,
        Fut: Future<Output = Result<T, E>> + Send,
    {
        futures::future::join_all(futures).await
    }
}
```

---

## 2. 并行执行场景分析

### 2.1 适用场景 ✅

| 场景 | 描述 | 示例 |
|------|------|------|
| **批量代码审查** | 同时审查多个文件 | 5 个 coder 代理分别审查不同模块 |
| **多源信息收集** | 并行搜索不同来源 | 3 个 explore 代理分别搜索文档、源码、Issues |
| **方案对比** | 同时生成多种实现方案 | 2 个 plan 代理生成不同架构方案 |
| **独立子任务** | 无依赖的子任务 | 重构 + 测试 + 文档 并行执行 |

### 2.2 不适用场景 ❌

| 场景 | 原因 | 解决方案 |
|------|------|----------|
| **有依赖的步骤** | 步骤 B 需要步骤 A 的结果 | 保持顺序执行 |
| **资源竞争** | 同时修改同一文件 | 文件锁或顺序化 |
| **Token 爆炸** | 多个代理同时消耗大量 Token | 限制并发数 |
| **上下文冲突** | 需要共享可变状态 | 独立上下文或同步机制 |

---

## 3. 实现方案

### 3.1 方案 A: 简单并行 (join_all)

```rust
use futures::future::join_all;

/// 并行执行多个子代理
pub async fn run_parallel(
    &self,
    specs: Vec<RunSpec>,
    store: &mut SubagentStore,
    max_concurrency: usize,
) -> Vec<Result<SubagentResult, SubagentError>> {
    // 限制并发数
    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrency));
    
    let futures: Vec<_> = specs
        .into_iter()
        .map(|spec| {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            async move {
                let result = self.run(spec, store, None).await;
                drop(permit);
                result
            }
        })
        .collect();
    
    join_all(futures).await
}
```

**优点**：
- 实现简单
- 资源控制（通过 Semaphore）

**缺点**：
- 无法单独取消某个任务
- 结果一次性返回

### 3.2 方案 B: 流式并行 (FuturesUnordered)

```rust
use futures::stream::FuturesUnordered;
use futures::StreamExt;

/// 流式并行执行，支持中间结果获取
pub async fn run_parallel_streaming(
    &self,
    specs: Vec<RunSpec>,
    store: &mut SubagentStore,
    max_concurrency: usize,
) -> mpsc::Receiver<Result<SubagentResult, SubagentError>> {
    let (tx, rx) = mpsc::channel(max_concurrency);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrency));
    
    tokio::spawn(async move {
        let mut futures = FuturesUnordered::new();
        
        for spec in specs {
            let sem = semaphore.clone();
            let tx = tx.clone();
            
            futures.push(async move {
                let _permit = sem.acquire().await.unwrap();
                let result = self.run(spec, store, None).await;
                let _ = tx.send(result).await;
            });
        }
        
        while let Some(_) = futures.next().await {}
    });
    
    rx
}
```

**优点**：
- 实时获取结果
- 支持动态添加任务

**缺点**：
- 实现复杂
- 需要处理背压

### 3.3 方案 C: 结构化并发 (tokio::spawn + JoinSet)

```rust
use tokio::task::JoinSet;

/// 结构化并发执行
pub async fn run_parallel_structured(
    &self,
    specs: Vec<RunSpec>,
    store: Arc<tokio::sync::RwLock<SubagentStore>>,
    max_concurrency: usize,
) -> ParallelExecutionResult {
    let mut join_set = JoinSet::new();
    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrency));
    
    for spec in specs {
        let sem = semaphore.clone();
        let store = store.clone();
        
        join_set.spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            // 注意：需要处理 store 的并发访问
            let mut store_guard = store.write().await;
            self.run(spec, &mut *store_guard, None).await
        });
    }
    
    let mut results = Vec::new();
    while let Some(result) = join_set.join_next().await {
        results.push(result.unwrap());
    }
    
    ParallelExecutionResult { results }
}
```

**优点**：
- 更好的错误处理
- 支持任务取消
- 自动清理

**缺点**：
- 需要 Arc<RwLock<>> 包装 store
- 代码复杂度较高

---

## 4. 推荐实现

### 4.1 分层设计

```rust
// ==================== 核心并行执行器 ====================

/// 并行执行配置
#[derive(Debug, Clone)]
pub struct ParallelConfig {
    /// 最大并发数
    pub max_concurrency: usize,
    /// 是否启用流式结果
    pub streaming: bool,
    /// 超时时间（秒）
    pub timeout_secs: Option<u64>,
    /// 是否取消所有任务当其中一个失败
    pub cancel_on_error: bool,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 3,
            streaming: false,
            timeout_secs: Some(300),
            cancel_on_error: false,
        }
    }
}

/// 并行执行结果
#[derive(Debug, Clone)]
pub struct ParallelResult {
    /// 所有子代理结果
    pub results: Vec<SubagentResult>,
    /// 失败的执行
    pub failures: Vec<(String, SubagentError)>,
    /// 总耗时
    pub total_elapsed_ms: u64,
    /// 实际并发数
    pub actual_concurrency: usize,
}

// ==================== SubagentRunner 扩展 ====================

impl SubagentRunner {
    /// 并行执行多个子代理（简化版）
    pub async fn run_parallel(
        &self,
        specs: Vec<RunSpec>,
        config: ParallelConfig,
    ) -> ParallelResult {
        // 实现...
    }
    
    /// 并行执行带上下文共享
    pub async fn run_parallel_with_shared_context(
        &self,
        specs: Vec<RunSpec>,
        shared_context: SharedContext,
        config: ParallelConfig,
    ) -> ParallelResult {
        // 实现...
    }
}

// ==================== 高级接口 ====================

/// 子代理批次执行器
pub struct SubagentBatch {
    runner: SubagentRunner,
    specs: Vec<RunSpec>,
    config: ParallelConfig,
}

impl SubagentBatch {
    pub fn new(runner: SubagentRunner) -> Self {
        Self {
            runner,
            specs: Vec::new(),
            config: ParallelConfig::default(),
        }
    }
    
    pub fn add(mut self, spec: RunSpec) -> Self {
        self.specs.push(spec);
        self
    }
    
    pub fn with_config(mut self, config: ParallelConfig) -> Self {
        self.config = config;
        self
    }
    
    pub async fn execute(self) -> ParallelResult {
        self.runner.run_parallel(self.specs, self.config).await
    }
}
```

### 4.2 使用示例

```rust
// 示例 1: 批量代码审查
let runner = SubagentRunner::new(registry, work_dir, context_dir)
    .with_llm(llm);

let result = SubagentBatch::new(runner)
    .add(RunSpec::new("Review auth module", "...").with_type("coder"))
    .add(RunSpec::new("Review database layer", "...").with_type("coder"))
    .add(RunSpec::new("Review API endpoints", "...").with_type("coder"))
    .with_config(ParallelConfig {
        max_concurrency: 3,
        cancel_on_error: false,
        ..Default::default()
    })
    .execute()
    .await;

// 汇总结果
for r in &result.results {
    println!("{}: {}", r.agent_id, r.summary);
}

// 示例 2: 方案对比（竞争执行）
let result = runner.run_parallel(
    vec![
        RunSpec::new("Design A", "使用微服务架构...").with_type("plan"),
        RunSpec::new("Design B", "使用单体架构...").with_type("plan"),
    ],
    ParallelConfig {
        max_concurrency: 2,
        cancel_on_error: false,
        ..Default::default()
    },
).await;

// 示例 3: 信息收集（流式结果）
let mut rx = runner.run_parallel_streaming(
    vec![
        RunSpec::new("Search docs", "...").with_type("explore"),
        RunSpec::new("Search source", "...").with_type("explore"),
        RunSpec::new("Search issues", "...").with_type("explore"),
    ],
    ParallelConfig::default(),
).await;

while let Some(result) = rx.recv().await {
    match result {
        Ok(r) => println!("Completed: {}", r.summary),
        Err(e) => println!("Failed: {}", e),
    }
}
```

---

## 5. 关键挑战与解决方案

### 5.1 存储并发访问

**问题**：`SubagentStore` 不是线程安全的

**解决方案**：
```rust
// 使用 Arc<RwLock<>> 包装
let store = Arc::new(tokio::sync::RwLock::new(SubagentStore::new(...)));

// 每个任务获取写锁
let mut store_guard = store.write().await;
self.run(spec, &mut *store_guard, None).await
```

### 5.2 上下文隔离

**问题**：并行代理可能产生上下文冲突

**解决方案**：
```rust
// 确保每个子代理有独立的上下文目录
fn generate_isolated_context_dir(&self, agent_id: &str) -> PathBuf {
    self.context_dir.join("parallel").join(agent_id)
}
```

### 5.3 结果聚合

**问题**：如何有效汇总多个子代理的结果

**解决方案**：
```rust
pub struct ResultAggregator;

impl ResultAggregator {
    /// 合并多个摘要
    pub fn merge_summaries(results: &[SubagentResult]) -> String {
        results.iter()
            .map(|r| format!("## {}\n{}\n", r.agent_type, r.summary))
            .collect::<Vec<_>>()
            .join("\n")
    }
    
    /// 提取共同结论
    pub fn extract_common_conclusions(results: &[SubagentResult]) -> Vec<String> {
        // 使用简单文本分析或调用 LLM
    }
}
```

---

## 6. 性能评估

### 6.1 理论加速比

```
| 任务数 | 顺序执行 | 并行(3并发) | 加速比 |
|--------|----------|-------------|--------|
| 1      | 30s      | 30s         | 1.0x   |
| 3      | 90s      | 32s         | 2.8x   |
| 5      | 150s     | 55s         | 2.7x   |
| 10     | 300s     | 105s        | 2.9x   |
```

### 6.2 实际考虑因素

| 因素 | 影响 | 建议 |
|------|------|------|
| LLM API 限流 | 可能触发速率限制 | 设置合理的 max_concurrency |
| Token 成本 | 并发不减少 Token 消耗 | 预算控制 |
| 网络延迟 | 并发减少总等待时间 | 适合 I/O 密集型 |
| CPU/内存 | 多个代理同时运行 | 监控资源使用 |

---

## 7. 实现建议

### 7.1 优先级

| 功能 | 优先级 | 工作量 | 价值 |
|------|--------|--------|------|
| 基础并行执行 (join_all) | P1 | 半天 | 高 |
| 并发控制 (Semaphore) | P1 | 半天 | 高 |
| 流式结果 | P2 | 1天 | 中 |
| 结构化并发 | P2 | 2天 | 中 |
| 结果聚合器 | P3 | 1天 | 中 |

### 7.2 下一步行动

1. **实现基础并行执行**
   ```bash
   # 新增文件
   crates/clarity-core/src/subagents/parallel.rs
   ```

2. **添加测试**
   ```bash
   cargo test -p clarity-core parallel
   ```

3. **更新文档**
   - README.md
   - PROJECT_REPORT.md

4. **集成到 Agent**
   - 作为工具暴露给 Agent
   - 支持从自然语言触发并行执行

---

## 8. 参考实现

### 8.1 Kimi CLI 参考

Kimi CLI 的并行执行主要通过：
- 异步任务 (`asyncio.create_task`)
- 后台任务 (`BackgroundTaskManager`)
- 不直接支持同一 Agent 内的并行子代理

### 8.2 Rust 生态参考

| 项目 | 用途 | 参考点 |
|------|------|--------|
| `futures::join_all` | 简单并行 | 基础实现 |
| `tokio::task::JoinSet` | 结构化并发 | 错误处理 |
| `futures::FuturesUnordered` | 流式结果 | 实时响应 |
| `async_channel` | 背压控制 | 生产者-消费者 |

---

**结论**: 子代理并行执行是**可行且有价值的**，建议优先实现基础并行功能（方案 A），后续根据需要扩展流式和结构化并发支持。
