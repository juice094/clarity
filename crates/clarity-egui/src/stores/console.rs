//! Console store — task execution log and terminal output buffer.
//!
//! Collects tool execution output, status messages, and step-begin
//! notifications from the agent loop into a ring-buffered log for the
//! right-rail Console panel.

use std::time::Instant;

/// Ring-buffered log for the Console panel (cap enforced on push).
#[allow(dead_code)] // follow field reserved for auto-scroll toggle
pub struct ConsoleStore {
    pub entries: Vec<ConsoleEntry>,
    pub filter: ConsoleFilter,
    pub auto_scroll: bool,
    pub follow: bool,
}

/// Maximum entries before oldest are pruned (ring-buffer ceiling).
pub const MAX_CONSOLE_ENTRIES: usize = 5000;

impl Default for ConsoleStore {
    fn default() -> Self {
        Self {
            entries: Vec::with_capacity(256),
            filter: ConsoleFilter::All,
            auto_scroll: true,
            follow: true,
        }
    }
}

impl ConsoleStore {
    /// Push a new entry, pruning oldest if over capacity.
    pub fn push(&mut self, entry: ConsoleEntry) {
        self.entries.push(entry);
        if self.entries.len() > MAX_CONSOLE_ENTRIES {
            let excess = self.entries.len() - MAX_CONSOLE_ENTRIES;
            self.entries.drain(0..excess);
        }
    }

    /// Filtered view of entries according to the active filter.
    pub fn filtered(&self) -> impl Iterator<Item = &ConsoleEntry> {
        let filter = self.filter;
        self.entries.iter().filter(move |e| filter.matches(e))
    }
}

/// A single log line in the console.
#[derive(Clone, Debug)]
#[allow(dead_code)] // Extension-point fields reserved for PTY integration
pub struct ConsoleEntry {
    pub timestamp: Instant,
    pub level: ConsoleLevel,
    /// Tool name (e.g. "file_write") or "agent" for status messages.
    pub source: String,
    pub message: String,
    pub truncated: bool,
    // === Extension points for future backend features ===
    /// PTY process ID — reserved for persistent terminal sessions.
    pub source_pid: Option<u32>,
    /// Parsed ANSI escape sequences — reserved for PTY output.
    pub ansi_styled: Option<Vec<AnsiSpan>>,
}

/// Console log severity / category.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsoleLevel {
    Info,
    Warn,
    Error,
    ToolOutput,
    Status,
}

/// Active filter for the console panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsoleFilter {
    All,
    Errors,
    Warnings,
    ToolOutput,
    Status,
}

impl ConsoleFilter {
    fn matches(&self, entry: &ConsoleEntry) -> bool {
        match self {
            Self::All => true,
            Self::Errors => entry.level == ConsoleLevel::Error,
            Self::Warnings => entry.level == ConsoleLevel::Warn,
            Self::ToolOutput => entry.level == ConsoleLevel::ToolOutput,
            Self::Status => entry.level == ConsoleLevel::Status,
        }
    }

    /// Human-readable label (translated via app.t() at render time).
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Errors => "Errors",
            Self::Warnings => "Warnings",
            Self::ToolOutput => "Tool Output",
            Self::Status => "Status",
        }
    }
}

/// Reserved type for ANSI escape sequence parse results.
/// Unused until backend PTY integration ships.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct AnsiSpan {
    pub text: String,
    pub fg: Option<[u8; 3]>,
    pub bg: Option<[u8; 3]>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_prunes_oldest() {
        let mut store = ConsoleStore::default();
        for i in 0..MAX_CONSOLE_ENTRIES + 100 {
            store.push(ConsoleEntry {
                timestamp: Instant::now(),
                level: ConsoleLevel::Info,
                source: "test".into(),
                message: format!("msg {}", i),
                truncated: false,
                source_pid: None,
                ansi_styled: None,
            });
        }
        assert_eq!(store.entries.len(), MAX_CONSOLE_ENTRIES);
        assert_eq!(store.entries[0].message, "msg 100");
    }

    #[test]
    fn filter_errors_only() {
        let mut store = ConsoleStore::default();
        store.push(entry(ConsoleLevel::Info, "info msg"));
        store.push(entry(ConsoleLevel::Error, "error msg"));
        store.push(entry(ConsoleLevel::Warn, "warn msg"));
        store.filter = ConsoleFilter::Errors;
        let filtered: Vec<_> = store.filtered().collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].message, "error msg");
    }

    #[test]
    fn filter_all_shows_everything() {
        let mut store = ConsoleStore::default();
        store.push(entry(ConsoleLevel::Info, "a"));
        store.push(entry(ConsoleLevel::ToolOutput, "b"));
        store.push(entry(ConsoleLevel::Status, "c"));
        store.filter = ConsoleFilter::All;
        assert_eq!(store.filtered().count(), 3);
    }

    fn entry(level: ConsoleLevel, msg: &str) -> ConsoleEntry {
        ConsoleEntry {
            timestamp: Instant::now(),
            level,
            source: "test".into(),
            message: msg.into(),
            truncated: false,
            source_pid: None,
            ansi_styled: None,
        }
    }
}
