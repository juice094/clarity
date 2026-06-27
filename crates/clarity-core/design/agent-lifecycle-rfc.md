# Agent Lifecycle and Tool Prompt Convergence — Interface Draft

> Companion to `docs/adr/ADR-019-agent-lifecycle-and-tool-prompt-convergence.md`

This document contains the concrete Rust-style interface drafts for the new components. It is intentionally a design file, not compiled code. The goal is to lock interfaces before implementation begins.

---

## 1. `ToolPromptManager`

### 1.1 Purpose

Eliminate repeated full-schema serialization of tools inside a single turn. Provide a single source of truth for the LLM-facing tool value.

### 1.2 Draft API

```rust
use serde_json::Value;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Stable, per-turn snapshot of the tool schema sent to the LLM.
pub(crate) struct ToolPromptManager {
    schema_hash: u64,
    tools_value: Value,
}

impl ToolPromptManager {
    /// Create from the filtered tool schema.
    pub fn new(tools: &Value) -> Self {
        let tools_value = tools.clone();
        let schema_hash = hash_value(&tools_value);
        Self {
            schema_hash,
            tools_value,
        }
    }

    /// Read-only access to the current LLM tool parameter.
    pub fn tools_value(&self) -> &Value {
        &self.tools_value
    }

    /// Remove a tool by name after a circuit-breaker failure.
    /// Returns true if a tool was removed.
    pub fn filter_tool(&mut self, name: &str) -> bool {
        let removed = filter_tool_from_schema(&mut self.tools_value, name);
        if removed {
            self.schema_hash = hash_value(&self.tools_value);
        }
        removed
    }

    /// True if the provided schema differs from the current snapshot.
    pub fn is_stale(&self, tools: &Value) -> bool {
        self.schema_hash != hash_value(tools)
    }
}

fn hash_value(value: &Value) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.to_string().hash(&mut hasher);
    hasher.finish()
}
```

### 1.3 Integration Points

- `Agent` gains a field: `tool_prompt_manager: Option<ToolPromptManager>`.
- `prepare_sync_turn` / `setup_turn` / `run_streaming_turn` call `ToolPromptManager::new(&filtered_tools)` and store it.
- `run_loop_iterations` receives `&ToolPromptManager` instead of `&Value` (or an `Option<&ToolPromptManager>` for backward compatibility during migration).
- `filter_tool_from_schema` in `loop_trait.rs` becomes a method on `ToolPromptManager`.

---

## 2. `LoopDetector` v2

### 2.1 Purpose

Detect repetitive or stagnating tool-call patterns beyond exact hash equality. Feed data into `YOLOGuardrails`.

### 2.2 Draft API

```rust
/// Metadata captured for each tool invocation.
pub struct ToolInvocation {
    pub tool_name: String,
    pub args: String,
    pub output: String,
    pub iteration: usize,
}

pub enum LoopDetection {
    Ok,
    Warning { tool_name: String, message: String },
    Break { tool_name: String, message: String },
}

pub struct LoopDetector {
    max_repetitions: usize,
    max_consecutive_same_tool: usize,
    stagnation_window: usize,
    tool_outputs: HashMap<String, Vec<u64>>,
    tool_patterns: HashMap<String, Vec<u64>>,
    recent_invocations: Vec<ToolInvocation>,
}

impl LoopDetector {
    pub fn new(config: &LoopDetectorConfig) -> Self;

    pub fn record(&mut self, invocation: ToolInvocation) -> LoopDetection;

    pub fn reset(&mut self);

    pub fn recent_invocations(&self) -> &[ToolInvocation];

    pub fn consecutive_same_tool_count(&self) -> (String, usize);

    pub fn result_diversity_score(&self, window: usize) -> f64;
}
```

### 2.3 Heuristics

1. **Exact repetition** (existing): identical output or identical (tool, args) pair.
2. **Consecutive same tool**: if `tool_name` repeats more than `max_consecutive_same_tool`, return `Warning`, then `Break`.
3. **Result diversity**: compute the ratio of distinct output hashes in the last `stagnation_window` invocations. If below threshold, signal stagnation.

All heuristics use stdlib hashing only — no new NLP dependency.

---

## 3. `YOLOGuardrails`

### 3.1 Purpose

Decide when an auto-execution loop should stop, ask the user, or continue. Applies to YOLO and any other high-autonomy mode.

### 3.2 Draft API

```rust
pub struct YoloGuardrails {
    pub max_tool_calls_per_turn: usize,
    pub max_consecutive_same_tool: usize,
    pub stagnation_window: usize,
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

pub enum GuardrailOutcome {
    Ok,
    Warning(String),
    AskUser { question: String },
    Stop { reason: String },
}

pub struct GuardrailState<'a> {
    pub iteration: usize,
    pub total_tool_calls: usize,
    pub detector: &'a LoopDetector,
}

impl YoloGuardrails {
    pub fn check(&self, state: &GuardrailState<'_>) -> GuardrailOutcome {
        // max tool calls per turn
        if state.total_tool_calls >= self.max_tool_calls_per_turn {
            return GuardrailOutcome::AskUser {
                question: format!(
                    "已调用 {} 次工具仍未收敛，请确认下一步方向或提供更多上下文。",
                    state.total_tool_calls
                ),
            };
        }

        // consecutive same tool
        let (name, count) = state.detector.consecutive_same_tool_count();
        if count >= self.max_consecutive_same_tool {
            return GuardrailOutcome::AskUser {
                question: format!(
                    "工具 '{}' 已连续调用 {} 次，未获得新进展。请提供更多信息或让我停止。",
                    name, count
                ),
            };
        }

        // stagnation
        let diversity = state.detector.result_diversity_score(self.stagnation_window);
        if diversity < self.stagnation_threshold {
            return GuardrailOutcome::AskUser {
                question: String::from(
                    "最近几次工具调用结果相似度很高，似乎没有新信息。请确认是否需要调整策略。",
                ),
            };
        }

        GuardrailOutcome::Ok
    }
}
```

### 3.3 Integration

- `AgentConfig` gains `yolo_guardrails: YoloGuardrails`.
- `run_loop_iterations` creates `GuardrailState` each iteration and calls `guardrails.check(&state)` after dispatching tool calls.
- `GuardrailOutcome::AskUser` is converted to `DispatchOutcome::Break { final_response: question, is_error: false }`.

---

## 4. `AgentLifecycle` State Machine

### 4.1 Purpose

Replace implicit flags with an explicit state enum. This is Phase 4 and can be deferred.

### 4.2 Draft Enum

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentState {
    Idle,
    Planning,
    AwaitingTools,
    Synthesizing,
    AwaitingUser,
    Interrupted,
    Complete,
    Error(AgentError),
}

impl AgentState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, AgentState::Complete | AgentState::Interrupted | AgentState::Error(_))
    }

    pub fn is_waiting_for_user(&self) -> bool {
        matches!(self, AgentState::AwaitingUser)
    }
}
```

### 4.3 Transition Sketch

```text
Idle ──UserTurn──► Planning ──LLM calls tool──► AwaitingTools
AwaitingTools ──dispatch success──► Synthesizing ──LLM no tool calls──► Complete
AwaitingTools ──ask_user triggered──► AwaitingUser ──UserTurn──► Planning
Any ──Interrupt/Cancel──► Interrupted
Any ──Fatal error──► Error
```

---

## 5. Open Questions

1. Should `ToolPromptManager` live in `Agent` or be passed as an explicit parameter to `run_loop_iterations`?
2. Should `YoloGuardrails` apply to `ApprovalMode::Smart` batch grants as well, or only to YOLO/Plan?
3. Do we need a new `WireMessage` variant for guardrail-triggered pauses, or can we reuse the existing `ask_user` flow?
4. For `result_diversity_score`, is `DefaultHasher` sufficient, or should we use a stable hash (e.g., `seahash`) to avoid process-dependent variation in tests?

---

## 6. Migration Checklist

- [ ] Phase 0: this RFC and ADR-019 approved.
- [ ] Phase 1: `ToolRegistry` factories consolidated; `is_tool_allowed` predicate introduced.
- [ ] Phase 2: `ToolPromptManager` integrated; unit test proves no repeated full-schema serialization.
- [ ] Phase 3: `LoopDetector` v2 + `YOLOGuardrails` integrated; integration test for stagnation.
- [ ] Phase 4: `AgentState` introduced; `AgentLoop` trait simplified.
