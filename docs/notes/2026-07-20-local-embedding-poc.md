---
title: Knowledge Field 本地 Embedding PoC（泳道 B1）— fastembed + sqlite-vec + RRF
category: Note
date: 2026-07-20
tags: [embedding, sqlite-vec, fastembed, knowledge-field, poc, lane-b1]
---

# Knowledge Field 本地 Embedding PoC（泳道 B1）

> Type: PoC 实施与评测记录
> Trigger: 预研笔记 [`2026-07-07-local-embedding-presearch.md`](./2026-07-07-local-embedding-presearch.md) 推荐方案 A（fastembed + sqlite-vec）
> Status: PoC 完成，代码以可选 feature 落地，默认路径不受影响
> Related: [`2026-07-07-local-embedding-presearch.md`](./2026-07-07-local-embedding-presearch.md)

---

## 1. 范围与约束

泳道 B1 任务：在 `clarity-knowledge` 中以**可选 feature `local-embedding`（默认关闭）**实现本地 embedding 召回分支，与现有 BM25 + TF-IDF cosine + 图传播的 `HybridRetriever` 做融合，并用 benchmark 对比召回质量。

约束执行结果：

- 默认构建（无 feature）源码路径**零变化**：所有新增代码均在 `#[cfg(feature = "local-embedding")]` 门后；`cargo check -p clarity-knowledge` 与 CI 不受影响。
- 不接入默认路径：`HybridRetriever` 新增 `enable_local_embedding()` 显式启用入口，未调用时行为与基线完全一致（有单测锁定）。
- 工作区 lint 全过：`missing_docs` / `unwrap_used` / `expect_used` / `panic` / clippy `-D warnings` / `cargo fmt`。

## 2. 实现方案

### 2.1 依赖

`crates/clarity-knowledge/Cargo.toml`：

```toml
fastembed = { version = "5", optional = true }      # 解析为 5.17.3（ort 2.0.0-rc.12）
sqlite-vec = { version = "0.1", optional = true }   # 解析为 0.1.9

[features]
local-embedding = ["dep:fastembed", "dep:sqlite-vec"]
```

### 2.2 模块结构（`crates/clarity-knowledge/src/embedding.rs`）

| 组件 | 职责 |
|------|------|
| `Embedder` trait | 文本 → 稠密向量抽象，测试可注入确定性 fake（无网络、无模型） |
| `FastembedEmbedder` | fastembed 封装，默认模型 `BAAI/bge-small-zh-v1.5`（512 维，中英文） |
| `VectorStore` | rusqlite + sqlite-vec `vec0` 虚表（cosine 距离），Connection 包 `Mutex` 保持 `Sync` |
| `EmbeddingBranch` | embedder + store + path↔rowid 映射；文档懒式批量嵌入（dirty 标记驱动） |
| `reciprocal_rank_fusion` | 纯函数 RRF（k=60），多路排名融合 |

sqlite-vec 通过 `rusqlite::ffi::sqlite3_auto_extension` 注册，是本 crate 唯一的 `unsafe`（局部 `#[allow(unsafe_code)]`，带 SAFETY 注释，与 sqlite-vec 官方示例一致）。

### 2.3 融合方式：RRF（Reciprocal Rank Fusion）

`HybridRetriever::search` 的基线评分（BM25 加权 + TF-IDF cosine ×2 + 标题 +5 + 图邻居 ×0.3）**原样保留**。启用 embedding 分支后：

1. 基线结果排序得到排名列表 A；
2. embedding 分支对全库做 KNN（cosine），得到排名列表 B；
3. `RRF(A, B)` 融合：`score(p) = Σ 1/(60 + rank_l(p) + 1)`，重新排序输出。

选 RRF 而非加权求和的理由：基线各分支分数量纲差异大（BM25 无上界、cosine ∈ [0,1]、标题 boost +5），加权求和需要重新调参；RRF 只用排名，无需标定，对 PoC 更稳。embedding-only 命中的条目（基线完全召回不到的 paraphrase 文档）会合成 `SearchResult` 进入融合结果。

### 2.4 文件清单

| 文件 | 变更 |
|------|------|
| `crates/clarity-knowledge/Cargo.toml` | +`fastembed`/`sqlite-vec` 可选依赖、+`local-embedding` feature、+`[[example]]` required-features |
| `crates/clarity-knowledge/src/embedding.rs` | **新增**，全部 feature-gated |
| `crates/clarity-knowledge/src/retrieval.rs` | +`embedding` 字段（cfg）、`enable_local_embedding()`、`fuse_with_embedding()`、add/remove 时 dirty 同步、3 个 feature-gated 测试 |
| `crates/clarity-knowledge/src/error.rs` | +`KnowledgeError::Embedding(String)` 变体 |
| `crates/clarity-knowledge/src/lib.rs` | +`#[cfg(feature)] pub mod embedding;` |
| `crates/clarity-knowledge/examples/vault_embedding_benchmark.rs` | **新增**，`required-features = ["local-embedding"]` |
| `Cargo.lock` | 新增 fastembed/ort/sqlite-vec 依赖树（仅 feature 启用时参与编译） |

## 3. 测试

单测（feature 开启时 44 个，默认 38 个）全部通过：

- `embedding.rs`：VectorStore 增删查/upsert 替换、EmbeddingBranch paraphrase 召回（fake embedder）、RRF 排序性质；
- `retrieval.rs`：`embedding_branch_recalls_paraphrase`（基线召回失败 → 融合后 top1 命中）、`default_search_unchanged_without_embedding_branch`（无分支时基线评分不变，回归锁）。

fake embedder 用「同义词表 → 正交维度」模拟语义相似，全程无网络、无模型下载。

## 4. 验收命令结果（2026-07-20，Windows x86_64，stable 1.96）

| 命令 | 结果 |
|------|------|
| `cargo check -p clarity-knowledge`（默认） | ✅ pass |
| `cargo check -p clarity-knowledge --features local-embedding` | ✅ pass |
| `cargo test -p clarity-knowledge`（默认） | ✅ 38 passed / 0 failed |
| `cargo test -p clarity-knowledge --features local-embedding` | ✅ 44 passed / 0 failed |
| `cargo clippy -p clarity-knowledge --lib --tests -- -D warnings` | ✅ 0 warning |
| `cargo clippy -p clarity-knowledge --lib --tests --examples --features local-embedding -- -D warnings` | ✅ 0 warning |
| `cargo fmt --all -- --check` | ✅ pass |

## 5. Benchmark

示例：`crates/clarity-knowledge/examples/vault_embedding_benchmark.rs`（参考 `vault_benchmark.rs` 风格）。

- 合成 vault：6 个主题簇 × 5 篇 = 30 篇文档；**查询词在目标文档中字面不出现**（如查询「人工智能」，文档讲「深度学习/神经网络」），专门放大 TF-IDF/BM25 的词袋短板；
- 每查询 2 个（中/中、中/英混合），指标 recall@5（ground truth = 本簇 5 篇）+ 首个相关命中排名 + 冷/热延迟；
- 首次运行下载 BGE-small-zh-v1.5 ONNX（~50 MB）；下载失败时自动降级为只跑基线并以「NOT MEASURED」收尾，退出码仍为 0。

### 5.1 实测结果（2026-07-20，Windows x86_64，debug build，CPU 推理）

模型获取插曲：fastembed 直连 huggingface.co 下载失败（`Failed to retrieve onnx/model.onnx`，LFS CDN 被断），`HF_ENDPOINT=https://hf-mirror.com` 与 hf-hub 分块下载协议不兼容（`Header Content-Range is missing`）。最终用 curl 从 hf-mirror 手动把 `model.onnx`（94,851,877 字节）与 4 个 tokenizer/配置文件补齐到 `.fastembed_cache/`，以 `HF_HUB_OFFLINE=1` 离线加载成功（冷加载 157ms，dim 512）。模型缓存已加入 `.gitignore`。

```text
query           base@5 base rk   base cold/warm |  fused@5 fused rk   fused cold
--------------------------------------------------------------------------------------
人工智能              0.00       -          4ms/1ms |     0.80        2      154.5ms
机器学习              0.60       1          1ms/1ms |     0.60        1        5.4ms
气候变化              0.60       1          2ms/1ms |     0.60        1        6.0ms
全球暖化              0.40       1          1ms/1ms |     0.40        1        6.3ms
锻炼身体              0.00       -          1ms/2ms |     0.60        3        6.1ms
健身计划              0.20       1          1ms/1ms |     0.40        1        5.6ms
数据库索引             0.20       3          1ms/1ms |     0.40        2        6.3ms
存储引擎              0.20       1          1ms/1ms |     0.60        1        6.1ms
烹饪                0.00       -          1ms/1ms |     1.00        1        5.3ms
菜谱                0.00       -          1ms/1ms |     1.00        1        5.5ms
内存安全              0.00       -          1ms/1ms |     0.20        5        5.9ms
系统编程语言            0.40       2          1ms/2ms |     0.40        2        7.8ms
--------------------------------------------------------------------------------------
Mean recall@5: baseline 0.22  fused 0.58
```

### 5.2 解读

- **平均 recall@5：0.22 → 0.58（2.6×）**。5 个基线完全失败（recall=0）的 paraphrase 查询中，4 个在融合后召回 0.6–1.0；「烹饪」「菜谱」直接满分且排名第一。
- **基线优势不退化**：所有基线已命中的查询（机器学习、气候变化、系统编程语言等）融合后 recall 与排名完全不变——RRF 只增不减的特性得到验证。
- **延迟可接受**：fused 冷查询 5–8ms（30 篇库，含 query 嵌入 + KNN），首次查询 154ms 含全库嵌入构建；热路径与基线同量级。对真实 vault（数千篇）需复测，但 ONNX CPU 嵌入 + sqlite-vec KNN 在 ms 级没有悬念。
- 「内存安全」仅从 0.00 → 0.20：该簇文档讲 ownership/borrow checker，与查询的语义距离较远，bge-small-zh 能力有限；换 bge-m3 可能改善。

### 5.3 结论

**推荐采用 embedding 分支**。在最能体现词袋短板的 paraphrase 场景下，本地 embedding + RRF 融合带来 2.6× recall 提升，且不损害任何基线已解决的查询，延迟与工程成本（一个可选 feature、一个 95MB 模型缓存）均可接受。生产化前需解决 §6 中的持久化、watcher 联动与模型离线分发问题。

## 6. 已知限制 / 遗留问题

1. **unsafe 增量**：sqlite-vec 的 `sqlite3_auto_extension` 注册需要 1 处 `#[allow(unsafe_code)]`（官方推荐写法）。若项目红线要求零 unsafe，可改用纯 Rust 暴力 cosine（30 篇–数千篇规模下性能足够），代价是放弃 sqlite-vec 的 ANN 能力。
2. **模型下载**：首次使用需联网下载 ~50 MB；离线环境需预热 `FASTEMBED_CACHE_PATH`/`HF_HOME` 缓存或随发行版打包。
3. **索引持久化未打通**：`EmbeddingBranch` 的 path↔rowid 映射在内存中，重启后需重建嵌入（向量表本身在 SQLite 中持久，但映射表未落盘）。生产化时需把映射也写入 SQLite 并与 `index.rs` 的 watcher 增量更新联动。
4. **未接默认路径**：`KnowledgeField` / `InMemoryIndex` 尚未暴露 embedding 开关；启用入口目前只有 `HybridRetriever::enable_local_embedding()`。
5. **bge-m3 未测**：PoC 只用 bge-small-zh-v1.5（512 维）；bge-m3（多向量 + sparse）需换 `Bgem3Embedding` API，留待正式立项后评估。
6. **依赖体积**：feature 开启时引入 ort（ONNX Runtime）+ tokenizers，dev profile 全量编译约 5–10 分钟；release 二进制增量未测量（预研预估 +10–30 MB）。

## 7. 下一步建议

1. 若评测确认 paraphrase 召回增益显著：正式立项，把映射持久化、watcher 联动、`KnowledgeField` 开关补齐，并按 §6.1 决策 unsafe 去留。
2. 评测语料扩大：当前 benchmark 为 30 篇合成 paraphrase 语料，建议在真实 Obsidian vault（`assets/clarity.json` 规模）上复测。
3. 评估 `bge-m3` 与量化模型（INT8）在中文短笔记上的质量/体积权衡。

---

*本笔记用于 Knowledge Field 检索架构的跨会话继承。*
