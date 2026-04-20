//! Document chunking utilities for RAG pipelines
//!
//! Splits long documents into smaller, overlapping chunks that fit within
//! a context-window limit while preserving semantic boundaries as much as
//! possible.

/// Configuration for the chunking strategy
#[derive(Debug, Clone)]
pub struct ChunkConfig {
    /// Maximum chunk length in characters (default: 512)
    pub chunk_size: usize,
    /// Overlap between consecutive chunks in characters (default: 50)
    pub chunk_overlap: usize,
    /// Separator used to split the text into logical segments (default: `"\n\n"`)
    pub separator: String,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self {
            chunk_size: 512,
            chunk_overlap: 50,
            separator: "\n\n".to_string(),
        }
    }
}

impl ChunkConfig {
    /// Create a new config with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the chunk size
    pub fn with_chunk_size(mut self, size: usize) -> Self {
        self.chunk_size = size;
        self
    }

    /// Set the chunk overlap
    pub fn with_overlap(mut self, overlap: usize) -> Self {
        self.chunk_overlap = overlap;
        self
    }

    /// Set the separator
    pub fn with_separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }
}

/// A single chunk produced by the [`Chunker`]
#[derive(Debug, Clone)]
pub struct Chunk {
    /// Unique identifier for this chunk
    pub id: String,
    /// The chunk text content
    pub content: String,
    /// Associated source document ID (e.g. fact id)
    pub source_id: i64,
    /// Index of this chunk within the original document
    pub index: usize,
}

/// Stateless document chunker
pub struct Chunker;

impl Chunker {
    /// Split `text` into chunks according to `config`.
    ///
    /// The returned chunks have `source_id` set to `0`. Callers should
    /// overwrite it with the real source id if needed.
    pub fn split(text: &str, config: &ChunkConfig) -> Vec<Chunk> {
        // Guard against overlap >= chunk_size which would cause infinite loops.
        let overlap = config
            .chunk_overlap
            .min(config.chunk_size.saturating_sub(1));
        let segments: Vec<&str> = text.split(&config.separator).collect();
        let mut chunk_texts: Vec<String> = Vec::new();
        let mut current = String::new();

        for segment in segments {
            let needed = if current.is_empty() {
                segment.len()
            } else {
                config.separator.len() + segment.len()
            };

            if needed > config.chunk_size && current.is_empty() {
                // Single segment exceeds chunk_size; split it directly.
                let mut seg = segment.to_string();
                while seg.len() > config.chunk_size {
                    chunk_texts.push(seg[..config.chunk_size].to_string());
                    let overlap_start = config.chunk_size.saturating_sub(overlap);
                    seg = seg[overlap_start..].to_string();
                }
                current = seg;
                continue;
            }

            if current.len() + needed <= config.chunk_size {
                if !current.is_empty() {
                    current.push_str(&config.separator);
                }
                current.push_str(segment);
            } else {
                // Finish current chunk.
                chunk_texts.push(current.clone());

                // Start next chunk with overlap from the previous chunk.
                let overlap_start = current.len().saturating_sub(overlap);
                current = current[overlap_start..].to_string();

                // Append the new segment.
                if !current.is_empty() {
                    current.push_str(&config.separator);
                }
                current.push_str(segment);

                // If the segment itself is so long that current still overflows,
                // split repeatedly.
                while current.len() > config.chunk_size {
                    chunk_texts.push(current[..config.chunk_size].to_string());
                    let overlap_start = config.chunk_size.saturating_sub(overlap);
                    current = current[overlap_start..].to_string();
                }
            }
        }

        if !current.is_empty() {
            chunk_texts.push(current);
        }

        chunk_texts
            .into_iter()
            .enumerate()
            .map(|(idx, content)| Chunk {
                id: format!("chunk-{}", idx),
                content,
                source_id: 0,
                index: idx,
            })
            .collect()
    }

    /// Split `text` into chunks and set the `source_id` on every chunk.
    pub fn split_with_source(text: &str, config: &ChunkConfig, source_id: i64) -> Vec<Chunk> {
        let mut chunks = Self::split(text, config);
        for (idx, chunk) in chunks.iter_mut().enumerate() {
            chunk.source_id = source_id;
            chunk.id = format!("{}-{}", source_id, idx);
        }
        chunks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_chunking() {
        let text = "Paragraph one.\n\nParagraph two.\n\nParagraph three.";
        let config = ChunkConfig::new().with_chunk_size(40).with_overlap(5);
        let chunks = Chunker::split(text, &config);

        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert!(
                chunk.content.len() <= config.chunk_size,
                "Chunk {} exceeds max size: len={}",
                chunk.index,
                chunk.content.len()
            );
        }
    }

    #[test]
    fn test_chunk_overlap() {
        let text = "A".repeat(200);
        let config = ChunkConfig::new().with_chunk_size(80).with_overlap(10);
        let chunks = Chunker::split(&text, &config);

        assert!(chunks.len() > 1);
        // Consecutive chunks should share some content.
        for window in chunks.windows(2) {
            let a = &window[0].content;
            let b = &window[1].content;
            let shared: String = a
                .chars()
                .rev()
                .take(config.chunk_overlap)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            assert!(
                b.starts_with(&shared) || shared.chars().all(|c| b.contains(c)),
                "Chunks should overlap"
            );
        }
    }

    #[test]
    fn test_single_chunk_fits() {
        let text = "Short text.";
        let config = ChunkConfig::new().with_chunk_size(100);
        let chunks = Chunker::split(text, &config);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, text);
        assert_eq!(chunks[0].index, 0);
    }

    #[test]
    fn test_empty_text() {
        let chunks = Chunker::split("", &ChunkConfig::new());
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_long_segment_split() {
        // A single segment longer than chunk_size
        let text = "A".repeat(300);
        let config = ChunkConfig::new().with_chunk_size(100).with_overlap(10);
        let chunks = Chunker::split(&text, &config);

        assert!(chunks.len() >= 2);
        for chunk in &chunks {
            assert!(chunk.content.len() <= config.chunk_size);
        }
    }

    #[test]
    fn test_chunk_indices_and_ids() {
        let text = "One.\n\nTwo.\n\nThree.";
        let config = ChunkConfig::new().with_chunk_size(10).with_overlap(2);
        let chunks = Chunker::split(text, &config);

        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.index, i);
            assert_eq!(chunk.id, format!("chunk-{}", i));
            assert_eq!(chunk.source_id, 0);
        }
    }

    #[test]
    fn test_split_with_source() {
        let text = "One.\n\nTwo.\n\nThree.";
        let config = ChunkConfig::new().with_chunk_size(10);
        let chunks = Chunker::split_with_source(text, &config, 42);

        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.source_id, 42);
            assert_eq!(chunk.id, format!("42-{}", i));
        }
    }

    #[test]
    fn test_config_builder() {
        let config = ChunkConfig::new()
            .with_chunk_size(256)
            .with_overlap(20)
            .with_separator("---");

        assert_eq!(config.chunk_size, 256);
        assert_eq!(config.chunk_overlap, 20);
        assert_eq!(config.separator, "---");
    }
}
