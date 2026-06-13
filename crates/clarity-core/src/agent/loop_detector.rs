use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

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
pub struct LoopDetector {
    /// Max number of identical outputs from the same tool before we treat it as a loop.
    max_repetitions: usize,
    /// Map: (tool_name) -> Vec<output_hash>
    tool_outputs: HashMap<String, Vec<u64>>,
    /// Map: (tool_name:args_hash) -> Vec<args_hash>
    tool_patterns: HashMap<String, Vec<u64>>,
}

impl LoopDetector {
    /// Create a new `LoopDetector`.
    pub fn new(max_repetitions: usize) -> Self {
        Self {
            max_repetitions,
            tool_outputs: HashMap::new(),
            tool_patterns: HashMap::new(),
        }
    }

    /// Record a tool execution result.
    ///
    /// Returns:
    /// - `LoopDetection::Ok` if no loop is detected.
    /// - `LoopDetection::Warning` if a repetitive pattern is detected (2 identical outputs
    ///   or 2 identical tool+args patterns).
    /// - `LoopDetection::Break` if the tool has produced the same output `max_repetitions` times.
    pub fn record(&mut self, tool_name: &str, args: &str, output: &str) -> LoopDetection {
        let mut hasher = DefaultHasher::new();
        output.hash(&mut hasher);
        let output_hash = hasher.finish();

        let mut hasher = DefaultHasher::new();
        args.hash(&mut hasher);
        let args_hash = hasher.finish();

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

        LoopDetection::Ok
    }

    /// Reset for a new turn.
    pub fn reset(&mut self) {
        self.tool_outputs.clear();
        self.tool_patterns.clear();
    }

    /// Return the configured max repetitions threshold.
    pub fn max_repetitions(&self) -> usize {
        self.max_repetitions
    }
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
}
