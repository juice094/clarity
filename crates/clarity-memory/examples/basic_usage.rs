//! Basic usage example of clarity-memory
//! 
//! Run with: cargo run --example basic_usage

use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

// Import clarity-memory types
use clarity_memory::{
    CompileConfig, FactExtractor, LlmClient, MemoryCompiler, MemoryStore, MemoryTicker, SessionStore,
};

/// A simple mock LLM client for demonstration
#[derive(Debug)]
struct DemoLlmClient;

#[async_trait]
impl LlmClient for DemoLlmClient {
    async fn complete(&self, prompt: &str, _model: &str) -> Result<String, clarity_memory::MemoryError> {
        // In a real implementation, this would call an actual LLM API
        info!("LLM prompt length: {} chars", prompt.len());
        
        // Simulate different responses based on prompt content
        if prompt.contains("memory extraction") || prompt.contains("factual information") {
            Ok(r#"[
                {"fact": "User is interested in Rust programming", "tags": ["preference", "tech", "rust"], "time": null},
                {"fact": "User is building an AI agent system", "tags": ["project", "ai", "goal"], "time": null}
            ]"#.to_string())
        } else {
            Ok("This is a summarized version of the conversation content. Key points were discussed about the project.".to_string())
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("=== Clarity Memory Basic Usage Example ===\n");

    // Setup paths
    let data_dir = PathBuf::from("./memory_example_data");
    let db_path = data_dir.join("memory.db");
    let sessions_dir = data_dir.join("sessions");
    let output_dir = data_dir.join("compiled");

    // Clean up previous run
    let _ = std::fs::remove_dir_all(&data_dir);

    // Initialize stores
    info!("1. Initializing memory stores...");
    let memory_store = MemoryStore::new(&db_path)?;
    let session_store = SessionStore::new(&sessions_dir)?;
    info!("   ✓ Stores initialized\n");

    // Create LLM client
    let llm_client: Arc<dyn LlmClient> = Arc::new(DemoLlmClient);

    // Configure compilation
    let config = CompileConfig {
        turns_per_summary: 4,
        max_tokens_today: 1024,
        max_tokens_week: 1024,
        max_tokens_longterm: 1024,
        compile_model: "gpt-4".to_string(),
        extractor_model: "gpt-4".to_string(),
    };

    // Create compiler
    info!("2. Creating memory compiler...");
    let compiler = MemoryCompiler::new(
        memory_store,
        session_store.clone(),
        llm_client.clone(),
        config.clone(),
    );
    info!("   ✓ Compiler ready\n");

    // Create ticker
    info!("3. Creating turn-based ticker (threshold: {} turns)...", config.turns_per_summary);
    let mut ticker = MemoryTicker::new(&output_dir, Some(config.turns_per_summary));
    
    // Set up compile callback
    let compiler_ref = Arc::new(Mutex::new(compiler));
    let compiler_clone = Arc::clone(&compiler_ref);
    let output_dir_clone = output_dir.clone();
    ticker.set_compile_callback(move || {
        let c = Arc::clone(&compiler_clone);
        let out = output_dir_clone.clone();
        Box::pin(async move {
            let mut compiler = c.lock().await;
            compiler.compile_all(&out).await
        })
    });
    info!("   ✓ Ticker ready\n");

    // Simulate conversation
    info!("4. Simulating conversation...");
    let session_id = "demo-session";
    
    let messages = vec![
        ("user", "Hi! I'm working on a new AI agent system in Rust."),
        ("assistant", "That sounds exciting! Rust is a great choice for systems programming."),
        ("user", "Yes, I'm particularly interested in memory management and async patterns."),
        ("assistant", "Those are strong areas for Rust. Have you looked into tokio for async?"),
        ("user", "Yes, I'm using tokio. I also want to build a memory system like OpenHanako."),
        ("assistant", "OpenHanako has an interesting multi-level memory approach!"),
        ("user", "Exactly! I want to implement the four-level compilation pipeline."),
        ("assistant", "That's a great goal. The levels are: today, week, long-term, and facts."),
    ];

    for (i, (role, content)) in messages.iter().enumerate() {
        info!("   Turn {}: {} says: {}", i + 1, role, &content[..content.len().min(50)]);
        session_store.append_message(session_id, role, content)?;
        
        // Notify ticker
        if let Some(future) = ticker.notify_turn(session_id) {
            info!("   → Threshold reached! Triggering compilation...");
            match future.await {
                Ok(results) => {
                    info!("   ✓ Compilation complete:");
                    for (level, status) in results {
                        info!("     - {}: {}", level, status);
                    }
                }
                Err(e) => {
                    eprintln!("   ✗ Compilation failed: {}", e);
                }
            }
        }
    }

    info!("");

    // Demonstrate fact extraction
    info!("5. Demonstrating fact extraction...");
    let extractor = FactExtractor::new(llm_client, "gpt-4");
    let summary = "The user is building an AI agent in Rust and wants to implement a memory system similar to OpenHanako.";
    
    match extractor.extract_facts(summary).await {
        Ok(facts) => {
            info!("   ✓ Extracted {} facts:", facts.len());
            for (i, fact) in facts.iter().enumerate() {
                info!("     {}. {}", i + 1, fact.fact);
                info!("        Tags: {}", fact.tags.join(", "));
            }
        }
        Err(e) => {
            eprintln!("   ✗ Extraction failed: {}", e);
        }
    }

    info!("");

    // Show stored data
    info!("6. Checking stored data...");
    let session_count = session_store.get_message_count(session_id)?;
    info!("   ✓ Messages in session '{}': {}", session_id, session_count);

    // Check if memory file was created
    let memory_path = output_dir.join("memory.md");
    if memory_path.exists() {
        info!("   ✓ Compiled memory file created at: {:?}", memory_path);
        let content = std::fs::read_to_string(&memory_path)?;
        info!("   ✓ Memory file size: {} bytes", content.len());
    } else {
        info!("   ! No compiled memory file yet (need more turns)");
    }

    info!("\n=== Example Complete ===");
    info!("Data stored in: {:?}", data_dir);
    
    // Cleanup
    info!("\nCleaning up...");
    let _ = std::fs::remove_dir_all(&data_dir);
    info!("Done!");

    Ok(())
}
