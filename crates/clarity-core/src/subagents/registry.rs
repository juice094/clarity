//! LaborMarket - Subagent type registry
//!
//! P1-1: `AgentTypeDefinition` and `LaborMarket` have been moved to `crate::types`
//! to break the `background↔subagents` circular dependency.
//! This module now only re-exports them for backwards compatibility.

pub use crate::types::{AgentTypeDefinition, LaborMarket};

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
