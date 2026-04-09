//! # Think Skill
//!
//! A skill for structured thinking and reasoning. Helps break down complex
//! problems into steps and document the thought process.
//!
//! ## Commands
//!
//! - `step <text>` - Add a reasoning step
//! - `analyze <problem>` - Analyze a problem with structured reasoning
//! - `question <question>` - Explore a specific question
//! - `alternatives <options>` - Evaluate alternative approaches
//! - `conclusion` - Summarize current thinking
//! - `clear` - Clear current reasoning chain
//! - `show` - Display current reasoning chain
//!
//! ## Example
//!
//! ```rust
//! use clarity_core::skill::{Skill, ThinkSkill};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let think = ThinkSkill::new();
//!
//! think.execute("analyze Should we use Rust for this project?").await?;
//! think.execute("step Rust has excellent performance").await?;
//! think.execute("step Rust has a steep learning curve").await?;
//! think.execute("conclusion").await?;
//! # Ok(())
//! # }
//! ```

use super::{Skill, SkillError, SkillResult};
use async_trait::async_trait;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Types of reasoning steps
#[derive(Debug, Clone, PartialEq)]
pub enum StepType {
    /// Initial analysis or problem statement
    Analysis,
    /// Exploring a question
    Question,
    /// A reasoning step
    Reasoning,
    /// Evaluating evidence
    Evidence,
    /// Considering an alternative
    Alternative,
    /// Noting a constraint or limitation
    Constraint,
    /// Drawing a conclusion
    Conclusion,
}

impl StepType {
    /// Get the display prefix for this step type
    pub fn prefix(&self) -> &'static str {
        match self {
            StepType::Analysis => "🔍",
            StepType::Question => "❓",
            StepType::Reasoning => "💭",
            StepType::Evidence => "📋",
            StepType::Alternative => "🔄",
            StepType::Constraint => "⚠️",
            StepType::Conclusion => "✓",
        }
    }

    /// Get the name of this step type
    pub fn name(&self) -> &'static str {
        match self {
            StepType::Analysis => "Analysis",
            StepType::Question => "Question",
            StepType::Reasoning => "Reasoning",
            StepType::Evidence => "Evidence",
            StepType::Alternative => "Alternative",
            StepType::Constraint => "Constraint",
            StepType::Conclusion => "Conclusion",
        }
    }
}

/// A single step in the reasoning chain
#[derive(Debug, Clone, PartialEq)]
pub struct ReasoningStep {
    /// The type of this step
    pub step_type: StepType,
    /// The content of the step
    pub content: String,
    /// Timestamp when the step was added
    pub timestamp: u64,
}

impl ReasoningStep {
    /// Create a new reasoning step
    pub fn new(step_type: StepType, content: impl Into<String>) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            step_type,
            content: content.into(),
            timestamp,
        }
    }

    /// Format the step for display
    pub fn format(&self, index: usize) -> String {
        format!(
            "{} {} {}: {}",
            index,
            self.step_type.prefix(),
            self.step_type.name(),
            self.content
        )
    }
}

/// Skill for structured thinking and reasoning
///
/// The ThinkSkill maintains a chain of reasoning steps that can be
/// built up incrementally and reviewed as a coherent thought process.
pub struct ThinkSkill {
    steps: Mutex<Vec<ReasoningStep>>,
    current_topic: Mutex<Option<String>>,
}

impl ThinkSkill {
    /// Create a new ThinkSkill with empty reasoning chain
    pub fn new() -> Self {
        Self {
            steps: Mutex::new(Vec::new()),
            current_topic: Mutex::new(None),
        }
    }

    /// Create a new ThinkSkill with a predefined topic
    pub fn with_topic(topic: impl Into<String>) -> Self {
        Self {
            steps: Mutex::new(Vec::new()),
            current_topic: Mutex::new(Some(topic.into())),
        }
    }

    /// Add a step to the reasoning chain
    fn add_step(&self, step_type: StepType, content: String) {
        let mut steps = self.steps.lock().unwrap();
        steps.push(ReasoningStep::new(step_type, content));
    }

    /// Get all reasoning steps
    fn get_steps(&self) -> Vec<ReasoningStep> {
        self.steps.lock().unwrap().clone()
    }

    /// Clear all reasoning steps
    fn clear_steps(&self) {
        let mut steps = self.steps.lock().unwrap();
        steps.clear();
        let mut topic = self.current_topic.lock().unwrap();
        *topic = None;
    }

    /// Set the current topic
    fn set_topic(&self, topic: String) {
        let mut t = self.current_topic.lock().unwrap();
        *t = Some(topic);
    }

    /// Get the current topic
    fn get_topic(&self) -> Option<String> {
        self.current_topic.lock().unwrap().clone()
    }

    /// Format and display all reasoning steps
    fn show_reasoning(&self) -> String {
        let steps = self.get_steps();
        let topic = self.get_topic();

        if steps.is_empty() {
            return if let Some(t) = topic {
                format!("Thinking about: {}\n\nNo reasoning steps yet. Use 'step' to add thoughts.", t)
            } else {
                "No active reasoning. Start with 'analyze <topic>' or 'step <thought>'.".to_string()
            };
        }

        let mut output = String::new();

        if let Some(t) = topic {
            output.push_str(&format!("🧠 Thinking about: {}\n\n", t));
        } else {
            output.push_str("🧠 Reasoning Chain:\n\n");
        }

        for (i, step) in steps.iter().enumerate() {
            output.push_str(&step.format(i + 1));
            output.push('\n');
        }

        output.push_str(&format!("\n---\nTotal steps: {}", steps.len()));
        output
    }

    /// Generate a conclusion based on all reasoning steps
    fn generate_conclusion(&self) -> String {
        let steps = self.get_steps();
        let topic = self.get_topic();

        if steps.is_empty() {
            return "No reasoning steps to conclude from.".to_string();
        }

        let mut output = String::new();

        if let Some(t) = topic {
            output.push_str(&format!("📋 Conclusion for '{}'\n\n", t));
        } else {
            output.push_str("📋 Conclusion\n\n");
        }

        // Summarize key points by step type
        let analysis_count = steps
            .iter()
            .filter(|s| s.step_type == StepType::Analysis)
            .count();
        let evidence_count = steps
            .iter()
            .filter(|s| s.step_type == StepType::Evidence)
            .count();
        let alternatives_count = steps
            .iter()
            .filter(|s| s.step_type == StepType::Alternative)
            .count();
        let constraints_count = steps
            .iter()
            .filter(|s| s.step_type == StepType::Constraint)
            .count();

        output.push_str(&format!(
            "Based on {} reasoning steps:\n",
            steps.len()
        ));
        output.push_str(&format!("  - {} analysis points\n", analysis_count));
        output.push_str(&format!("  - {} evidence points\n", evidence_count));
        output.push_str(&format!("  - {} alternatives considered\n", alternatives_count));
        output.push_str(&format!("  - {} constraints identified\n\n", constraints_count));

        // List all reasoning steps for reference
        output.push_str("Key reasoning:\n");
        for step in &steps {
            output.push_str(&format!(
                "  {} {}\n",
                step.step_type.prefix(),
                step.content
            ));
        }

        // Add the conclusion step
        self.add_step(StepType::Conclusion, "Conclusion generated".to_string());

        output
    }

    /// Parse and execute a command
    async fn execute_command(&self, command: &str, args: &str) -> SkillResult<String> {
        match command {
            "step" | "think" => {
                if args.trim().is_empty() {
                    return Err(SkillError::invalid_input("Thought cannot be empty"));
                }
                self.add_step(StepType::Reasoning, args.to_string());
                Ok(format!("Added reasoning step: {}", args))
            }
            "analyze" => {
                if args.trim().is_empty() {
                    return Err(SkillError::invalid_input("Analysis topic cannot be empty"));
                }
                self.set_topic(args.to_string());
                self.add_step(StepType::Analysis, format!("Analyzing: {}", args));
                Ok(format!("Started analysis of: {}\n\n{}", args, self.show_reasoning()))
            }
            "question" => {
                if args.trim().is_empty() {
                    return Err(SkillError::invalid_input("Question cannot be empty"));
                }
                self.add_step(StepType::Question, args.to_string());
                Ok(format!("Added question: {}", args))
            }
            "evidence" => {
                if args.trim().is_empty() {
                    return Err(SkillError::invalid_input("Evidence cannot be empty"));
                }
                self.add_step(StepType::Evidence, args.to_string());
                Ok(format!("Added evidence: {}", args))
            }
            "alternative" | "alt" => {
                if args.trim().is_empty() {
                    return Err(SkillError::invalid_input("Alternative cannot be empty"));
                }
                self.add_step(StepType::Alternative, args.to_string());
                Ok(format!("Added alternative: {}", args))
            }
            "constraint" => {
                if args.trim().is_empty() {
                    return Err(SkillError::invalid_input("Constraint cannot be empty"));
                }
                self.add_step(StepType::Constraint, args.to_string());
                Ok(format!("Added constraint: {}", args))
            }
            "conclusion" | "conclude" => Ok(self.generate_conclusion()),
            "show" | "list" => Ok(self.show_reasoning()),
            "clear" | "reset" => {
                self.clear_steps();
                Ok("Cleared reasoning chain".to_string())
            }
            "help" => Ok(self.help_text()),
            _ => Err(SkillError::invalid_input(format!(
                "Unknown command: '{}'. Type 'help' for available commands.",
                command
            ))),
        }
    }

    /// Get help text
    fn help_text(&self) -> String {
        r#"Think Skill - Commands:
  analyze <topic>      - Start analyzing a topic
  step <thought>       - Add a reasoning step
  question <question>  - Add an exploration question
  evidence <fact>      - Add supporting evidence
  alternative <option> - Add an alternative approach
  constraint <limit>   - Note a constraint or limitation
  conclusion           - Generate conclusion from reasoning
  show                 - Show current reasoning chain
  clear                - Clear reasoning chain
  help                 - Show this help message

Examples:
  think analyze Should we use microservices?
  think step Microservices add deployment complexity
  think alternative Use a modular monolith instead
  think conclusion"#
            .to_string()
    }
}

impl Default for ThinkSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for ThinkSkill {
    fn name(&self) -> &str {
        "think"
    }

    fn description(&self) -> &str {
        "Structured thinking and reasoning tool. Break down complex problems into steps."
    }

    async fn execute(&self, input: &str) -> SkillResult<String> {
        let input = input.trim();

        if input.is_empty() || input == "help" {
            return Ok(self.help_text());
        }

        // Parse command and arguments
        let parts: Vec<&str> = input.splitn(2, ' ').collect();
        let command = parts[0].to_lowercase();
        let args = parts.get(1).copied().unwrap_or("");

        self.execute_command(&command, args).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_type_prefix() {
        assert_eq!(StepType::Analysis.prefix(), "🔍");
        assert_eq!(StepType::Question.prefix(), "❓");
        assert_eq!(StepType::Reasoning.prefix(), "💭");
        assert_eq!(StepType::Evidence.prefix(), "📋");
        assert_eq!(StepType::Alternative.prefix(), "🔄");
        assert_eq!(StepType::Constraint.prefix(), "⚠️");
        assert_eq!(StepType::Conclusion.prefix(), "✓");
    }

    #[test]
    fn test_step_type_name() {
        assert_eq!(StepType::Analysis.name(), "Analysis");
        assert_eq!(StepType::Question.name(), "Question");
        assert_eq!(StepType::Reasoning.name(), "Reasoning");
    }

    #[test]
    fn test_reasoning_step_new() {
        let step = ReasoningStep::new(StepType::Reasoning, "Test thought");
        assert_eq!(step.step_type, StepType::Reasoning);
        assert_eq!(step.content, "Test thought");
        assert!(step.timestamp > 0);
    }

    #[test]
    fn test_reasoning_step_format() {
        let step = ReasoningStep::new(StepType::Analysis, "Analyzing the problem");
        let formatted = step.format(1);
        assert!(formatted.contains("1"));
        assert!(formatted.contains("🔍"));
        assert!(formatted.contains("Analysis"));
        assert!(formatted.contains("Analyzing the problem"));
    }

    #[test]
    fn test_think_skill_new() {
        let skill = ThinkSkill::new();
        assert_eq!(skill.name(), "think");
        assert!(skill.get_steps().is_empty());
        assert!(skill.get_topic().is_none());
    }

    #[test]
    fn test_think_skill_with_topic() {
        let skill = ThinkSkill::with_topic("Test Topic");
        assert_eq!(skill.get_topic(), Some("Test Topic".to_string()));
    }

    #[tokio::test]
    async fn test_think_analyze() {
        let skill = ThinkSkill::new();
        let result = skill.execute("analyze Should we use Rust?").await.unwrap();
        assert!(result.contains("Started analysis"));
        assert!(result.contains("Should we use Rust?"));
        assert_eq!(skill.get_steps().len(), 1);
        assert_eq!(skill.get_topic(), Some("Should we use Rust?".to_string()));
    }

    #[tokio::test]
    async fn test_think_step() {
        let skill = ThinkSkill::new();
        let result = skill.execute("step Rust has great performance").await.unwrap();
        assert!(result.contains("Added reasoning step"));
        assert_eq!(skill.get_steps().len(), 1);
        assert_eq!(skill.get_steps()[0].content, "Rust has great performance");
    }

    #[tokio::test]
    async fn test_think_step_empty() {
        let skill = ThinkSkill::new();
        let result = skill.execute("step  ").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[tokio::test]
    async fn test_think_question() {
        let skill = ThinkSkill::new();
        skill.execute("question What about learning curve?").await.unwrap();
        assert_eq!(skill.get_steps()[0].step_type, StepType::Question);
    }

    #[tokio::test]
    async fn test_think_evidence() {
        let skill = ThinkSkill::new();
        skill.execute("evidence Benchmarks show 10x speedup").await.unwrap();
        assert_eq!(skill.get_steps()[0].step_type, StepType::Evidence);
    }

    #[tokio::test]
    async fn test_think_alternative() {
        let skill = ThinkSkill::new();
        skill.execute("alternative Use Go instead").await.unwrap();
        assert_eq!(skill.get_steps()[0].step_type, StepType::Alternative);
    }

    #[tokio::test]
    async fn test_think_constraint() {
        let skill = ThinkSkill::new();
        skill.execute("constraint Team is new to Rust").await.unwrap();
        assert_eq!(skill.get_steps()[0].step_type, StepType::Constraint);
    }

    #[tokio::test]
    async fn test_think_show_empty() {
        let skill = ThinkSkill::new();
        let result = skill.execute("show").await.unwrap();
        assert!(result.contains("No active reasoning"));
    }

    #[tokio::test]
    async fn test_think_show_with_topic() {
        let skill = ThinkSkill::with_topic("Test Topic");
        skill.execute("step First thought").await.unwrap();
        let result = skill.execute("show").await.unwrap();
        assert!(result.contains("Thinking about: Test Topic"));
        assert!(result.contains("First thought"));
    }

    #[tokio::test]
    async fn test_think_conclusion_empty() {
        let skill = ThinkSkill::new();
        let result = skill.execute("conclusion").await.unwrap();
        assert!(result.contains("No reasoning steps"));
    }

    #[tokio::test]
    async fn test_think_conclusion() {
        let skill = ThinkSkill::new();
        skill.execute("analyze Test").await.unwrap();
        skill.execute("step Thought 1").await.unwrap();
        skill.execute("evidence Fact 1").await.unwrap();

        let result = skill.execute("conclusion").await.unwrap();
        assert!(result.contains("Conclusion"));
        assert!(result.contains("1 analysis points"));
        assert!(result.contains("1 evidence points"));
        assert!(result.contains("Thought 1"));
        assert!(result.contains("Fact 1"));
    }

    #[tokio::test]
    async fn test_think_clear() {
        let skill = ThinkSkill::new();
        skill.execute("analyze Test").await.unwrap();
        skill.execute("step Thought").await.unwrap();

        let result = skill.execute("clear").await.unwrap();
        assert!(result.contains("Cleared"));
        assert!(skill.get_steps().is_empty());
        assert!(skill.get_topic().is_none());
    }

    #[tokio::test]
    async fn test_think_help() {
        let skill = ThinkSkill::new();
        let result = skill.execute("").await.unwrap();
        assert!(result.contains("Think Skill"));
        assert!(result.contains("analyze"));
        assert!(result.contains("step"));
        assert!(result.contains("conclusion"));
    }

    #[tokio::test]
    async fn test_think_unknown_command() {
        let skill = ThinkSkill::new();
        let result = skill.execute("unknown").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown command"));
    }

    #[tokio::test]
    async fn test_think_complete_workflow() {
        let skill = ThinkSkill::new();

        // Start analysis
        skill.execute("analyze Should we use microservices?").await.unwrap();

        // Add reasoning steps
        skill.execute("step Microservices enable independent scaling").await.unwrap();
        skill.execute("step Adds operational complexity").await.unwrap();
        skill.execute("alternative Consider modular monolith").await.unwrap();
        skill.execute("evidence AWS recommends starting with monolith").await.unwrap();
        skill.execute("constraint Small team of 3 developers").await.unwrap();

        // Show reasoning
        let show = skill.execute("show").await.unwrap();
        assert!(show.contains("Should we use microservices?"));
        assert_eq!(skill.get_steps().len(), 6);

        // Generate conclusion
        let conclusion = skill.execute("conclusion").await.unwrap();
        assert!(conclusion.contains("1 analysis points"));
        assert!(conclusion.contains("1 evidence points"));
        assert!(conclusion.contains("1 alternatives considered"));
        assert!(conclusion.contains("1 constraints identified"));
    }

    #[tokio::test]
    async fn test_think_shortcuts() {
        let skill = ThinkSkill::new();

        // Test 'alt' shortcut
        skill.execute("alt Option A").await.unwrap();
        assert_eq!(skill.get_steps()[0].step_type, StepType::Alternative);

        // Test 'conclude' shortcut
        skill.execute("conclude").await.unwrap();
        assert_eq!(skill.get_steps().last().unwrap().step_type, StepType::Conclusion);

        // Test 'list' shortcut
        let result = skill.execute("list").await.unwrap();
        assert!(result.contains("Reasoning Chain"));

        // Test 'reset' shortcut
        skill.execute("reset").await.unwrap();
        assert!(skill.get_steps().is_empty());
    }
}
