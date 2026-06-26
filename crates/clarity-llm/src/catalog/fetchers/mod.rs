//! Concrete catalog fetchers.

pub mod ollama;
pub mod openai_compatible;

pub use ollama::OllamaFetcher;
pub use openai_compatible::OpenAiCompatibleFetcher;
