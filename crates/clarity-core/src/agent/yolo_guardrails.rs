//! Convergence guardrails for high-autonomy execution modes.
//!
//! Guardrails act as a safety net under YOLO and any other auto-execution mode.
//! When the agent repeatedly calls tools without making progress, the guardrails
//! convert the runaway loop into an `ask_user` break instead of waiting for
//! `max_iterations` or budget exhaustion.

use super::loop_detector::LoopDetector;

/// Configuration for auto-execution convergence guardrails.
#[derive(Debug, Clone, Copy)]
pub struct YoloGuardrails {
    /// Maximum number of tool calls allowed in a single turn.
    pub max_tool_calls_per_turn: usize,
    /// Maximum consecutive calls to the same tool name before asking the user.
    pub max_consecutive_same_tool: usize,
    /// Number of recent tool results to consider for stagnation detection.
    pub stagnation_window: usize,
    /// Minimum ratio of distinct outputs in the stagnation window.
    /// A value of 1.0 means all outputs are distinct; 0.0 means all identical.
    pub stagnation_threshold: f64,
}

impl Default for YoloGuardrails {
    fn default() -> Self {
        Self {
            max_tool_calls_per_turn: 32,
            max_consecutive_same_tool: 5,
            stagnation_window: 6,
            stagnation_threshold: 0.25,
        }
    }
}

impl YoloGuardrails {
    /// Create disabled guardrails (no early stopping beyond existing limits).
    pub fn disabled() -> Self {
        Self {
            max_tool_calls_per_turn: usize::MAX,
            max_consecutive_same_tool: usize::MAX,
            stagnation_window: 0,
            stagnation_threshold: 0.0,
        }
    }

    /// Set the maximum tool calls per turn.
    pub fn with_max_tool_calls_per_turn(mut self, max: usize) -> Self {
        self.max_tool_calls_per_turn = max;
        self
    }

    /// Set the maximum consecutive calls to the same tool.
    pub fn with_max_consecutive_same_tool(mut self, max: usize) -> Self {
        self.max_consecutive_same_tool = max;
        self
    }

    /// Set the stagnation detection window and threshold.
    pub fn with_stagnation(mut self, window: usize, threshold: f64) -> Self {
        self.stagnation_window = window;
        self.stagnation_threshold = threshold;
        self
    }
}

/// Outcome of a guardrail check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardrailOutcome {
    /// No guardrail triggered.
    Ok,
    /// The loop should break and ask the user a clarifying question.
    AskUser { question: String },
    /// The loop should stop with a final summary.
    Stop { reason: String },
}

/// Runtime state passed to the guardrail check.
pub struct GuardrailState<'a> {
    /// Total number of tool calls executed so far in this turn.
    pub total_tool_calls: usize,
    /// Loop detector with recent invocation history.
    pub detector: &'a LoopDetector,
}

impl YoloGuardrails {
    /// Evaluate whether the current loop state should converge.
    pub fn check(&self, state: &GuardrailState<'_>) -> GuardrailOutcome {
        // Too many tool calls in one turn.
        if state.total_tool_calls >= self.max_tool_calls_per_turn {
            return GuardrailOutcome::AskUser {
                question: format!(
                    "本回合已调用 {} 次工具仍未收敛。请确认下一步方向，或提供更多上下文。",
                    state.total_tool_calls
                ),
            };
        }

        // Same tool called repeatedly without progress.
        let (name, count) = state.detector.consecutive_same_tool_count();
        if count >= self.max_consecutive_same_tool {
            return GuardrailOutcome::AskUser {
                question: format!(
                    "工具 '{}' 已连续调用 {} 次，未获得明显进展。请提供更多信息或让我停止。",
                    name, count
                ),
            };
        }

        // Recent outputs are too similar (low diversity).
        if self.stagnation_window > 0 {
            let diversity = state
                .detector
                .result_diversity_score(self.stagnation_window);
            if diversity < self.stagnation_threshold {
                return GuardrailOutcome::AskUser {
                    question: String::from(
                        "最近几次工具调用结果相似度很高，似乎没有新信息。请确认是否需要调整策略。",
                    ),
                };
            }
        }

        GuardrailOutcome::Ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_guardrails_ok_for_fresh_state() {
        let guardrails = YoloGuardrails::default();
        let detector = LoopDetector::new(3);
        let state = GuardrailState {
            total_tool_calls: 0,
            detector: &detector,
        };
        assert_eq!(guardrails.check(&state), GuardrailOutcome::Ok);
    }

    #[test]
    fn max_tool_calls_triggers_ask_user() {
        let guardrails = YoloGuardrails::default().with_max_tool_calls_per_turn(3);
        let detector = LoopDetector::new(3);
        let state = GuardrailState {
            total_tool_calls: 3,
            detector: &detector,
        };
        let outcome = guardrails.check(&state);
        assert!(matches!(outcome, GuardrailOutcome::AskUser { .. }));
    }

    #[test]
    fn consecutive_same_tool_triggers_ask_user() {
        let guardrails = YoloGuardrails::default().with_max_consecutive_same_tool(3);
        let mut detector = LoopDetector::new(3);
        detector.record("grep", r#"{"pattern": "a"}"#, "a");
        detector.record("grep", r#"{"pattern": "b"}"#, "b");
        detector.record("grep", r#"{"pattern": "c"}"#, "c");
        let state = GuardrailState {
            total_tool_calls: 3,
            detector: &detector,
        };
        let outcome = guardrails.check(&state);
        assert!(matches!(outcome, GuardrailOutcome::AskUser { .. }));
    }

    #[test]
    fn stagnation_triggers_ask_user() {
        let guardrails = YoloGuardrails::default().with_stagnation(4, 0.3);
        let mut detector = LoopDetector::new(3);
        detector.record("read", r#"{"path": "a"}"#, "same output");
        detector.record("read", r#"{"path": "b"}"#, "same output");
        detector.record("read", r#"{"path": "c"}"#, "same output");
        detector.record("read", r#"{"path": "d"}"#, "same output");
        let state = GuardrailState {
            total_tool_calls: 4,
            detector: &detector,
        };
        let outcome = guardrails.check(&state);
        assert!(matches!(outcome, GuardrailOutcome::AskUser { .. }));
    }
}
