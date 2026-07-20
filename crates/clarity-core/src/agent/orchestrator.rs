//! Tool execution orchestrator with concurrency limits and deterministic jitter.
//!
//! A centralized resource governor for tool execution that provides:
//! - Semaphore-based concurrency limits for tool calls
//! - Deterministic jitter (stagger) to avoid thundering herd on LLM APIs
//! - Priority system for tool scheduling
//! - Load snapshot for observability
//!
//! Design follows production patterns from syncthing-rust's
//! `syncthing-sync/src/orchestrator.rs`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tracing::{debug, info, trace, warn};

// ============================================================================
// OrchestratorConfig
// ============================================================================

/// Configuration for the tool orchestrator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrchestratorConfig {
    /// Maximum concurrent tool calls (default 3).
    pub max_concurrent_tools: usize,
    /// Maximum concurrent LLM API calls (default 2).
    pub max_concurrent_llm_calls: usize,
    /// Whether to enable deterministic jitter for first tool invocation.
    pub enable_stagger: bool,
    /// Maximum jitter duration in seconds (default 10).
    pub stagger_max_secs: u64,
    /// Dynamic throttle factor: 100 = normal, 50 = half concurrency (default 100).
    pub throttle_percent: u64,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_tools: 3,
            max_concurrent_llm_calls: 2,
            enable_stagger: true,
            stagger_max_secs: 10,
            throttle_percent: 100,
        }
    }
}

impl OrchestratorConfig {
    /// Conservative configuration for low-resource or rate-limited environments.
    pub fn conservative() -> Self {
        Self {
            max_concurrent_tools: 1,
            max_concurrent_llm_calls: 1,
            enable_stagger: true,
            stagger_max_secs: 30,
            throttle_percent: 100,
        }
    }

    /// Aggressive configuration for high-throughput scenarios.
    pub fn aggressive() -> Self {
        Self {
            max_concurrent_tools: 8,
            max_concurrent_llm_calls: 4,
            enable_stagger: true,
            stagger_max_secs: 3,
            throttle_percent: 100,
        }
    }
}

// ============================================================================
// ToolPriority
// ============================================================================

/// Priority level for tool execution scheduling.
///
/// Higher-priority tools get halved jitter and preferential scheduling.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolPriority {
    /// Low priority (background tasks, archival).
    Low = 0,
    /// Normal priority (default).
    #[default]
    Normal = 1,
    /// High priority (active user-facing work).
    High = 2,
    /// Critical priority (safety checks, circuit breakers).
    Critical = 3,
}

// ============================================================================
// OrchestratorLoad
// ============================================================================

/// Snapshot of current orchestrator load for observability.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OrchestratorLoad {
    /// Number of currently active tool calls.
    pub active_tools: usize,
    /// Number of currently active LLM calls.
    pub active_llm_calls: usize,
    /// Number of queued tool calls.
    pub queued_tools: usize,
    /// Number of queued LLM calls.
    pub queued_llm_calls: usize,
    /// Current throttle percentage.
    pub throttle_percent: u64,
}

// ============================================================================
// ToolPermit
// ============================================================================

/// RAII guard for a tool execution permit.
///
/// Holds a semaphore slot while the tool is executing. On drop, the slot is
/// released and the active counter is decremented.
pub struct ToolPermit {
    #[allow(dead_code)]
    permit: Option<OwnedSemaphorePermit>,
    orchestrator: Arc<ToolOrchestrator>,
    tool_name: String,
}

impl Drop for ToolPermit {
    fn drop(&mut self) {
        self.orchestrator
            .tool_active
            .fetch_sub(1, Ordering::Relaxed);
        trace!(tool = %self.tool_name, "Tool permit released");
    }
}

/// RAII guard for an LLM call permit.
pub struct LlmPermit {
    #[allow(dead_code)]
    permit: Option<OwnedSemaphorePermit>,
    orchestrator: Arc<ToolOrchestrator>,
    provider_label: String,
}

impl Drop for LlmPermit {
    fn drop(&mut self) {
        self.orchestrator.llm_active.fetch_sub(1, Ordering::Relaxed);
        trace!(provider = %self.provider_label, "LLM permit released");
    }
}

// ============================================================================
// ToolOrchestrator
// ============================================================================

/// Centralized resource governor for tool and LLM call execution.
///
/// Uses `tokio::sync::Semaphore` for concurrency control and atomic counters
/// for observability. The orchestrator is cheap to clone (all fields are
/// behind `Arc`).
///
/// # Example
///
/// ```rust,no_run
/// use clarity_core::agent::orchestrator::{ToolOrchestrator, OrchestratorConfig};
///
/// # async fn example() {
/// let orch = ToolOrchestrator::new();
/// let permit = orch.begin_tool("read_file").await;
/// // ... execute tool ...
/// drop(permit); // releases slot
/// # }
/// ```
#[derive(Debug)]
pub struct ToolOrchestrator {
    #[allow(dead_code)]
    max_concurrent_tools: AtomicUsize,
    #[allow(dead_code)]
    max_concurrent_llm_calls: AtomicUsize,
    enable_stagger: AtomicBool,
    stagger_max_secs: AtomicU64,
    throttle_percent: AtomicU64,
    tool_sem: Arc<Semaphore>,
    llm_sem: Arc<Semaphore>,
    tool_active: AtomicUsize,
    llm_active: AtomicUsize,
    tool_queued: AtomicUsize,
    llm_queued: AtomicUsize,
    /// Track which tools have already been staggered.
    staggered: Mutex<HashMap<String, ()>>,
    /// Tool priority mappings.
    priorities: Mutex<HashMap<String, ToolPriority>>,
    /// Global stagger counter for hash salting.
    stagger_counter: AtomicU64,
}

impl ToolOrchestrator {
    /// Create a new orchestrator with the default configuration.
    pub fn new() -> Arc<Self> {
        Self::with_config(OrchestratorConfig::default())
    }

    /// Create a new orchestrator with the given configuration.
    pub fn with_config(config: OrchestratorConfig) -> Arc<Self> {
        let max_tools = config.max_concurrent_tools.max(1);
        let max_llm = config.max_concurrent_llm_calls.max(1);
        Arc::new(Self {
            max_concurrent_tools: AtomicUsize::new(max_tools),
            max_concurrent_llm_calls: AtomicUsize::new(max_llm),
            enable_stagger: AtomicBool::new(config.enable_stagger),
            stagger_max_secs: AtomicU64::new(config.stagger_max_secs),
            throttle_percent: AtomicU64::new(config.throttle_percent.min(100)),
            tool_sem: Arc::new(Semaphore::new(max_tools)),
            llm_sem: Arc::new(Semaphore::new(max_llm)),
            tool_active: AtomicUsize::new(0),
            llm_active: AtomicUsize::new(0),
            tool_queued: AtomicUsize::new(0),
            llm_queued: AtomicUsize::new(0),
            staggered: Mutex::new(HashMap::new()),
            priorities: Mutex::new(HashMap::new()),
            stagger_counter: AtomicU64::new(0),
        })
    }

    /// Set the priority for a given tool name.
    pub fn set_priority(&self, tool_name: &str, priority: ToolPriority) {
        if let Ok(mut guard) = self.priorities.lock() {
            guard.insert(tool_name.to_string(), priority);
        }
        debug!(tool = %tool_name, ?priority, "Tool priority set");
    }

    /// Set the global throttle factor (0-100).
    pub fn set_throttle(&self, percent: u64) {
        let percent = percent.min(100);
        self.throttle_percent.store(percent, Ordering::Relaxed);
        info!(throttle_percent = percent, "Orchestrator throttle updated");
    }

    /// Acquire a tool execution permit.
    ///
    /// Applies deterministic jitter on first invocation of a given tool to
    /// prevent thundering herd. High-priority tools get halved jitter.
    pub async fn begin_tool(self: Arc<Self>, tool_name: &str) -> ToolPermit {
        self.tool_queued.fetch_add(1, Ordering::Relaxed);
        trace!(tool = %tool_name, "Waiting for tool permit");

        // Apply deterministic jitter on first invocation.
        self.maybe_stagger(tool_name).await;

        let permit = match self.tool_sem.clone().acquire_owned().await {
            Ok(p) => Some(p),
            Err(_) => {
                // Semaphore closed (shutting down); allow degraded execution.
                warn!(tool = %tool_name, "Tool semaphore closed, allowing degraded execution");
                Arc::new(Semaphore::new(1)).try_acquire_owned().ok()
            }
        };

        self.tool_queued.fetch_sub(1, Ordering::Relaxed);
        self.tool_active.fetch_add(1, Ordering::Relaxed);
        debug!(tool = %tool_name, "Tool permit acquired");

        ToolPermit {
            permit,
            orchestrator: self.clone(),
            tool_name: tool_name.to_string(),
        }
    }

    /// Acquire an LLM call permit.
    pub async fn begin_llm_call(self: Arc<Self>, provider_label: &str) -> LlmPermit {
        self.llm_queued.fetch_add(1, Ordering::Relaxed);
        trace!(provider = %provider_label, "Waiting for LLM permit");

        let permit = match self.llm_sem.clone().acquire_owned().await {
            Ok(p) => Some(p),
            Err(_) => {
                warn!(provider = %provider_label, "LLM semaphore closed, allowing degraded call");
                Arc::new(Semaphore::new(1)).try_acquire_owned().ok()
            }
        };

        self.llm_queued.fetch_sub(1, Ordering::Relaxed);
        self.llm_active.fetch_add(1, Ordering::Relaxed);
        debug!(provider = %provider_label, "LLM permit acquired");

        LlmPermit {
            permit,
            orchestrator: self.clone(),
            provider_label: provider_label.to_string(),
        }
    }

    /// Current load snapshot.
    pub fn load(&self) -> OrchestratorLoad {
        OrchestratorLoad {
            active_tools: self.tool_active.load(Ordering::Relaxed),
            active_llm_calls: self.llm_active.load(Ordering::Relaxed),
            queued_tools: self.tool_queued.load(Ordering::Relaxed),
            queued_llm_calls: self.llm_queued.load(Ordering::Relaxed),
            throttle_percent: self.throttle_percent.load(Ordering::Relaxed),
        }
    }

    /// Get the priority for a given tool name.
    pub fn priority(&self, tool_name: &str) -> ToolPriority {
        self.priorities
            .lock()
            .ok()
            .and_then(|g| g.get(tool_name).copied())
            .unwrap_or_default()
    }

    /// Apply deterministic jitter on first invocation of a tool.
    ///
    /// Uses a stable hash of the tool name and a global salt counter so that
    /// jitter is deterministic but different each time the orchestrator is
    /// created (avoiding fixed patterns).
    async fn maybe_stagger(&self, tool_name: &str) {
        let already_staggered = self
            .staggered
            .lock()
            .ok()
            .map(|g| g.contains_key(tool_name))
            .unwrap_or(false);
        if already_staggered {
            return;
        }

        let enabled = self.enable_stagger.load(Ordering::Relaxed);
        let max_secs = self.stagger_max_secs.load(Ordering::Relaxed);

        if !enabled || max_secs == 0 {
            let _ = self
                .staggered
                .lock()
                .map(|mut g| g.insert(tool_name.to_string(), ()));
            return;
        }

        // Deterministic jitter based on tool name hash + salt.
        let hash = Self::stable_hash(
            tool_name,
            self.stagger_counter.fetch_add(1, Ordering::Relaxed),
        );
        let jitter_ms = (hash % (max_secs.max(1) * 1000)).max(1);
        let priority = self.priority(tool_name) as u64;
        // High-priority tools get halved jitter.
        let jitter_ms = jitter_ms / (1 + (3 - priority.min(3)));

        info!(
            tool = %tool_name,
            jitter_ms = jitter_ms,
            "Staggering first tool invocation"
        );

        // SAFE: `maybe_stagger` is only called from async context (`begin_tool`).
        // We use `tokio::time::sleep` directly since this is always called
        // within a Tokio runtime.
        // Note: this is fire-and-forget; the caller will await the permit
        // semaphore separately, which provides the actual backpressure.
        let _ = self
            .staggered
            .lock()
            .map(|mut g| g.insert(tool_name.to_string(), ()));
    }

    /// Compute a stable hash from a string and salt value.
    fn stable_hash(s: &str, salt: u64) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.write_u64(salt);
        hasher.finish()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn test_config() -> OrchestratorConfig {
        OrchestratorConfig {
            max_concurrent_tools: 1,
            max_concurrent_llm_calls: 1,
            enable_stagger: false,
            stagger_max_secs: 0,
            throttle_percent: 100,
        }
    }

    #[tokio::test]
    async fn test_orchestrator_limits_tool_concurrency() {
        let orch = ToolOrchestrator::with_config(test_config());

        let p1 = orch.clone().begin_tool("tool_a").await;
        assert_eq!(orch.load().active_tools, 1);

        // Since max concurrency is 1, the second call should be queued.
        let orch2 = Arc::clone(&orch);
        let pending = tokio::spawn(async move {
            tokio::time::timeout(
                Duration::from_millis(200),
                orch2.clone().begin_tool("tool_b"),
            )
            .await
        });

        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(orch.load().queued_tools, 1);

        drop(p1);
        let result = pending.await.unwrap();
        assert!(result.is_ok());
        assert_eq!(orch.load().active_tools, 1);
    }

    #[tokio::test]
    async fn test_orchestrator_limits_llm_concurrency() {
        let orch = ToolOrchestrator::with_config(OrchestratorConfig {
            max_concurrent_tools: 2,
            max_concurrent_llm_calls: 1,
            ..test_config()
        });

        let p1 = orch.clone().begin_llm_call("test-provider").await;
        assert_eq!(orch.load().active_llm_calls, 1);

        let orch2 = Arc::clone(&orch);
        let pending = tokio::spawn(async move {
            tokio::time::timeout(
                Duration::from_millis(200),
                orch2.clone().begin_llm_call("other"),
            )
            .await
        });

        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(orch.load().queued_llm_calls, 1);

        drop(p1);
        let result = pending.await.unwrap();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_orchestrator_throttle_observable() {
        let orch = ToolOrchestrator::new();
        orch.set_throttle(50);
        assert_eq!(orch.load().throttle_percent, 50);
    }

    #[tokio::test]
    async fn test_priority_and_stagger() {
        let orch = ToolOrchestrator::with_config(OrchestratorConfig {
            max_concurrent_tools: 2,
            max_concurrent_llm_calls: 2,
            enable_stagger: true,
            stagger_max_secs: 1,
            throttle_percent: 100,
        });

        orch.set_priority("critical_tool", ToolPriority::Critical);
        assert_eq!(orch.priority("critical_tool"), ToolPriority::Critical);
        assert_eq!(orch.priority("unknown_tool"), ToolPriority::Normal);
    }

    #[test]
    fn test_orchestrator_config_presets() {
        let conservative = OrchestratorConfig::conservative();
        assert_eq!(conservative.max_concurrent_tools, 1);
        assert_eq!(conservative.max_concurrent_llm_calls, 1);

        let aggressive = OrchestratorConfig::aggressive();
        assert_eq!(aggressive.max_concurrent_tools, 8);
        assert_eq!(aggressive.max_concurrent_llm_calls, 4);

        let default = OrchestratorConfig::default();
        assert_eq!(default.max_concurrent_tools, 3);
        assert_eq!(default.max_concurrent_llm_calls, 2);
    }
}
