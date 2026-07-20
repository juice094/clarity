# clarity-knowledge

本地知识索引与 AI 原生交互：把文件系统当作 Agent 可以查询、导航和激活的知识库。

## 职责

- **文件系统索引** — 增量扫描 Markdown、YAML frontmatter、wikilink、标签与附件，构建可搜索的知识图
- **混合检索** — 结合 BM25 关键词、向量相似度和图结构进行多路召回
- **知识图** — 内存中的有向图：`File` / `Heading` / `Block` / `Tag` / `Attachment` / `Session` / `Message` 节点与 `LinksTo` / `TaggedWith` / `Contains` / `Embeds` 边
- **知识场** — 在知识图之上引入激活动力学：查询注入能量、沿边传播、横向抑制、时间衰减与休眠，使相关区域动态浮现
- **文件监听** — 通过 `notify` 监听 vault 变更并增量更新索引

## 关键类型

- `KnowledgeIndex` — 面向 vault 的增量索引与搜索入口
- `HybridRetriever` — BM25 + 向量的混合检索器
- `KnowledgeGraph` — 内存知识图，支持节点/边操作、传播激活、横向抑制、休眠
- `KnowledgeField` — 封装 `KnowledgeGraph` + `HybridRetriever`，提供 `index_document` / `search` / `inject_activation` / `decay` / `top_activated`
- `FieldConfig` — 激活动力学参数：传播迭代数、半衰期、抑制强度、休眠阈值等
- `MarkdownExtractor` — 解析 Markdown 中的标题、wikilink、标签、frontmatter
- `VaultConfig` — 兼容 Obsidian 风格的 vault 配置（`app.json`、`appearance.json`、`daily-notes.json` 等）

## 测试

```bash
cargo test -p clarity-knowledge --lib
```

## 边界与稳定性

- **Stability tier**: Experimental → Stable-ing
  - API 在 0.4.x 期间可能随知识场迭代微调
- **MSRV**: 1.85（跟随 workspace）
- **反向依赖禁止**:
  - 不得依赖任何 frontend/network crate
  - 不依赖 Obsidian、Syncthing 或外部服务；仅依赖本地文件与开放约定
- **Library/binary classification**:
  - Library: designed for `use` by other crates
