use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// Transport Types
// =============================================================================

/// MCP transport types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "transport", rename_all = "lowercase")]
pub enum McpTransport {
    /// Stdio transport (local process)
    Stdio {
        /// Command to execute.
        command: String,
        /// Command-line arguments.
        #[serde(default)]
        args: Vec<String>,
        /// Environment variables passed to the subprocess.
        #[serde(default)]
        env: HashMap<String, String>,
    },
    /// HTTP transport (POST requests)
    Http {
        /// Endpoint URL.
        url: String,
        /// Custom HTTP headers.
        #[serde(default)]
        headers: HashMap<String, String>,
        /// Request timeout in seconds.
        #[serde(default = "default_timeout")]
        timeout_seconds: u64,
    },
    /// SSE transport (Server-Sent Events)
    Sse {
        /// Endpoint URL.
        url: String,
        /// Custom HTTP headers.
        #[serde(default)]
        headers: HashMap<String, String>,
        /// Request timeout in seconds.
        #[serde(default = "default_timeout")]
        timeout_seconds: u64,
        /// Delay before reconnecting after a dropped stream, in milliseconds.
        #[serde(default = "default_reconnect_delay")]
        reconnect_delay_ms: u64,
    },
    /// WebSocket transport (bidirectional JSON-RPC over WS)
    WebSocket {
        /// Endpoint URL.
        url: String,
        /// Custom HTTP headers for the handshake.
        #[serde(default)]
        headers: HashMap<String, String>,
        /// Request timeout in seconds.
        #[serde(default = "default_timeout")]
        timeout_seconds: u64,
    },
}

fn default_timeout() -> u64 {
    30
}
fn default_reconnect_delay() -> u64 {
    5000
}

/// OAuth 2.0 configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OAuthConfig {
    /// OAuth client identifier.
    pub client_id: String,
    /// OAuth client secret.
    pub client_secret: Option<String>,
    /// Token endpoint URL.
    pub token_url: String,
    /// Authorization endpoint URL, if applicable.
    pub auth_url: Option<String>,
    /// Requested OAuth scopes.
    pub scope: Option<String>,
    /// Current access token.
    pub access_token: Option<String>,
    /// Refresh token for obtaining new access tokens.
    pub refresh_token: Option<String>,
    /// Unix timestamp when the access token expires.
    pub expires_at: Option<u64>,
}

/// MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Human-readable server name.
    pub name: String,
    /// Transport-specific connection details.
    #[serde(flatten)]
    pub transport: McpTransport,
    /// Optional OAuth authentication configuration.
    #[serde(default)]
    pub oauth: Option<OAuthConfig>,
}

impl McpServerConfig {
    /// Create a new stdio server config
    pub fn stdio(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            transport: McpTransport::Stdio {
                command: command.into(),
                args: Vec::new(),
                env: HashMap::new(),
            },
            oauth: None,
        }
    }

    /// Create a new HTTP server config
    pub fn http(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            transport: McpTransport::Http {
                url: url.into(),
                headers: HashMap::new(),
                timeout_seconds: 30,
            },
            oauth: None,
        }
    }

    /// Create a new SSE server config
    pub fn sse(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            transport: McpTransport::Sse {
                url: url.into(),
                headers: HashMap::new(),
                timeout_seconds: 30,
                reconnect_delay_ms: 5000,
            },
            oauth: None,
        }
    }

    /// Create a new WebSocket server config
    pub fn websocket(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            transport: McpTransport::WebSocket {
                url: url.into(),
                headers: HashMap::new(),
                timeout_seconds: 30,
            },
            oauth: None,
        }
    }

    /// Add header for HTTP/SSE/WebSocket transports
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        let value = value.into();
        match &mut self.transport {
            McpTransport::Http { headers, .. }
            | McpTransport::Sse { headers, .. }
            | McpTransport::WebSocket { headers, .. } => {
                headers.insert(key, value);
            }
            _ => {}
        }
        self
    }

    /// Add an argument for stdio transport
    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        if let McpTransport::Stdio { args, .. } = &mut self.transport {
            args.push(arg.into());
        }
        self
    }

    /// Add multiple arguments for stdio transport
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        if let McpTransport::Stdio { args: a, .. } = &mut self.transport {
            a.extend(args);
        }
        self
    }

    /// Add multiple environment variables for stdio transport
    pub fn with_envs(mut self, env: std::collections::HashMap<String, String>) -> Self {
        if let McpTransport::Stdio { env: e, .. } = &mut self.transport {
            e.extend(env);
        }
        self
    }
}
