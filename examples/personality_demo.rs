//! Personality System Demo
//!
//! This example demonstrates how to use the personality system
//! to create agents with different character traits.

use clarity_core::agent::{Agent, AgentConfig};
use clarity_core::personality::{
    presets, PersonalityConfig, PersonalityLoader, SystemPromptBuilder, YuanType,
};
use clarity_core::registry::ToolRegistry;

fn main() -> anyhow::Result<()> {
    println!("=== Clarity Personality System Demo ===\n");

    // Demo 1: Load different personalities
    println!("1. Loading Personalities\n");

    let loader = PersonalityLoader::new();

    // Hanako - Balanced
    let hanako_config = PersonalityConfig::new()
        .with_agent_name("Hanako")
        .with_user_name("User")
        .with_yuan_type(YuanType::Hanako)
        .with_locale("zh-CN");

    let hanako = loader.load(&hanako_config)?;
    println!("✓ Hanako loaded");
    println!("  Identity: {}", hanako.identity.lines().next().unwrap_or(""));
    println!("  Yuan module: MOOD");
    println!();

    // Butter -感性优先
    let butter_config = PersonalityConfig::new()
        .with_agent_name("Butter")
        .with_user_name("User")
        .with_yuan_type(YuanType::Butter)
        .with_locale("zh-CN");

    let butter = loader.load(&butter_config)?;
    println!("✓ Butter loaded");
    println!("  Identity: {}", butter.identity.lines().next().unwrap_or(""));
    println!("  Yuan module: PULSE");
    println!();

    // Ming -理性优先
    let ming_config = PersonalityConfig::new()
        .with_agent_name("Ming")
        .with_user_name("User")
        .with_yuan_type(YuanType::Ming)
        .with_locale("zh-CN");

    let ming = loader.load(&ming_config)?;
    println!("✓ Ming loaded");
    println!("  Identity: {}", ming.identity.lines().next().unwrap_or(""));
    println!("  Yuan module: Contemplation");
    println!();

    // Demo 2: Build system prompts
    println!("2. Building System Prompts\n");

    // Minimal prompt
    let minimal = presets::minimal(hanako.clone());
    println!("--- Minimal Prompt (Hanako) ---");
    println!("{}\n", minimal.lines().take(10).collect::<Vec<_>>().join("\n"));

    // Full prompt with skills
    let skills = vec![
        "file_read: Read files from the filesystem".to_string(),
        "file_write: Write files to the filesystem".to_string(),
        "bash: Execute shell commands".to_string(),
    ];

    let full_prompt = SystemPromptBuilder::new(hanako.clone())
        .with_user_profile("User is a software developer who likes Rust.")
        .with_memory("Previous conversation about setting up a new project.")
        .with_skills(skills)
        .build();

    println!("--- Full Prompt (Hanako with context) ---");
    println!("{}\n", full_prompt.lines().take(20).collect::<Vec<_>>().join("\n"));
    println!("... (truncated)\n");

    // Demo 3: Create an agent with personality
    println!("3. Creating Agent with Personality\n");

    let registry = ToolRegistry::with_builtin_tools();

    let config = AgentConfig::new()
        .with_personality(hanako_config)
        .with_max_iterations(10);

    let _agent = Agent::with_config(registry, config);
    println!("✓ Agent created with Hanako personality\n");

    // Demo 4: Hot reload example
    println!("4. Hot Reload Example\n");

    let mut agent = Agent::new(ToolRegistry::with_builtin_tools());

    // Initial personality
    let initial_config = PersonalityConfig::new()
        .with_agent_name("Assistant")
        .with_yuan_type(YuanType::Hanako);

    agent.update_personality(initial_config)?;
    println!("✓ Initial personality: Hanako");

    // Switch to Butter
    let butter_config = PersonalityConfig::new()
        .with_agent_name("Assistant")
        .with_yuan_type(YuanType::Butter);

    agent.update_personality(butter_config)?;
    println!("✓ Switched to: Butter");

    // Switch to Ming
    let ming_config = PersonalityConfig::new()
        .with_agent_name("Assistant")
        .with_yuan_type(YuanType::Ming);

    agent.update_personality(ming_config)?;
    println!("✓ Switched to: Ming");
    println!();

    // Demo 5: Yuan Type details
    println!("5. Yuan Type Comparison\n");

    println!("| Type   | Display | Thinking Mode | Module        |");
    println!("|--------|---------|---------------|---------------|");
    println!(
        "| Hanako | {:7} | Balanced      | MOOD          |",
        YuanType::Hanako.display_name()
    );
    println!(
        "| Butter | {:7} |感性优先      | PULSE         |",
        YuanType::Butter.display_name()
    );
    println!(
        "| Ming   | {:7} |理性优先      | Contemplation |",
        YuanType::Ming.display_name()
    );
    println!();

    println!("=== Demo Complete ===");
    println!();
    println!("To use in your code:");
    println!("  use clarity_core::personality::{{PersonalityConfig, YuanType}};");
    println!("  use clarity_core::agent::AgentConfig;");
    println!();
    println!("  let personality = PersonalityConfig::new()");
    println!("      .with_yuan_type(YuanType::Hanako);");
    println!("  let config = AgentConfig::new().with_personality(personality);");

    Ok(())
}
