# Agent 指引 — clarity-knowledge

## 构建

```bash
cargo build -p clarity-knowledge
```

## 测试

```bash
cargo test -p clarity-knowledge --lib
```

## 关键文件

- `src/lib.rs` — 入口与核心类型重导出
- `src/index.rs` — `KnowledgeIndex`：文件扫描、增量索引、搜索 API
- `src/extract.rs` — `MarkdownExtractor`、wikilink / frontmatter / 标签解析
- `src/graph.rs` — `KnowledgeGraph`、节点/边、激活传播、横向抑制、休眠
- `src/field.rs` — `KnowledgeField`：图 + 检索器的动态知识场封装
- `src/retrieval.rs` — `HybridRetriever`：BM25 + 向量混合检索
- `src/search.rs` — `SearchQuery` / `SearchResult` 类型
- `src/watcher.rs` — `FileWatcher`：文件系统变更监听
- `src/vault_config.rs` — Obsidian 风格 vault 配置解析
- `src/error.rs` — `KnowledgeError` / `Result`
- `examples/scan_vault.rs` — 扫描 vault 的示例程序
- `examples/vault_index_qa.rs` — 真实 vault 索引性能与搜索召回诊断工具

## 约定

- 文件路径统一使用 `Path` / `PathBuf`，跨平台由调用方处理路径分隔符
- 节点 ID 对文件使用 source-relative 路径字符串；标签用 `tag:<name>`；会话/消息用各自命名空间
- 激活值范围 `[0.0, 1.0]`；外部调用 `inject_activation` 时自行 clamp 或依赖内部 clamp
- 休眠节点不参与 `top_activated` 排名，但仍保留在图中
- `KnowledgeField::search` 每次查询都会重置图激活状态，避免前序查询污染当前结果
- 直接命中（BM25 / cosine / 标题匹配）在搜索结果中优先返回，图传播邻居仅填充剩余槽位
- frontmatter YAML 解析失败时跳过 frontmatter、继续索引正文，避免整文件被丢弃
- 中文/CJK 正文通过字符级 tokenizer 支持检索，无需额外分词依赖
- 故意简化的实现用 `// ponytail:` 标注上限与升级路径
