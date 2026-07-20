use super::{
    HttpMcpClient, McpClientInstance, McpServerConfig, McpTransport, OAuthConfig, SseMcpClient,
    StdioMcpClient, WebSocketMcpClient,
};
// =============================================================================
// Client Builder
// =============================================================================

/// Builder for constructing MCP client instances from configuration.
pub struct McpClientBuilder;

impl McpClientBuilder {
    /// Build an `McpClientInstance` from a complete server configuration.
    pub fn from_config(config: McpServerConfig) -> McpClientInstance {
        match &config.transport {
            McpTransport::Stdio { .. } => {
                McpClientInstance::Stdio(Box::new(StdioMcpClient::new(config)))
            }
            McpTransport::Http { .. } => McpClientInstance::Http(HttpMcpClient::new(config)),
            McpTransport::Sse { .. } => McpClientInstance::Sse(SseMcpClient::new(config)),
            McpTransport::WebSocket { .. } => {
                McpClientInstance::WebSocket(WebSocketMcpClient::new(config))
            }
        }
    }

    /// Start building a stdio MCP client.
    pub fn stdio(name: impl Into<String>, command: impl Into<String>) -> StdioClientBuilder {
        StdioClientBuilder {
            config: McpServerConfig::stdio(name, command),
        }
    }

    /// Start building an HTTP MCP client.
    pub fn http(name: impl Into<String>, url: impl Into<String>) -> HttpClientBuilder {
        HttpClientBuilder {
            config: McpServerConfig::http(name, url),
        }
    }

    /// Start building an SSE MCP client.
    pub fn sse(name: impl Into<String>, url: impl Into<String>) -> SseClientBuilder {
        SseClientBuilder {
            config: McpServerConfig::sse(name, url),
        }
    }

    /// Start building a WebSocket MCP client.
    pub fn websocket(name: impl Into<String>, url: impl Into<String>) -> WebSocketClientBuilder {
        WebSocketClientBuilder {
            config: McpServerConfig::websocket(name, url),
        }
    }

    /// Build an `McpClientInstance` from an `McpServerEntry` (config-file format).
    /// Automatically selects stdio, HTTP, SSE, or WebSocket based on the `transport` field.
    pub fn from_mcp_entry(
        name: impl Into<String>,
        entry: &crate::config::McpServerEntry,
    ) -> McpClientInstance {
        let name = name.into();
        match entry.transport.as_deref() {
            Some("http") | Some("Http") => {
                let url = entry.url.clone().unwrap_or_default();
                let mut builder = Self::http(&name, url);
                for (k, v) in &entry.headers {
                    builder = builder.header(k, v);
                }
                builder.build()
            }
            Some("sse") | Some("Sse") => {
                let url = entry.url.clone().unwrap_or_default();
                let mut builder = Self::sse(&name, url);
                for (k, v) in &entry.headers {
                    builder = builder.header(k, v);
                }
                builder.build()
            }
            Some("websocket") | Some("WebSocket") | Some("ws") | Some("Ws") => {
                let url = entry.url.clone().unwrap_or_default();
                let mut builder = Self::websocket(&name, url);
                for (k, v) in &entry.headers {
                    builder = builder.header(k, v);
                }
                builder.build()
            }
            _ => {
                let mut builder = Self::stdio(&name, &entry.command);
                for arg in &entry.args {
                    builder = builder.arg(arg);
                }
                for (k, v) in &entry.env {
                    builder = builder.env(k, v);
                }
                builder.build()
            }
        }
    }
}

/// Builder for configuring a stdio MCP client.
pub struct StdioClientBuilder {
    config: McpServerConfig,
}

impl StdioClientBuilder {
    /// Append a command-line argument for the spawned subprocess.
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        if let McpTransport::Stdio { args, .. } = &mut self.config.transport {
            args.push(arg.into());
        }
        self
    }

    /// Add an environment variable for the spawned subprocess.
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        if let McpTransport::Stdio { env, .. } = &mut self.config.transport {
            env.insert(key.into(), value.into());
        }
        self
    }

    /// Build the configured `McpClientInstance`.
    pub fn build(self) -> McpClientInstance {
        McpClientInstance::Stdio(Box::new(StdioMcpClient::new(self.config)))
    }
}

/// Builder for configuring an HTTP MCP client.
pub struct HttpClientBuilder {
    config: McpServerConfig,
}

impl HttpClientBuilder {
    /// Add a custom HTTP header to every request.
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config = self.config.with_header(key, value);
        self
    }

    /// Attach OAuth configuration for bearer-token authentication.
    pub fn oauth(mut self, oauth: OAuthConfig) -> Self {
        self.config.oauth = Some(oauth);
        self
    }

    /// Build the configured `McpClientInstance`.
    pub fn build(self) -> McpClientInstance {
        McpClientInstance::Http(HttpMcpClient::new(self.config))
    }
}

/// Builder for configuring an SSE MCP client.
pub struct SseClientBuilder {
    config: McpServerConfig,
}

impl SseClientBuilder {
    /// Add a custom HTTP header to every SSE request.
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config = self.config.with_header(key, value);
        self
    }

    /// Attach OAuth configuration for bearer-token authentication.
    pub fn oauth(mut self, oauth: OAuthConfig) -> Self {
        self.config.oauth = Some(oauth);
        self
    }

    /// Build the configured `McpClientInstance`.
    pub fn build(self) -> McpClientInstance {
        McpClientInstance::Sse(SseMcpClient::new(self.config))
    }
}

/// Builder for configuring a WebSocket MCP client.
pub struct WebSocketClientBuilder {
    config: McpServerConfig,
}

impl WebSocketClientBuilder {
    /// Add a custom HTTP header to the WebSocket handshake.
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config = self.config.with_header(key, value);
        self
    }

    /// Attach OAuth configuration for bearer-token authentication.
    pub fn oauth(mut self, oauth: OAuthConfig) -> Self {
        self.config.oauth = Some(oauth);
        self
    }

    /// Build the configured `McpClientInstance`.
    pub fn build(self) -> McpClientInstance {
        McpClientInstance::WebSocket(WebSocketMcpClient::new(self.config))
    }
}
