use super::McpError;

// =============================================================================
// Command validation for MCP stdio transport
// =============================================================================

/// Validate an MCP stdio command to prevent command-injection attacks.
///
/// Rules:
/// 1. If `CLARITY_MCP_ALLOWLIST` is set, the command must match or start with
///    one of the comma-separated entries.
/// 2. Reject shell metacharacters and `..` sequences.
/// 3. Absolute paths are allowed only if they exist and are files.
/// 4. Bare names (no path separators) are allowed — the OS resolves them via PATH.
/// 5. Relative paths are rejected.
pub fn validate_mcp_command(command: &str) -> Result<(), McpError> {
    let allowlist = std::env::var("CLARITY_MCP_ALLOWLIST").ok();
    validate_mcp_command_with_allowlist(command, allowlist.as_deref())
}

/// Validate an MCP stdio command against an explicit allowlist.
pub fn validate_mcp_command_with_allowlist(
    command: &str,
    allowlist: Option<&str>,
) -> Result<(), McpError> {
    // 1. Explicit allowlist takes precedence.
    if let Some(allowlist) = allowlist {
        let allowed: Vec<&str> = allowlist
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if !allowed.is_empty() {
            let matched = allowed
                .iter()
                .any(|prefix| command == *prefix || command.starts_with(&format!("{}/", prefix)));
            if !matched {
                return Err(McpError::CommandNotAllowed(format!(
                    "Command '{}' not in CLARITY_MCP_ALLOWLIST",
                    command
                )));
            }
            return Ok(());
        }
    }

    // 2. Default hardening.
    const BAD_CHARS: &[char] = &[
        ';', '|', '&', '$', '`', '<', '>', '(', ')', '{', '}', '*', '?', '~', '\'', '"',
    ];
    if command.contains("..") || command.contains(BAD_CHARS) {
        return Err(McpError::CommandNotAllowed(format!(
            "Command '{}' contains unsafe characters",
            command
        )));
    }

    let path = std::path::Path::new(command);

    // Absolute path: must exist and be a file.
    if path.is_absolute() {
        if path.exists() && path.is_file() {
            return Ok(());
        }
        return Err(McpError::CommandNotAllowed(format!(
            "Absolute command '{}' does not exist or is not a file",
            command
        )));
    }

    // Bare name: allowed, OS resolves via PATH.
    if !command.contains('/') && !command.contains('\\') {
        return Ok(());
    }

    // Anything else is treated as a relative path and rejected.
    Err(McpError::CommandNotAllowed(format!(
        "Relative command paths are not allowed: '{}'",
        command
    )))
}
