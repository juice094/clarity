//! Computer Use Tool — GUI automation via Python bridge.

use crate::{helpers, Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use clarity_contract::ToolError;
use serde_json::{json, Value};

/// Tool for GUI automation: screenshot, click, type, scroll.
#[derive(Clone)]
pub struct ComputerUseTool {
    python_cmd: String,
}

impl ComputerUseTool {
    /// Create a new ComputerUseTool instance
    pub fn new() -> Self {
        let python_cmd = Self::detect_python().unwrap_or_else(|| "python3".to_string());
        Self { python_cmd }
    }

    fn detect_python() -> Option<String> {
        for cmd in &["python3", "python", "py"] {
            if std::process::Command::new(cmd)
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                return Some(cmd.to_string());
            }
        }
        None
    }

    fn call_python_bridge(&self, action: &str, args: Value) -> ToolResult<String> {
        let payload = json!({"action": action, "args": args});
        let script_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("scripts")
            .join("computer_bridge.py");
        let output = std::process::Command::new(&self.python_cmd)
            .arg(&script_path)
            .arg(payload.to_string())
            .output()
            .map_err(|e| {
                ToolError::execution_failed(format!("Failed to spawn python bridge: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::execution_failed(format!(
                "Python bridge error: {}",
                stderr
            )));
        }

        String::from_utf8(output.stdout)
            .map_err(|e| ToolError::execution_failed(format!("Invalid UTF-8 from python: {}", e)))
    }
}

impl Default for ComputerUseTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ComputerUseTool {
    fn name(&self) -> &str {
        "computer_use"
    }

    fn check_readiness(&self) -> Option<String> {
        if self.python_cmd == "python3" && Self::detect_python().is_none() {
            Some("No Python interpreter found (tried python3, python, py).".to_string())
        } else {
            None
        }
    }

    fn description(&self) -> &str {
        "Control the computer desktop: take a screenshot, click at coordinates, type text, or scroll. \
         Use screenshot to see the current state, then click/type to interact."
    }

    fn requires_approval(&self) -> bool {
        true
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["screenshot", "click", "type", "scroll"],
                    "description": "The action to perform"
                },
                "x": { "type": "integer", "description": "X coordinate for click/scroll" },
                "y": { "type": "integer", "description": "Y coordinate for click/scroll" },
                "text": { "type": "string", "description": "Text to type (for action=type)" },
                "amount": { "type": "integer", "description": "Scroll amount (for action=scroll)" }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let action = helpers::required_str(&args, "action")?;

        let result = match action {
            "screenshot" => self.call_python_bridge("screenshot", json!({})),
            "click" => {
                let x = args
                    .get("x")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| ToolError::invalid_params("Missing 'x'"))?;
                let y = args
                    .get("y")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| ToolError::invalid_params("Missing 'y'"))?;
                self.call_python_bridge("click", json!({"x": x, "y": y}))
            }
            "type" => {
                let text = args
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::invalid_params("Missing 'text'"))?;
                self.call_python_bridge("type", json!({"text": text}))
            }
            "scroll" => {
                let x = args.get("x").and_then(|v| v.as_i64()).unwrap_or(0);
                let y = args.get("y").and_then(|v| v.as_i64()).unwrap_or(0);
                let amount = args
                    .get("amount")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| ToolError::invalid_params("Missing 'amount'"))?;
                self.call_python_bridge("scroll", json!({"x": x, "y": y, "amount": amount}))
            }
            _ => Err(ToolError::invalid_params(format!(
                "Unknown action: {}",
                action
            ))),
        }?;

        Ok(Value::String(result))
    }
}
