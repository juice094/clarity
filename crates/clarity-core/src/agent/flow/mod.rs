//! Agent Flow — Mermaid flowchart-driven task orchestration.
//!
//! Inspired by Kimi CLI's Agent Flow (KLIP-10). Allows an Agent to execute
//! a structured multi-step workflow defined as a Mermaid flowchart.
//!
//! Each node in the flow becomes one Agent turn. Decision nodes branch
//! based on the LLM's `<choice>...</choice>` output.

use std::collections::HashMap;

/// Kinds of nodes in a flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowNodeKind {
    Begin,
    End,
    Task,
    Decision,
}

/// A single node in a flowchart.
#[derive(Debug, Clone)]
pub struct FlowNode {
    pub id: String,
    pub label: String,
    pub kind: FlowNodeKind,
}

/// A directed edge between two nodes.
#[derive(Debug, Clone)]
pub struct FlowEdge {
    pub src: String,
    pub dst: String,
    pub label: Option<String>,
}

/// A parsed flowchart.
#[derive(Debug, Clone, Default)]
pub struct Flow {
    pub nodes: HashMap<String, FlowNode>,
    pub outgoing: HashMap<String, Vec<FlowEdge>>,
    pub begin_id: String,
    pub end_id: String,
}

/// Errors that can occur during flow validation or execution.
#[derive(Debug, thiserror::Error)]
pub enum FlowError {
    #[error("Flow validation failed: {0}")]
    Validation(String),
    #[error("Node '{0}' not found")]
    MissingNode(String),
    #[error("Node '{0}' has no outgoing edges")]
    NoOutgoingEdges(String),
    #[error("Decision node '{0}' has duplicate edge labels")]
    DuplicateEdgeLabels(String),
    #[error("Max steps reached ({0})")]
    MaxStepsReached(usize),
    #[error("Execution error: {0}")]
    Execution(String),
    #[error("Parse error: {0}")]
    Parse(String),
}

/// Validate a flow for structural correctness.
///
/// Checks:
/// - Exactly one Begin and one End node.
/// - All nodes reachable from Begin.
/// - Decision nodes have unique, non-empty edge labels.
pub fn validate_flow(flow: &Flow) -> Result<(), FlowError> {
    let begins: Vec<_> = flow
        .nodes
        .values()
        .filter(|n| n.kind == FlowNodeKind::Begin)
        .collect();
    let ends: Vec<_> = flow
        .nodes
        .values()
        .filter(|n| n.kind == FlowNodeKind::End)
        .collect();

    if begins.len() != 1 {
        return Err(FlowError::Validation(format!(
            "Expected exactly one Begin node, found {}",
            begins.len()
        )));
    }
    if ends.len() != 1 {
        return Err(FlowError::Validation(format!(
            "Expected exactly one End node, found {}",
            ends.len()
        )));
    }

    // Reachability from begin
    let mut visited = std::collections::HashSet::new();
    let mut stack = vec![flow.begin_id.clone()];
    while let Some(id) = stack.pop() {
        if !visited.insert(id.clone()) {
            continue;
        }
        if let Some(edges) = flow.outgoing.get(&id) {
            for edge in edges {
                stack.push(edge.dst.clone());
            }
        }
    }

    for node_id in flow.nodes.keys() {
        if !visited.contains(node_id) {
            return Err(FlowError::Validation(format!(
                "Node '{}' is unreachable from Begin",
                node_id
            )));
        }
    }

    // Decision node edge labels
    for node in flow.nodes.values() {
        if node.kind == FlowNodeKind::Decision {
            let edges = flow
                .outgoing
                .get(&node.id)
                .ok_or_else(|| FlowError::NoOutgoingEdges(node.id.clone()))?;
            let mut labels = std::collections::HashSet::new();
            for edge in edges {
                let label = edge.label.as_deref().unwrap_or("");
                if label.is_empty() {
                    return Err(FlowError::Validation(format!(
                        "Decision node '{}' has an edge with empty label",
                        node.id
                    )));
                }
                if !labels.insert(label.to_string()) {
                    return Err(FlowError::DuplicateEdgeLabels(node.id.clone()));
                }
            }
        }
    }

    Ok(())
}

/// Extract `<choice>...</choice>` from text (case-insensitive, last wins).
pub fn parse_choice(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let mut last_choice: Option<String> = None;
    let mut search_from = 0;
    while let Some(start) = lower[search_from..].find("<choice>") {
        let abs_start = search_from + start;
        let after_start = abs_start + "<choice>".len();
        if let Some(end) = lower[after_start..].find("</choice>") {
            let abs_end = after_start + end;
            let choice = text[after_start..abs_end].trim().to_string();
            last_choice = Some(choice);
            search_from = abs_end + "</choice>".len();
        } else {
            break;
        }
    }
    last_choice
}

pub mod mermaid;
pub mod runner;
pub use runner::{FlowExecutor, FlowRunner};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_choice_basic() {
        assert_eq!(
            parse_choice("I choose <choice>option-a</choice>"),
            Some("option-a".to_string())
        );
    }

    #[test]
    fn test_parse_choice_last_wins() {
        assert_eq!(
            parse_choice("<choice>first</choice> then <choice>second</choice>"),
            Some("second".to_string())
        );
    }

    #[test]
    fn test_parse_choice_case_insensitive() {
        assert_eq!(
            parse_choice("I choose <CHOICE>Option-A</CHOICE>"),
            Some("Option-A".to_string())
        );
    }

    #[test]
    fn test_validate_flow_ok() {
        let mut flow = Flow::default();
        flow.nodes.insert(
            "start".to_string(),
            FlowNode {
                id: "start".to_string(),
                label: "Start".to_string(),
                kind: FlowNodeKind::Begin,
            },
        );
        flow.nodes.insert(
            "end".to_string(),
            FlowNode {
                id: "end".to_string(),
                label: "End".to_string(),
                kind: FlowNodeKind::End,
            },
        );
        flow.begin_id = "start".to_string();
        flow.end_id = "end".to_string();
        flow.outgoing.insert(
            "start".to_string(),
            vec![FlowEdge {
                src: "start".to_string(),
                dst: "end".to_string(),
                label: None,
            }],
        );
        assert!(validate_flow(&flow).is_ok());
    }

    #[test]
    fn test_validate_flow_missing_begin() {
        let mut flow = Flow::default();
        flow.nodes.insert(
            "end".to_string(),
            FlowNode {
                id: "end".to_string(),
                label: "End".to_string(),
                kind: FlowNodeKind::End,
            },
        );
        flow.end_id = "end".to_string();
        assert!(matches!(
            validate_flow(&flow),
            Err(FlowError::Validation(_))
        ));
    }

    #[test]
    fn test_validate_flow_unreachable() {
        let mut flow = Flow::default();
        flow.nodes.insert(
            "start".to_string(),
            FlowNode {
                id: "start".to_string(),
                label: "Start".to_string(),
                kind: FlowNodeKind::Begin,
            },
        );
        flow.nodes.insert(
            "end".to_string(),
            FlowNode {
                id: "end".to_string(),
                label: "End".to_string(),
                kind: FlowNodeKind::End,
            },
        );
        flow.nodes.insert(
            "orphan".to_string(),
            FlowNode {
                id: "orphan".to_string(),
                label: "Orphan".to_string(),
                kind: FlowNodeKind::Task,
            },
        );
        flow.begin_id = "start".to_string();
        flow.end_id = "end".to_string();
        flow.outgoing.insert(
            "start".to_string(),
            vec![FlowEdge {
                src: "start".to_string(),
                dst: "end".to_string(),
                label: None,
            }],
        );
        assert!(matches!(
            validate_flow(&flow),
            Err(FlowError::Validation(_))
        ));
    }

    #[test]
    fn test_validate_flow_duplicate_labels() {
        let mut flow = Flow::default();
        flow.nodes.insert(
            "start".to_string(),
            FlowNode {
                id: "start".to_string(),
                label: "Start".to_string(),
                kind: FlowNodeKind::Begin,
            },
        );
        flow.nodes.insert(
            "dec".to_string(),
            FlowNode {
                id: "dec".to_string(),
                label: "Decide".to_string(),
                kind: FlowNodeKind::Decision,
            },
        );
        flow.nodes.insert(
            "end".to_string(),
            FlowNode {
                id: "end".to_string(),
                label: "End".to_string(),
                kind: FlowNodeKind::End,
            },
        );
        flow.begin_id = "start".to_string();
        flow.end_id = "end".to_string();
        flow.outgoing.insert(
            "start".to_string(),
            vec![FlowEdge {
                src: "start".to_string(),
                dst: "dec".to_string(),
                label: None,
            }],
        );
        flow.outgoing.insert(
            "dec".to_string(),
            vec![
                FlowEdge {
                    src: "dec".to_string(),
                    dst: "end".to_string(),
                    label: Some("yes".to_string()),
                },
                FlowEdge {
                    src: "dec".to_string(),
                    dst: "end".to_string(),
                    label: Some("yes".to_string()),
                },
            ],
        );
        assert!(matches!(
            validate_flow(&flow),
            Err(FlowError::DuplicateEdgeLabels(_))
        ));
    }
}
