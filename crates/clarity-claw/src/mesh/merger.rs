//! CRDT merger for Claw Mesh role contexts.
//!
//! Takes a stream of [`ClawContextEvent`]s from any transport (Gateway sync or
//! syncthing-rust) and converges them into a single [`RoleContext`].
//!
//! # Conflict resolution
//!
//! When two events from different devices target the same scalar field
//! concurrently (i.e., neither device has seen the other's change), the merger
//! applies a deterministic tiebreaker: lower `origin_device` wins, with
//! `origin_clock` as secondary comparison. This follows syncthing-rust's
//! `conflict_resolver.rs` pattern of version-vector dominance + timestamp
//! fallback, adapted for Clarity's scalar clock model.
//!
//! The tiebreaker is surfaced via [`ConflictEvent`] so the UI can notify users
//! when their edit was shadowed.

use clarity_contract::{
    ClawContextEvent, ContextEventKind, RoleContext, RoleContextId, RoleContextMessage,
};
use std::collections::{HashMap, HashSet};

// ============================================================================
// Conflict resolution types
// ============================================================================

/// Strategy for resolving concurrent edits to the same target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictStrategy {
    /// Keep the first-seen event (by event_id insertion order).
    UseFirst,
    /// Replace with the later event (based on clock/device tiebreaker).
    UseLater,
    /// Signal a conflict for external resolution (currently not implemented).
    Signal,
}

/// A conflict detected during merge, surfaced for UI observability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictEvent {
    /// The event_id that was kept.
    pub winner_event_id: String,
    /// The event_id that was discarded.
    pub loser_event_id: String,
    /// The target field that conflicted (e.g., "lifecycle", "EditMessage:<id>").
    pub target: String,
    /// The strategy used to resolve.
    pub strategy: ConflictStrategy,
}

/// Result of a merge operation, including any conflicts detected.
#[derive(Debug, Clone)]
pub struct MergeResult {
    /// The converged role context.
    pub context: RoleContext,
    /// Conflicts detected and resolved during the merge.
    pub conflicts: Vec<ConflictEvent>,
}

/// Determine which of two concurrent events should win for a scalar field.
///
/// This is a deterministic tiebreaker: the event from the lexicographically
/// lower `origin_device` wins. If devices are equal (same device racing with
/// itself), the lower `origin_clock` wins.
///
/// This follows syncthing-rust's pattern of using a total order as fallback
/// when version vectors are incomparable.
pub fn resolve_scalar_conflict(a: &ClawContextEvent, b: &ClawContextEvent) -> ConflictStrategy {
    match a.origin_device.cmp(&b.origin_device) {
        std::cmp::Ordering::Less => ConflictStrategy::UseFirst,
        std::cmp::Ordering::Greater => ConflictStrategy::UseLater,
        std::cmp::Ordering::Equal => {
            // Same device: higher clock wins (newer).
            if a.origin_clock >= b.origin_clock {
                ConflictStrategy::UseFirst
            } else {
                ConflictStrategy::UseLater
            }
        }
    }
}

/// Check if event `a` dominates event `b` for the same target.
///
/// In the current scalar clock model, dominance is determined by:
/// - Same device: higher clock dominates lower.
/// - Different devices: neither dominates (concurrent) — tiebreaker needed.
///
/// Returns `true` if `a` definitively supersedes `b`.
pub fn dominates(a: &ClawContextEvent, b: &ClawContextEvent) -> bool {
    if a.origin_device == b.origin_device {
        a.origin_clock > b.origin_clock
    } else {
        // Different devices: neither dominates without vector clocks.
        false
    }
}

/// Merge events into a role context.
///
/// Events are de-duplicated by `event_id`, sorted by `(origin_clock, event_id)`,
/// and applied in order. Scalar field conflicts (two concurrent events updating
/// the same field from different devices) are resolved deterministically: lower
/// `origin_device` wins, with `origin_clock` as secondary tiebreaker.
///
/// For detailed conflict reporting, use [`merge_events_detailed`].
pub fn merge_events(role_id: RoleContextId, events: &[ClawContextEvent]) -> RoleContext {
    merge_events_detailed(role_id, events).context
}

/// Merge events into a role context with conflict reporting.
///
/// Returns a [`MergeResult`] containing both the converged context and any
/// conflicts that were detected and resolved during the merge.
pub fn merge_events_detailed(role_id: RoleContextId, events: &[ClawContextEvent]) -> MergeResult {
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
    let mut conflicts: Vec<ConflictEvent> = Vec::new();

    // Track which event last modified each scalar field for conflict detection.
    let mut last_lifecycle_event: Option<&ClawContextEvent> = None;
    let mut last_archive_event: Option<&ClawContextEvent> = None;
    let mut last_metadata_events: HashMap<String, &ClawContextEvent> = HashMap::new();
    let mut edit_targets: HashMap<String, &ClawContextEvent> = HashMap::new();

    for event in &ordered {
        match &event.kind {
            ContextEventKind::SetLifecycle { .. } => {
                if let Some(prev) = last_lifecycle_event
                    && !dominates(event, prev)
                    && !dominates(prev, event)
                {
                    let strategy = resolve_scalar_conflict(event, prev);
                    conflicts.push(ConflictEvent {
                        winner_event_id: match strategy {
                            ConflictStrategy::UseFirst => event.event_id.clone(),
                            ConflictStrategy::UseLater => prev.event_id.clone(),
                            ConflictStrategy::Signal => event.event_id.clone(),
                        },
                        loser_event_id: match strategy {
                            ConflictStrategy::UseFirst => prev.event_id.clone(),
                            ConflictStrategy::UseLater => event.event_id.clone(),
                            ConflictStrategy::Signal => prev.event_id.clone(),
                        },
                        target: "lifecycle".to_string(),
                        strategy,
                    });
                    if matches!(strategy, ConflictStrategy::UseLater) {
                        // Skip this event — the previous one wins.
                        continue;
                    }
                }
                last_lifecycle_event = Some(event);
            }
            ContextEventKind::Archive { .. } => {
                if let Some(prev) = last_archive_event
                    && !dominates(event, prev)
                    && !dominates(prev, event)
                {
                    let strategy = resolve_scalar_conflict(event, prev);
                    conflicts.push(ConflictEvent {
                        winner_event_id: match strategy {
                            ConflictStrategy::UseFirst => event.event_id.clone(),
                            _ => prev.event_id.clone(),
                        },
                        loser_event_id: match strategy {
                            ConflictStrategy::UseFirst => prev.event_id.clone(),
                            _ => event.event_id.clone(),
                        },
                        target: "archived".to_string(),
                        strategy,
                    });
                    if matches!(strategy, ConflictStrategy::UseLater) {
                        continue;
                    }
                }
                last_archive_event = Some(event);
            }
            ContextEventKind::EditMessage {
                target_event_id,
                content: _,
            } => {
                if let Some(prev) = edit_targets.get(target_event_id)
                    && !dominates(event, prev)
                    && !dominates(prev, event)
                {
                    let strategy = resolve_scalar_conflict(event, prev);
                    conflicts.push(ConflictEvent {
                        winner_event_id: match strategy {
                            ConflictStrategy::UseFirst => event.event_id.clone(),
                            _ => prev.event_id.clone(),
                        },
                        loser_event_id: match strategy {
                            ConflictStrategy::UseFirst => prev.event_id.clone(),
                            _ => event.event_id.clone(),
                        },
                        target: format!("EditMessage:{}", target_event_id),
                        strategy,
                    });
                    if matches!(strategy, ConflictStrategy::UseLater) {
                        continue;
                    }
                }
                edit_targets.insert(target_event_id.clone(), event);
            }
            ContextEventKind::UpdateMetadata { deltas } => {
                for key in deltas.keys() {
                    if let Some(prev) = last_metadata_events.get(key)
                        && !dominates(event, prev)
                        && !dominates(prev, event)
                    {
                        let strategy = resolve_scalar_conflict(event, prev);
                        conflicts.push(ConflictEvent {
                            winner_event_id: match strategy {
                                ConflictStrategy::UseFirst => event.event_id.clone(),
                                _ => prev.event_id.clone(),
                            },
                            loser_event_id: match strategy {
                                ConflictStrategy::UseFirst => prev.event_id.clone(),
                                _ => event.event_id.clone(),
                            },
                            target: format!("metadata:{}", key),
                            strategy,
                        });
                    }
                    last_metadata_events.insert(key.clone(), event);
                }
            }
            // AppendMessage does not conflict — messages are additive.
            ContextEventKind::AppendMessage { .. } => {}
        }

        apply_event(event, &mut context, &mut message_index);
    }

    MergeResult { context, conflicts }
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
            if let Some(&idx) = message_index.get(target_event_id)
                && let Some(msg) = context.messages.get_mut(idx)
            {
                msg.content = content.clone();
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

    // ======================================================================
    // Conflict resolution tests
    // ======================================================================

    #[test]
    fn dominates_same_device_higher_clock_wins() {
        let newer = append("a", "dev-1", 5, "newer");
        let older = append("b", "dev-1", 3, "older");
        assert!(dominates(&newer, &older));
        assert!(!dominates(&older, &newer));
    }

    #[test]
    fn dominates_different_devices_neither_wins() {
        let a = append("a", "dev-1", 5, "from-dev1");
        let b = append("b", "dev-2", 3, "from-dev2");
        assert!(!dominates(&a, &b));
        assert!(!dominates(&b, &a));
    }

    #[test]
    fn resolve_scalar_conflict_lower_device_wins() {
        let a = append("a", "device-a", 1, "from-a");
        let b = append("b", "device-b", 5, "from-b");
        // device-a < device-b lexicographically, so a wins.
        assert_eq!(resolve_scalar_conflict(&a, &b), ConflictStrategy::UseFirst);
    }

    #[test]
    fn resolve_scalar_conflict_same_device_higher_clock_wins() {
        let newer = append("a", "dev-1", 10, "newer");
        let older = append("b", "dev-1", 5, "older");
        assert_eq!(
            resolve_scalar_conflict(&newer, &older),
            ConflictStrategy::UseFirst
        );
        assert_eq!(
            resolve_scalar_conflict(&older, &newer),
            ConflictStrategy::UseLater
        );
    }

    #[test]
    fn merge_detected_conflict_on_concurrent_lifecycle() {
        let events = vec![
            ClawContextEvent {
                event_id: "life-1".into(),
                origin_device: "dev-a".into(),
                origin_clock: 1,
                kind: ContextEventKind::SetLifecycle {
                    lifecycle: "draft".into(),
                },
            },
            ClawContextEvent {
                event_id: "life-2".into(),
                origin_device: "dev-b".into(),
                origin_clock: 1,
                kind: ContextEventKind::SetLifecycle {
                    lifecycle: "review".into(),
                },
            },
        ];
        let result = merge_events_detailed(RoleContextId::new("op"), &events);

        // dev-a < dev-b, so dev-a's "draft" wins (the previous event).
        assert_eq!(result.context.lifecycle, "draft");
        assert!(!result.conflicts.is_empty());
        let conflict = &result.conflicts[0];
        assert_eq!(conflict.target, "lifecycle");
        assert_eq!(conflict.strategy, ConflictStrategy::UseLater);
    }

    #[test]
    fn merge_no_conflict_when_one_dominates() {
        let events = vec![
            ClawContextEvent {
                event_id: "life-1".into(),
                origin_device: "dev-1".into(),
                origin_clock: 1,
                kind: ContextEventKind::SetLifecycle {
                    lifecycle: "early".into(),
                },
            },
            ClawContextEvent {
                event_id: "life-2".into(),
                origin_device: "dev-1".into(),
                origin_clock: 5, // Same device, higher clock dominates.
                kind: ContextEventKind::SetLifecycle {
                    lifecycle: "later".into(),
                },
            },
        ];
        let result = merge_events_detailed(RoleContextId::new("op"), &events);
        assert_eq!(result.context.lifecycle, "later");
        assert!(result.conflicts.is_empty());
    }
}
