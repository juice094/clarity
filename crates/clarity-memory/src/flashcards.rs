//! Flashcard export from compiled memory facts.
//!
//! Generates Anki-compatible flashcard JSON that can be consumed by
//! the `flashcard-studio` skill (`--mode json`).

use crate::types::{Flashcard, MetaFact};
use std::collections::HashSet;
use std::path::Path;

/// Export a list of facts as flashcards in JSON format.
pub fn export_facts_to_flashcards(facts: &[MetaFact], output_path: &Path) -> crate::Result<()> {
    let cards: Vec<Flashcard> = facts.iter().map(fact_to_flashcard).collect();
    let json = serde_json::to_string_pretty(&cards)?;
    std::fs::write(output_path, json)?;
    Ok(())
}

/// Convert a single MetaFact into a Flashcard using lightweight heuristics.
fn fact_to_flashcard(fact: &MetaFact) -> Flashcard {
    let back = fact.fact.clone();
    let front = generate_front(&back, &fact.tags);
    let tags = if fact.tags.is_empty() {
        "memory".to_string()
    } else {
        fact.tags.join(",")
    };
    Flashcard { front, back, tags }
}

/// Generate a question-style front from a fact statement.
fn generate_front(fact: &str, tags: &[String]) -> String {
    let lower = fact.to_lowercase();
    let tag_set: HashSet<String> = tags.iter().map(|t| t.to_lowercase()).collect();

    // Heuristic 1: preference / like / prefer
    if tag_set.contains("preference")
        || lower.contains("likes")
        || lower.contains("prefers")
        || lower.contains("favorite")
    {
        return "What is a known preference?".to_string();
    }

    // Heuristic 2: goal / objective / target
    if tag_set.contains("goal") || lower.contains("wants to") || lower.contains("aims to") {
        return "What is a stated goal or objective?".to_string();
    }

    // Heuristic 3: identity / person / role
    if tag_set.contains("identity")
        || tag_set.contains("person")
        || lower.contains("works at")
        || lower.contains("is a")
    {
        // Extract subject before "is a" or "works at"
        if let Some(idx) = lower.find(" is a ") {
            let subject = &fact[..idx];
            return format!("What is {}?", subject);
        }
        if let Some(idx) = lower.find(" works at ") {
            let subject = &fact[..idx];
            return format!("Where does {} work?", subject);
        }
        return "What is a known identity fact?".to_string();
    }

    // Heuristic 4: tech / tool / language
    if tag_set.contains("tech") || tag_set.contains("tool") || tag_set.contains("language") {
        return "What is a known tech preference or fact?".to_string();
    }

    // Heuristic 5: decision / choice
    if tag_set.contains("decision")
        || lower.contains("decided")
        || lower.contains("chose")
        || lower.contains("chosen")
    {
        return "What decision was made?".to_string();
    }

    // Heuristic 6: project / work
    if tag_set.contains("project") || tag_set.contains("work") {
        return "What is a known project or work fact?".to_string();
    }

    // Fallback: use first sentence fragment as prompt
    let truncated = if fact.len() > 50 {
        format!("{}...", &fact[..50])
    } else {
        fact.to_string()
    };
    format!("Recall: {}", truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fact_to_flashcard() {
        let fact = MetaFact {
            fact: "User prefers Rust over Python".to_string(),
            tags: vec!["preference".to_string(), "tech".to_string()],
            time: None,
        };
        let card = fact_to_flashcard(&fact);
        assert_eq!(card.back, "User prefers Rust over Python");
        assert!(!card.front.is_empty());
        assert!(card.tags.contains("preference"));
    }

    #[test]
    fn test_generate_front_identity() {
        let fact = "Alice is a senior engineer at Acme";
        let tags = vec!["identity".to_string()];
        let front = generate_front(fact, &tags);
        assert!(front.contains("Alice"));
    }
}
