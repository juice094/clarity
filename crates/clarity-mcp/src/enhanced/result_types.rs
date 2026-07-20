use serde::Deserialize;
use serde_json::Value;
// =============================================================================
// Types
// =============================================================================

/// Metadata describing an MCP tool exposed by a server.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTool {
    /// Tool name.
    pub name: String,
    /// Human-readable description.
    pub description: Option<String>,
    /// JSON Schema describing the tool's input parameters.
    pub input_schema: Value,
}

/// Result of the MCP `tools/call` method.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallResult {
    /// Content items returned by the tool call.
    pub content: Vec<ToolContent>,
    /// Whether the tool reported an application-level error.
    #[serde(default)]
    pub is_error: bool,
}

/// Content item returned by an MCP tool call.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ToolContent {
    /// Plain text content.
    #[serde(rename = "text")]
    Text {
        /// Text payload.
        text: String,
    },
    /// Base64-encoded image content.
    #[serde(rename = "image")]
    Image {
        /// Base64 image data.
        data: String,
        /// MIME type of the image.
        mime_type: String,
    },
    /// Resource reference.
    #[serde(rename = "resource")]
    Resource {
        /// Referenced resource.
        resource: McpResource,
    },
}

/// Resource reference embedded in MCP tool or prompt content.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResource {
    /// Resource URI.
    pub uri: String,
    /// MIME type, if known.
    pub mime_type: Option<String>,
    /// Text contents, if available.
    pub text: Option<String>,
    /// Base64-encoded binary contents, if available.
    pub blob: Option<String>,
}

// =============================================================================
// Resource Types
// =============================================================================

/// Metadata describing an MCP resource exposed by a server.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResourceMeta {
    /// Resource URI.
    pub uri: String,
    /// Display name.
    pub name: Option<String>,
    /// MIME type, if known.
    pub mime_type: Option<String>,
    /// Human-readable description.
    pub description: Option<String>,
}

/// Result of the MCP `resources/list` method.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResourcesResult {
    /// Available resources.
    pub resources: Vec<McpResourceMeta>,
}

/// Text contents returned by the MCP `resources/read` method.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextResourceContents {
    /// Resource URI.
    pub uri: String,
    /// MIME type, if known.
    pub mime_type: Option<String>,
    /// Text payload.
    pub text: String,
}

/// Binary contents returned by the MCP `resources/read` method.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobResourceContents {
    /// Resource URI.
    pub uri: String,
    /// MIME type, if known.
    pub mime_type: Option<String>,
    /// Base64-encoded binary payload.
    pub blob: String,
}

/// Discriminated resource contents returned by `resources/read`.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ResourceContents {
    /// Text resource contents.
    Text(TextResourceContents),
    /// Binary resource contents.
    Blob(BlobResourceContents),
}

/// Result of the MCP `resources/read` method.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadResourceResult {
    /// Contents of the resource.
    pub contents: Vec<ResourceContents>,
}

// =============================================================================
// Prompt Types
// =============================================================================

/// Argument accepted by an MCP prompt template.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptArgument {
    /// Argument name.
    pub name: String,
    /// Human-readable description.
    pub description: Option<String>,
    /// Whether the argument is required.
    pub required: Option<bool>,
}

/// Prompt template exposed by an MCP server.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPrompt {
    /// Prompt name.
    pub name: String,
    /// Human-readable description.
    pub description: Option<String>,
    /// Accepted arguments, if any.
    pub arguments: Option<Vec<PromptArgument>>,
}

/// Result of the MCP `prompts/list` method.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPromptsResult {
    /// Available prompts.
    pub prompts: Vec<McpPrompt>,
}

/// Role of a message within a rendered prompt.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PromptMessageRole {
    /// Message from the user.
    User,
    /// Message from the assistant.
    Assistant,
}

/// Content of a message within a rendered prompt.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum PromptContent {
    /// Plain text content.
    #[serde(rename = "text")]
    Text {
        /// Text payload.
        text: String,
    },
    /// Base64-encoded image content.
    #[serde(rename = "image")]
    Image {
        /// Base64 image data.
        data: String,
        /// MIME type of the image.
        mime_type: String,
    },
    /// Resource reference.
    #[serde(rename = "resource")]
    Resource {
        /// Referenced resource.
        resource: McpResource,
    },
}

/// A single message in a rendered prompt.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptMessage {
    /// Message role.
    pub role: PromptMessageRole,
    /// Message content.
    pub content: PromptContent,
}

/// Result of the MCP `prompts/get` method.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPromptResult {
    /// Prompt description.
    pub description: Option<String>,
    /// Rendered prompt messages.
    pub messages: Vec<PromptMessage>,
}
