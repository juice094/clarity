# Personality System (人格系统)

A three-layer personality system for Clarity agents, inspired by OpenHanako.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   Personality                           │
├─────────────────────────────────────────────────────────┤
│  identity.md   │   Short identity (one-liner)           │
├─────────────────────────────────────────────────────────┤
│  yuan.md       │   Thinking structure (MOOD/PULSE/沉思) │
├─────────────────────────────────────────────────────────┤
│  ishiki.md     │   Detailed personality (behavior)      │
└─────────────────────────────────────────────────────────┘
```

## Yuan Types (元类型)

| Type   | Thinking Mode      | Module        | Best For                    |
|--------|-------------------|---------------|----------------------------|
| Hanako | Balanced 感性与理性 | MOOD          | General purpose, warm assistant |
| Butter |感性优先          | PULSE         | Creative, emotional support    |
| Ming   |理性优先          | Contemplation | Analysis, deep thinking        |

## Quick Start

```rust
use clarity_core::agent::{Agent, AgentConfig};
use clarity_core::personality::{PersonalityConfig, YuanType};
use clarity_core::registry::ToolRegistry;

// Configure personality
let personality_config = PersonalityConfig::new()
    .with_agent_name("Clarity")
    .with_user_name("Alice")
    .with_yuan_type(YuanType::Hanako)
    .with_locale("zh-CN");

// Create agent with personality
let config = AgentConfig::new()
    .with_personality(personality_config);

let agent = Agent::with_config(ToolRegistry::with_builtin_tools(), config);
```

## Advanced Usage

### Building System Prompts

```rust
use clarity_core::personality::{PersonalityLoader, SystemPromptBuilder};

let loader = PersonalityLoader::new();
let personality = loader.load(&config)?;

let system_prompt = SystemPromptBuilder::new(personality)
    .with_memory("User likes Rust programming")
    .with_user_profile("Software engineer, 5 years experience")
    .with_skills(vec!["file_read".to_string(), "bash".to_string()])
    .build();
```

### Hot Reload

```rust
// Change personality at runtime
agent.update_personality(new_config)?;
```

### Custom Templates

Place custom templates in your agent directory:

```
my-agent/
  ├── identity.md    # Override identity
  ├── yuan.md        # Override thinking module
  └── ishiki.md      # Override personality definition
```

Then set `agent_dir` in config:

```rust
let config = PersonalityConfig::new()
    .with_agent_dir("./my-agent");
```

## Template Loading Priority

1. **Custom Agent Directory** (`agent_dir/identity.md`)
2. **Locale-specific** (`templates/identity-templates/{locale}/{yuan}.md`)
3. **Generic** (`templates/identity-templates/{yuan}.md`)
4. **Embedded Default** (compiled into binary)

## Variable Substitution

Templates support the following variables:

- `{{agentName}}` / `{{agent_name}}` - Agent name
- `{{userName}}` / `{{user_name}}` - User name
- `{{yuanType}}` / `{{yuan_type}}` - Yuan type display name
- `{{locale}}` - Locale code

## Examples

See `examples/personality_demo.rs` for a complete working example.

Run the demo:

```bash
cargo run --example personality_demo
```
