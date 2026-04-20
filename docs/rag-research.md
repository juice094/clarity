# Wave 4: RAG 向量知识库技术调研报告

> 调研日期: 2026-04-15
> 基于 clarity-memory v0.1.0 当前架构

---

## 一、当前架构盘点

### 1.1 已有能力

| 模块 | 功能 | 状态 |
|------|------|------|
| `SqliteStore` | SQLite + FTS5 全文搜索 | ✅ 生产级 |
| `FileStore` | JSON 原子写入 | ✅ 可用 |
| `HybridStore` | 热缓存 + 冷存储 | ✅ 可用 |
| `TF-IDF + CosineIndex` | 内存向量索引 | ✅ 可用 |
| `VectorStore` | 事实向量存储 | ✅ 可用 |
| `FactExtractor` | LLM/规则事实提取 | ✅ 可用 |
| `MemoryCompiler` | 四级编译+去重+遗忘 | ✅ 可用 |

### 1.2 现有搜索路径

```
search_similar(query) → 全表读取所有 facts → 内存构建 TF-IDF → 余弦相似度排序
```

**问题**: 每次查询都全表扫描 + 重建索引，O(n) 复杂度，facts 增多后性能急剧下降。

### 1.3 缺失能力

- ❌ 向量索引**持久化**（每次重建）
- ❌ **BM25** 评分（比 TF-IDF 更适合短文本检索）
- ❌ **Hybrid Search**（关键词 + 语义融合）
- ❌ 文档**分块/Chunking**
- ❌ **重排序/Rerank**
- ❌ 与 LLM 的 **RAG 管道**集成

---

## 二、方案评估

### 2.1 方案 A: 纯增强现有 TF-IDF（零新依赖）

在 `embedding.rs` 基础上添加 BM25，优化 Hybrid Search。

```rust
// 新增: BM25Vectorizer
pub struct Bm25Vectorizer { ... }

// 新增: HybridSearcher (FTS5 召回 + BM25/TF-IDF 重排)
pub struct HybridSearcher {
    fts_recorder: SqliteStore,  // 关键词召回
    vector_index: Bm25Index,    // 语义重排
}
```

**优点**:
- 零新依赖，`cargo run` 直接运行
- 与现有架构 100% 兼容
- 短文本（fact 级别）BM25 效果往往优于 TF-IDF

**缺点**:
- 仍是词袋模型，无真语义理解
- "Rust 编程" vs "Rust 语言" 视为不同

**工作量**: 2-3 天

---

### 2.2 方案 B: sqlite-vec（+1 个依赖）

将 `sqlite-vec` 作为 SQLite 扩展引入，在现有 `facts` 表旁增加 `vec0` 虚拟表。

```sql
-- 新增向量表
CREATE VIRTUAL TABLE fact_embeddings USING vec0(
    embedding float[384]  -- 或稀疏向量
);

-- 向量检索
SELECT f.*, distance 
FROM facts f
JOIN fact_embeddings e ON f.id = e.rowid
WHERE e.embedding MATCH vec_normalize(?)
ORDER BY distance
LIMIT 10;
```

**优点**:
- 纯 C，无额外依赖，与 rusqlite 完美集成
- 向量持久化，查询 O(log n) 而非 O(n)
- 支持 float/int8/binary 向量
- Mozilla Builders 项目，社区活跃

**缺点**:
- 仅存储/检索向量，不生成 embedding
- 需要自行提供向量（TF-IDF 稀疏向量或外部 embedding）

**工作量**: 3-4 天（含 Rust binding 适配）

**关键 crate**: `sqlite-vec = "0.1"`

---

### 2.3 方案 C: FastEmbed-rs + ort（重依赖）

本地 ONNX 模型生成高质量 embedding。

```rust
use fastembed::{TextEmbedding, EmbeddingModel};

let model = TextEmbedding::try_new(
    InitOptions::new(EmbeddingModel::AllMiniLML6V2)
)?;
let embeddings = model.embed(vec!["text"], None)?;
```

**优点**:
- 真正的语义理解（"编程"≈"开发"≈"coding"）
- CPU 推理足够快（~1000 docs/sec）
- FastEmbed-rs 是 Rust 原生实现

**缺点**:
- `ort` 编译复杂，Windows 可能有 DLL 问题
- 二进制体积 +50~100MB（ONNX Runtime + 模型）
- 首次运行需下载模型（~20-100MB）
- 与"零构建步骤"约束冲突

**工作量**: 5-7 天（含跨平台适配）

**关键 crates**: `fastembed = "..."`, `ort = "2.0"`

---

### 2.4 方案 D: 外部 Embedding API（零新依赖，需网络）

调用 Kimi/月之暗面 Embedding API。

```rust
async fn embed_texts(texts: &[String]) -> Result<Vec<Vec<f32>>> {
    // POST https://api.moonshot.cn/v1/embeddings
    // model: "moonshot-embedding-1"
}
```

**优点**:
- 最高质量 embedding（大规模预训练模型）
- 零本地模型负担
- 与现有 API 调用模式一致

**缺点**:
- 需要网络连接
- 有 API 费用/速率限制
- 隐私敏感数据需考虑

**工作量**: 1-2 天

---

## 三、推荐路线：渐进式三阶段

```
┌─────────────────────────────────────────────────────────────┐
│  Phase 1: 纯 Rust 增强 (Wave 4A-B)                          │
│  ─────────────────────────────────                          │
│  • BM25 评分替代/增强 TF-IDF                                │
│  • Hybrid Search: FTS5 召回 + BM25 重排                     │
│  • 增量向量索引（避免全表重建）                               │
│  • 文档分块 Chunking                                        │
│  目标: 10x 性能提升，零新依赖                                 │
├─────────────────────────────────────────────────────────────┤
│  Phase 2: sqlite-vec 持久化 (Wave 4C)                       │
│  ─────────────────────────────────                          │
│  • 引入 sqlite-vec 作为可选 feature                         │
│  • 向量索引持久化到 SQLite                                   │
│  • 支持稠密向量占位（为外部 embedding 做准备）                │
│  目标: 向量查询从 O(n) → O(log n)                           │
├─────────────────────────────────────────────────────────────┤
│  Phase 3: 外部 Embedding API (Wave 4D-E)                    │
│  ─────────────────────────────────                          │
│  • 添加 Kimi embedding API 调用                             │
│  • 完整 RAG 管道: 检索 → 重排 → 注入上下文                   │
│  • 可选降级到本地 BM25（离线时）                             │
│  目标: 生产级语义 RAG                                       │
└─────────────────────────────────────────────────────────────┘
```

### 为什么这样选？

1. **约束兼容**: 全程零 Node.js，Phase 1 零新依赖，`cargo run` 直接运行
2. **风险可控**: 每阶段独立可验证，失败可回退
3. **性能渐进**: 从"能用"(TF-IDF) → "好用"(BM25+Hybrid) → "聪明"(语义 Embedding)
4. **架构兼容**: 与现有 `StorageBackend` trait 完全兼容，不破坏已有接口

---

## 四、Phase 1 详细设计

### 4.1 BM25 实现

```rust
/// BM25 评分器（Okapi BM25）
pub struct Bm25Scorer {
    k1: f32,      // 词频饱和度（默认 1.5）
    b: f32,       // 文档长度归一化（默认 0.75）
    avg_dl: f32,  // 平均文档长度
    doc_freq: HashMap<String, u32>,
    doc_lengths: Vec<u32>,
    total_docs: u32,
}

impl Bm25Scorer {
    pub fn score(&self, query: &str, doc_idx: usize) -> f32 {
        // BM25(query, doc) = Σ IDF(q_i) * (f(q_i, D) * (k1 + 1)) 
        //                    / (f(q_i, D) + k1 * (1 - b + b * |D| / avg_dl))
    }
}
```

### 4.2 Hybrid Search 流程

```
用户查询 "Rust 异步编程最佳实践"
         │
         ▼
┌─────────────────┐
│  Step 1: FTS5   │  → 关键词召回 Top-50（快速过滤）
│  全文搜索        │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Step 2: BM25   │  → 对 Top-50 重排序（ relevance scoring ）
│  评分重排        │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Step 3: 返回   │  → Top-K 结果
│  (Fact, score)  │
└─────────────────┘
```

### 4.3 增量索引

当前 `search_similar` 每次全表读取所有 facts 重建 VectorStore：

```rust
// 当前实现（问题）
let facts = /* SELECT * FROM facts */;  // 全表扫描！
let mut vector_store = VectorStore::new();
vector_store.index_facts(&fact_tuples);  // 重建索引！
```

优化方案：在 `SqliteStore` 中维护一个**内存缓存的 BM25 索引**，通过 WAL/触发器感知变更：

```rust
pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
    // 新增: 增量维护的索引
    bm25_index: Arc<RwLock<Bm25Index>>,
    last_indexed_id: AtomicI64,
}

// 查询时只索引新增的 facts
fn refresh_index(&self) {
    let new_facts = /* SELECT * FROM facts WHERE id > last_indexed_id */;
    self.bm25_index.write().add_documents(&new_facts);
}
```

### 4.4 Chunk 支持

当前 `Fact` 是原子单位，适合短事实。RAG 需要处理长文档：

```rust
/// 文档分块配置
#[derive(Debug, Clone)]
pub struct ChunkConfig {
    pub chunk_size: usize,      // 默认 512 tokens/字符
    pub chunk_overlap: usize,   // 默认 50
    pub separator: String,      // 默认 "\n\n"
}

/// 分块后的知识单元
#[derive(Debug, Clone)]
pub struct Chunk {
    pub id: String,
    pub content: String,
    pub source_id: i64,         // 关联的 fact id
    pub index: usize,           // 在原文中的块序号
    pub metadata: ChunkMetadata,
}
```

---

## 五、与工具系统的集成设计

### 5.1 新增工具: `knowledge_search`

```json
{
  "name": "knowledge_search",
  "description": "Search the knowledge base for relevant context using semantic + keyword hybrid search.",
  "parameters": {
    "query": "string (required) - The search query",
    "limit": "number (optional) - Max results, default 5",
    "mode": "string (optional) - 'hybrid' (default), 'semantic', 'keyword'"
  }
}
```

### 5.2 RAG 管道集成

在 `clarity-core` 的 `turn` 处理中：

```rust
// 1. 提取用户查询意图
// 2. 调用 knowledge_search 检索相关 facts
// 3. 将检索结果注入 system prompt 上下文
// 4. 发送给 LLM 生成回答

let context = knowledge.search(&user_query, 5).await?;
let enriched_prompt = format!(
    "以下是与用户问题相关的背景信息:\n{}\n\n用户问题: {}",
    format_facts(&context),
    user_query
);
```

---

## 六、工作量估算

| 阶段 | 内容 | 预估工期 | 新依赖 |
|------|------|---------|--------|
| 4A | BM25 + Hybrid Search | 2-3 天 | 0 |
| 4B | 增量索引 + Chunking | 2-3 天 | 0 |
| 4C | sqlite-vec 持久化 | 3-4 天 | `sqlite-vec` |
| 4D | 外部 Embedding API | 1-2 天 | 0 |
| 4E | RAG 管道 + knowledge_search 工具 | 2-3 天 | 0 |
| **合计** | | **10-15 天** | **1** |

---

## 七、决策检查点

- [x] 技术调研完成
- [ ] 用户确认路线（推荐 Phase 1 起步）
- [ ] Phase 1 实现完成 + 测试通过
- [ ] Phase 1 性能基准测试（对比当前全表扫描）
- [ ] 决定是否进入 Phase 2（sqlite-vec）
- [ ] 决定是否进入 Phase 3（外部 Embedding）

---

## 附录：参考资源

1. [sqlite-vec Rust 文档](https://alexgarcia.xyz/sqlite-vec/rust.html)
2. [sqlite-vec GitHub](https://github.com/asg017/sqlite-vec)
3. [vecstore crate](https://docs.rs/vecstore)
4. [FastEmbed-rs + Rig](https://dev.to/joshmo_dev/local-embeddings-with-fastembed-rig-rust-3581)
5. [ort crate (ONNX Runtime)](https://lib.rs/crates/ort)
6. [Rust ML Frameworks 对比](https://dev.to/mayu2008/building-sentence-transformers-in-rust-281k)
