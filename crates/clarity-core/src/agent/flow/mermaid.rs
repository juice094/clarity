//! Mermaid flowchart parser — minimal subset.
//!
//! Supports:
//! - `flowchart TD` / `graph TD` headers
//! - `%%` comments
//! - Nodes: `ID[label]`, `ID([label])`, `ID{label}`
//! - Edges: `-->`, `-->|label|`, `-- label -->`
//! - Auto-typing: nodes labeled "begin"/"end" become Begin/End
//! - Decision inference: nodes with >1 outgoing edge become Decision

use super::{Flow, FlowEdge, FlowError, FlowNode, FlowNodeKind};

/// Parse a minimal Mermaid flowchart into a `Flow`.
pub fn parse_mermaid_flowchart(input: &str) -> Result<Flow, FlowError> {
    let mut flow = Flow::default();
    let mut node_labels: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut edges: Vec<(String, String, Option<String>)> = Vec::new();
    let mut first_node_id: Option<String> = None;

    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }
        if line.starts_with("flowchart") || line.starts_with("graph") {
            continue;
        }
        if line.starts_with("classDef")
            || line.starts_with("style")
            || line.starts_with("linkStyle")
            || line.starts_with("subgraph")
        {
            continue;
        }

        // Extract all node definitions from this line (including inline on edges)
        for (id, label) in extract_all_nodes(line) {
            if first_node_id.is_none() {
                first_node_id = Some(id.clone());
            }
            node_labels.entry(id).or_insert(label);
        }

        // Extract edge
        if let Some((src, dst, label)) = parse_edge(line) {
            if first_node_id.is_none() {
                first_node_id = Some(src.clone());
            }
            edges.push((src, dst, label));
        }
    }

    // Build nodes from explicit definitions
    for (id, label) in &node_labels {
        let kind = infer_node_kind(id, label);
        flow.nodes.insert(
            id.clone(),
            FlowNode {
                id: id.clone(),
                label: label.clone(),
                kind,
            },
        );
    }

    // Ensure edge endpoints exist as nodes
    for (src, dst, _) in &edges {
        if !flow.nodes.contains_key(src) {
            let label = src.clone();
            let kind = infer_node_kind(src, &label);
            flow.nodes.insert(
                src.clone(),
                FlowNode {
                    id: src.clone(),
                    label,
                    kind,
                },
            );
        }
        if !flow.nodes.contains_key(dst) {
            let label = dst.clone();
            let kind = infer_node_kind(dst, &label);
            flow.nodes.insert(
                dst.clone(),
                FlowNode {
                    id: dst.clone(),
                    label,
                    kind,
                },
            );
        }
    }

    // Build edges
    for (src, dst, label) in edges {
        let edge = FlowEdge {
            src: src.clone(),
            dst,
            label,
        };
        flow.outgoing.entry(src).or_default().push(edge);
    }

    // Infer decision nodes
    for (id, edges) in &flow.outgoing {
        if edges.len() > 1 {
            if let Some(node) = flow.nodes.get_mut(id) {
                node.kind = FlowNodeKind::Decision;
            }
        }
    }

    // Find begin and end
    let mut begin_id = None;
    let mut end_id = None;
    for node in flow.nodes.values() {
        if node.kind == FlowNodeKind::Begin {
            begin_id = Some(node.id.clone());
        }
        if node.kind == FlowNodeKind::End {
            end_id = Some(node.id.clone());
        }
    }

    // Fallback: auto-assign begin/end if missing
    if begin_id.is_none() && !flow.nodes.is_empty() {
        let candidate = first_node_id
            .and_then(|id| flow.nodes.get(&id).map(|n| n.id.clone()))
            .or_else(|| flow.nodes.values().next().map(|n| n.id.clone()));
        if let Some(id) = candidate {
            if let Some(node) = flow.nodes.get_mut(&id) {
                node.kind = FlowNodeKind::Begin;
            }
            begin_id = Some(id);
        }
    }
    if end_id.is_none() && !flow.nodes.is_empty() {
        let sink = flow
            .nodes
            .keys()
            .find(|id| !flow.outgoing.contains_key(*id))
            .cloned();
        if let Some(sink_id) = sink {
            if let Some(node) = flow.nodes.get_mut(&sink_id) {
                node.kind = FlowNodeKind::End;
            }
            end_id = Some(sink_id);
        } else if let Some(id) = begin_id.clone() {
            // No sink found; use begin as end (degenerate flow)
            end_id = Some(id);
        }
    }

    flow.begin_id = begin_id.unwrap_or_default();
    flow.end_id = end_id.unwrap_or_default();

    super::validate_flow(&flow)?;
    Ok(flow)
}

fn parse_edge(line: &str) -> Option<(String, String, Option<String>)> {
    // Find the last "-->" which is the actual arrow
    if let Some(arrow_idx) = line.rfind("-->") {
        let left_raw = line[..arrow_idx].trim();
        let after_arrow = line[arrow_idx + 3..].trim_start();

        // Extract dst (may have inline shapes like B[label])
        let dst = extract_node_id(after_arrow);

        // Check for |label| after -->
        let mut label: Option<String> = None;
        let dst_clean = if after_arrow.starts_with('|') {
            let after_pipe = &after_arrow[1..];
            if let Some(pipe_end) = after_pipe.find('|') {
                label = Some(after_pipe[..pipe_end].to_string());
                extract_node_id(after_pipe[pipe_end + 1..].trim())
            } else {
                dst
            }
        } else {
            dst
        };

        // Extract src from left side, handling -- label --> pattern
        let src = if let Some(dash_idx) = left_raw.rfind(" -- ") {
            let maybe_label = left_raw[dash_idx + 4..].trim();
            let src_raw = left_raw[..dash_idx].trim();
            if label.is_none() && !maybe_label.is_empty() {
                label = Some(maybe_label.to_string());
            }
            extract_node_id(src_raw)
        } else {
            extract_node_id(left_raw)
        };

        if !src.is_empty() && !dst_clean.is_empty() {
            return Some((src, dst_clean, label));
        }
    }

    None
}

fn extract_node_id(s: &str) -> String {
    let s = s.trim();
    // Strip shape wrappers — check longest patterns first
    if let Some(paren) = s.find("([") {
        return s[..paren].trim().to_string();
    }
    if let Some(bracket) = s.find('[') {
        return s[..bracket].trim().to_string();
    }
    if let Some(brace) = s.find('{') {
        return s[..brace].trim().to_string();
    }
    s.to_string()
}

/// Extract all node definitions from a line (including inline on edges).
fn extract_all_nodes(line: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < line.len() {
        let rest = &line[i..];
        if rest.starts_with("[\"") {
            // Find closing "\"]"
            if let Some(close_quote) = rest[2..].find("\"]") {
                let id = line[..i].trim();
                let label = &rest[2..2 + close_quote];
                if is_valid_node_id(id) {
                    result.push((id.to_string(), label.to_string()));
                }
                i += close_quote + 4;
                continue;
            }
        } else if rest.starts_with('[') {
            if let Some(end) = rest[1..].find(']') {
                let id = line[..i].trim();
                let label = &rest[1..=end];
                if is_valid_node_id(id) {
                    result.push((id.to_string(), label.to_string()));
                }
                i += end + 2;
                continue;
            }
        } else if rest.starts_with("([") {
            if let Some(end) = rest[2..].find("])") {
                let id = line[..i].trim();
                let label = &rest[2..2 + end];
                if is_valid_node_id(id) {
                    result.push((id.to_string(), label.to_string()));
                }
                i += end + 4;
                continue;
            }
        } else if rest.starts_with('{') {
            if let Some(end) = rest[1..].find('}') {
                let id = line[..i].trim();
                let label = &rest[1..=end];
                if is_valid_node_id(id) {
                    result.push((id.to_string(), label.to_string()));
                }
                i += end + 2;
                continue;
            }
        }
        i += 1;
    }
    result
}

fn is_valid_node_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_alphanumeric() || c == '_')
}

fn infer_node_kind(id: &str, label: &str) -> FlowNodeKind {
    let lower_label = label.to_lowercase();
    let lower_id = id.to_lowercase();
    if lower_label == "begin"
        || lower_id == "begin"
        || lower_label == "start"
        || lower_id == "start"
    {
        FlowNodeKind::Begin
    } else if lower_label == "end"
        || lower_id == "end"
        || lower_label == "stop"
        || lower_id == "stop"
    {
        FlowNodeKind::End
    } else {
        FlowNodeKind::Task
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_flow() {
        let input = r#"
flowchart TD
    start[Begin] --> step1[Analyze code]
    step1 --> step2[Run tests]
    step2 --> end[Done]
"#;
        let flow = parse_mermaid_flowchart(input).unwrap();
        assert_eq!(flow.begin_id, "start");
        assert_eq!(flow.end_id, "end");
        assert_eq!(flow.nodes.len(), 4);
        assert!(flow.outgoing.contains_key("step1"));
    }

    #[test]
    fn test_parse_decision_flow() {
        let input = r#"
flowchart TD
    start[Begin] --> decide{Tests pass?}
    decide -->|yes| end[Done]
    decide -->|no| fix[Fix bugs]
    fix --> decide
"#;
        let flow = parse_mermaid_flowchart(input).unwrap();
        assert_eq!(
            flow.nodes.get("decide").unwrap().kind,
            FlowNodeKind::Decision
        );
        let edges = flow.outgoing.get("decide").unwrap();
        assert_eq!(edges.len(), 2);
    }

    #[test]
    fn test_parse_quoted_label() {
        let input = r#"flowchart TD
    start["Begin step"] --> end["End step"]"#;
        let flow = parse_mermaid_flowchart(input).unwrap();
        assert_eq!(flow.nodes.get("start").unwrap().label, "Begin step");
    }

    #[test]
    fn test_parse_inline_shapes() {
        let input = r#"flowchart TD
    start([Begin]) --> step1[Analyze]
    step1 --> end{Stop}"#;
        let flow = parse_mermaid_flowchart(input).unwrap();
        assert_eq!(flow.nodes.get("start").unwrap().kind, FlowNodeKind::Begin);
        assert_eq!(flow.nodes.get("end").unwrap().kind, FlowNodeKind::End);
    }

    #[test]
    fn test_parse_edge_label_inline() {
        let input = r#"flowchart TD
    start[Begin] -- check --> end[Done]"#;
        let flow = parse_mermaid_flowchart(input).unwrap();
        let edge = &flow.outgoing["start"][0];
        assert_eq!(edge.label, Some("check".to_string()));
    }
}
