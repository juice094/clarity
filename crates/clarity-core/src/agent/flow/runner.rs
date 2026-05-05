//! Flow execution engine.
//!
//! Drives a `Flow` by executing one node at a time, where each node
//! becomes one Agent turn.

use super::{parse_choice, Flow, FlowError, FlowNodeKind};
use clarity_contract::AgentError;
use std::sync::Arc;

use crate::agent::jumpy::predictor::OutcomePredictor;
use crate::agent::jumpy::state::JumpyState;

#[derive(Debug, serde::Deserialize)]
struct CheckpointExpected {
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    min_progress: f32,
}

fn parse_skill_invocation(label: &str) -> (&str, &str) {
    if let Some(start) = label.find('(') {
        if let Some(end) = label.rfind(')') {
            if end > start {
                return (label[..start].trim(), label[start + 1..end].trim());
            }
        }
    }
    (label.trim(), "")
}

/// Trait for executing a single flow node prompt.
#[async_trait::async_trait]
pub trait FlowExecutor: Send + Sync {
    /// Execute one turn with the given prompt.
    async fn execute(&self, prompt: &str) -> Result<String, AgentError>;
    /// Execute an external skill with the given id and parameters.
    async fn execute_skill(&self, _skill_id: &str, _params: &str) -> Result<String, AgentError> {
        Err(AgentError::FlowExecution(
            "execute_skill not implemented".to_string(),
        ))
    }
}

/// Runner that walks a flow from Begin to End.
pub struct FlowRunner<'a> {
    flow: &'a Flow,
    max_steps: usize,
    predictor: Option<Arc<dyn OutcomePredictor>>,
}

impl<'a> FlowRunner<'a> {
    /// Create a new runner for the given flow.
    pub fn new(flow: &'a Flow) -> Self {
        Self {
            flow,
            max_steps: 1000,
            predictor: None,
        }
    }

    /// Attach an outcome predictor for PredictCheckpoint nodes.
    pub fn with_predictor(mut self, predictor: Arc<dyn OutcomePredictor>) -> Self {
        self.predictor = Some(predictor);
        self
    }

    /// Set max steps (default 1000).
    pub fn with_max_steps(mut self, max: usize) -> Self {
        self.max_steps = max;
        self
    }

    /// Execute the flow.
    pub async fn run<E: FlowExecutor>(
        &self,
        executor: &E,
        _initial_args: &str,
    ) -> Result<String, FlowError> {
        let mut current_id = self.flow.begin_id.clone();
        let mut step_count = 0;
        let mut final_response = String::new();
        let mut last_skill_id: Option<String> = None;
        let mut last_skill_params: Option<String> = None;
        let mut current_state = JumpyState::default();

        while step_count < self.max_steps {
            let node = self
                .flow
                .nodes
                .get(&current_id)
                .ok_or_else(|| FlowError::MissingNode(current_id.clone()))?;

            match node.kind {
                FlowNodeKind::End => return Ok(final_response),
                FlowNodeKind::Begin => {
                    current_id = self.follow_edge(node, None)?;
                }
                FlowNodeKind::Task => {
                    let response = executor
                        .execute(&node.label)
                        .await
                        .map_err(|e| FlowError::Execution(e.to_string()))?;
                    final_response = response.clone();
                    current_state.context_summary = response.chars().take(200).collect();
                    current_id = self.follow_edge(node, None)?;
                }
                FlowNodeKind::Decision => {
                    let edges = self
                        .flow
                        .outgoing
                        .get(&node.id)
                        .ok_or_else(|| FlowError::NoOutgoingEdges(node.id.clone()))?;

                    let choices: Vec<String> =
                        edges.iter().filter_map(|e| e.label.clone()).collect();

                    let prompt = if choices.is_empty() {
                        node.label.clone()
                    } else {
                        format!(
                            "{}\n\nChoose the next step by responding with <choice>YOUR_CHOICE</choice>. Available choices: {}",
                            node.label,
                            choices.join(", ")
                        )
                    };

                    let response = executor
                        .execute(&prompt)
                        .await
                        .map_err(|e| FlowError::Execution(e.to_string()))?;
                    final_response = response.clone();
                    current_state.context_summary = response.chars().take(200).collect();

                    let choice = parse_choice(&response)
                        .or_else(|| choices.first().cloned())
                        .unwrap_or_default();

                    current_id = self.follow_edge(node, Some(&choice))?;
                }
                FlowNodeKind::InvokeSkill => {
                    let (skill_id, params) = parse_skill_invocation(&node.label);
                    let response = executor
                        .execute_skill(skill_id, params)
                        .await
                        .map_err(|e| FlowError::Execution(e.to_string()))?;
                    last_skill_id = Some(skill_id.to_string());
                    last_skill_params = Some(params.to_string());
                    final_response = response.clone();
                    current_state.memory.insert("last_result".to_string(), response);
                    current_id = self.follow_edge(node, None)?;
                }
                FlowNodeKind::PredictCheckpoint => {
                    let expected: CheckpointExpected = serde_json::from_str(&node.label)
                        .map_err(|e| FlowError::Parse(format!("invalid checkpoint JSON: {}", e)))?;

                    let skill_id = last_skill_id.as_deref().ok_or_else(|| {
                        FlowError::Execution(
                            "PredictCheckpoint requires a preceding InvokeSkill node".to_string(),
                        )
                    })?;
                    let params = last_skill_params.as_deref().ok_or_else(|| {
                        FlowError::Execution(
                            "PredictCheckpoint requires a preceding InvokeSkill node".to_string(),
                        )
                    })?;

                    let predictor = self.predictor.as_ref().ok_or_else(|| {
                        FlowError::Execution(
                            "PredictCheckpoint requires a predictor to be configured".to_string(),
                        )
                    })?;

                    let predicted = predictor
                        .predict(skill_id, params, &current_state, 0.9)
                        .await
                        .map_err(|e| FlowError::Execution(format!("prediction failed: {}", e)))?;

                    if !predicted.satisfies(&expected.tags) {
                        return Err(FlowError::Execution(format!(
                            "predicted state mismatch: missing tags {:?} in {:?}",
                            expected.tags, predicted.tags
                        )));
                    }
                    if predicted.progress < expected.min_progress {
                        return Err(FlowError::Execution(format!(
                            "predicted state mismatch: progress {} < {}",
                            predicted.progress, expected.min_progress
                        )));
                    }

                    current_id = self.follow_edge(node, None)?;
                }
            }

            step_count += 1;
        }

        Err(FlowError::MaxStepsReached(self.max_steps))
    }

    fn follow_edge(
        &self,
        node: &super::FlowNode,
        choice: Option<&str>,
    ) -> Result<String, FlowError> {
        let edges = self
            .flow
            .outgoing
            .get(&node.id)
            .ok_or_else(|| FlowError::NoOutgoingEdges(node.id.clone()))?;

        if edges.is_empty() {
            return Err(FlowError::NoOutgoingEdges(node.id.clone()));
        }

        match node.kind {
            FlowNodeKind::Decision => {
                let choice = choice.unwrap_or("");
                let matched = edges
                    .iter()
                    .find(|e| e.label.as_deref() == Some(choice))
                    .or_else(|| edges.first());
                matched
                    .map(|e| e.dst.clone())
                    .ok_or_else(|| FlowError::NoOutgoingEdges(node.id.clone()))
            }
            _ => edges
                .first()
                .map(|e| e.dst.clone())
                .ok_or_else(|| FlowError::NoOutgoingEdges(node.id.clone())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::flow::{Flow, FlowEdge, FlowNode, FlowNodeKind};

    struct MockExecutor;

    #[async_trait::async_trait]
    impl FlowExecutor for MockExecutor {
        async fn execute(&self, prompt: &str) -> Result<String, AgentError> {
            Ok(prompt.to_string())
        }
    }

    #[tokio::test]
    async fn test_runner_linear_flow() {
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
            "task".to_string(),
            FlowNode {
                id: "task".to_string(),
                label: "Do work".to_string(),
                kind: FlowNodeKind::Task,
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
                dst: "task".to_string(),
                label: None,
            }],
        );
        flow.outgoing.insert(
            "task".to_string(),
            vec![FlowEdge {
                src: "task".to_string(),
                dst: "end".to_string(),
                label: None,
            }],
        );

        let runner = FlowRunner::new(&flow);
        let result = runner.run(&MockExecutor, "").await.unwrap();
        assert_eq!(result, "Do work");
    }

    #[tokio::test]
    async fn test_runner_decision_flow() {
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
            "decide".to_string(),
            FlowNode {
                id: "decide".to_string(),
                label: "Choose path".to_string(),
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
                dst: "decide".to_string(),
                label: None,
            }],
        );
        flow.outgoing.insert(
            "decide".to_string(),
            vec![
                FlowEdge {
                    src: "decide".to_string(),
                    dst: "end".to_string(),
                    label: Some("yes".to_string()),
                },
                FlowEdge {
                    src: "decide".to_string(),
                    dst: "end".to_string(),
                    label: Some("no".to_string()),
                },
            ],
        );

        struct ChoiceExecutor;
        #[async_trait::async_trait]
        impl FlowExecutor for ChoiceExecutor {
            async fn execute(&self, _prompt: &str) -> Result<String, AgentError> {
                Ok("<choice>yes</choice>".to_string())
            }
        }

        let runner = FlowRunner::new(&flow);
        let result = runner.run(&ChoiceExecutor, "").await.unwrap();
        // Decision node response is the LLM's output (including <choice> tag)
        assert_eq!(result, "<choice>yes</choice>");
    }

    #[tokio::test]
    async fn test_runner_max_steps() {
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
            "loop".to_string(),
            FlowNode {
                id: "loop".to_string(),
                label: "Loop".to_string(),
                kind: FlowNodeKind::Task,
            },
        );
        flow.begin_id = "start".to_string();
        flow.end_id = "loop".to_string(); // fake end to avoid validation error
        flow.outgoing.insert(
            "start".to_string(),
            vec![FlowEdge {
                src: "start".to_string(),
                dst: "loop".to_string(),
                label: None,
            }],
        );
        // loop points back to itself
        flow.outgoing.insert(
            "loop".to_string(),
            vec![FlowEdge {
                src: "loop".to_string(),
                dst: "loop".to_string(),
                label: None,
            }],
        );

        let runner = FlowRunner::new(&flow).with_max_steps(3);
        let result = runner.run(&MockExecutor, "").await;
        assert!(matches!(result, Err(FlowError::MaxStepsReached(3))));
    }
}
