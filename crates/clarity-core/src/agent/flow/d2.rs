//! D2 diagram parser — minimal subset.
//!
//! Supports:
//! - `direction: down` / `direction: right` headers
//! - `#` comments
//! - Nodes: `id: label`
//! - Shapes: `id.shape: oval` / `diamond` / `rectangle` / `circle`
//! - Edges: `src -> dst`, `src -> dst: label`
//! - Containers (ignored in this subset)
//! - Auto-typing: nodes labeled "begin"/"end" become Begin/End
//! - Decision inference: nodes with >1 outgoing edge become Decision

use super::{Flow, FlowEdge, FlowError, FlowNode, FlowNodeKind};
use std::collections::HashMap;

/// Parse a minimal D2 diagram into a `Flow`.
pub fn parse_d2_diagram(input: &str) -> Result<Flow, FlowError> {
    let mut flow = Flow::default();
    let mut node_labels: HashMap<String, String> = HashMap::new();
    let mut node_shapes: HashMap<String, FlowNodeKind> = HashMap::new();
    let mut edges: Vec<(String, String, Option<String>)> = Vec::new();
    let mut first_node_id: Option<String> = None;

    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Direction declaration
        if line.starts_with("direction:") {
            continue;
        }

        // Shape annotation: `id.shape: oval`
        if let Some((id, kind)) = parse_shape_annotation(line) {
            node_shapes.insert(id, kind);
            continue;
        }

        // Edge: `src -> dst` or `src -> dst: label`
        if let Some((src, dst, label)) = parse_d2_edge(line) {
            if first_node_id.is_none() {
                first_node_id = Some(src.clone());
            }
            edges.push((src, dst, label));
            continue;
        }

        // Node definition: `id: label` (but not a shape annotation or container start)
        if let Some((id, label)) = parse_node_definition(line) {
            if first_node_id.is_none() {
                first_node_id = Some(id.clone());
            }
            node_labels.entry(id).or_insert(label);
        }
    }

    // Build nodes from explicit definitions
    for (id, label) in &node_labels {
        let kind = node_shapes
            .get(id)
            .copied()
            .unwrap_or_else(|| infer_node_kind(id, label));
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
            let kind = node_shapes
                .get(src)
                .copied()
                .unwrap_or_else(|| infer_node_kind(src, &label));
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
            let kind = node_shapes
                .get(dst)
                .copied()
                .unwrap_or_else(|| infer_node_kind(dst, &label));
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

    // Collect all candidates so we can disambiguate deterministically
    let mut begin_candidates = Vec::new();
    let mut end_candidates = Vec::new();
    for node in flow.nodes.values() {
        if node.kind == FlowNodeKind::Begin {
            begin_candidates.push(node.id.clone());
        }
        if node.kind == FlowNodeKind::End {
            end_candidates.push(node.id.clone());
        }
    }

    // Prefer first_node_id for begin; otherwise take first candidate
    if !begin_candidates.is_empty() {
        begin_id = begin_candidates
            .iter()
            .find(|id| Some(*id) == first_node_id.as_ref())
            .cloned()
            .or_else(|| begin_candidates.into_iter().next());
    }
    if !end_candidates.is_empty() {
        end_id = end_candidates.into_iter().next();
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
            end_id = Some(id);
        }
    }

    flow.begin_id = begin_id.unwrap_or_default();
    flow.end_id = end_id.unwrap_or_default();

    super::validate_flow(&flow)?;
    Ok(flow)
}

/// Parse `id: label` node definition.
///
/// Returns `None` if the line looks like a shape annotation or container.
fn parse_node_definition(line: &str) -> Option<(String, String)> {
    // Skip shape annotations and edges
    if line.contains("->") || line.contains(".shape:") {
        return None;
    }

    let mut parts = line.splitn(2, ':');
    let id = parts.next()?.trim();
    let label = parts.next()?.trim();

    if id.is_empty() || label.is_empty() {
        return None;
    }

    // Skip container blocks (label starts with `{`)
    if label.starts_with('{') {
        return None;
    }

    Some((id.to_string(), label.to_string()))
}

/// Parse `id.shape: oval` shape annotation.
fn parse_shape_annotation(line: &str) -> Option<(String, FlowNodeKind)> {
    let mut parts = line.splitn(2, ".shape:");
    let id = parts.next()?.trim();
    let shape = parts.next()?.trim();

    if id.is_empty() || shape.is_empty() {
        return None;
    }

    let kind = match shape {
        "oval" | "circle" | "ellipse" => FlowNodeKind::Begin,
        "diamond" => FlowNodeKind::Decision,
        _ => FlowNodeKind::Task,
    };

    Some((id.to_string(), kind))
}

/// Parse D2 edge: `src -> dst` or `src -> dst: label`.
fn parse_d2_edge(line: &str) -> Option<(String, String, Option<String>)> {
    if !line.contains("->") {
        return None;
    }

    // Split by `->` (first occurrence for src, rest for dst)
    let arrow_idx = line.find("->")?;
    let src = line[..arrow_idx].trim().to_string();
    let after_arrow = line[arrow_idx + 2..].trim_start();

    if src.is_empty() {
        return None;
    }

    // Check for `: label` after dst
    let (dst, label) = if let Some(colon_idx) = after_arrow.find(':') {
        let dst = after_arrow[..colon_idx].trim().to_string();
        let label = after_arrow[colon_idx + 1..].trim().to_string();
        (dst, Some(label))
    } else {
        (after_arrow.to_string(), None)
    };

    if dst.is_empty() {
        return None;
    }

    Some((src, dst, label))
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
    fn test_parse_simple_d2() {
        let input = r#"
direction: down

start: Begin
step1: Analyze code
step2: Run tests
end: Done

start -> step1
step1 -> step2
step2 -> end
"#;
        let flow = parse_d2_diagram(input).unwrap();
        assert_eq!(flow.begin_id, "start");
        assert_eq!(flow.end_id, "end");
        assert_eq!(flow.nodes.len(), 4);
        assert!(flow.outgoing.contains_key("step1"));
    }

    #[test]
    fn test_parse_decision_d2() {
        let input = r#"
direction: down

start: Begin
decide: Tests pass?
end: Done
fix: Fix bugs

start -> decide
decide -> end: yes
decide -> fix: no
fix -> decide
"#;
        let flow = parse_d2_diagram(input).unwrap();
        assert_eq!(
            flow.nodes.get("decide").unwrap().kind,
            FlowNodeKind::Decision
        );
        let edges = flow.outgoing.get("decide").unwrap();
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].label, Some("yes".to_string()));
        assert_eq!(edges[1].label, Some("no".to_string()));
    }

    #[test]
    fn test_parse_shape_annotation() {
        let input = r#"
start: Begin
start.shape: oval
decide: Tests pass?
decide.shape: diamond
end: Done
end.shape: circle
fix: Fix bugs

start -> decide
decide -> end: yes
decide -> fix: no
fix -> decide
"#;
        let flow = parse_d2_diagram(input).unwrap();
        assert_eq!(flow.nodes.get("start").unwrap().kind, FlowNodeKind::Begin);
        assert_eq!(
            flow.nodes.get("decide").unwrap().kind,
            FlowNodeKind::Decision
        );
        assert_eq!(flow.nodes.get("end").unwrap().kind, FlowNodeKind::End);
    }

    #[test]
    fn test_parse_edge_label() {
        let input = r#"
start: Begin
end: Done

start -> end: check
"#;
        let flow = parse_d2_diagram(input).unwrap();
        let edge = &flow.outgoing["start"][0];
        assert_eq!(edge.label, Some("check".to_string()));
    }

    #[test]
    fn test_parse_comments_and_empty_lines() {
        let input = r#"
# This is a comment

start: Begin
# Another comment
end: Done

start -> end
"#;
        let flow = parse_d2_diagram(input).unwrap();
        assert_eq!(flow.nodes.len(), 2);
    }

    #[test]
    fn test_parse_implicit_nodes() {
        let input = r#"
start -> step1
step1 -> end
"#;
        let flow = parse_d2_diagram(input).unwrap();
        assert!(flow.nodes.contains_key("start"));
        assert!(flow.nodes.contains_key("step1"));
        assert!(flow.nodes.contains_key("end"));
    }
}
