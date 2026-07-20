//! In-memory knowledge graph for navigating linked documents.

use crate::error::Result;
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;

/// Kind of node in the knowledge graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind {
    /// A file on disk.
    File,
    /// A heading inside a file.
    Heading,
    /// A tagged block inside a file.
    Block,
    /// A tag value.
    Tag,
    /// An embedded attachment (image, PDF, etc.).
    Attachment,
    /// A conversation session.
    Session,
    /// A single message inside a session.
    Message,
}

/// Importance level of a node.
///
/// Determines how much a node contributes to spreading activation and how
/// quickly its activation decays. Aligned with the two-memory-model idea:
/// durable concepts keep high importance, ephemeral chatter decays fast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Importance {
    /// Persistent, high-impact knowledge. Weight 1.0, no decay.
    Critical,
    /// Important but not permanent. Weight 0.8, half-normal decay.
    High,
    /// Default node importance. Weight 0.6, normal decay.
    #[default]
    Medium,
    /// Background or noisy nodes. Weight 0.3, 2× decay.
    Low,
    /// Transient activations (e.g. a single message). Weight 0.1, 5× decay.
    Ephemeral,
}

impl Importance {
    /// Weight multiplier for ranking and spreading activation.
    pub fn weight(&self) -> f32 {
        match self {
            Importance::Critical => 1.0,
            Importance::High => 0.8,
            Importance::Medium => 0.6,
            Importance::Low => 0.3,
            Importance::Ephemeral => 0.1,
        }
    }

    /// Multiplier applied to the decay rate. Zero means the node does not decay.
    pub fn decay_multiplier(&self) -> f32 {
        match self {
            Importance::Critical => 0.0,
            Importance::High => 0.5,
            Importance::Medium => 1.0,
            Importance::Low => 2.0,
            Importance::Ephemeral => 5.0,
        }
    }
}

/// Stable identifier for a node in the knowledge graph.
///
/// For files, this is typically the source-relative path. For headings and
/// blocks, it is a composite such as `path#heading` or `path#^block-id`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeId(pub String);

impl NodeId {
    /// Create a node id from anything that can become a string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// Kind of edge in the knowledge graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeKind {
    /// One file links to another.
    LinksTo,
    /// A file is tagged with a tag.
    TaggedWith,
    /// A file contains a heading or block.
    Contains,
    /// A file embeds an attachment.
    Embeds,
}

/// An in-memory directed graph representing the knowledge base.
#[derive(Debug, Default, Clone)]
pub struct KnowledgeGraph {
    graph: DiGraph<Node, EdgeKind>,
    lookup: HashMap<NodeId, NodeIndex>,
}

/// A node in the knowledge graph.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Stable identifier.
    pub id: NodeId,
    /// Display label.
    pub label: String,
    /// Node kind.
    pub kind: NodeKind,
    /// Current activation level in `[0.0, 1.0]`.
    pub activation: f32,
    /// When this node was last explicitly activated, if ever.
    pub last_activated_at: Option<std::time::Instant>,
    /// Whether this node has fallen below the dormant threshold.
    pub dormant: bool,
    /// Importance level. Affects spreading weight, ranking, and decay.
    pub importance: Importance,
}

impl Node {
    /// Effective activation used for ranking and spreading weighting.
    pub fn effective_activation(&self) -> f32 {
        self.activation * self.importance.weight()
    }
}

impl KnowledgeGraph {
    /// Create an empty knowledge graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or update a node and return its index.
    ///
    /// New nodes are created with [`Importance::Medium`]. Use
    /// [`Self::upsert_node_with_importance`] to set a different level.
    pub fn upsert_node(
        &mut self,
        id: NodeId,
        label: impl Into<String>,
        kind: NodeKind,
    ) -> NodeIndex {
        self.upsert_node_with_importance(id, label, kind, Importance::Medium)
    }

    /// Insert or update a node with an explicit importance level.
    pub fn upsert_node_with_importance(
        &mut self,
        id: NodeId,
        label: impl Into<String>,
        kind: NodeKind,
        importance: Importance,
    ) -> NodeIndex {
        if let Some(&idx) = self.lookup.get(&id) {
            let node = &mut self.graph[idx];
            node.label = label.into();
            node.kind = kind;
            node.importance = importance;
            return idx;
        }

        let idx = self.graph.add_node(Node {
            id: id.clone(),
            label: label.into(),
            kind,
            activation: 0.0,
            last_activated_at: None,
            dormant: false,
            importance,
        });
        self.lookup.insert(id, idx);
        idx
    }

    /// Set the importance of an existing node.
    ///
    /// Returns `true` if the node existed and was updated.
    pub fn set_importance(&mut self, id: &NodeId, importance: Importance) -> bool {
        if let Some(idx) = self.node_index(id) {
            self.graph[idx].importance = importance;
            true
        } else {
            false
        }
    }

    /// Return the internal petgraph index for a node id, if it exists.
    pub fn node_index(&self, id: &NodeId) -> Option<NodeIndex> {
        self.lookup.get(id).copied()
    }

    /// Return the importance of the node at the given index.
    pub fn node_importance(&self, idx: NodeIndex) -> Importance {
        self.graph[idx].importance
    }

    /// Add a directed edge between two nodes.
    ///
    /// If either node does not exist, it is created with an empty label.
    /// Ensure a node exists without changing its kind if it already exists.
    ///
    /// Used by [`Self::add_edge`] so that connecting two nodes does not
    /// overwrite a more specific kind (e.g. turning a `Tag` back into `File`).
    /// New nodes created by edges default to [`Importance::Medium`].
    fn get_or_insert_node(
        &mut self,
        id: NodeId,
        label: impl Into<String>,
        default_kind: NodeKind,
    ) -> NodeIndex {
        if let Some(&idx) = self.lookup.get(&id) {
            self.graph[idx].label = label.into();
            return idx;
        }
        let idx = self.graph.add_node(Node {
            id: id.clone(),
            label: label.into(),
            kind: default_kind,
            activation: 0.0,
            last_activated_at: None,
            dormant: false,
            importance: Importance::Medium,
        });
        self.lookup.insert(id, idx);
        idx
    }

    /// Add a directed edge between two nodes, creating either endpoint if it
    /// does not already exist. Existing node kinds are preserved.
    pub fn add_edge(
        &mut self,
        from: NodeId,
        to: NodeId,
        kind: EdgeKind,
        from_label: impl Into<String>,
        to_label: impl Into<String>,
    ) {
        let from_idx = self.get_or_insert_node(from.clone(), from_label, NodeKind::File);
        let to_idx = self.get_or_insert_node(to.clone(), to_label, NodeKind::File);
        self.graph.add_edge(from_idx, to_idx, kind);
    }

    /// Remove a node and all of its edges from the graph.
    ///
    /// If the node does not exist, this is a no-op. The internal node index is
    /// rebuilt so that remaining lookups remain valid.
    pub fn remove_node(&mut self, id: &NodeId) -> Result<()> {
        let Some(idx) = self.lookup.remove(id) else {
            return Ok(());
        };
        self.graph.remove_node(idx);
        self.lookup.clear();
        for new_idx in self.graph.node_indices() {
            self.lookup.insert(self.graph[new_idx].id.clone(), new_idx);
        }
        Ok(())
    }

    /// Inject activation energy into a node.
    ///
    /// The energy is added to the node's current activation and clamped to
    /// `[0.0, 1.0]`. The timestamp is updated to `now`.
    pub fn inject_activation(&mut self, id: &NodeId, energy: f32, now: std::time::Instant) {
        let Some(idx) = self.lookup.get(id).copied() else {
            return;
        };
        let node = &mut self.graph[idx];
        node.activation = (node.activation + energy).clamp(0.0, 1.0);
        node.last_activated_at = Some(now);
        node.dormant = false;
    }

    /// Decay every node's activation using exponential decay.
    ///
    /// `half_life` is the duration after which an unchanged activation is
    /// reduced by half for a node of [`Importance::Medium`]. Higher-importance
    /// nodes decay more slowly; ephemeral nodes decay faster. Critical nodes
    /// do not decay.
    pub fn decay_activation(&mut self, now: std::time::Instant, half_life: std::time::Duration) {
        if half_life.as_secs_f32() <= 0.0 {
            return;
        }
        let base_lambda = std::f32::consts::LN_2 / half_life.as_secs_f32();
        for idx in self.graph.node_indices() {
            let node = &mut self.graph[idx];
            let Some(last) = node.last_activated_at else {
                continue;
            };
            let multiplier = node.importance.decay_multiplier();
            if multiplier == 0.0 {
                continue;
            }
            let elapsed = now.saturating_duration_since(last).as_secs_f32();
            node.activation *= (-base_lambda * multiplier * elapsed).exp();
            node.activation = node.activation.clamp(0.0, 1.0);
        }
    }

    /// Mark nodes as dormant when their activation stays below `threshold`
    /// for at least `min_age`.
    ///
    /// Dormant nodes remain in the graph but can be excluded from retrieval.
    /// They are reactivated automatically by [`Self::inject_activation`].
    pub fn mark_dormant(
        &mut self,
        now: std::time::Instant,
        threshold: f32,
        min_age: std::time::Duration,
    ) {
        for idx in self.graph.node_indices() {
            let node = &mut self.graph[idx];
            if node.activation >= threshold {
                continue;
            }
            let Some(last) = node.last_activated_at else {
                continue;
            };
            if now.saturating_duration_since(last) >= min_age {
                node.dormant = true;
            }
        }
    }

    /// Reset all activation state to zero and clear the dormant flag.
    ///
    /// Useful when starting a fresh query so previous activations do not
    /// pollute the result ranking.
    pub fn reset_activation(&mut self) {
        for idx in self.graph.node_indices() {
            let node = &mut self.graph[idx];
            node.activation = 0.0;
            node.last_activated_at = None;
            node.dormant = false;
        }
    }

    /// Propagate activation through the graph for a fixed number of iterations.
    ///
    /// At each step, neighbor activations are spread along outgoing edges,
    /// diluted by the source node's fan-out (fan effect), weighted by the
    /// source node's importance, and merged with the node's retained
    /// activation. A sigmoid keeps values in `[0.0, 1.0]`.
    pub fn spreading_activation(&mut self, iterations: usize) {
        if iterations == 0 {
            return;
        }

        const RETENTION: f32 = 0.5;
        const SPREAD: f32 = 0.8;
        const GAMMA: f32 = 1.0;
        const THETA: f32 = 0.0;

        let node_indices: Vec<NodeIndex> = self.graph.node_indices().collect();

        for _ in 0..iterations {
            let mut next = vec![0.0f32; self.graph.node_count()];

            for (i, idx) in node_indices.iter().copied().enumerate() {
                let current = self.graph[idx].activation;
                let mut incoming = 0.0f32;

                for neighbor in self
                    .graph
                    .neighbors_directed(idx, petgraph::Direction::Incoming)
                {
                    let neighbor_node = &self.graph[neighbor];
                    let out_degree = self
                        .graph
                        .neighbors_directed(neighbor, petgraph::Direction::Outgoing)
                        .count()
                        .max(1);
                    incoming += neighbor_node.activation * neighbor_node.importance.weight()
                        / out_degree as f32;
                }

                let potential = RETENTION * current + SPREAD * incoming;
                next[i] = 1.0 / (1.0 + (-GAMMA * (potential - THETA)).exp());
            }

            for (i, idx) in node_indices.iter().copied().enumerate() {
                self.graph[idx].activation = next[i].clamp(0.0, 1.0);
            }
        }
    }

    /// Apply lateral inhibition to sharpen the activation pattern.
    ///
    /// The `top_k` most active nodes suppress all other nodes by a fraction of
    /// the average top-k potential. This is a simplified version of the
    /// winner-take-all competition used in spreading-activation memory models.
    pub fn lateral_inhibition(&mut self, top_k: usize, beta: f32) {
        if top_k == 0 || beta <= 0.0 {
            return;
        }

        let mut potentials: Vec<(NodeIndex, f32)> = self
            .graph
            .node_indices()
            .map(|idx| (idx, self.graph[idx].activation))
            .collect();
        potentials.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let top_avg = potentials.iter().take(top_k).map(|(_, p)| *p).sum::<f32>()
            / top_k.min(potentials.len()).max(1) as f32;

        for (idx, potential) in potentials.iter().copied().skip(top_k) {
            let inhibited = potential - beta * (top_avg - potential);
            self.graph[idx].activation = inhibited.clamp(0.0, 1.0);
        }
    }

    /// Return the `k` most activated non-dormant nodes, sorted from highest to
    /// lowest.
    ///
    /// Ranking uses an *effective activation* that combines raw activation with
    /// node importance: `activation * importance.weight()`.
    pub fn top_activated(&self, k: usize) -> Vec<&Node> {
        let mut nodes: Vec<&Node> = self.graph.node_weights().filter(|n| !n.dormant).collect();
        nodes.sort_by(|a, b| {
            b.effective_activation()
                .partial_cmp(&a.effective_activation())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        nodes.into_iter().take(k).collect()
    }

    /// Return all nodes.
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.graph.node_weights()
    }

    /// Return all direct neighbors of a node, if it exists.
    pub fn neighbors(&self, id: &NodeId) -> Option<Vec<&Node>> {
        let idx = self.lookup.get(id)?;
        Some(self.graph.neighbors(*idx).map(|n| &self.graph[n]).collect())
    }

    /// Return all nodes that link to the given node (backlinks).
    pub fn backlinks(&self, id: &NodeId) -> Option<Vec<&Node>> {
        let idx = self.lookup.get(id)?;
        Some(
            self.graph
                .neighbors_directed(*idx, petgraph::Direction::Incoming)
                .map(|n| &self.graph[n])
                .collect(),
        )
    }

    /// Return the number of nodes.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Return the number of edges.
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_graph_operations() {
        let mut graph = KnowledgeGraph::new();
        graph.add_edge(
            NodeId::new("a.md"),
            NodeId::new("b.md"),
            EdgeKind::LinksTo,
            "A",
            "B",
        );

        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);

        let neighbors = graph.neighbors(&NodeId::new("a.md")).unwrap();
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].id.0, "b.md");

        let backlinks = graph.backlinks(&NodeId::new("b.md")).unwrap();
        assert_eq!(backlinks.len(), 1);
        assert_eq!(backlinks[0].id.0, "a.md");
    }

    #[test]
    fn inject_activation_clamps_and_records_time() {
        let mut graph = KnowledgeGraph::new();
        graph.upsert_node(NodeId::new("a.md"), "A", NodeKind::File);

        let now = std::time::Instant::now();
        graph.inject_activation(&NodeId::new("a.md"), 0.7, now);

        let node = graph.nodes().next().unwrap();
        assert!((node.activation - 0.7).abs() < f32::EPSILON);
        assert_eq!(node.last_activated_at, Some(now));

        graph.inject_activation(&NodeId::new("a.md"), 0.5, now);
        let node = graph.nodes().next().unwrap();
        assert!((node.activation - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn decay_activation_reduces_over_time() {
        let mut graph = KnowledgeGraph::new();
        graph.upsert_node(NodeId::new("a.md"), "A", NodeKind::File);

        let now = std::time::Instant::now();
        graph.inject_activation(&NodeId::new("a.md"), 1.0, now);
        graph.decay_activation(
            now + std::time::Duration::from_secs(60),
            std::time::Duration::from_secs(60),
        );

        let node = graph.nodes().next().unwrap();
        assert!((node.activation - 0.5).abs() < 0.01);
    }

    #[test]
    fn spreading_activation_reaches_linked_nodes() {
        let mut graph = KnowledgeGraph::new();
        graph.add_edge(
            NodeId::new("a.md"),
            NodeId::new("b.md"),
            EdgeKind::LinksTo,
            "A",
            "B",
        );

        let now = std::time::Instant::now();
        graph.inject_activation(&NodeId::new("a.md"), 1.0, now);
        graph.spreading_activation(3);

        let b = graph.nodes().find(|n| n.id.0 == "b.md").unwrap();
        assert!(b.activation > 0.1, "b should be activated by spreading");
    }

    #[test]
    fn top_activated_sorted_correctly() {
        let mut graph = KnowledgeGraph::new();
        graph.upsert_node(NodeId::new("a.md"), "A", NodeKind::File);
        graph.upsert_node(NodeId::new("b.md"), "B", NodeKind::File);
        graph.upsert_node(NodeId::new("c.md"), "C", NodeKind::File);

        let now = std::time::Instant::now();
        graph.inject_activation(&NodeId::new("a.md"), 0.3, now);
        graph.inject_activation(&NodeId::new("b.md"), 0.9, now);
        graph.inject_activation(&NodeId::new("c.md"), 0.6, now);

        let top = graph.top_activated(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].id.0, "b.md");
        assert_eq!(top[1].id.0, "c.md");
    }
}
