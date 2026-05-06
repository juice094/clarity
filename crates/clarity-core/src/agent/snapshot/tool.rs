//! `git_restore` tool — allows the Agent to roll back to a previous snapshot.

use crate::agent::snapshot::SnapshotService;
use crate::tools::{Tool, ToolContext, ToolResult};
use serde_json::Value;
use std::sync::Arc;

/// Tool that restores the workspace to a previous snapshot.
pub struct GitRestoreTool {
    service: Arc<SnapshotService>,
}

impl GitRestoreTool {
    pub fn new(service: Arc<SnapshotService>) -> Self {
        Self { service }
    }
}

#[async_trait::async_trait]
impl Tool for GitRestoreTool {
    fn name(&self) -> &str {
        "git_restore"
    }

    fn description(&self) -> &str {
        "Restore the workspace to a previous snapshot. \
         Provide the snapshot index number (use 0 for the earliest snapshot). \
         WARNING: This will overwrite tracked files."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "index": {
                    "type": "integer",
                    "description": "Snapshot index to restore to"
                }
            },
            "required": ["index"]
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let index = args
            .get("index")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| crate::error::ToolError::invalid_params("missing index"))?;

        self.service.restore(index as usize).await.map_err(|e| {
            crate::error::ToolError::execution_failed(format!("Restore failed: {}", e))
        })?;

        // Build list of restored files for the response
        let files: Vec<String> = {
            let list = self.service.list();
            let target = list.iter().find(|s| s.id == index as usize);
            match target {
                Some(t) => vec![format!("Restored to snapshot {} ({})", t.id, t.label)],
                None => vec![format!("Restored to snapshot index {}", index)],
            }
        };

        Ok(serde_json::json!({
            "restored": true,
            "index": index,
            "files": files,
        }))
    }
}
