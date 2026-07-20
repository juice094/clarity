//! Benchmark: index and search a synthetic large vault.
//!
//! Usage:
//!   cargo run -p clarity-knowledge --example vault_benchmark -- [FILE_COUNT]
//!
//! Defaults to 1000 markdown files.

use clarity_knowledge::search::SearchQuery;
use clarity_knowledge::{FieldConfig, KnowledgeField};
use std::io;
use std::time::Instant;

const DEFAULT_COUNT: usize = 1000;

fn main() {
    if let Err(e) = run() {
        eprintln!("benchmark failed: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let count = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_COUNT);

    let dir = tempfile::tempdir()?;
    println!("Generating {} markdown files in {:?}...", count, dir.path());
    let start = Instant::now();
    generate_vault(dir.path(), count)?;
    println!("  generated in {:?}", start.elapsed());

    let field = KnowledgeField::new(FieldConfig::default());

    println!("\nIndexing...");
    let start = Instant::now();
    let indexed = field.index_directory(dir.path())?;
    let index_elapsed = start.elapsed();
    println!("  indexed: {} files", indexed);
    println!(
        "  elapsed: {:?} ({:.1} files/s)",
        index_elapsed,
        indexed as f64 / index_elapsed.as_secs_f64().max(0.001)
    );

    println!("\nGraph stats:");
    println!(
        "  total file nodes: {}",
        field.top_activated(usize::MAX).len()
    );

    let queries = ["Rust", "map", "大数据", "笔记", "file:note", "tag:course"];
    println!("\nSearch latency (cold -> warm):");
    for q in queries {
        let query = SearchQuery::new(q).with_limit(10);
        let cold_start = Instant::now();
        let cold_results = field.search(&query)?;
        let cold_elapsed = cold_start.elapsed();

        let warm_start = Instant::now();
        let warm_results = field.search(&query)?;
        let warm_elapsed = warm_start.elapsed();

        println!(
            "  query '{}': cold {:?} ({} results) -> warm {:?} ({} results)",
            q,
            cold_elapsed,
            cold_results.len(),
            warm_elapsed,
            warm_results.len()
        );
    }

    Ok(())
}

fn generate_vault(root: &std::path::Path, count: usize) -> io::Result<()> {
    let topics = [
        ("Rust", "Rust 是一种系统编程语言，注重安全与性能。"),
        ("Python", "Python 适合数据科学与快速原型。"),
        ("大数据", "大数据技术包括 Hadoop、Spark 与 Flink。"),
        ("农业", "农业大数据应用在作物监测与产量预测。"),
        ("笔记", "笔记是知识管理的基础单元。"),
        ("Map", "MapReduce 是分布式计算的经典模型。"),
        ("AI", "人工智能正在改变软件开发方式。"),
        ("Obsidian", "Obsidian 使用本地 Markdown 文件管理知识。"),
    ];

    for i in 0..count {
        let topic = &topics[i % topics.len()];
        let file_name = format!("note_{:04}.md", i);
        let path = root.join(&file_name);

        let content = format!(
            "---\ntitle: {}\ntags: [course, {}]\n---\n\n# {}\n\n{}{}\n\n## 要点\n\n- 要点一：关于 {} 的说明。\n- 要点二：相关实践与案例。\n- 要点三：进一步阅读建议。\n\n参见 [[{}]] 与 [[{}]]。\n",
            topic.0,
            if i % 3 == 0 { "exam" } else { "review" },
            topic.0,
            topic.1,
            generate_paragraph(i),
            topic.0,
            topics[(i + 1) % topics.len()].0,
            topics[(i + 2) % topics.len()].0,
        );

        std::fs::write(&path, content)?;
    }

    Ok(())
}

fn generate_paragraph(i: usize) -> String {
    let sentences = [
        "这是一段补充说明。",
        "在实际项目中，需要根据场景权衡取舍。",
        "相关概念可以参考官方文档与社区讨论。",
        "测试与验证是确保质量的关键步骤。",
        "持续迭代能够逐步完善知识体系。",
    ];
    sentences[i % sentences.len()].to_string()
}
