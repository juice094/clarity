//! Search tools: Glob and Grep

use async_trait::async_trait;
use glob::glob;
use regex::Regex;
use serde_json::json;
use serde_json::Value;
use std::path::Path;
use tokio::fs;
use tracing::{debug, warn};

use crate::error::ToolError;
use crate::tools::helpers;
use crate::tools::{Tool, ToolContext, ToolResult};

/// Tool for file globbing (pattern matching)
///
/// Supports glob patterns like `*.rs`, `src/**/*.js`, etc.
pub struct GlobTool;

impl GlobTool {
    /// Create a new GlobTool instance
    pub fn new() -> Self {
        Self
    }
    
    /// Execute glob pattern matching
    fn execute_glob(
        &self,
        pattern: &str,
        working_dir: &Path,
    ) -> ToolResult<Vec<String>> {
        let pattern_path = if Path::new(pattern).is_absolute() {
            pattern.to_string()
        } else {
            working_dir.join(pattern).to_string_lossy().to_string()
        };
        
        debug!("Glob pattern: {}", pattern_path);
        
        let mut matches = Vec::new();
        
        match glob(&pattern_path) {
            Ok(paths) => {
                for entry in paths {
                    match entry {
                        Ok(path) => {
                            matches.push(path.to_string_lossy().to_string());
                        }
                        Err(e) => {
                            warn!("Glob error: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                return Err(ToolError::invalid_params(format!("Invalid glob pattern: {}", e)));
            }
        }
        
        Ok(matches)
    }
}

impl Default for GlobTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }
    
    fn description(&self) -> &str {
        "Find files matching a glob pattern. Returns a list of matching file paths. \
         Supports patterns like '*.rs', 'src/**/*.js', etc."
    }
    
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match files against"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results to return",
                    "minimum": 1,
                    "maximum": 1000,
                    "default": 100
                }
            },
            "required": ["pattern"]
        })
    }
    
    async fn execute(&self, args: Value, ctx: ToolContext) -> ToolResult<Value> {
        let pattern = helpers::required_str(&args, "pattern")?;
        let limit = args.get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| (v as usize).min(1000))
            .unwrap_or(100);
        
        let mut matches = self.execute_glob(pattern, &ctx.working_dir)?;
        let total = matches.len();
        
        // Apply limit
        if matches.len() > limit {
            matches.truncate(limit);
        }
        
        Ok(json!({
            "pattern": pattern,
            "matches": matches,
            "total": total,
            "returned": matches.len()
        }))
    }
}

/// Match result from a grep search
#[derive(Debug, Clone, serde::Serialize)]
pub struct GrepMatch {
    /// File path
    pub file: String,
    /// Line number (1-indexed)
    pub line_number: usize,
    /// Content of the matching line
    pub content: String,
    /// Match groups if using regex with groups
    pub groups: Option<Vec<String>>,
}

/// Tool for searching file contents
///
/// Supports both literal string search and regex patterns
pub struct GrepTool;

impl GrepTool {
    /// Create a new GrepTool instance
    pub fn new() -> Self {
        Self
    }
    
    /// Search for pattern in files
    async fn search_files(
        &self,
        pattern: &str,
        paths: &[String],
        case_sensitive: bool,
        use_regex: bool,
        working_dir: &Path,
    ) -> ToolResult<Vec<GrepMatch>> {
        let regex = if use_regex {
            let regex_pattern = if case_sensitive {
                pattern.to_string()
            } else {
                format!("(?i){}", pattern)
            };
            Some(Regex::new(&regex_pattern).map_err(|e| {
                ToolError::invalid_params(format!("Invalid regex: {}", e))
            })?)
        } else {
            None
        };
        
        let mut matches = Vec::new();
        
        for path_str in paths {
            let path = if Path::new(path_str).is_absolute() {
                PathBuf::from(path_str)
            } else {
                working_dir.join(path_str)
            };
            
            if path.is_dir() {
                // Skip directories for now (could be enhanced to recurse)
                continue;
            }
            
            if !path.is_file() {
                continue;
            }
            
            let content = match fs::read_to_string(&path).await {
                Ok(c) => c,
                Err(_) => continue, // Skip binary or unreadable files
            };
            
            for (line_num, line) in content.lines().enumerate() {
                let is_match = if let Some(ref re) = regex {
                    re.is_match(line)
                } else {
                    if case_sensitive {
                        line.contains(pattern)
                    } else {
                        line.to_lowercase().contains(&pattern.to_lowercase())
                    }
                };
                
                if is_match {
                    let groups = if let Some(ref re) = regex {
                        re.captures(line).map(|caps| {
                            caps.iter()
                                .skip(1) // Skip full match
                                .filter_map(|m| m.map(|m| m.as_str().to_string()))
                                .collect()
                        })
                    } else {
                        None
                    };
                    
                    matches.push(GrepMatch {
                        file: path.to_string_lossy().to_string(),
                        line_number: line_num + 1, // 1-indexed
                        content: line.to_string(),
                        groups,
                    });
                }
            }
        }
        
        Ok(matches)
    }
}

impl Default for GrepTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }
    
    fn description(&self) -> &str {
        "Search for patterns in file contents. Supports both literal text \
         and regex matching. Returns matching lines with file paths and line numbers."
    }
    
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Pattern to search for"
                },
                "paths": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "File or directory paths to search in"
                },
                "regex": {
                    "type": "boolean",
                    "description": "Use regex pattern matching (default: false)",
                    "default": false
                },
                "case_sensitive": {
                    "type": "boolean",
                    "description": "Case-sensitive search (default: false)",
                    "default": false
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of matches to return",
                    "minimum": 1,
                    "maximum": 1000,
                    "default": 100
                }
            },
            "required": ["pattern", "paths"]
        })
    }
    
    async fn execute(&self, args: Value, ctx: ToolContext) -> ToolResult<Value> {
        let pattern = helpers::required_str(&args, "pattern")?;
        let paths = helpers::required_string_array(&args, "paths")?;
        let use_regex = helpers::optional_bool(&args, "regex", false);
        let case_sensitive = helpers::optional_bool(&args, "case_sensitive", false);
        let limit = args.get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| (v as usize).min(1000))
            .unwrap_or(100);
        
        debug!("Grepping for '{}' in {:?}", pattern, paths);
        
        let mut matches = self.search_files(
            pattern,
            &paths,
            case_sensitive,
            use_regex,
            &ctx.working_dir,
        ).await?;
        
        let total = matches.len();
        
        // Apply limit
        if matches.len() > limit {
            matches.truncate(limit);
        }
        
        Ok(json!({
            "pattern": pattern,
            "matches": matches,
            "total": total,
            "returned": matches.len()
        }))
    }
}

use std::path::PathBuf;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_glob_pattern() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();
        
        // Create test files
        fs::write(base.join("file1.rs"), "").await.unwrap();
        fs::write(base.join("file2.rs"), "").await.unwrap();
        fs::write(base.join("file.txt"), "").await.unwrap();
        
        let tool = GlobTool::new();
        let ctx = ToolContext::new().with_working_dir(base);
        
        let args = json!({"pattern": "*.rs"});
        let result = tool.execute(args, ctx).await.unwrap();
        
        let matches = result["matches"].as_array().unwrap();
        assert_eq!(matches.len(), 2);
    }
    
    #[tokio::test]
    async fn test_grep_literal() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "Hello World\nFoo Bar\nHello Rust").await.unwrap();
        
        let tool = GrepTool::new();
        let ctx = ToolContext::new().with_working_dir(temp_dir.path());
        
        let args = json!({
            "pattern": "Hello",
            "paths": ["test.txt"],
            "regex": false
        });
        
        let result = tool.execute(args, ctx).await.unwrap();
        let matches = result["matches"].as_array().unwrap();
        
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0]["line_number"], 1);
        assert_eq!(matches[1]["line_number"], 3);
    }
    
    #[tokio::test]
    async fn test_grep_regex() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "func1()\nfunc2()\nvar x").await.unwrap();
        
        let tool = GrepTool::new();
        let ctx = ToolContext::new().with_working_dir(temp_dir.path());
        
        let args = json!({
            "pattern": r"func\d+\(\)",
            "paths": ["test.txt"],
            "regex": true
        });
        
        let result = tool.execute(args, ctx).await.unwrap();
        let matches = result["matches"].as_array().unwrap();
        
        assert_eq!(matches.len(), 2);
    }
}
