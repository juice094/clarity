//! Web browser automation tool
//!
//! This module provides `WebBrowserTool` for interactive web automation.
//!
//! **Implementation note**: This is the lightweight (Scheme A) implementation.
//! It uses `reqwest` + `scraper` for page navigation and content extraction.
//! Actions requiring a real browser engine (`click`, `type`, `screenshot`)
//! are not supported and return a descriptive error. This ensures zero
//! external configuration (no geckodriver / chromedriver required) while
//! still covering the most common read-only automation workflows.

use async_trait::async_trait;
use regex::Regex;
use scraper::{Html, Selector};
use serde_json::{json, Value};
use std::sync::Mutex;
use tracing::{debug, error};

use crate::helpers;
use crate::{Tool, ToolContext, ToolResult};
use clarity_contract::ToolError;

/// Internal mutable state kept between actions.
#[derive(Debug, Clone, Default)]
struct BrowserState {
    current_url: Option<String>,
    current_html: Option<String>,
}

/// Lightweight web-browser tool.
///
/// Supports navigation and DOM extraction without requiring an external
/// WebDriver. Actions that need JavaScript execution or visual interaction
/// (`click`, `type`, `screenshot`) are reported as unsupported so the
/// caller can fall back to other tools if necessary.
pub struct WebBrowserTool {
    client: reqwest::Client,
    state: Mutex<BrowserState>,
}

impl WebBrowserTool {
    /// Create a new `WebBrowserTool` with a default HTTP client.
    pub fn new() -> Result<Self, ToolError> {
        let client = reqwest::Client::builder()
            .user_agent(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            )
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| {
                ToolError::execution_failed(format!("Failed to build HTTP client: {e}"))
            })?;

        Ok(Self {
            client,
            state: Mutex::new(BrowserState::default()),
        })
    }

    /// Create a new `WebBrowserTool` with a custom `reqwest` client.
    pub fn with_client(client: reqwest::Client) -> Self {
        Self {
            client,
            state: Mutex::new(BrowserState::default()),
        }
    }

    // ------------------------------------------------------------------
    // Action handlers
    // ------------------------------------------------------------------

    async fn navigate(&self, url: &str) -> ToolResult<Value> {
        let parsed = url::Url::parse(url)
            .map_err(|e| ToolError::invalid_params(format!("Invalid URL: {}", e)))?;

        if parsed.scheme() != "http" && parsed.scheme() != "https" {
            return Err(ToolError::invalid_params(
                "Only HTTP and HTTPS URLs are supported",
            ));
        }

        debug!("Navigating to: {}", url);

        let response = self.client.get(url).send().await.map_err(|e| {
            error!("Failed to navigate to {}: {}", url, e);
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

        let final_url = response.url().to_string();
        let html = response
            .text()
            .await
            .map_err(|e| ToolError::execution_failed(format!("Failed to read response: {}", e)))?;

        let title = Self::extract_title(&html).unwrap_or_else(|| final_url.clone());

        {
            let mut state = self.state.lock().map_err(|_| {
                ToolError::execution_failed("Browser state lock poisoned".to_string())
            })?;
            state.current_url = Some(final_url.clone());
            state.current_html = Some(html);
        }

        Ok(json!({
            "url": final_url,
            "title": title,
            "status": status.as_u16(),
        }))
    }

    async fn click(&self, _selector: &str) -> ToolResult<Value> {
        Err(ToolError::execution_failed(
            "The 'click' action requires a real browser engine (WebDriver). \
             Please install geckodriver or chromedriver and use a full browser-automation setup."
                .to_string(),
        ))
    }

    async fn type_text(&self, _selector: &str, _text: &str) -> ToolResult<Value> {
        Err(ToolError::execution_failed(
            "The 'type' action requires a real browser engine (WebDriver). \
             Please install geckodriver or chromedriver and use a full browser-automation setup."
                .to_string(),
        ))
    }

    async fn screenshot(&self) -> ToolResult<Value> {
        Err(ToolError::execution_failed(
            "The 'screenshot' action requires a real browser engine (WebDriver). \
             Please install geckodriver or chromedriver and use a full browser-automation setup."
                .to_string(),
        ))
    }

    async fn get_text(&self, selector: Option<&str>) -> ToolResult<Value> {
        let html = self.current_html()?;

        let text = if let Some(sel) = selector {
            Self::extract_element_text(&html, sel)?
        } else {
            Self::html_to_text(&html)
        };

        Ok(json!({ "text": text }))
    }

    async fn get_html(&self, selector: Option<&str>) -> ToolResult<Value> {
        let html = self.current_html()?;

        let output = if let Some(sel) = selector {
            Self::extract_element_html(&html, sel)?
        } else {
            html
        };

        Ok(json!({ "html": output }))
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    fn current_html(&self) -> ToolResult<String> {
        let state = self
            .state
            .lock()
            .map_err(|_| ToolError::execution_failed("Browser state lock poisoned".to_string()))?;
        state.current_html.clone().ok_or_else(|| {
            ToolError::execution_failed("No page loaded. Use 'navigate' first.".to_string())
        })
    }

    fn extract_title(html: &str) -> Option<String> {
        let re = Regex::new(r"<title[^>]*>([^<]*)</title>").ok()?;
        re.captures(html).and_then(|cap| cap.get(1)).map(|m| {
            html_escape::decode_html_entities(m.as_str())
                .trim()
                .to_string()
        })
    }

    fn extract_element_text(html: &str, selector: &str) -> ToolResult<String> {
        let document = Html::parse_document(html);
        let sel = Selector::parse(selector).map_err(|e| {
            ToolError::invalid_params(format!("Invalid CSS selector '{}': {:?}", selector, e))
        })?;

        let mut texts = Vec::new();
        for element in document.select(&sel) {
            texts.push(element.text().collect::<String>());
        }

        if texts.is_empty() {
            return Err(ToolError::execution_failed(format!(
                "No elements matched selector '{}'",
                selector
            )));
        }

        Ok(texts.join("\n"))
    }

    fn extract_element_html(html: &str, selector: &str) -> ToolResult<String> {
        let document = Html::parse_document(html);
        let sel = Selector::parse(selector).map_err(|e| {
            ToolError::invalid_params(format!("Invalid CSS selector '{}': {:?}", selector, e))
        })?;

        let mut fragments = Vec::new();
        for element in document.select(&sel) {
            fragments.push(element.html());
        }

        if fragments.is_empty() {
            return Err(ToolError::execution_failed(format!(
                "No elements matched selector '{}'",
                selector
            )));
        }

        Ok(fragments.join("\n"))
    }

    /// Convert raw HTML to readable plain text.
    fn html_to_text(html: &str) -> String {
        // Pre-compiled regexes — avoid re-compilation on every call.
        static SCRIPT_STYLE_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
        static BLOCK_OPEN_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
        static BLOCK_CLOSE_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
        static TAG_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();

        let mut text = html.to_string();

        // Strip script / style / nav / footer / header tags entirely.
        text = SCRIPT_STYLE_RE
            .get_or_init(|| Regex::new(r"<(?:script|style|nav|footer|header)[^>]*>[\s\S]*?</(?:script|style|nav|footer|header)>").unwrap())
            .replace_all(&text, "")
            .to_string();

        // Replace common block elements with newlines.
        text = BLOCK_OPEN_RE
            .get_or_init(|| Regex::new(r"<(?:p|div|h[1-6]|li|tr)[^>]*>").unwrap())
            .replace_all(&text, "\n")
            .to_string();
        text = BLOCK_CLOSE_RE
            .get_or_init(|| Regex::new(r"</(?:p|div|h[1-6]|li|tr)>").unwrap())
            .replace_all(&text, "\n")
            .to_string();

        text = text
            .replace("<br>", "\n")
            .replace("<br/>", "\n")
            .replace("<br />", "\n");

        text = TAG_RE
            .get_or_init(|| Regex::new(r"<[^>]+>").unwrap())
            .replace_all(&text, "")
            .to_string();

        text = html_escape::decode_html_entities(&text).to_string();

        let lines: Vec<_> = text
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();

        text = lines.join("\n\n");

        if text.len() > 50_000 {
            text = text[..50_000].to_string();
            text.push_str("\n\n[Content truncated due to length]");
        }

        text.trim().to_string()
    }
}

impl Default for WebBrowserTool {
    fn default() -> Self {
        Self::with_client(reqwest::Client::new())
    }
}

#[async_trait]
impl Tool for WebBrowserTool {
    fn name(&self) -> &str {
        "web_browser"
    }

    fn description(&self) -> &str {
        "Control a web browser to navigate pages, click elements, type text, take screenshots, \
         or extract content. Use this for interactive web automation when simple HTTP fetching \
         is not enough."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["navigate", "get_text", "get_html"],
                    "description": "The browser action to perform. This is a lightweight implementation: navigate fetches a page, get_text/get_html extract content. Interactive actions (click, type, screenshot) are not supported."
                },
                "url": {
                    "type": "string",
                    "description": "URL to navigate to (required for 'navigate')"
                },
                "selector": {
                    "type": "string",
                    "description": "CSS selector for targeting an element (used by 'get_text', 'get_html')"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let action = helpers::required_str(&args, "action")?;

        debug!("Executing web_browser action: {}", action);

        match action {
            "navigate" => {
                let url = helpers::required_str(&args, "url")?;
                self.navigate(url).await
            }
            "click" => {
                let selector = helpers::required_str(&args, "selector")?;
                self.click(selector).await
            }
            "type" => {
                let selector = helpers::required_str(&args, "selector")?;
                let text = helpers::required_str(&args, "text")?;
                self.type_text(selector, text).await
            }
            "screenshot" => self.screenshot().await,
            "get_text" => {
                let selector = helpers::optional_str(&args, "selector");
                self.get_text(selector).await
            }
            "get_html" => {
                let selector = helpers::optional_str(&args, "selector");
                self.get_html(selector).await
            }
            _ => Err(ToolError::invalid_params(format!(
                "Unknown action '{}'. Supported actions: navigate, get_text, get_html",
                action
            ))),
        }
    }

    fn requires_approval(&self) -> bool {
        true
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webbrowser_tool_name() {
        let tool = WebBrowserTool::new().unwrap();
        assert_eq!(tool.name(), "web_browser");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_webbrowser_requires_approval() {
        let tool = WebBrowserTool::new().unwrap();
        assert!(tool.requires_approval());
    }

    #[test]
    fn test_webbrowser_parameters_schema() {
        let tool = WebBrowserTool::new().unwrap();
        let params = tool.parameters();

        assert_eq!(params.get("type").unwrap().as_str().unwrap(), "object");
        assert!(params.get("properties").unwrap().get("action").is_some());
        assert!(params.get("properties").unwrap().get("url").is_some());
        assert!(params.get("properties").unwrap().get("selector").is_some());

        let required = params.get("required").unwrap().as_array().unwrap();
        assert!(required.contains(&json!("action")));
    }

    #[test]
    fn test_html_to_text() {
        let html = "<p>First paragraph</p><p>Second paragraph</p>";
        let text = WebBrowserTool::html_to_text(html);
        assert!(text.contains("First paragraph"));
        assert!(text.contains("Second paragraph"));
    }

    #[test]
    fn test_html_to_text_removes_scripts() {
        let html = r#"<p>Content</p><script>alert("xss")</script><p>More content</p>"#;
        let text = WebBrowserTool::html_to_text(html);
        assert!(text.contains("Content"));
        assert!(text.contains("More content"));
        assert!(!text.contains("script"));
        assert!(!text.contains("alert"));
    }

    #[test]
    fn test_extract_title() {
        let html = "<html><head><title>Test Page</title></head><body></body></html>";
        assert_eq!(
            WebBrowserTool::extract_title(html),
            Some("Test Page".to_string())
        );

        let no_title = "<html><body>No title</body></html>";
        assert!(WebBrowserTool::extract_title(no_title).is_none());
    }

    #[test]
    fn test_extract_element_text() {
        let html = r#"<div><p class="foo">Hello</p><p class="foo">World</p></div>"#;
        let result = WebBrowserTool::extract_element_text(html, "p.foo").unwrap();
        assert_eq!(result, "Hello\nWorld");
    }

    #[test]
    fn test_extract_element_text_no_match() {
        let html = r#"<div><p>No match</p></div>"#;
        let result = WebBrowserTool::extract_element_text(html, "span.missing");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_element_html() {
        let html = r#"<div><p class="bar"><b>Bold</b></p></div>"#;
        let result = WebBrowserTool::extract_element_html(html, "p.bar").unwrap();
        assert!(result.contains("<b>Bold</b>"));
    }

    #[test]
    fn test_get_text_without_navigation_fails() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let tool = WebBrowserTool::new().unwrap();
            let ctx = ToolContext::new();
            let args = json!({"action": "get_text"});
            let result = tool.execute(args, ctx).await;
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("No page loaded"));
        });
    }

    #[test]
    fn test_get_html_without_navigation_fails() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let tool = WebBrowserTool::new().unwrap();
            let ctx = ToolContext::new();
            let args = json!({"action": "get_html"});
            let result = tool.execute(args, ctx).await;
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("No page loaded"));
        });
    }

    #[test]
    fn test_click_returns_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let tool = WebBrowserTool::new().unwrap();
            let ctx = ToolContext::new();
            let args = json!({"action": "click", "selector": "button"});
            let result = tool.execute(args, ctx).await;
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("WebDriver"));
        });
    }

    #[test]
    fn test_type_returns_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let tool = WebBrowserTool::new().unwrap();
            let ctx = ToolContext::new();
            let args = json!({"action": "type", "selector": "input", "text": "hello"});
            let result = tool.execute(args, ctx).await;
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("WebDriver"));
        });
    }

    #[test]
    fn test_screenshot_returns_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let tool = WebBrowserTool::new().unwrap();
            let ctx = ToolContext::new();
            let args = json!({"action": "screenshot"});
            let result = tool.execute(args, ctx).await;
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("WebDriver"));
        });
    }

    #[test]
    fn test_unknown_action_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let tool = WebBrowserTool::new().unwrap();
            let ctx = ToolContext::new();
            let args = json!({"action": "invalid_action"});
            let result = tool.execute(args, ctx).await;
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("Unknown action"));
        });
    }

    #[test]
    fn test_navigate_invalid_url() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let tool = WebBrowserTool::new().unwrap();
            let ctx = ToolContext::new();
            let args = json!({"action": "navigate", "url": "not-a-url"});
            let result = tool.execute(args, ctx).await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_navigate_non_http_url() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let tool = WebBrowserTool::new().unwrap();
            let ctx = ToolContext::new();
            let args = json!({"action": "navigate", "url": "ftp://example.com/file.txt"});
            let result = tool.execute(args, ctx).await;
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("HTTP"));
        });
    }

    // Integration test – requires network
    #[tokio::test]
    #[ignore = "Requires network access"]
    async fn test_navigate_and_get_text_integration() {
        let tool = WebBrowserTool::new().unwrap();
        let ctx = ToolContext::new();

        let args = json!({"action": "navigate", "url": "https://www.rust-lang.org"});
        let result = tool.execute(args, ctx.clone()).await;
        assert!(result.is_ok(), "Navigate failed: {:?}", result.err());

        let args = json!({"action": "get_text"});
        let result = tool.execute(args, ctx).await;
        assert!(result.is_ok(), "get_text failed: {:?}", result.err());
        let val = result.unwrap();
        let text = val.get("text").unwrap().as_str().unwrap();
        assert!(!text.is_empty());
    }
}
