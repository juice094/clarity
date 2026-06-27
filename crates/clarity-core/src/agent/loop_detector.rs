use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::collections::HashSet;

/// Metadata captured for a single tool invocation.
#[derive(Debug, Clone)]
pub struct ToolInvocation {
    /// Tool name.
    pub tool_name: String,
    /// Serialized arguments.
    pub args: String,
    /// Tool output.
    pub output: String,
    /// Cached hash of the output.
    pub output_hash: u64,
}

impl ToolInvocation {
    fn new(tool_name: &str, args: &str, output: &str) -> Self {
        Self {
            tool_name: tool_name.to_string(),
            args: args.to_string(),
            output: output.to_string(),
            output_hash: hash_u64(output),
        }
    }
}

/// Compute a 64-bit hash from SHA-256 (first 8 bytes).
///
/// Uses SHA-256 for collision resistance, truncated to u64 for storage
/// compactness. Birthday bound at ~2³² invocations — well above any
/// practical per-turn tool call volume.
fn hash_u64(data: &str) -> u64 {
    let digest = Sha256::digest(data.as_bytes());
    u64::from_le_bytes(digest[..8].try_into().unwrap_or([0; 8]))
}

/// Result of a loop detection check.
#[derive(Debug)]
pub enum LoopDetection {
    /// No loop detected.
    Ok,
    /// Warning: repetitive pattern detected; a system message should be injected.
    Warning {
        /// Tool name.
        tool_name: String,
        /// Warning message.
        message: String,
    },
    /// Break: loop confirmed; the turn should be hard-terminated.
    Break {
        /// Tool name.
        tool_name: String,
        /// Break message.
        message: String,
    },
}

/// Detects repeated tool-call patterns within a single turn to prevent
/// the LLM from getting stuck in an infinite retry loop.
///
/// Detection strategies:
/// 1. **Hash-based**: identical outputs from the same tool trigger break.
/// 2. **Identical-pattern**: same tool+args called repeatedly triggers warning.
/// 3. **Semantic similarity** (Jaccard on lines): near-duplicate outputs from
///    the same tool (e.g. reading overlapping file ranges) trigger a warning
///    before the hash-based break.
///
/// ponytail: uses line-set Jaccard similarity (O(lines) per check). Upgrade to
/// MinHash or embedding-based similarity if outputs routinely exceed 10k lines
/// and overhead becomes measurable.
pub struct LoopDetector {
    /// Max number of identical outputs from the same tool before we treat it as a loop.
    max_repetitions: usize,
    /// Map: (tool_name) -> Vec<output_hash>
    tool_outputs: HashMap<String, Vec<u64>>,
    /// Map: (tool_name:args_hash) -> Vec<args_hash>
    tool_patterns: HashMap<String, Vec<u64>>,
    /// Ordered history of recent tool invocations for stagnation analysis.
    recent_invocations: Vec<ToolInvocation>,
    /// Recent output line-sets for semantic similarity detection, keyed by tool name.
    /// Stores up to `semantic_history_len` entries per tool.
    semantic_history: HashMap<String, Vec<HashSet<String>>>,
    /// How many recent outputs per tool to compare against for semantic similarity.
    semantic_history_len: usize,
    /// Jaccard similarity threshold above which two outputs are considered "too similar".
    /// Range 0.0–1.0. Default 0.85.
    semantic_threshold: f64,
}

impl LoopDetector {
    /// Create a new `LoopDetector`.
    pub fn new(max_repetitions: usize) -> Self {
        Self {
            max_repetitions,
            tool_outputs: HashMap::new(),
            tool_patterns: HashMap::new(),
            recent_invocations: Vec::new(),
            semantic_history: HashMap::new(),
            semantic_history_len: 4,
            semantic_threshold: 0.85,
        }
    }

    /// Configure semantic similarity detection.
    ///
    /// `history_len` — how many recent outputs (per tool) to compare against.
    /// `threshold` — Jaccard similarity threshold (0.0–1.0). 0.85 means outputs
    /// that share ≥85% of their lines are flagged as suspicious.
    pub fn with_semantic_detection(mut self, history_len: usize, threshold: f64) -> Self {
        self.semantic_history_len = history_len;
        self.semantic_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Disable semantic similarity detection (hash-only mode).
    pub fn without_semantic_detection(mut self) -> Self {
        self.semantic_history_len = 0;
        self
    }

    /// Record a tool execution result.
    ///
    /// Returns:
    /// - `LoopDetection::Ok` if no loop is detected.
    /// - `LoopDetection::Warning` if a repetitive pattern is detected (2 identical outputs
    ///   or 2 identical tool+args patterns).
    /// - `LoopDetection::Break` if the tool has produced the same output `max_repetitions` times.
    pub fn record(&mut self, tool_name: &str, args: &str, output: &str) -> LoopDetection {
        let output_hash = hash_u64(output);
        let args_hash = hash_u64(args);

        // Record the invocation before any early returns so stagnation analysis
        // sees the full history.
        self.recent_invocations
            .push(ToolInvocation::new(tool_name, args, output));

        // 1. Check identical-output repetition.
        let output_hashes = self.tool_outputs.entry(tool_name.to_string()).or_default();
        output_hashes.push(output_hash);
        let output_count = output_hashes.iter().filter(|&&h| h == output_hash).count();

        if output_count >= self.max_repetitions {
            return LoopDetection::Break {
                tool_name: tool_name.to_string(),
                message: format!(
                    "Loop detected: tool '{}' produced identical output {} times",
                    tool_name, output_count
                ),
            };
        }

        if output_count == 2 {
            return LoopDetection::Warning {
                tool_name: tool_name.to_string(),
                message: format!(
                    "注意：工具 '{}' 连续返回相同结果，请尝试不同策略",
                    tool_name
                ),
            };
        }

        // 2. Check identical (tool + args) pattern repetition.
        let pattern_key = format!("{}:{:x}", tool_name, args_hash);
        let patterns = self.tool_patterns.entry(pattern_key).or_default();
        patterns.push(args_hash);
        let pattern_count = patterns.len();

        if pattern_count >= 2 {
            return LoopDetection::Warning {
                tool_name: tool_name.to_string(),
                message: format!(
                    "注意：工具 '{}' 被以相同参数连续调用，请尝试不同策略",
                    tool_name
                ),
            };
        }

        // 3. Semantic similarity check: flag near-duplicate outputs that differ
        //    slightly (e.g. same file read with adjacent offsets) but are
        //    structurally nearly identical.
        if self.semantic_history_len > 0 {
            if let Some(detection) = self.check_semantic_loop(tool_name, output) {
                return detection;
            }
        }

        LoopDetection::Ok
    }

    /// Reset for a new turn.
    pub fn reset(&mut self) {
        self.tool_outputs.clear();
        self.tool_patterns.clear();
        self.recent_invocations.clear();
        self.semantic_history.clear();
    }

    /// Return the configured max repetitions threshold.
    pub fn max_repetitions(&self) -> usize {
        self.max_repetitions
    }

    /// Count consecutive calls to the same tool at the end of the invocation history.
    ///
    /// Returns `(tool_name, count)`. If the history is empty, returns `("", 0)`.
    pub fn consecutive_same_tool_count(&self) -> (String, usize) {
        let last = match self.recent_invocations.last() {
            Some(last) => last,
            None => return (String::new(), 0),
        };
        let last_name = last.tool_name.clone();
        let count = self
            .recent_invocations
            .iter()
            .rev()
            .take_while(|inv| inv.tool_name == last_name)
            .count();
        (last_name, count)
    }

    /// Compute the diversity score of the last `window` outputs.
    ///
    /// Score = distinct_output_hashes / window_size.
    /// Returns 1.0 when the window is empty or smaller than requested.
    pub fn result_diversity_score(&self, window: usize) -> f64 {
        if window == 0 || self.recent_invocations.is_empty() {
            return 1.0;
        }
        let window_size = self.recent_invocations.len().min(window);
        let start = self.recent_invocations.len() - window_size;
        let recent = &self.recent_invocations[start..];
        let distinct: std::collections::HashSet<u64> =
            recent.iter().map(|inv| inv.output_hash).collect();
        distinct.len() as f64 / window_size as f64
    }

    /// Read-only access to recent invocations for diagnostics.
    #[allow(dead_code)]
    pub fn recent_invocations(&self) -> &[ToolInvocation] {
        &self.recent_invocations
    }

    // ── Semantic similarity detection ──────────────────────────────────────

    /// Check whether `output` is semantically too similar to recent outputs
    /// of the same tool (using line-set Jaccard similarity).
    ///
    /// Returns a [`LoopDetection::Warning`] if the Jaccard similarity with any
    /// recent output exceeds the configured threshold and the outputs are not
    /// hash-identical (hash-identical outputs are caught by the primary check).
    fn check_semantic_loop(&mut self, tool_name: &str, output: &str) -> Option<LoopDetection> {
        if output.is_empty() {
            return None;
        }

        let current_lines: HashSet<String> = output.lines().map(|l| l.to_string()).collect();

        let history = self
            .semantic_history
            .entry(tool_name.to_string())
            .or_default();

        for past_lines in history.iter() {
            let similarity = jaccard_similarity(&current_lines, past_lines);
            if similarity >= self.semantic_threshold {
                // Don't double-warn if the hash-based check already caught it.
                return Some(LoopDetection::Warning {
                    tool_name: tool_name.to_string(),
                    message: format!(
                        "注意：工具 '{}' 的输出与最近一次调用的输出高度相似（{:.0}% 重叠），\
                         可能陷入重复查询循环，请尝试更具体的参数或不同策略",
                        tool_name,
                        similarity * 100.0
                    ),
                });
            }
        }

        // Store for future comparisons, trimming to history limit.
        if history.len() >= self.semantic_history_len {
            history.remove(0);
        }
        history.push(current_lines);

        None
    }
}

/// Compute Jaccard similarity between two line-sets.
///
/// Jaccard = |A ∩ B| / |A ∪ B|.
/// Returns 1.0 when both sets are empty, 0.0 when disjoint.
fn jaccard_similarity(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let intersection = a.intersection(b).count();
    let union = a.union(b).count(); // union() already deduplicates
    if union == 0 {
        return 1.0;
    }
    intersection as f64 / union as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_loop_different_outputs() {
        let mut detector = LoopDetector::new(3);
        assert!(matches!(
            detector.record("read_file", r#"{"path": "a"}"#, "content A"),
            LoopDetection::Ok
        ));
        assert!(matches!(
            detector.record("read_file", r#"{"path": "b"}"#, "content B"),
            LoopDetection::Ok
        ));
        assert!(matches!(
            detector.record("read_file", r#"{"path": "c"}"#, "content C"),
            LoopDetection::Ok
        ));
    }

    #[test]
    fn test_loop_same_output() {
        let mut detector = LoopDetector::new(3);
        assert!(matches!(
            detector.record("read_file", "{}", "same content"),
            LoopDetection::Ok
        ));
        assert!(matches!(
            detector.record("read_file", "{}", "same content"),
            LoopDetection::Warning { .. }
        ));
        assert!(matches!(
            detector.record("read_file", "{}", "same content"),
            LoopDetection::Break { .. }
        ));
    }

    #[test]
    fn test_loop_reset() {
        let mut detector = LoopDetector::new(3);
        assert!(matches!(
            detector.record("read_file", "{}", "same content"),
            LoopDetection::Ok
        ));
        assert!(matches!(
            detector.record("read_file", "{}", "same content"),
            LoopDetection::Warning { .. }
        ));
        detector.reset();
        assert!(matches!(
            detector.record("read_file", "{}", "same content"),
            LoopDetection::Ok
        ));
        assert!(matches!(
            detector.record("read_file", "{}", "same content"),
            LoopDetection::Warning { .. }
        ));
        assert!(matches!(
            detector.record("read_file", "{}", "same content"),
            LoopDetection::Break { .. }
        ));
    }

    #[test]
    fn test_different_tools_no_interference() {
        let mut detector = LoopDetector::new(3);
        // tool_a builds up to threshold
        let r1 = detector.record("tool_a", r#"{"arg": 1}"#, "output");
        assert!(matches!(r1, LoopDetection::Ok), "expected Ok, got {:?}", r1);
        let r2 = detector.record("tool_a", r#"{"arg": 1}"#, "output");
        assert!(
            matches!(r2, LoopDetection::Warning { .. }),
            "expected Warning, got {:?}",
            r2
        );
        // tool_b with different args and output has its own counter
        let r3 = detector.record("tool_b", r#"{"arg": 2}"#, "different");
        assert!(
            matches!(r3, LoopDetection::Ok),
            "expected Ok for tool_b first call, got {:?}",
            r3
        );
        // tool_a triggers on its 3rd identical output
        let r4 = detector.record("tool_a", r#"{"arg": 1}"#, "output");
        assert!(
            matches!(r4, LoopDetection::Break { .. }),
            "expected Break, got {:?}",
            r4
        );
        // tool_b second call with same output -> Warning (output_count == 2)
        let r5 = detector.record("tool_b", r#"{"arg": 2}"#, "different");
        assert!(
            matches!(r5, LoopDetection::Warning { .. }),
            "expected Warning for tool_b second call, got {:?}",
            r5
        );
    }

    #[test]
    fn test_warning_same_output_twice() {
        let mut detector = LoopDetector::new(3);
        assert!(matches!(
            detector.record("tool", "{}", "output"),
            LoopDetection::Ok
        ));
        let result = detector.record("tool", "{}", "output");
        assert!(matches!(&result, LoopDetection::Warning { tool_name, .. } if tool_name == "tool"));
    }

    #[test]
    fn test_break_same_output_three_times() {
        let mut detector = LoopDetector::new(3);
        detector.record("tool", "{}", "output");
        detector.record("tool", "{}", "output");
        let result = detector.record("tool", "{}", "output");
        assert!(matches!(&result, LoopDetection::Break { tool_name, .. } if tool_name == "tool"));
    }

    #[test]
    fn test_warning_same_pattern_twice() {
        let mut detector = LoopDetector::new(3);
        assert!(matches!(
            detector.record("tool", r#"{"path": "a"}"#, "output A"),
            LoopDetection::Ok
        ));
        let result = detector.record("tool", r#"{"path": "a"}"#, "output B");
        assert!(matches!(&result, LoopDetection::Warning { tool_name, .. } if tool_name == "tool"));
        // A third identical pattern call is still a Warning (not Break) because
        // the outputs differ, so output repetition never reaches the threshold.
        let result = detector.record("tool", r#"{"path": "a"}"#, "output C");
        assert!(matches!(&result, LoopDetection::Warning { tool_name, .. } if tool_name == "tool"));
    }

    #[test]
    fn test_ok_different_args_same_output() {
        let mut detector = LoopDetector::new(3);
        // Same tool, different args, same output -> output counter accumulates,
        // pattern counter is separate per args.
        assert!(matches!(
            detector.record("tool", r#"{"path": "a"}"#, "same"),
            LoopDetection::Ok
        ));
        // 2nd identical output -> Warning (output repetition)
        assert!(matches!(
            detector.record("tool", r#"{"path": "b"}"#, "same"),
            LoopDetection::Warning { .. }
        ));
        // 3rd identical output -> Break
        assert!(matches!(
            detector.record("tool", r#"{"path": "c"}"#, "same"),
            LoopDetection::Break { .. }
        ));
    }

    // ── Semantic similarity tests ──────────────────────────────────────────

    #[test]
    fn semantic_near_duplicate_triggers_warning() {
        let mut detector = LoopDetector::new(5).with_semantic_detection(3, 0.6);

        // First call: Ok (no history yet). Use distinct args to avoid
        // the identical-pattern check triggering first.
        let r1 = detector.record(
            "read",
            r#"{"path": "file.txt", "offset": 0}"#,
            "line A\nline B\nline C\nline D\nline E",
        );
        assert!(matches!(r1, LoopDetection::Ok));

        // Second call: 4/5 lines shared with first -> Jaccard = 4/6 ≈ 0.667 >= 0.6
        // -> semantic Warning
        let r2 = detector.record(
            "read",
            r#"{"path": "file.txt", "offset": 1}"#,
            "line A\nline B\nline C\nline D\nline F",
        );
        assert!(
            matches!(&r2, LoopDetection::Warning { tool_name, .. } if tool_name == "read"),
            "Expected semantic Warning, got {:?}",
            r2
        );
    }

    #[test]
    fn semantic_different_outputs_no_warning() {
        let mut detector = LoopDetector::new(5).with_semantic_detection(3, 0.5);

        // Use DIFFERENT args to avoid the identical-pattern check.
        detector.record("read", r#"{"a":1}"#, "apple\nbanana\ncherry");
        let r = detector.record("read", r#"{"a":2}"#, "xylophone\nyak\nzebra");

        // 0/6 overlap = 0.0 → Ok (no semantic Warning)
        assert!(matches!(r, LoopDetection::Ok));
    }

    #[test]
    fn semantic_disabled_no_warning() {
        let mut detector = LoopDetector::new(5).without_semantic_detection();

        // Different args to avoid identical-pattern check.
        detector.record(
            "read",
            r#"{"a":1}"#,
            "line A\nline B\nline C\nline D\nline E",
        );
        let r = detector.record(
            "read",
            r#"{"a":2}"#,
            "line A\nline B\nline C\nline D\nline F",
        );

        // Semantic detection is off → no semantic Warning for near-duplicates.
        // Different args means no identical-pattern Warning either → Ok.
        assert!(matches!(r, LoopDetection::Ok));
    }

    #[test]
    fn semantic_reset_clears_history() {
        let mut detector = LoopDetector::new(5).with_semantic_detection(3, 0.5);

        detector.record(
            "read",
            r#"{"a":1}"#,
            "line A\nline B\nline C\nline D\nline E",
        );
        detector.reset();
        // After reset, no history → a near-duplicate is Ok (no past to compare against)
        let r = detector.record(
            "read",
            r#"{"a":2}"#,
            "line A\nline B\nline C\nline D\nline F",
        );
        assert!(matches!(r, LoopDetection::Ok));
    }

    #[test]
    fn semantic_different_tools_no_cross_contamination() {
        let mut detector = LoopDetector::new(5).with_semantic_detection(3, 0.5);

        // Different args for each call to avoid identical-pattern check.
        detector.record(
            "grep",
            r#"{"a":1}"#,
            "line A\nline B\nline C\nline D\nline E",
        );
        // Same output lines but different tool → semantic history is per-tool → Ok
        let r = detector.record(
            "read",
            r#"{"a":2}"#,
            "line A\nline B\nline C\nline D\nline F",
        );
        assert!(matches!(r, LoopDetection::Ok));
    }

    #[test]
    fn jaccard_empty_sets() {
        let a: HashSet<String> = HashSet::new();
        let b: HashSet<String> = HashSet::new();
        assert!((jaccard_similarity(&a, &b) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn jaccard_identical() {
        let a: HashSet<String> = ["x".into(), "y".into()].into();
        let b: HashSet<String> = ["x".into(), "y".into()].into();
        assert!((jaccard_similarity(&a, &b) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn jaccard_disjoint() {
        let a: HashSet<String> = ["a".into()].into();
        let b: HashSet<String> = ["b".into()].into();
        assert!((jaccard_similarity(&a, &b) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn jaccard_half_overlap() {
        let a: HashSet<String> = ["a".into(), "b".into()].into();
        let b: HashSet<String> = ["b".into(), "c".into()].into();
        // intersection = {"b"} = 1, union = {"a","b","c"} = 3
        assert!((jaccard_similarity(&a, &b) - 1.0 / 3.0).abs() < f64::EPSILON);
    }
}
