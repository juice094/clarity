//! Web tools: WebSearch and WebFetch
//!
//! This module provides tools for interacting with web resources:
//! - `WebSearchTool`: Search the internet using DuckDuckGo
//! - `WebFetchTool`: Fetch and extract content from web pages

use async_trait::async_trait;
use reqwest;
use serde::Deserialize;
use serde_json::json;
use serde_json::Value;
use tracing::{debug, error, warn};

use crate::error::ToolError;
use crate::tools::helpers;
use crate::tools::{Tool, ToolContext, ToolResult};

// =============================================================================
// WebSearchTool - Search the internet using DuckDuckGo
// =============================================================================

/// A search result from DuckDuckGo
#[derive(Debug, Clone, Deserialize)]
pub struct SearchResult {
    /// The title of the search result
    pub title: String,
    /// The URL of the result
    pub url: String,
    /// A snippet/description of the content
    pub snippet: String,
}

/// Tool for searching the web using DuckDuckGo
///
/// This tool performs web searches without requiring an API key
/// by using DuckDuckGo's HTML interface.
///
/// # Example
///
/// ```rust,no_run
/// use clarity_core::tools::WebSearchTool;
/// use clarity_core::tools::{Tool, ToolContext};
/// use serde_json::json;
///
/// # async fn example() -> anyhow::Result<()> {
/// let tool = WebSearchTool::new();
/// let ctx = ToolContext::new();
/// let args = json!({
///     "query": "Rust programming language",
///     "num_results": 5
/// });
/// let result = tool.execute(args, ctx).await?;
/// # Ok(())
/// # }
/// ```
pub struct WebSearchTool {
    client: reqwest::Client,
}

impl WebSearchTool {
    /// Create a new WebSearchTool instance
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");
        
        Self { client }
    }

    /// Create a new WebSearchTool with a custom reqwest client
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// Search using DuckDuckGo HTML interface
    async fn search_duckduckgo(
        &self,
        query: &str,
        num_results: usize,
    ) -> ToolResult<Vec<SearchResult>> {
        let url = format!(
            "https://html.duckduckgo.com/html/?q={}",
            urlencoding::encode(query)
        );

        debug!("Searching DuckDuckGo: {}", query);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to send search request: {}", e);
                ToolError::execution_failed(format!("Network error: {}", e))
            })?;

        let status = response.status();
        if !status.is_success() {
            error!("DuckDuckGo returned error status: {}", status);
            return Err(ToolError::execution_failed(format!(
                "Search service returned error: {}",
                status
            )));
        }

        let html = response
            .text()
            .await
            .map_err(|e| ToolError::execution_failed(format!("Failed to read response: {}", e)))?;

        let results = self.parse_duckduckgo_results(&html, num_results)?;
        
        debug!("Found {} search results", results.len());
        
        Ok(results)
    }

    /// Parse DuckDuckGo HTML response to extract search results
    fn parse_duckduckgo_results(
        &self,
        html: &str,
        max_results: usize,
    ) -> ToolResult<Vec<SearchResult>> {
        use regex::Regex;

        let mut results = Vec::new();

        // DuckDuckGo HTML result pattern
        // Each result is in a <div class="result"> element
        let result_regex = Regex::new(
            r#"<div class="result[^"]*"[^>]*>.*?<a[^>]*href="([^"]*)"[^>]*class="result__a"[^>]*>(.*?)</a>.*?<a[^>]*class="result__url"[^>]*href="[^"]*"[^>]*>(.*?)</a>.*?<a[^>]*class="result__snippet"[^>]*>(.*?)</a>.*?</div>"#,
        )
        .map_err(|e| ToolError::execution_failed(format!("Regex error: {}", e)))?;

        // Alternative pattern for different DDG HTML structure
        let alt_regex = Regex::new(
            r#"<div class="links_main[^"]*"[^>]*>.*?<a[^>]*href="/l/[^?]*\?uddg=([^&"]+)[^"]*"[^>]*>(.*?)</a>.*?(?:<div class="result__snippet"[^>]*>(.*?)</div>)?"#,
        )
        .map_err(|e| ToolError::execution_failed(format!("Regex error: {}", e)))?;

        // Try primary pattern
        for cap in result_regex.captures_iter(html) {
            if results.len() >= max_results {
                break;
            }

            let url = html_escape::decode_html_entities(&cap[1]).to_string();
            let title = self.clean_html(&cap[2]);
            let _display_url = html_escape::decode_html_entities(&cap[3]).to_string();
            let snippet = self.clean_html(&cap[4]);

            // Skip ads and invalid results
            if url.starts_with("http") && !title.is_empty() {
                results.push(SearchResult {
                    title,
                    url,
                    snippet,
                });
            }
        }

        // If no results with primary pattern, try alternative
        if results.is_empty() {
            for cap in alt_regex.captures_iter(html) {
                if results.len() >= max_results {
                    break;
                }

                let encoded_url = &cap[1];
                let url = urlencoding::decode(encoded_url)
                    .unwrap_or_else(|_| encoded_url.into())
                    .to_string();
                let title = self.clean_html(&cap[2]);
                let snippet = cap
                    .get(3)
                    .map(|m| self.clean_html(m.as_str()))
                    .unwrap_or_default();

                if url.starts_with("http") && !title.is_empty() {
                    results.push(SearchResult {
                        title,
                        url,
                        snippet,
                    });
                }
            }
        }

        // Fallback: Use a simpler regex if still no results
        if results.is_empty() {
            let simple_regex = Regex::new(
                r#"<a[^>]*class="result__a"[^>]*href="([^"]*)"[^>]*>(.*?)</a>"#,
            )
            .map_err(|e| ToolError::execution_failed(format!("Regex error: {}", e)))?;

            for cap in simple_regex.captures_iter(html) {
                if results.len() >= max_results {
                    break;
                }

                let url = html_escape::decode_html_entities(&cap[1]).to_string();
                let title = self.clean_html(&cap[2]);

                if url.starts_with("http") && !title.is_empty() && !results.iter().any(|r: &SearchResult| r.url == url) {
                    results.push(SearchResult {
                        title,
                        url,
                        snippet: String::new(),
                    });
                }
            }
        }

        Ok(results)
    }

    /// Clean HTML tags and entities from text
    fn clean_html(&self, html: &str) -> String {
        // Remove HTML tags
        let tag_regex = regex::Regex::new(r"<[^>]+>").unwrap();
        let text = tag_regex.replace_all(html, "");

        // Decode HTML entities
        let text = html_escape::decode_html_entities(&text);

        // Normalize whitespace
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");

        text.trim().to_string()
    }

    /// Search with retry logic
    async fn search_with_retry(
        &self,
        query: &str,
        num_results: usize,
        max_retries: u32,
    ) -> ToolResult<Vec<SearchResult>> {
        let mut last_error = None;
        let mut delay = std::time::Duration::from_millis(500);

        for attempt in 0..max_retries {
            match self.search_duckduckgo(query, num_results).await {
                Ok(results) if !results.is_empty() => return Ok(results),
                Ok(_) => {
                    // Empty results, might be rate limit or parsing issue
                    warn!("Search returned empty results, attempt {}/{}", attempt + 1, max_retries);
                }
                Err(e) => {
                    warn!("Search failed (attempt {}/{}): {}", attempt + 1, max_retries, e);
                    last_error = Some(e);
                }
            }

            if attempt < max_retries - 1 {
                tokio::time::sleep(delay).await;
                delay *= 2; // Exponential backoff
            }
        }

        Err(last_error.unwrap_or_else(|| {
            ToolError::execution_failed("Search failed after retries".to_string())
        }))
    }
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the internet for information. Uses DuckDuckGo to find relevant web pages. \
         Returns search results with titles, URLs, and snippets."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query to execute"
                },
                "num_results": {
                    "type": "integer",
                    "description": "Number of results to return (default: 5, max: 20)",
                    "minimum": 1,
                    "maximum": 20,
                    "default": 5
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let query = helpers::required_str(&args, "query")?;
        let num_results = args
            .get("num_results")
            .and_then(|v| v.as_u64())
            .map(|v| (v as usize).min(20))
            .unwrap_or(5);

        if query.trim().is_empty() {
            return Err(ToolError::invalid_params("Search query cannot be empty"));
        }

        debug!("Executing web search: query='{}', num_results={}", query, num_results);

        let results = self.search_with_retry(query, num_results, 3).await?;

        Ok(json!({
            "query": query,
            "num_results": results.len(),
            "results": results.iter().map(|r| {
                json!({
                    "title": r.title,
                    "url": r.url,
                    "snippet": r.snippet
                })
            }).collect::<Vec<_>>()
        }))
    }
}

// =============================================================================
// WebFetchTool - Fetch and extract content from web pages
// =============================================================================

/// Tool for fetching web page content
///
/// This tool fetches a web page and extracts the main content,
/// optionally converting it to markdown or plain text.
///
/// # Example
///
/// ```rust,no_run
/// use clarity_core::tools::WebFetchTool;
/// use clarity_core::tools::{Tool, ToolContext};
/// use serde_json::json;
///
/// # async fn example() -> anyhow::Result<()> {
/// let tool = WebFetchTool::new();
/// let ctx = ToolContext::new();
/// let args = json!({
///     "url": "https://www.rust-lang.org",
///     "format": "markdown"
/// });
/// let result = tool.execute(args, ctx).await?;
/// # Ok(())
/// # }
/// ```
pub struct WebFetchTool {
    client: reqwest::Client,
}

impl WebFetchTool {
    /// Create a new WebFetchTool instance
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");
        
        Self { client }
    }

    /// Create a new WebFetchTool with a custom reqwest client
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// Fetch a URL and extract content
    async fn fetch_url(&self, url: &str) -> ToolResult<(String, String, String)> {
        debug!("Fetching URL: {}", url);

        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to fetch URL: {}", e);
                if e.is_timeout() {
                    ToolError::Timeout(30)
                } else {
                    ToolError::execution_failed(format!("Network error: {}", e))
                }
            })?;

        let status = response.status();
        if !status.is_success() {
            return Err(ToolError::execution_failed(format!(
                "HTTP error {}: {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or("Unknown")
            )));
        }

        // Try to get the title from headers or URL
        let final_url = response.url().to_string();
        let html = response
            .text()
            .await
            .map_err(|e| ToolError::execution_failed(format!("Failed to read response: {}", e)))?;

        let title = self.extract_title(&html).unwrap_or_else(|| final_url.clone());
        let content = self.extract_content(&html);

        Ok((title, final_url, content))
    }

    /// Extract title from HTML
    fn extract_title(&self, html: &str) -> Option<String> {
        let title_regex = regex::Regex::new(r"<title[^>]*>([^<]*)</title>").ok()?;
        title_regex
            .captures(html)
            .and_then(|cap| cap.get(1))
            .map(|m| html_escape::decode_html_entities(m.as_str()).trim().to_string())
    }

    /// Extract main content from HTML (simplified)
    fn extract_content(&self, html: &str) -> String {
        // Remove script and style tags with their content
        let script_regex = regex::Regex::new(r"<script[^>]*>[\s\S]*?</script>").unwrap();
        let style_regex = regex::Regex::new(r"<style[^>]*>[\s\S]*?</style>").unwrap();
        let nav_regex = regex::Regex::new(r"<nav[^>]*>[\s\S]*?</nav>").unwrap();
        let footer_regex = regex::Regex::new(r"<footer[^>]*>[\s\S]*?</footer>").unwrap();
        let header_regex = regex::Regex::new(r"<header[^>]*>[\s\S]*?</header>").unwrap();

        let mut text = html.to_string();
        text = script_regex.replace_all(&text, "").to_string();
        text = style_regex.replace_all(&text, "").to_string();
        text = nav_regex.replace_all(&text, "").to_string();
        text = footer_regex.replace_all(&text, "").to_string();
        text = header_regex.replace_all(&text, "").to_string();

        // Try to extract main or article content
        let main_regex = regex::Regex::new(r"<main[^>]*>([\s\S]*?)</main>").unwrap();
        let article_regex = regex::Regex::new(r"<article[^>]*>([\s\S]*?)</article>").unwrap();
        let content_regex = regex::Regex::new(r#"<div[^>]*class=["'][^"']*(?:content|main|body)[^"']*["'][^>]*>([\s\S]*?)</div>"#).unwrap();

        let content = if let Some(cap) = main_regex.captures(&text) {
            cap.get(1).map(|m| m.as_str()).unwrap_or(&text)
        } else if let Some(cap) = article_regex.captures(&text) {
            cap.get(1).map(|m| m.as_str()).unwrap_or(&text)
        } else if let Some(cap) = content_regex.captures(&text) {
            cap.get(1).map(|m| m.as_str()).unwrap_or(&text)
        } else {
            &text
        };

        // Convert remaining HTML to text
        self.html_to_text(content)
    }

    /// Convert HTML to plain text
    fn html_to_text(&self, html: &str) -> String {
        // Replace common block elements with newlines
        let mut text = html.to_string();
        
        // Add newlines around block elements
        let block_tags = ["p", "div", "h1", "h2", "h3", "h4", "h5", "h6", "li", "tr"];
        for tag in &block_tags {
            let open_regex = regex::Regex::new(&format!(r"<{}[^>]*>", tag)).unwrap();
            let close_regex = regex::Regex::new(&format!(r"</{}>", tag)).unwrap();
            text = open_regex.replace_all(&text, "\n").to_string();
            text = close_regex.replace_all(&text, "\n").to_string();
        }
        
        // Handle line breaks
        text = text.replace("<br>", "\n").replace("<br/>", "\n").replace("<br />", "\n");
        
        // Remove remaining HTML tags
        let tag_regex = regex::Regex::new(r"<[^>]+>").unwrap();
        text = tag_regex.replace_all(&text, "").to_string();
        
        // Decode HTML entities
        text = html_escape::decode_html_entities(&text).to_string();
        
        // Normalize whitespace
        let lines: Vec<_> = text
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect();
        
        text = lines.join("\n\n");
        
        // Limit length
        if text.len() > 50000 {
            text = text[..50000].to_string();
            text.push_str("\n\n[Content truncated due to length]");
        }
        
        text.trim().to_string()
    }

    /// Convert text to simple markdown
    fn text_to_markdown(&self, text: &str) -> String {
        // Basic conversion - just return the text for now
        // In a full implementation, this would add proper markdown formatting
        text.to_string()
    }
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch content from a web page. Returns the page title, URL, and extracted content. \
         Automatically handles redirects and extracts readable text from HTML."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch"
                },
                "format": {
                    "type": "string",
                    "description": "Output format: 'text' (default), 'markdown', or 'html'",
                    "enum": ["text", "markdown", "html"],
                    "default": "text"
                },
                "max_length": {
                    "type": "integer",
                    "description": "Maximum content length in characters (default: 50000)",
                    "minimum": 100,
                    "maximum": 100000,
                    "default": 50000
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let url = helpers::required_str(&args, "url")?;
        let format = helpers::optional_str(&args, "format").unwrap_or("text");
        let max_length = args
            .get("max_length")
            .and_then(|v| v.as_u64())
            .map(|v| (v as usize).min(100000))
            .unwrap_or(50000);

        // Validate URL
        let parsed_url = url::Url::parse(url)
            .map_err(|e| ToolError::invalid_params(format!("Invalid URL: {}", e)))?;
        
        if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
            return Err(ToolError::invalid_params(
                "Only HTTP and HTTPS URLs are supported"
            ));
        }

        debug!("Executing web fetch: url='{}', format='{}'", url, format);

        let (title, final_url, content) = self.fetch_url(url).await?;

        // Apply format
        let formatted_content = match format {
            "markdown" => self.text_to_markdown(&content),
            "html" => content, // Already extracted as text, would need different approach for raw HTML
            _ => content, // "text" is default
        };

        // Apply max length
        let truncated = if formatted_content.len() > max_length {
            let truncated = &formatted_content[..max_length];
            format!("{}\n\n[Content truncated, {} characters remaining]", 
                truncated, 
                formatted_content.len() - max_length
            )
        } else {
            formatted_content
        };

        Ok(json!({
            "title": title,
            "url": final_url,
            "content": truncated,
            "format": format,
            "length": truncated.len()
        }))
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;



    #[test]
    fn test_websearch_tool_name() {
        let tool = WebSearchTool::new();
        assert_eq!(tool.name(), "web_search");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_webfetch_tool_name() {
        let tool = WebFetchTool::new();
        assert_eq!(tool.name(), "web_fetch");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_websearch_parse_results() {
        let tool = WebSearchTool::new();
        
        // Test with a simplified HTML that matches the fallback regex pattern
        // The fallback pattern is more reliable for testing
        let simple_html = r#"
        <a class="result__a" href="https://example.com/1">First Result</a>
        <a class="result__a" href="https://example.com/2">Second Result</a>
        "#;
        
        let results = tool.parse_duckduckgo_results(simple_html, 10).unwrap();
        
        // The fallback regex should find these results
        assert!(!results.is_empty(), "Should find at least one result");
        assert_eq!(results[0].title, "First Result");
        assert_eq!(results[0].url, "https://example.com/1");
    }

    #[test]
    fn test_websearch_empty_query_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let tool = WebSearchTool::new();
            let ctx = ToolContext::new();
            
            let args = json!({"query": ""});
            let result = tool.execute(args, ctx).await;
            
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("empty"));
        });
    }

    #[test]
    fn test_webfetch_invalid_url_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let tool = WebFetchTool::new();
            let ctx = ToolContext::new();
            
            let args = json!({"url": "not-a-valid-url"});
            let result = tool.execute(args, ctx).await;
            
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_webfetch_non_http_url_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let tool = WebFetchTool::new();
            let ctx = ToolContext::new();
            
            let args = json!({"url": "ftp://example.com/file.txt"});
            let result = tool.execute(args, ctx).await;
            
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("HTTP"));
        });
    }

    #[test]
    fn test_websearch_parameters_schema() {
        let tool = WebSearchTool::new();
        let params = tool.parameters();
        
        assert!(params.get("type").unwrap().as_str().unwrap() == "object");
        assert!(params.get("properties").unwrap().get("query").is_some());
        assert!(params.get("properties").unwrap().get("num_results").is_some());
        
        let required = params.get("required").unwrap().as_array().unwrap();
        assert!(required.contains(&json!("query")));
    }

    #[test]
    fn test_webfetch_parameters_schema() {
        let tool = WebFetchTool::new();
        let params = tool.parameters();
        
        assert!(params.get("type").unwrap().as_str().unwrap() == "object");
        assert!(params.get("properties").unwrap().get("url").is_some());
        assert!(params.get("properties").unwrap().get("format").is_some());
        assert!(params.get("properties").unwrap().get("max_length").is_some());
        
        let required = params.get("required").unwrap().as_array().unwrap();
        assert!(required.contains(&json!("url")));
    }

    #[test]
    fn test_websearch_clean_html() {
        let tool = WebSearchTool::new();
        
        let html = "<b>Bold</b> and <i>italic</i> text";
        let cleaned = tool.clean_html(html);
        assert_eq!(cleaned, "Bold and italic text");
        
        let html_entities = "Tom &amp; Jerry";
        let cleaned = tool.clean_html(html_entities);
        assert_eq!(cleaned, "Tom & Jerry");
    }

    #[test]
    fn test_webfetch_extract_title() {
        let tool = WebFetchTool::new();
        
        let html = "<html><head><title>Test Page</title></head><body></body></html>";
        let title = tool.extract_title(html);
        assert_eq!(title, Some("Test Page".to_string()));
        
        let no_title = "<html><body>No title here</body></html>";
        assert!(tool.extract_title(no_title).is_none());
    }

    #[test]
    fn test_webfetch_html_to_text() {
        let tool = WebFetchTool::new();
        
        let html = "<p>First paragraph</p><p>Second paragraph</p>";
        let text = tool.html_to_text(html);
        assert!(text.contains("First paragraph"));
        assert!(text.contains("Second paragraph"));
    }

    #[test]
    fn test_webfetch_remove_scripts() {
        let tool = WebFetchTool::new();
        
        let html = r#"<p>Content</p><script>alert("xss")</script><p>More content</p>"#;
        let text = tool.extract_content(html);
        assert!(text.contains("Content"));
        assert!(text.contains("More content"));
        assert!(!text.contains("script"));
        assert!(!text.contains("alert"));
    }

    // Integration test - requires network
    #[tokio::test]
    #[ignore = "Requires network access"]
    async fn test_websearch_integration() {
        let tool = WebSearchTool::new();
        let ctx = ToolContext::new();
        
        let args = json!({
            "query": "Rust programming language",
            "num_results": 3
        });
        
        let result = tool.execute(args, ctx).await;
        
        assert!(result.is_ok(), "Search failed: {:?}", result.err());
        let value = result.unwrap();
        
        assert!(!value.get("results").unwrap().as_array().unwrap().is_empty());
        assert!(value.get("query").unwrap().as_str().unwrap() == "Rust programming language");
    }

    // Integration test - requires network
    #[tokio::test]
    #[ignore = "Requires network access"]
    async fn test_webfetch_integration() {
        let tool = WebFetchTool::new();
        let ctx = ToolContext::new();
        
        let args = json!({
            "url": "https://www.rust-lang.org",
            "format": "text"
        });
        
        let result = tool.execute(args, ctx).await;
        
        assert!(result.is_ok(), "Fetch failed: {:?}", result.err());
        let value = result.unwrap();
        
        assert!(value.get("title").is_some());
        assert!(!value.get("content").unwrap().as_str().unwrap().is_empty());
    }
}
