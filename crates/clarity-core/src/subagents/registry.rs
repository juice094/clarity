//! LaborMarket - Subagent type registry
//! 
//! Manages built-in subagent types like coder, explore, plan.

use std::collections::HashMap;

/// Definition of a subagent type
#[derive(Debug, Clone)]
pub struct AgentTypeDefinition {
    pub name: String,
    pub description: String,
    pub system_prompt: String,  // 直接存储而非文件路径
    pub allowed_tools: Option<Vec<String>>, // None = all tools
    pub max_iterations: usize,
}

/// Registry for subagent types (LaborMarket)
pub struct LaborMarket {
    types: HashMap<String, AgentTypeDefinition>,
}

impl Default for LaborMarket {
    fn default() -> Self {
        Self::new()
    }
}

impl LaborMarket {
    /// Create with default built-in types
    pub fn new() -> Self {
        let mut market = Self {
            types: HashMap::new(),
        };
        market.register_builtin_types();
        market
    }
    
    /// Register built-in types (coder, explore, plan)
    fn register_builtin_types(&mut self) {
        // coder: 用于代码工程任务
        self.register(AgentTypeDefinition {
            name: "coder".to_string(),
            description: "Code engineering tasks - implementation, refactoring, debugging".to_string(),
            system_prompt: CODER_SYSTEM_PROMPT.to_string(),
            allowed_tools: None, // 所有工具
            max_iterations: 20,
        });
        
        // explore: 用于代码库探索
        self.register(AgentTypeDefinition {
            name: "explore".to_string(),
            description: "Codebase exploration and research".to_string(),
            system_prompt: EXPLORE_SYSTEM_PROMPT.to_string(),
            allowed_tools: Some(vec![
                "file_read".to_string(),
                "glob".to_string(),
                "grep".to_string(),
            ]), // 只读工具
            max_iterations: 10,
        });
        
        // plan: 用于实现规划
        self.register(AgentTypeDefinition {
            name: "plan".to_string(),
            description: "Implementation planning and design".to_string(),
            system_prompt: PLAN_SYSTEM_PROMPT.to_string(),
            allowed_tools: Some(vec![
                "file_read".to_string(),
                "glob".to_string(),
                "grep".to_string(),
                "file_write".to_string(), // 可以写入计划文件
            ]),
            max_iterations: 5,
        });
    }
    
    /// Register a new type
    pub fn register(&mut self, type_def: AgentTypeDefinition) {
        self.types.insert(type_def.name.clone(), type_def);
    }
    
    /// Get a type by name
    pub fn get(&self, name: &str) -> Option<&AgentTypeDefinition> {
        self.types.get(name)
    }
    
    /// Get a type or panic
    pub fn require(&self, name: &str) -> &AgentTypeDefinition {
        self.get(name).unwrap_or_else(|| panic!("Unknown agent type: {}", name))
    }
    
    /// List all registered types
    pub fn list(&self) -> Vec<&AgentTypeDefinition> {
        self.types.values().collect()
    }
}

// System prompts for built-in types
const CODER_SYSTEM_PROMPT: &str = r#"You are a code engineering assistant.
Your task is to implement, refactor, or debug code.

Guidelines:
- Write clean, idiomatic code
- Add appropriate error handling
- Follow existing code style
- Test your changes when possible
"#;

const EXPLORE_SYSTEM_PROMPT: &str = r#"You are a codebase exploration assistant.
Your task is to understand and explain code structure.

Guidelines:
- Use file_read, glob, grep to explore
- Provide clear summaries
- Ask questions if unclear
- Focus on understanding, not changing
"#;

const PLAN_SYSTEM_PROMPT: &str = r#"You are an implementation planning assistant.
Your task is to design solutions before implementation.

Guidelines:
- Explore first to understand context
- Design approach before coding
- Write plan to file if needed
- Get user approval before implementation
"#;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_labor_market_default_types() {
        let market = LaborMarket::new();
        assert!(market.get("coder").is_some());
        assert!(market.get("explore").is_some());
        assert!(market.get("plan").is_some());
    }
    
    #[test]
    fn test_require_existing() {
        let market = LaborMarket::new();
        let coder = market.require("coder");
        assert_eq!(coder.name, "coder");
    }
    
    #[test]
    #[should_panic]
    fn test_require_unknown() {
        let market = LaborMarket::new();
        market.require("unknown");
    }
    
    #[test]
    fn test_register_custom() {
        let mut market = LaborMarket::new();
        market.register(AgentTypeDefinition {
            name: "custom".to_string(),
            description: "Custom agent".to_string(),
            system_prompt: "You are custom".to_string(),
            allowed_tools: None,
            max_iterations: 10,
        });
        
        assert!(market.get("custom").is_some());
    }
}
