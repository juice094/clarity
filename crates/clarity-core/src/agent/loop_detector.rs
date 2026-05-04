use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// Detects repeated tool-call patterns within a single turn to prevent
/// the LLM from getting stuck in an infinite retry loop.
pub struct LoopDetector {
    /// Max number of identical outputs from the same tool before we treat it as a loop.
    max_repetitions: usize,
    /// Map: (tool_name) -> Vec<output_hash>
    tool_outputs: HashMap<String, Vec<u64>>,
}

impl LoopDetector {
    pub fn new(max_repetitions: usize) -> Self {
        Self {
            max_repetitions,
            tool_outputs: HashMap::new(),
        }
    }

    /// Record a tool execution result.
    /// Returns true if this tool has produced the same output too many times.
    pub fn record(&mut self, tool_name: &str, output: &str) -> bool {
        let mut hasher = DefaultHasher::new();
        output.hash(&mut hasher);
        let hash = hasher.finish();

        let hashes = self.tool_outputs.entry(tool_name.to_string()).or_default();
        hashes.push(hash);

        let count = hashes.iter().filter(|&&h| h == hash).count();
        count >= self.max_repetitions
    }

    /// Reset for a new turn.
    pub fn reset(&mut self) {
        self.tool_outputs.clear();
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
        assert!(!detector.record("read_file", "content A"));
        assert!(!detector.record("read_file", "content B"));
        assert!(!detector.record("read_file", "content C"));
    }

    #[test]
    fn test_loop_same_output() {
        let mut detector = LoopDetector::new(3);
        assert!(!detector.record("read_file", "same content"));
        assert!(!detector.record("read_file", "same content"));
        assert!(detector.record("read_file", "same content"));
    }

    #[test]
    fn test_loop_reset() {
        let mut detector = LoopDetector::new(3);
        assert!(!detector.record("read_file", "same content"));
        assert!(!detector.record("read_file", "same content"));
        detector.reset();
        assert!(!detector.record("read_file", "same content"));
        assert!(!detector.record("read_file", "same content"));
        assert!(detector.record("read_file", "same content"));
    }

    #[test]
    fn test_different_tools_no_interference() {
        let mut detector = LoopDetector::new(3);
        // tool_a builds up to threshold
        assert!(!detector.record("tool_a", "output"));
        assert!(!detector.record("tool_a", "output"));
        // tool_b with different output has its own counter
        assert!(!detector.record("tool_b", "different"));
        // tool_a triggers on its 3rd identical output
        assert!(detector.record("tool_a", "output"));
        // tool_b is still below threshold (only 1 occurrence so far)
        assert!(!detector.record("tool_b", "different"));
    }
}
