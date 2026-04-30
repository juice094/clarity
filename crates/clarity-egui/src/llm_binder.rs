//! Layer 3 — Binder: idempotent, synchronous Agent↔LLM attachment.

use std::sync::Arc;

/// Idempotent binder: attach a loaded backend to an Agent.
///
/// # Reversibility
/// - Safe to call multiple times (replaces previous binding).
/// - Pair with `unbind_llm` to detach.
pub fn bind_llm(
    agent: &clarity_core::Agent,
    backend: Arc<dyn clarity_core::llm::LlmProvider>,
    label: &str,
) {
    agent.set_llm(backend);
    agent.set_provider_label(label);
}

/// Detach the current LLM from the agent.
pub fn unbind_llm(agent: &clarity_core::Agent) {
    agent.unset_llm();
}
