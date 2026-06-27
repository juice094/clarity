//! Rollout persistence policy.
//!
//! Decides which rollout items are durable and which are ephemeral. Modeled
//! after `codex_rollout::policy` from the OpenAI Codex project, licensed under
//! Apache-2.0. See `NOTICES.md` for attribution.

use clarity_contract::{RolloutEventMsg, RolloutItem, RolloutResponseItem};

/// Whether a rollout item should be persisted in durable replay history.
pub fn is_persisted_rollout_item(item: &RolloutItem) -> bool {
    match item {
        RolloutItem::ResponseItem(item) => is_persisted_response_item(item),
        RolloutItem::InterAgentCommunication(_) => true,
        RolloutItem::EventMsg(ev) => is_persisted_event_msg(ev),
        RolloutItem::Compacted(_) | RolloutItem::TurnContext(_) | RolloutItem::SessionMeta(_) => {
            true
        }
    }
}

/// Return the canonical rollout items that should be persisted for a live append.
pub fn persisted_rollout_items(items: &[RolloutItem]) -> Vec<RolloutItem> {
    items
        .iter()
        .filter(|item| is_persisted_rollout_item(item))
        .cloned()
        .collect()
}

/// Whether a response item should be persisted in durable replay history.
#[inline]
pub fn is_persisted_response_item(item: &RolloutResponseItem) -> bool {
    match item {
        RolloutResponseItem::Message { .. }
        | RolloutResponseItem::FunctionCall { .. }
        | RolloutResponseItem::FunctionCallOutput { .. }
        | RolloutResponseItem::Reasoning { .. }
        | RolloutResponseItem::Compaction
        | RolloutResponseItem::ContextCompaction => true,
        RolloutResponseItem::Other(_) => false,
    }
}

/// Whether a response item should be persisted for memory generation.
#[inline]
pub fn is_persisted_response_item_for_memories(item: &RolloutResponseItem) -> bool {
    match item {
        RolloutResponseItem::Message { role, .. } => role != "developer",
        RolloutResponseItem::FunctionCall { .. }
        | RolloutResponseItem::FunctionCallOutput { .. } => true,
        RolloutResponseItem::Reasoning { .. }
        | RolloutResponseItem::Compaction
        | RolloutResponseItem::ContextCompaction
        | RolloutResponseItem::Other(_) => false,
    }
}

/// Whether an event message should be persisted in durable replay history.
#[inline]
pub fn is_persisted_event_msg(ev: &RolloutEventMsg) -> bool {
    match ev {
        RolloutEventMsg::UserMessage(_)
        | RolloutEventMsg::AgentMessage(_)
        | RolloutEventMsg::AgentReasoning(_)
        | RolloutEventMsg::TokenCount(_)
        | RolloutEventMsg::ThreadGoalUpdated(_)
        | RolloutEventMsg::ContextCompacted { .. }
        | RolloutEventMsg::TurnStarted { .. }
        | RolloutEventMsg::TurnComplete { .. }
        | RolloutEventMsg::TurnAborted { .. }
        | RolloutEventMsg::Lifecycle { .. }
        | RolloutEventMsg::SubAgentActivity(_) => true,
        RolloutEventMsg::Error(_) | RolloutEventMsg::Other(_) => false,
    }
}
