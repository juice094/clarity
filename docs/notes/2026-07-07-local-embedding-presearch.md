---
title: 本地 Embedding 方案预研 — fastembed / candle / ort / rust-bert
category: Note
date: 2026-07-07
tags: [research, embedding, sqlite-vec, knowledge-field, clarity-memory]
---

# 本地 Embedding 方案预研 — fastembed / candle / ort / rust-bert

> Type: Technical presearch for Knowledge Field next-gen retrieval
> Trigger: `clarity-memory` 仍使用本地 TF-IDF + cosine，`clarity-knowledge` 的 `HybridRetriever` 依赖 BM25 + TF-IDF cosine，计划评估真正的本地 embedding
> Status: 预研结论，供 PoC 立项使用，不改动代码
> Related: [`2026-07-07-basidiocarp-reference.md`](./2026-07-07-basidiocarp-reference.md)

---

## 1. 背景与目标

当前 `clarity-memory` 使用 TF-IDF 向量 + 余弦相似度做“语义”搜索，本质仍是词袋统计，无法处理同义词、跨语言、长程语义。`clarity-knowledge` 的 `HybridRetriever` 同样基于 TF-IDF cosine 作为语义分支。

本笔记评估在 Clarity 中引入真正的本地 embedding 模型 + 本地向量存储的可行路径，目标：

1. 不引入外部向量服务（本地优先）。
2. 与现有 SQLite 后端自然衔接。
3. 中英文混合场景可用。
4. 二进制体积与推理成本可控。

---

## 2. 候选方案对比

### 2.1 方案 A：`fastembed-rs` + `sqlite-vec`（Basidiocarp hyphae 同路线）

| 维度 | 评估 |
|------|------|
| **依赖/构建复杂度** | 低。`fastembed` 基于 `ort`（ONNX Runtime Rust 绑定）和 `tokenizers`，API 封装完整，一行 `cargo add fastembed` 即可。`sqlite-vec` 是 C 扩展，通过 `rusqlite` 的 `sqlite3_auto_extension` 注册。 |
| **二进制体积影响** | `fastembed` 不静态链接模型，模型在运行时下载/缓存；crate 本身增加约 `ort` + `tokenizers` 的体积（预估 +10–30 MB 依赖增量，视 ONNX Runtime 构建配置而定）。`sqlite-vec` 是纯 C 扩展，扩展体积极小（~2 MB）。 |
| **推理速度 / 首次加载** | 首次使用需下载模型（BGE-small 约 50–100 MB），之后本地缓存。ONNX Runtime CPU 推理对小批量文本足够快（~ms 级/条）。不支持 GPU 时纯 CPU 仍可接受。 |
| **模型质量（CJK）** | `fastembed-rs` 直接支持 `BAAI/bge-small-zh-v1.5`、`BAAI/bge-m3`、`intfloat/multilingual-e5-*`、`jina-embeddings-v2-base-*` 等多语言模型，中文检索质量明显优于纯英文模型。 |
| **与 SQLite 后端契合度** | **最高**。`sqlite-vec` 直接作为 SQLite 扩展运行，与 `clarity-memory` 已有的 `rusqlite` 后端天然契合；向量与元数据同库，ACID 一致。 |

**参考**
- [`fastembed` crates.io 5.x](https://crates.io/crates/fastembed/5.3.1)
- [fastembed-rs GitHub — 支持模型列表](https://github.com/Anush008/fastembed-rs)
- [Using sqlite-vec in Rust](https://alexgarcia.xyz/sqlite-vec/rust.html)
- [On-device vector databases 2026 对 sqlite-vec 的体积评估](https://objectbox.io/262454-2/)

---

### 2.2 方案 B：`candle` 自托管 embedding

| 维度 | 评估 |
|------|------|
| **依赖/构建复杂度** | 中。`candle`（Hugging Face 的 Rust ML 框架）不依赖 Python/ONNX Runtime，纯 Rust + CUDA/Metal 可选后端。但需要自己写模型加载、tokenizer、pooling 逻辑；对非标准模型适配成本较高。 |
| **二进制体积影响** | candle-core 本身较轻量，但模型权重需额外下载/打包。若启用 CUDA/Metal 后端，动态库体积会显著增加。 |
| **推理速度 / 首次加载** | 首次加载需下载 SafeTensors 权重；CPU 上 candle 的推理速度通常略逊于高度优化的 ONNX Runtime，但差距在可接受范围。 |
| **模型质量（CJK）** | 可加载任何 Hugging Face 上的 transformer 模型（`BAAI/bge-*`、`intfloat/e5-*`、Qwen3-Embedding 等），CJK 质量取决于所选模型。`candle` 不自带模型 zoo，需要自行处理 tokenizer 配置。 |
| **与 SQLite 后端契合度** | 中。embedding 生成与存储解耦，仍需 `sqlite-vec` 或自研向量索引存储向量；与 SQLite 后端无天然绑定。 |

**适用场景**：希望完全摆脱 ONNX Runtime、对构建产物可控度要求极高，或需要加载 `fastembed` 不支持的最新模型（如 Qwen3-Embedding 系列）时使用。`fastembed` 5.x 也已通过 feature flag 引入 candle 后端支持部分模型（`qwen3`、`nomic-v2-moe`），可作为中间路线。

---

### 2.3 方案 C：`ort` + 预训练 ONNX 模型

| 维度 | 评估 |
|------|------|
| **依赖/构建复杂度** | 中–高。`ort` 是 ONNX Runtime 的 Rust 绑定，需要自己下载/转换 ONNX 模型、准备 tokenizer 文件、处理输入输出节点。比 `fastembed` 更底层，但灵活性更高。 |
| **二进制体积影响** | `ort` 依赖 ONNX Runtime 动态/静态库，体积较大；静态链接到 release 二进制可能增加 20–50 MB。模型文件单独缓存，不计入二进制。 |
| **推理速度 / 首次加载** | 与 `fastembed` 相当（底层同一引擎）。首次需自行下载 ONNX 模型和 tokenizer。 |
| **模型质量（CJK）** | 完全取决于选择的 ONNX 模型。中文可选 `bge-small-zh-v1.5`、`bge-m3`、`multilingual-e5-small/base` 等；需自行从 Hugging Face 导出或寻找社区 ONNX 版本。 |
| **与 SQLite 后端契合度** | 中。生成与存储分离，需额外接入 `sqlite-vec`。 |

**适用场景**：已有固定模型资产、需要自定义量化（INT8/FP16）或必须使用特定 ONNX 模型时使用。否则不如 `fastembed` 开箱即用。

---

### 2.4 可选项：`rust-bert`

| 维度 | 评估 |
|------|------|
| **依赖/构建复杂度** | 中。`rust-bert` 提供 BERT/DistilBERT/RoBERTa/GPT2 等预置 pipeline，API 高层。但生态更新较慢，embedding 支持不如 `fastembed` 专注。 |
| **二进制体积影响** | 依赖 `tch`（LibTorch 绑定）或可选 `ort`，体积较大；LibTorch 动态库可达数百 MB。 |
| **推理速度 / 首次加载** | LibTorch 首次加载慢；`ort` feature 可改善，但配置复杂。 |
| **模型质量（CJK）** | 原生支持的多语言模型有限；中文需自行加载 `bert-base-chinese` 或 `bge-*` 的 rust-bert 兼容版本，维护成本高。 |
| **与 SQLite 后端契合度** | 低。需要额外桥接存储。 |

**结论**：`rust-bert` 不是本地 embedding 的首选，更适合需要完整 NLP pipeline（NER、QA、摘要）的场景。

---

## 3. 综合评估矩阵

| 方案 | 构建复杂度 | 二进制增量 | 推理速度 | CJK 质量 | SQLite 契合 | 维护成本 | 推荐度 |
|------|-----------|-----------|---------|---------|------------|---------|--------|
| **fastembed-rs + sqlite-vec** | 低 | 中 | 快 | 高 | **高** | 低 | **★★★ 首选** |
| **candle 自托管** | 中 | 中 | 中 | 高 | 中 | 中 | ★★ 次选 |
| **ort + ONNX 模型** | 中–高 | 大 | 快 | 高 | 中 | 高 | ★★ fallback |
| **rust-bert** | 中 | 大 | 慢 | 中 | 低 | 高 | ★ 不推荐 |

---

## 4. 推荐路径

### 4.1 首选 PoC 方案：`fastembed-rs` + `sqlite-vec`

**理由**：
- 与 Basidiocarp hyphae 路线一致，已被验证为本地优先 RAG 的可行架构。
- `sqlite-vec` 直接嵌入现有 SQLite 后端，不需要额外数据库进程。
- `fastembed` 开箱支持 `bge-small-zh-v1.5`、`bge-m3`、`multilingual-e5-small` 等中英文模型，CJK 支持最好。
- API 同步、无需 Tokio，与当前 `clarity-memory` 的同步/异步混合代码兼容成本低。

**建议 PoC 模型**：`BAAI/bge-small-zh-v1.5`（384 dim）或 `intfloat/multilingual-e5-small`（384 dim）。两者体积小、速度快、中英文兼备；PoC 通过后再评估升级到 `bge-m3`（1024/4096 dim，dense+sparse+ColBERT）。

### 4.2 Fallback：`candle` 自托管

如果 `ort` 的构建/部署问题在目标平台（尤其是 Android 或某些 Linux 发行版）上难以解决，可回退到 `candle-core` + `candle-nn` + Hugging Face 模型。需要额外投入 tokenizer 和 pooling 实现，但避免了 ONNX Runtime 依赖。

---

## 5. 集成草图（替换 `HybridRetriever` 的 TF-IDF cosine）

若决定将 embedding 引入，需要修改的文件大致如下：

### 5.1 依赖与 feature 声明

- `crates/clarity-memory/Cargo.toml`
  - 新增 `fastembed = "5"`（或指定版本）。
  - 新增 `sqlite-vec = "..."`。
  - 可选：新增 feature flag `embedding = ["fastembed", "sqlite-vec"]`，保留 `sqlite` feature 的纯 FTS5 路径。

- `crates/clarity-knowledge/Cargo.toml`
  - 若需要直接访问 embedding 类型，新增 `clarity-memory` 的 `embedding` feature 依赖。

### 5.2 `clarity-memory` 内部

- `crates/clarity-memory/src/embedding.rs`
  - 保留现有 `TfidfVectorizer`、`CosineIndex`、`VectorStore` 作为 fallback / 无模型场景。
  - 新增 `EmbeddingModel` 包装（封装 `fastembed::TextEmbedding`）和 `DenseVectorIndex` trait。
  - 提供统一的 `EmbeddingIndex` API：`index(docs) -> ids`、`search(query, top_k) -> results`。

- `crates/clarity-memory/src/backends/sqlite_store.rs`（如存在独立文件）或 `store.rs`
  - 在 SQLite schema 中增加 `vec0` 虚拟表或 `BLOB` 向量列。
  - `save_fact` / `bulk_save_facts` 在写入文本时同步生成/缓存 embedding。
  - `search_semantic(query)` 使用 `sqlite-vec` 的 `vec_distance_cosine` 或 Top-K 查询。

- `crates/clarity-memory/src/semantic.rs`
  - 当前 `build_index` 使用 TF-IDF；改为可选使用 embedding index。

### 5.3 `clarity-knowledge` 内部

- `crates/clarity-knowledge/src/retrieval.rs`
  - `HybridRetriever` 中：
    - 将 `vectorizer: TfidfVectorizer` 与 `cosine_index: Option<CosineIndex>` 替换为 `embedding_index: Option<EmbeddingIndex>`。
    - 在 `ensure_cosine_index()` 位置改为 `ensure_embedding_index()`，调用 `fastembed` 生成文档向量并写入 `sqlite-vec`。
    - 保留 BM25 分支和 graph neighbor boost；将 cosine 得分替换为 embedding cosine 得分，并重新调权（参考 Basidiocarp 30% BM25 + 70% cosine）。

- `crates/clarity-knowledge/src/index.rs`
  - `InMemoryIndex` 目前是纯内存实现；若接入持久化向量，需要把 `retriever` 持久化到 SQLite，或新增 `PersistentIndex` 实现 `KnowledgeIndex`。
  - 文件系统 watcher（`notify`）触发的事件需同步更新向量索引。

### 5.4 测试与示例

- `crates/clarity-memory/src/lib.rs` 中的 `test_vector_search`、`test_embedding_integration` 需要新增 embedding 版本或保留 TF-IDF 版本作为 feature-gated 测试。
- `crates/clarity-knowledge/src/retrieval.rs` 的 `hybrid_search_finds_by_tag_and_text` 等测试需覆盖中英文混合查询。

### 5.5 新增/修改文件清单（核心）

```
crates/clarity-memory/Cargo.toml                  # +fastembed, +sqlite-vec, +feature
crates/clarity-memory/src/embedding.rs            # 新增 EmbeddingModel / EmbeddingIndex
crates/clarity-memory/src/store.rs                # save_fact 生成 embedding
crates/clarity-memory/src/backends/sqlite_store.rs # schema + vec0 表
crates/clarity-memory/src/semantic.rs             # 可选 embedding 构建
crates/clarity-knowledge/Cargo.toml               # feature 透传
crates/clarity-knowledge/src/retrieval.rs         # 替换 cosine 分支
crates/clarity-knowledge/src/index.rs             # 持久化 index 接入点
```

---

## 6. 风险与待验证问题

| 风险 | 描述 | 缓解/待验证 |
|------|------|-------------|
| **模型下载** | 首次启动需从 Hugging Face / Qdrant 下载模型，可能受网络影响。 | 支持 `FASTEMBED_CACHE_PATH` 指定缓存路径；PoC 验证离线打包模型的可行性。 |
| **离线可用性** | 完全离线环境无法下载模型。 | 预先下载并随 Clarity 分发模型缓存；或支持用户指定本地 ONNX/Tokenizer 路径。 |
| **CJK 效果** | 虽然 `bge-small-zh`/`multilingual-e5` 对中文友好，但具体在 Clarity 的短文本、代码片段、笔记混合语料上效果未知。 | 用真实 vault 数据做 recall@K 评测，对比 TF-IDF baseline。 |
| **量化精度** | `fastembed` 默认使用量化 ONNX；INT8/Q 量化可能在长文本或专业术语上损失精度。 | PoC 同时测试量化版与 FP32/FP16 版，观察检索质量差异。 |
| **二进制与启动时间** | `ort` 静态链接会显著增加 release 二进制体积；首次模型加载有延迟。 | 测试 release build 体积与冷启动时间，评估是否在可接受范围。 |
| **多平台构建** | Android / Windows / macOS / Linux 对 ONNX Runtime 的预编译库支持不同。 | 在目标平台（尤其 Android）上跑通 `cargo build` 与单元测试。 |
| **数据迁移** | 已有 `clarity-memory` SQLite 数据库无向量列，需要 schema migration。 | 设计增量迁移：旧数据保留 TF-IDF 索引，新数据写入 embedding；或一次性 backfill。 |

---

## 7. 下一步建议

1. **PoC（2–3 天）**：在 `clarity-memory` 中新增 `embedding` feature，接入 `fastembed` + `sqlite-vec`，用 1000 条真实/合成笔记跑 recall 对比。
2. **评测指标**：对比 TF-IDF baseline 与 `bge-small-zh-v1.5` / `multilingual-e5-small` 在中文、英文、混合查询下的 Top-5 / Top-10 recall。
3. **构建验收**：在 Windows、Linux、Android 三个目标平台验证 release 二进制体积与首次启动下载/加载时间。
4. **决策点**：PoC 通过后再决定是否替换 `HybridRetriever` 的 TF-IDF cosine，或保留两者作为可配置策略。

---

*本笔记用于 Knowledge Field 下一代检索架构的跨会话继承。*
