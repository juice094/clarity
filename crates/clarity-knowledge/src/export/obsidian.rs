//! Obsidian 单向导出 PoC。
//!
//! 把 `KnowledgeField` 中的文件节点与 tag 节点投影为一个只读的 Obsidian vault。
//! Clarity 是真相源；导出的 vault 仅供人类阅读，修改不会写回。
//!
//! 已知阻塞 / 未决设计点（按发现顺序）：
//!
//! 1. `KnowledgeField` 不暴露内部 retriever，因此无法从 field 中读回原始
//!    `ExtractedDocument`。解决方式：在 `field.rs` 新增 `get_document` 公共方法
//!    （向后兼容）。
//! 2. `index_directory` 存储的是绝对路径，而导出期望的是相对于 vault 根的路径。
//!    `ObsidianExporter` 通过构造函数传入 `source_root`，把可 strip 前缀的绝对
//!    路径转成相对路径；无法 strip 时回退到原路径。这是 PoC 行为，后续若
//!    `KnowledgeField` 自己记住来源根目录，可删掉此参数。
//! 3. tag 节点文件放在 `tags/<tag>.md`，链接使用相对 wikilink。若原始文件本身
//!    就位于 `tags/` 下，相对链接会冗余地上溯再返回；PoC 未特殊处理，因为 tag
//!    节点链接只指向文件节点。
//! 4. 文件名清理只处理 Windows 禁止字符 `\ / : * ? " < > |`，未处理保留名、
//!    末尾空格/句点等边界情况。
//! 5. 当前只导出 `NodeKind::File` 与 `NodeKind::Tag`；heading、block、session、
//!    message、attachment 节点被忽略。

use crate::error::{KnowledgeError, Result};
use crate::extract::{ExtractedDocument, split_frontmatter};
use crate::field::KnowledgeField;
use crate::graph::NodeKind;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 把 `KnowledgeField` 导出为只读 Obsidian vault。
#[derive(Debug)]
pub struct ObsidianExporter {
    field: KnowledgeField,
    source_root: PathBuf,
}

impl ObsidianExporter {
    /// 创建 exporter。
    ///
    /// `source_root` 用于把 `KnowledgeField` 中可能存储的绝对路径还原为 vault 内
    /// 相对路径。若文档路径不在 `source_root` 下，则回退到原路径。
    pub fn new(field: KnowledgeField, source_root: impl Into<PathBuf>) -> Self {
        Self {
            field,
            source_root: source_root.into(),
        }
    }

    /// 把 field 内容写入 `vault_path`，返回写入的 Markdown 文件数量。
    pub fn export(&self, vault_path: &Path) -> Result<usize> {
        std::fs::create_dir_all(vault_path)?;

        let graph = self.field.graph();
        let mut written = 0usize;
        // node id (原始路径) -> 导出的相对路径
        let mut file_rel_paths: HashMap<String, String> = HashMap::new();

        // 第一遍：导出文件节点。
        for node in graph.nodes().filter(|n| n.kind == NodeKind::File) {
            let Some(doc) = self.field.get_document(Path::new(&node.id.0)) else {
                continue;
            };

            let rel_path = make_relative_path(&doc.path, &self.source_root);
            let safe_rel_path = sanitize_path(&rel_path);
            let out_path = vault_path.join(&safe_rel_path);

            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let content = serialize_file(&doc, &safe_rel_path)?;
            std::fs::write(&out_path, content)?;
            written += 1;
            file_rel_paths.insert(node.id.0.clone(), safe_rel_path);
        }

        // 第二遍：导出 tag 节点。
        let tags_dir = vault_path.join("tags");
        std::fs::create_dir_all(&tags_dir)?;

        for node in graph.nodes().filter(|n| n.kind == NodeKind::Tag) {
            let tag_name = node.label.clone();
            let tag_file_name = sanitize_filename(&tag_name);
            let tag_path = tags_dir.join(format!("{tag_file_name}.md"));

            let mut links: Vec<String> = Vec::new();
            if let Some(backlinks) = graph.backlinks(&node.id) {
                for file_node in backlinks.iter().filter(|n| n.kind == NodeKind::File) {
                    if let Some(rel) = file_rel_paths.get(&file_node.id.0) {
                        links.push(relative_wikilink("tags", rel));
                    }
                }
            }

            let content = serialize_tag(&tag_name, &links)?;
            std::fs::write(&tag_path, content)?;
            written += 1;
        }

        Ok(written)
    }
}

/// 把路径转成相对于 `source_root` 的 Unix 风格字符串。
fn make_relative_path(path: &Path, source_root: &Path) -> String {
    let rel = path.strip_prefix(source_root).unwrap_or(path);
    rel.to_string_lossy().replace('\\', "/")
}

/// 清理相对路径中的每个路径分量，替换 Windows 禁止字符为 `-`。
fn sanitize_path(rel_path: &str) -> String {
    rel_path
        .split('/')
        .map(sanitize_filename)
        .collect::<Vec<_>>()
        .join("/")
}

/// 替换文件/目录名中的禁止字符。
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) { '-' } else { c })
        .collect()
}

/// 从 `from_dir`（如 `tags`）出发，到 `to_rel_path` 的相对 wikilink。
fn relative_wikilink(from_dir: &str, to_rel_path: &str) -> String {
    let from_depth = from_dir.split('/').filter(|s| !s.is_empty()).count();
    let mut parts: Vec<&str> = Vec::with_capacity(from_depth + to_rel_path.split('/').count());
    parts.extend(std::iter::repeat_n("..", from_depth));
    parts.extend(to_rel_path.split('/'));

    let target = parts.join("/");
    let target_no_ext = target.strip_suffix(".md").unwrap_or(&target);
    format!("[[{target_no_ext}]]")
}

/// 序列化文件节点的 Markdown：合并 frontmatter 后追加原始正文。
fn serialize_file(doc: &ExtractedDocument, rel_path: &str) -> Result<String> {
    let mut frontmatter = doc.frontmatter.clone();

    match frontmatter.as_object_mut() {
        Some(obj) => {
            obj.insert(
                "clarity_id".to_string(),
                Value::String(rel_path.to_string()),
            );
            obj.insert("type".to_string(), Value::String("file".to_string()));
            if !obj.contains_key("source") {
                obj.insert(
                    "source".to_string(),
                    Value::String("clarity-export-v1".to_string()),
                );
            }

            let mut tags: Vec<String> = doc.tags.clone();
            if let Some(existing) = obj.get("tags").and_then(Value::as_array) {
                for item in existing {
                    if let Some(s) = item.as_str() {
                        if !tags.iter().any(|t| t == s) {
                            tags.push(s.to_string());
                        }
                    }
                }
            }
            tags.sort_unstable();
            tags.dedup();
            if !tags.is_empty() {
                obj.insert(
                    "tags".to_string(),
                    Value::Array(tags.into_iter().map(Value::String).collect()),
                );
            }
        }
        None => {
            let mut obj = serde_json::Map::new();
            obj.insert(
                "clarity_id".to_string(),
                Value::String(rel_path.to_string()),
            );
            obj.insert("type".to_string(), Value::String("file".to_string()));
            obj.insert(
                "source".to_string(),
                Value::String("clarity-export-v1".to_string()),
            );
            if !doc.tags.is_empty() {
                obj.insert(
                    "tags".to_string(),
                    Value::Array(doc.tags.iter().cloned().map(Value::String).collect()),
                );
            }
            frontmatter = Value::Object(obj);
        }
    }

    let yaml = serde_yaml::to_string(&frontmatter)
        .map_err(|e| KnowledgeError::Io(std::io::Error::other(format!("frontmatter yaml: {e}"))))?;
    let (_, body) = split_frontmatter(&doc.content);

    Ok(format!("---\n{yaml}---\n{body}"))
}

/// 序列化 tag 节点的 Markdown：frontmatter + 指向相关文件的链接列表。
fn serialize_tag(tag_name: &str, links: &[String]) -> Result<String> {
    let mut obj = serde_json::Map::new();
    obj.insert("type".to_string(), Value::String("tag".to_string()));
    obj.insert(
        "clarity_id".to_string(),
        Value::String(format!("tag:{tag_name}")),
    );

    let yaml = serde_yaml::to_string(&Value::Object(obj))
        .map_err(|e| KnowledgeError::Io(std::io::Error::other(format!("tag yaml: {e}"))))?;

    let mut body = format!("# Tag: {tag_name}\n\n");
    for link in links {
        body.push_str("- ");
        body.push_str(link);
        body.push('\n');
    }

    Ok(format!("---\n{yaml}---\n{body}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::{ExtractedDocument, MarkdownExtractor};
    use crate::field::{FieldConfig, KnowledgeField};
    use std::io::Write;

    fn sample_doc(path: &str) -> ExtractedDocument {
        let content = r#"---
title: Sample Note
author: clarity
---
# Sample Note

This note links to [[Another Note]] and uses #rust.
"#;
        let extractor = MarkdownExtractor::new().unwrap();
        extractor.extract(Path::new(path), content).unwrap()
    }

    #[test]
    fn export_creates_file_and_tag_nodes() {
        let dir = tempfile::tempdir().unwrap();
        let vault_out = tempfile::tempdir().unwrap();

        let field = KnowledgeField::new(FieldConfig::default());
        let doc = sample_doc(&dir.path().join("notes/sample.md").to_string_lossy());
        field.index_document(doc).unwrap();

        let exporter = ObsidianExporter::new(field, dir.path());
        let count = exporter.export(vault_out.path()).unwrap();
        assert_eq!(count, 2, "one file + one tag");

        let file_path = vault_out.path().join("notes/sample.md");
        assert!(file_path.exists(), "exported file should exist");
        let file_content = std::fs::read_to_string(&file_path).unwrap();
        assert!(file_content.contains("clarity_id:"));
        assert!(file_content.contains("type: file"));
        assert!(file_content.contains("source: clarity-export-v1"));
        assert!(file_content.contains("rust"));
        assert!(file_content.contains("[[Another Note]]"));

        let tag_path = vault_out.path().join("tags/rust.md");
        assert!(tag_path.exists(), "exported tag file should exist");
        let tag_content = std::fs::read_to_string(&tag_path).unwrap();
        assert!(tag_content.contains("type: tag"));
        assert!(tag_content.contains("[[../notes/sample]]"));
    }

    #[test]
    fn export_preserves_existing_source_frontmatter() {
        let field = KnowledgeField::new(FieldConfig::default());
        let content = "---\nsource: manual\n---\n# Body\n";
        let extractor = MarkdownExtractor::new().unwrap();
        let doc = extractor.extract(Path::new("keep.md"), content).unwrap();
        field.index_document(doc).unwrap();

        let exporter = ObsidianExporter::new(field, Path::new(""));
        let out = tempfile::tempdir().unwrap();
        exporter.export(out.path()).unwrap();

        let exported = std::fs::read_to_string(out.path().join("keep.md")).unwrap();
        assert!(exported.contains("source: manual"));
        assert!(!exported.contains("source: clarity-export-v1"));
    }

    #[test]
    fn export_merges_frontmatter_tags() {
        let field = KnowledgeField::new(FieldConfig::default());
        let content = "---\ntags: [rust, ai]\n---\n# Body\n#ml\n";
        let extractor = MarkdownExtractor::new().unwrap();
        let doc = extractor.extract(Path::new("merge.md"), content).unwrap();
        field.index_document(doc).unwrap();

        let exporter = ObsidianExporter::new(field, Path::new(""));
        let out = tempfile::tempdir().unwrap();
        exporter.export(out.path()).unwrap();

        let exported = std::fs::read_to_string(out.path().join("merge.md")).unwrap();
        assert!(exported.contains("- ai"));
        assert!(exported.contains("- ml"));
        assert!(exported.contains("- rust"));
    }

    #[test]
    fn sanitize_replaces_forbidden_characters() {
        assert_eq!(
            sanitize_filename("a:b*c?d\"e<f>g|h\\i/j"),
            "a-b-c-d-e-f-g-h-i-j"
        );
    }

    #[test]
    fn relative_wikilink_from_tags() {
        assert_eq!(
            relative_wikilink("tags", "notes/sample.md"),
            "[[../notes/sample]]"
        );
        assert_eq!(relative_wikilink("tags", "root.md"), "[[../root]]");
    }

    #[test]
    fn export_sanitizes_file_names() {
        let field = KnowledgeField::new(FieldConfig::default());
        let content = "# Bad\n";
        let extractor = MarkdownExtractor::new().unwrap();
        let doc = extractor
            .extract(Path::new("bad:name.md"), content)
            .unwrap();
        field.index_document(doc).unwrap();

        let exporter = ObsidianExporter::new(field, Path::new(""));
        let out = tempfile::tempdir().unwrap();
        exporter.export(out.path()).unwrap();

        assert!(out.path().join("bad-name.md").exists());
    }

    #[test]
    fn export_indexes_real_directory() {
        let dir = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();

        let a = dir.path().join("a.md");
        let mut f = std::fs::File::create(&a).unwrap();
        writeln!(f, "# A\nTag #rust and link [[b]].").unwrap();

        let b = dir.path().join("b.md");
        let mut f = std::fs::File::create(&b).unwrap();
        writeln!(f, "# B\nAlso #rust.").unwrap();

        let field = KnowledgeField::new(FieldConfig::default());
        let indexed = field.index_directory(dir.path()).unwrap();
        assert_eq!(indexed, 2);

        let exporter = ObsidianExporter::new(field, dir.path());
        let count = exporter.export(out.path()).unwrap();
        assert_eq!(count, 3); // 2 files + 1 tag

        assert!(out.path().join("a.md").exists());
        assert!(out.path().join("b.md").exists());
        assert!(out.path().join("tags/rust.md").exists());

        let tag = std::fs::read_to_string(out.path().join("tags/rust.md")).unwrap();
        assert!(tag.contains("[[../a]]"));
        assert!(tag.contains("[[../b]]"));
    }
}
