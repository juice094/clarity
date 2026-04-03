//! Integration example: Using clarity-memory with clarity-core
//! 
//! This example shows how to integrate the memory system into clarity-core.
//! 
//! Run with: cargo run --example integration_core

use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn, Level};
use tracing_subscriber::FmtSubscriber;

use clarity_memory::{
    CompileConfig, CompileStatus, Fact, LlmClient, MemoryCompiler, MemoryStore, MemoryTicker, SessionStore,
};

// ============================================================================
// Example: How to integrate into clarity-core
// ============================================================================

/// Trait for core components that need memory capabilities
pub trait MemoryAware {
    fn memory_store(&self) -> &MemoryStore;
    fn session_store(&self) -> &SessionStore;
}

/// Example Agent struct from clarity-core with memory integration
pub struct Agent {
    name: String,
    memory_store: MemoryStore,
    session_store: SessionStore,
    compiler: Arc<Mutex<MemoryCompiler>>,
    ticker: Option<MemoryTicker>,
    current_session: Option<String>,
}

impl Agent {
    /// Create a new agent with memory capabilities
    pub fn new(
        name: impl Into<String>,
        data_dir: impl Into<PathBuf>,
    ) -> anyhow::Result<Self> {
        let data_dir = data_dir.into();
        let name = name.into();
        
        // Initialize stores
        let memory_db = data_dir.join("memory.db");
        let sessions_dir = data_dir.join("sessions");
        
        let memory_store = MemoryStore::new(&memory_db)?;
        let session_store = SessionStore::new(&sessions_dir)?;
        
        Ok(Self {
            name,
            memory_store,
            session_store,
            compiler: Arc::new(Mutex::new(MemoryCompiler::new(
                MemoryStore::new_in_memory()?, // Placeholder, will be replaced in init
                SessionStore::new("/tmp/placeholder")?,
                Arc::new(MockCoreClient),
                CompileConfig::default(),
            ))),
            ticker: None,
            current_session: None,
        })
    }

    /// Initialize the memory compiler and ticker
    pub fn init_memory_system(
        &mut self,
        llm_client: Arc<dyn LlmClient>,
        config: CompileConfig,
        output_dir: impl Into<PathBuf>,
    ) -> anyhow::Result<()> {
        let compiler = MemoryCompiler::new(
            MemoryStore::new_in_memory()?,
            self.session_store.clone(),
            llm_client,
            config.clone(),
        );

        let compiler_arc = Arc::new(Mutex::new(compiler));
        self.compiler = Arc::clone(&compiler_arc);

        let output_dir = output_dir.into();
        let mut ticker = MemoryTicker::new(&output_dir, Some(config.turns_per_summary));
        
        // Set up compile callback
        let c = Arc::clone(&compiler_arc);
        let out = output_dir.clone();
        ticker.set_compile_callback(move || {
            let c = Arc::clone(&c);
            let out = out.clone();
            Box::pin(async move {
                let mut compiler = c.lock().await;
                compiler.compile_all(&out).await
            })
        });

        self.ticker = Some(ticker);
        
        info!("Memory system initialized for agent '{}'", self.name);
        Ok(())
    }

    /// Start a new session
    pub fn start_session(&mut self, session_id: impl Into<String>) {
        self.current_session = Some(session_id.into());
        info!("Started new session: {:?}", self.current_session);
    }

    /// Process a user message and store it
    pub async fn process_message(
        &mut self,
        content: &str,
    ) -> anyhow::Result<Option<std::collections::HashMap<String, CompileStatus>>> {
        // Store the message
        if let Some(session_id) = &self.current_session {
            self.session_store.append_message(session_id, "user", content)?;
            info!("Stored user message in session '{}'", session_id);

            // Notify ticker and potentially trigger compilation
            if let Some(ref mut ticker) = self.ticker {
                let result = ticker.notify_turn_and_wait(session_id).await;
                if let Some(Ok(statuses)) = &result {
                    return Ok(Some(statuses.clone()));
                }
            }
        } else {
            warn!("No active session, message not stored");
        }

        Ok(None)
    }

    /// Store assistant response
    pub fn store_response(&self, content: &str) -> anyhow::Result<()> {
        if let Some(session_id) = &self.current_session {
            self.session_store.append_message(session_id, "assistant", content)?;
            debug!("Stored assistant response");
        }
        Ok(())
    }

    /// Search memory by tags
    pub fn search_memory(&self, tags: &[String]) -> Result<Vec<Fact>, clarity_memory::MemoryError> {
        self.memory_store.search_by_tags(tags, 20)
    }

    /// Full-text search memory
    pub fn search_memory_text(&self, query: &str) -> Result<Vec<Fact>, clarity_memory::MemoryError> {
        self.memory_store.search_fulltext(query, 20)
    }

    /// Get conversation history
    pub fn get_conversation_history(&self) -> anyhow::Result<Vec<clarity_memory::Message>> {
        match &self.current_session {
            Some(session_id) => Ok(self.session_store.get_messages(session_id)?),
            None => Ok(Vec::new()),
        }
    }
}

// ============================================================================
// Example LLM Client implementations
// ============================================================================

/// Simple mock client for testing
#[derive(Debug)]
struct MockCoreClient;

#[async_trait]
impl LlmClient for MockCoreClient {
    async fn complete(&self, prompt: &str, _model: &str) -> Result<String, clarity_memory::MemoryError> {
        if prompt.contains("memory extraction") {
            Ok(r#"[
                {"fact": "User prefers concise responses", "tags": ["preference", "style"], "time": null},
                {"fact": "User is working on a Rust project", "tags": ["tech", "rust"], "time": null}
            ]"#.to_string())
        } else {
            Ok(format!("Summary of: {}", &prompt[..prompt.len().min(100)]))
        }
    }
}

// ============================================================================
// Main example
// ============================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("=== Clarity Memory + Core Integration Example ===\n");

    // Setup
    let data_dir = PathBuf::from("./agent_data");
    let _ = std::fs::remove_dir_all(&data_dir);

    // Create agent
    info!("1. Creating Agent with memory...");
    let mut agent = Agent::new("ClarityBot", &data_dir)?;

    // Initialize memory system
    info!("2. Initializing memory system...");
    let llm_client: Arc<dyn LlmClient> = Arc::new(MockCoreClient);
    let config = CompileConfig {
        turns_per_summary: 3,
        max_tokens_today: 512,
        max_tokens_week: 512,
        max_tokens_longterm: 512,
        compile_model: "gpt-4".to_string(),
        extractor_model: "gpt-4".to_string(),
    };
    agent.init_memory_system(llm_client, config, data_dir.join("compiled"))?;

    // Start session
    info!("3. Starting conversation session...");
    agent.start_session("session-001");

    // Simulate conversation
    info!("4. Simulating conversation...\n");

    let exchanges = vec![
        "Hi! I'm working on a Rust project.",
        "Can you help me understand async patterns?",
        "I prefer concise responses with examples.",
        "How do I use tokio::spawn correctly?",
        "What about error handling in async functions?",
        "Thanks, that's helpful!",
    ];

    for (i, user_msg) in exchanges.iter().enumerate() {
        info!("Turn {}:", i + 1);
        info!("  User: {}", user_msg);
        
        // Process user message
        match agent.process_message(user_msg).await? {
            Some(statuses) => {
                info!("  → Memory compilation triggered!");
                for (level, status) in statuses {
                    info!("     {}: {}", level, status);
                }
            }
            None => {
                // Normal turn, no compilation
            }
        }

        // Simulate assistant response
        let response = format!("Response to: '{}'", &user_msg[..user_msg.len().min(30)]);
        agent.store_response(&response)?;
        info!("  Assistant: {}...\n", &response[..response.len().min(40)]);
    }

    // Search memory
    info!("5. Searching memory...");
    match agent.search_memory(&["preference".to_string()]) {
        Ok(facts) => {
            info!("   Found {} facts with tag 'preference'", facts.len());
            for fact in &facts {
                info!("   - {}", fact.fact);
            }
        }
        Err(e) => {
            warn!("   Search failed: {}", e);
        }
    }

    // Get conversation history
    info!("\n6. Conversation history:");
    let history = agent.get_conversation_history()?;
    for (i, msg) in history.iter().enumerate() {
        info!("   {}. {}: {}", i + 1, msg.role, &msg.content[..msg.content.len().min(40)]);
    }

    // Cleanup
    info!("\n=== Example Complete ===");
    let _ = std::fs::remove_dir_all(&data_dir);

    Ok(())
}
