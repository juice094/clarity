//! CRDT merger for Claw Mesh role contexts.
//!
//! Takes a stream of [`ClawContextEvent`]s from any transport (Gateway sync or
//! syncthing-rust) and converges them into a single [`RoleContext`].

use clarity_contract::{
    ClawContextEvent, ContextEventKind, RoleContext, RoleContextId, RoleContextMessage,
};
use std::collections::{HashMap, HashSet};

/// Merge events into a role context.
///
/// The algorithm is intentionally simple: events are de-duplicated by
/// `event_id`, sorted by `(origin_clock, event_id)`, and applied in order.
/// Concurrent edits to the same scalar field resolve to last-write-wins using
/// the same total order.
///
/// ponytail: covers independent agent-state updates; if cross-role causal
/// dependencies appear later, introduce HLC/vector clocks.
pub fn merge_events(role_id: RoleContextId, events: &[ClawContextEvent]) -> RoleContext {
    let mut seen = HashSet::new();
    let mut ordered: Vec<&ClawContextEvent> = Vec::with_capacity(events.len());

    for event in events {
        if seen.insert(event.event_id.clone()) {
            ordered.push(event);
        }
    }

    ordered.sort_by(|a, b| {
        a.origin_clock
            .cmp(&b.origin_clock)
            .then_with(|| a.event_id.cmp(&b.event_id))
    });

    let mut context = RoleContext {
        role_id,
        messages: Vec::new(),
        lifecycle: String::new(),
        archived: false,
        metadata: HashMap::new(),
    };

    let mut message_index: HashMap<String, usize> = HashMap::new();

    for event in ordered {
        apply_event(event, &mut context, &mut message_index);
    }

    context
}

fn apply_event(
    event: &ClawContextEvent,
    context: &mut RoleContext,
    message_index: &mut HashMap<String, usize>,
) {
    match &event.kind {
        ContextEventKind::AppendMessage { role, content } => {
            if let Some(idx) = message_index.get(&event.event_id) {
                // Idempotent: already have this message.
                let _ = *idx;
            } else {
                let idx = context.messages.len();
                context.messages.push(RoleContextMessage {
                    event_id: event.event_id.clone(),
                    origin_device: event.origin_device.clone(),
                    origin_clock: event.origin_clock,
                    role: role.clone(),
                    content: content.clone(),
                });
                message_index.insert(event.event_id.clone(), idx);
            }
        }
        ContextEventKind::EditMessage {
            target_event_id,
            content,
        } => {
            if let Some(&idx) = message_index.get(target_event_id) {
                if let Some(msg) = context.messages.get_mut(idx) {
                    msg.content = content.clone();
                }
            }
            // If target not yet seen, drop the edit. A real-time system should
            // buffer or re-apply; ponytail: assume causal delivery via sync.
        }
        ContextEventKind::SetLifecycle { lifecycle } => {
            context.lifecycle = lifecycle.clone();
        }
        ContextEventKind::UpdateMetadata { deltas } => {
            for (k, v) in deltas {
                context.metadata.insert(k.clone(), v.clone());
            }
        }
        ContextEventKind::Archive { archived } => {
            context.archived = *archived;
        }
    }
}

/// Incremental merge: apply newly arrived events to an existing context.
///
/// Events already present (by `event_id`) are skipped.
pub fn merge_into(context: &mut RoleContext, new_events: &[ClawContextEvent]) {
    let role_id = context.role_id.clone();
    let mut combined: Vec<ClawContextEvent> = context
        .messages
        .iter()
        .map(|m| ClawContextEvent {
            event_id: m.event_id.clone(),
            origin_device: m.origin_device.clone(),
            origin_clock: m.origin_clock,
            kind: ContextEventKind::AppendMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            },
        })
        .collect();

    // Replay scalar events by synthesizing them from current state. This is a
    // simplification: we lose the original event ids for scalar updates, but
    // merge_events will produce the same converged state from the inputs.
    // ponytail: for production, persist the raw event log instead of the
    // converged snapshot.
    if !context.lifecycle.is_empty() {
        combined.push(ClawContextEvent {
            event_id: "__lifecycle".into(),
            origin_device: "__local".into(),
            origin_clock: 0,
            kind: ContextEventKind::SetLifecycle {
                lifecycle: context.lifecycle.clone(),
            },
        });
    }
    if context.archived {
        combined.push(ClawContextEvent {
            event_id: "__archive".into(),
            origin_device: "__local".into(),
            origin_clock: 0,
            kind: ContextEventKind::Archive { archived: true },
        });
    }
    for (k, v) in &context.metadata {
        let mut deltas = HashMap::new();
        deltas.insert(k.clone(), v.clone());
        combined.push(ClawContextEvent {
            event_id: format!("__meta:{}", k),
            origin_device: "__local".into(),
            origin_clock: 0,
            kind: ContextEventKind::UpdateMetadata { deltas },
        });
    }

    combined.extend(new_events.iter().cloned());
    *context = merge_events(role_id, &combined);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn append(event_id: &str, device: &str, clock: u64, content: &str) -> ClawContextEvent {
        ClawContextEvent {
            event_id: event_id.into(),
            origin_device: device.into(),
            origin_clock: clock,
            kind: ContextEventKind::AppendMessage {
                role: "user".into(),
                content: content.into(),
            },
        }
    }

    #[test]
    fn merge_sorts_by_clock_then_event_id() {
        let events = vec![
            append("b", "dev-1", 2, "second"),
            append("a", "dev-1", 2, "first by id"),
            append("c", "dev-1", 1, "earliest"),
        ];
        let ctx = merge_events(RoleContextId::new("op"), &events);
        assert_eq!(ctx.messages.len(), 3);
        assert_eq!(ctx.messages[0].content, "earliest");
        assert_eq!(ctx.messages[1].content, "first by id");
        assert_eq!(ctx.messages[2].content, "second");
    }

    #[test]
    fn merge_deduplicates_by_event_id() {
        let events = vec![
            append("a", "dev-1", 1, "once"),
            append("a", "dev-2", 1, "twice"),
        ];
        let ctx = merge_events(RoleContextId::new("op"), &events);
        assert_eq!(ctx.messages.len(), 1);
        assert_eq!(ctx.messages[0].content, "once");
    }

    #[test]
    fn merge_applies_lifecycle_and_archive() {
        let events = vec![
            append("msg-1", "dev-1", 1, "hi"),
            ClawContextEvent {
                event_id: "arch".into(),
                origin_device: "dev-1".into(),
                origin_clock: 2,
                kind: ContextEventKind::Archive { archived: true },
            },
        ];
        let ctx = merge_events(RoleContextId::new("op"), &events);
        assert!(ctx.archived);
        assert_eq!(ctx.messages.len(), 1);
    }

    #[test]
    fn merge_into_preserves_existing_state() {
        let mut ctx = merge_events(
            RoleContextId::new("op"),
            &[append("a", "dev-1", 1, "original")],
        );
        merge_into(&mut ctx, &[append("b", "dev-2", 2, "new")]);
        assert_eq!(ctx.messages.len(), 2);
        assert_eq!(ctx.messages[1].content, "new");
    }
}
