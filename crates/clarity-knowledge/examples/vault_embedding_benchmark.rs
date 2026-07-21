//! Benchmark: TF-IDF baseline vs +local-embedding RRF fusion (PoC lane B1).
//!
//! Builds one synthetic vault of paraphrase-heavy documents (the query term
//! never appears literally in the target documents), then compares two
//! configurations:
//!
//! 1. baseline — `HybridRetriever` as-is (BM25 + TF-IDF cosine + graph);
//! 2. fused — baseline + local embedding branch (fastembed + sqlite-vec),
//!    fused via Reciprocal Rank Fusion.
//!
//! Metrics per query: recall@5 against the ground-truth topic documents and
//! rank of the first relevant hit, plus cold/warm search latency.
//!
//! Usage:
//!
//! ```text
//! cargo run -p clarity-knowledge --example vault_embedding_benchmark \
//!     --features local-embedding
//! ```
//!
//! The first run downloads the BGE-small-zh-v1.5 ONNX model (~50 MB). If the
//! download fails (offline), the benchmark prints the baseline only and
//! exits successfully with a "NOT MEASURED" note.

use clarity_knowledge::embedding::{Embedder, EmbeddingBranch, FastembedEmbedder};
use clarity_knowledge::{ExtractedDocument, HybridRetriever, KnowledgeGraph, SearchQuery};
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    if let Err(e) = run() {
        eprintln!("benchmark failed: {e}");
        std::process::exit(1);
    }
}

/// One topic cluster: documents talk about the subject without ever naming
/// the query term, so keyword/TF-IDF recall must fail while a semantic
/// embedding can still hit.
struct Topic {
    /// Directory-like path prefix for the cluster's documents.
    slug: &'static str,
    /// Document titles (Chinese/English mixed, no query term).
    titles: [&'static str; 5],
    /// One-line bodies, appended to the title to form document content.
    bodies: [&'static str; 5],
    /// Evaluation queries whose ground truth is this cluster.
    queries: [&'static str; 2],
}

const TOPICS: &[Topic] = &[
    Topic {
        slug: "ai",
        titles: [
            "深度学习入门",
            "神经网络调参笔记",
            "反向传播推导",
            "梯度下降变体对比",
            "Transformer 结构梳理",
        ],
        bodies: [
            "多层感知机与非线性激活函数的基本原理。",
            "学习率、批大小与正则化之间的权衡。",
            "链式法则在计算图上的应用。",
            "SGD、Momentum 与 Adam 的收敛特性。",
            "自注意力机制与位置编码的作用。",
        ],
        queries: ["人工智能", "机器学习"],
    },
    Topic {
        slug: "climate",
        titles: [
            "全球变暖观测记录",
            "温室气体排放清单",
            "极端天气事件分析",
            "碳汇与森林覆盖率",
            "海平面上升数据集",
        ],
        bodies: [
            "近五十年地表均温距平的变化趋势。",
            "二氧化碳与甲烷的主要来源构成。",
            "热浪与强降水事件的频率统计。",
            "植被固碳能力的区域差异。",
            "潮汐站长期监测数据的整理。",
        ],
        queries: ["气候变化", "全球暖化"],
    },
    Topic {
        slug: "fitness",
        titles: [
            "力量训练周期计划",
            "有氧与无氧的搭配",
            "蛋白质摄入记录",
            "柔韧性拉伸指南",
            "恢复与睡眠质量",
        ],
        bodies: [
            "深蹲、硬拉与卧推的渐进超负荷安排。",
            "心率区间与训练目标的对应关系。",
            "每日宏量营养素的估算方法。",
            "动态拉伸与静态拉伸的适用时机。",
            "超量恢复与训练间隔的关系。",
        ],
        queries: ["锻炼身体", "健身计划"],
    },
    Topic {
        slug: "db",
        titles: [
            "B+ 树页分裂分析",
            "LSM-tree 压缩策略",
            "WAL 崩溃恢复流程",
            "查询优化器代价模型",
            "缓冲池淘汰算法",
        ],
        bodies: [
            "聚簇存储中叶节点的有序链表维护。",
            "分层合并与布隆过滤器的读放大控制。",
            "重做日志的 checkpoint 与回放。",
            "统计直方图驱动的基数估计。",
            "LRU-K 与 Clock 的命中率对比。",
        ],
        queries: ["数据库索引", "存储引擎"],
    },
    Topic {
        slug: "cook",
        titles: [
            "意面酱汁配比",
            "低温慢煮时间表",
            "发酵面团管理",
            "香辛料风味轮",
            "高汤吊制要点",
        ],
        bodies: [
            "番茄基底与乳化油脂的平衡。",
            "不同厚度肉类的核心温度曲线。",
            "天然酵种的喂养频率与酸度。",
            "风味化合物的前中后调分类。",
            "清汤与浓汤的原料处理差异。",
        ],
        queries: ["烹饪", "菜谱"],
    },
    Topic {
        slug: "rust",
        titles: [
            "所有权与借用检查",
            "生命周期标注实践",
            "迭代器适配器清单",
            "错误处理模式对比",
            "并发原语选型",
        ],
        bodies: [
            "move 语义与借用规则在编译期的静态检查。",
            "省略规则失效时的显式标注策略。",
            "map、filter 与 fold 的惰性求值链。",
            "Result 与 thiserror 的组合方式。",
            "channel、Mutex 与原子类型的取舍。",
        ],
        queries: ["内存安全", "系统编程语言"],
    },
];

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let documents = build_vault();
    println!(
        "Vault: {} documents across {} paraphrase topic clusters.",
        documents.len(),
        TOPICS.len()
    );

    // --- Baseline retriever ------------------------------------------------
    let mut baseline = HybridRetriever::new();
    for doc in &documents {
        baseline.add_document(doc.clone())?;
    }

    // --- Fused retriever (baseline + local embedding branch) ---------------
    let mut fused = HybridRetriever::new();
    for doc in &documents {
        fused.add_document(doc.clone())?;
    }

    // `_vec_dir` keeps the store file alive until the end of `run`.
    let _vec_dir = tempfile::tempdir()?;
    let embedding_available = match try_build_embedder() {
        Some(embedder) => {
            let store_path = _vec_dir.path().join("vec.db");
            let branch = EmbeddingBranch::new(&store_path, embedder)?;
            fused.enable_local_embedding(branch);
            true
        }
        None => false,
    };

    let graph = KnowledgeGraph::new();
    let limit = 5;

    println!();
    println!(
        "{:<14} {:>7} {:>7} {:>16} | {:>8} {:>8} {:>12}",
        "query", "base@5", "base rk", "base cold/warm", "fused@5", "fused rk", "fused cold"
    );
    println!("{}", "-".repeat(86));

    let mut sum_base = 0.0;
    let mut sum_fused = 0.0;
    let mut n_queries = 0usize;

    for topic in TOPICS {
        let expected: Vec<PathBuf> = (0..5)
            .map(|i| PathBuf::from(format!("{}/note_{i}.md", topic.slug)))
            .collect();
        for q in topic.queries {
            let query = SearchQuery::new(q).with_limit(limit);

            let base_start = Instant::now();
            let base_results = baseline.search(&query, &graph)?;
            let base_cold = base_start.elapsed();
            let warm_start = Instant::now();
            let base_results2 = baseline.search(&query, &graph)?;
            let base_warm = warm_start.elapsed();
            debug_assert_eq!(base_results.len(), base_results2.len());

            let base_recall = recall_at_k(&base_results, &expected, limit);
            let base_rank = first_relevant_rank(&base_results, &expected);

            let (fused_recall, fused_rank, fused_cold_ms) = if embedding_available {
                let cold_start = Instant::now();
                let fused_results = fused.search(&query, &graph)?;
                let fused_cold = cold_start.elapsed();
                (
                    format!("{:.2}", recall_at_k(&fused_results, &expected, limit)),
                    format_rank(first_relevant_rank(&fused_results, &expected)),
                    format!("{:>8.1?}", fused_cold),
                )
            } else {
                ("n/a".to_string(), "n/a".to_string(), "n/a".to_string())
            };

            println!(
                "{:<14} {:>7.2} {:>7} {:>16} | {:>8} {:>8} {:>12}",
                q,
                base_recall,
                format_rank(base_rank),
                format!("{:.0?}/{:.0?}", base_cold, base_warm),
                fused_recall,
                fused_rank,
                fused_cold_ms,
            );

            sum_base += base_recall;
            if embedding_available {
                let fused_results = fused.search(&query, &graph)?;
                sum_fused += recall_at_k(&fused_results, &expected, limit);
            }
            n_queries += 1;
        }
    }

    println!("{}", "-".repeat(80));
    println!(
        "Mean recall@{limit}: baseline {:.2}{}",
        sum_base / n_queries as f64,
        if embedding_available {
            format!("  fused {:.2}", sum_fused / n_queries as f64)
        } else {
            "  fused NOT MEASURED (embedding model unavailable — see above)".to_string()
        }
    );

    if !embedding_available {
        println!();
        println!("RESULT: embedding branch NOT MEASURED (model download/load failed).");
        println!("Baseline numbers above are real; re-run with network access to compare.");
    }

    Ok(())
}

/// Try to load the real fastembed model; return None (and say why) when the
/// model cannot be downloaded or initialized.
fn try_build_embedder() -> Option<Box<dyn Embedder>> {
    println!("Loading fastembed model BGE-small-zh-v1.5 (downloads ~50 MB on first run)...");
    let start = Instant::now();
    match FastembedEmbedder::new(fastembed::EmbeddingModel::BGESmallZHV15, None) {
        Ok(embedder) => {
            println!(
                "  model ready in {:?} (dim {}).",
                start.elapsed(),
                embedder.dim()
            );
            Some(Box::new(embedder))
        }
        Err(e) => {
            println!("  model unavailable: {e}");
            println!("  -> embedding fusion will NOT be measured in this run.");
            None
        }
    }
}

/// Materialize the synthetic vault as in-memory documents (no file I/O).
fn build_vault() -> Vec<ExtractedDocument> {
    let mut docs = Vec::new();
    for topic in TOPICS {
        for i in 0..5 {
            docs.push(ExtractedDocument {
                path: PathBuf::from(format!("{}/note_{i}.md", topic.slug)),
                title: Some(topic.titles[i].to_string()),
                content: format!("{}\n\n{}", topic.titles[i], topic.bodies[i]),
                frontmatter: serde_json::Value::Null,
                links: Vec::new(),
                tags: vec![topic.slug.to_string()],
                headings: Vec::new(),
            });
        }
    }
    docs
}

/// Fraction of the expected documents present in the top-`k` results.
fn recall_at_k(results: &[clarity_knowledge::SearchResult], expected: &[PathBuf], k: usize) -> f64 {
    let hits = results
        .iter()
        .take(k)
        .filter(|r| expected.contains(&r.path))
        .count();
    hits as f64 / expected.len().min(k) as f64
}

/// 1-based rank of the first expected document, or None when absent.
fn first_relevant_rank(
    results: &[clarity_knowledge::SearchResult],
    expected: &[PathBuf],
) -> Option<usize> {
    results
        .iter()
        .position(|r| expected.contains(&r.path))
        .map(|pos| pos + 1)
}

fn format_rank(rank: Option<usize>) -> String {
    rank.map(|r| r.to_string())
        .unwrap_or_else(|| "-".to_string())
}
