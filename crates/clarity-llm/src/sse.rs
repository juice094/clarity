//! SSE (Server-Sent Events) parser for OpenAI-compatible streaming responses.

use crate::api::StreamDelta;
use clarity_contract::{PartialToolCallInfo, ToolCall};

/// Accumulates partial tool-call fragments from streaming deltas.
#[derive(Default)]
struct PartialToolCall {
    id: String,
    call_type: String,
    name: String,
    arguments: String,
}

/// Stateful parser for a single SSE stream.
pub struct SseParser {
    partial_calls: Vec<PartialToolCall>,
    last_seen_index: Option<usize>,
}

impl Default for SseParser {
    fn default() -> Self {
        Self::new()
    }
}

impl SseParser {
    /// Create a fresh SSE parser.
    pub fn new() -> Self {
        Self {
            partial_calls: Vec::new(),
            last_seen_index: None,
        }
    }

    /// Build the current partial tool calls snapshot for incremental emission.
    fn partial_snapshot(&self) -> Vec<PartialToolCallInfo> {
        self.partial_calls
            .iter()
            .enumerate()
            .filter(|(_, ptc)| !ptc.name.is_empty())
            .map(|(i, ptc)| PartialToolCallInfo {
                index: i,
                name: ptc.name.clone(),
                arguments_so_far: ptc.arguments.clone(),
            })
            .collect()
    }

    /// Parse one SSE `data:` line and return any deltas it produces.
    pub fn process_line(&mut self, data: &str) -> Vec<StreamDelta> {
        let mut out = Vec::new();

        if data == "[DONE]" {
            if let Some(call) = self.flush_last() {
                out.push(StreamDelta {
                    content: None,
                    reasoning_content: None,
                    tool_calls: vec![call],
                    ..Default::default()
                });
            }
            return out;
        }

        let Ok(event) = serde_json::from_str::<serde_json::Value>(data) else {
            return out;
        };

        let Some(choices) = event.get("choices").and_then(|c| c.as_array()) else {
            return out;
        };

        for choice in choices {
            let Some(delta) = choice.get("delta") else {
                continue;
            };

            // Content delta
            if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                if !content.is_empty() {
                    let mut d = StreamDelta {
                        content: Some(content.to_string()),
                        reasoning_content: None,
                        tool_calls: vec![],
                        ..Default::default()
                    };
                    d.partial_tool_calls = self.partial_snapshot();
                    out.push(d);
                }
            }

            // Reasoning content delta (Kimi Code API)
            if let Some(reasoning) = delta.get("reasoning_content").and_then(|c| c.as_str()) {
                if !reasoning.is_empty() {
                    let mut d = StreamDelta {
                        content: Some(reasoning.to_string()),
                        reasoning_content: None,
                        tool_calls: vec![],
                        ..Default::default()
                    };
                    d.partial_tool_calls = self.partial_snapshot();
                    out.push(d);
                }
            }

            // Tool call deltas
            if let Some(tool_calls) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                for tc_delta in tool_calls {
                    let Some(index) = tc_delta
                        .get("index")
                        .and_then(|i| i.as_u64())
                        .map(|i| i as usize)
                    else {
                        continue;
                    };

                    // Flush previous index when a new one appears
                    if let Some(last) = self.last_seen_index {
                        if index > last {
                            if let Some(call) = self.flush_last() {
                                let mut d = StreamDelta {
                                    content: None,
                                    reasoning_content: None,
                                    tool_calls: vec![call],
                                    ..Default::default()
                                };
                                d.partial_tool_calls = self.partial_snapshot();
                                out.push(d);
                            }
                        }
                    }

                    self.last_seen_index = Some(index);

                    if index >= self.partial_calls.len() {
                        self.partial_calls
                            .resize_with(index + 1, PartialToolCall::default);
                    }

                    if let Some(id) = tc_delta.get("id").and_then(|i| i.as_str()) {
                        self.partial_calls[index].id.push_str(id);
                    }
                    if let Some(call_type) = tc_delta.get("type").and_then(|t| t.as_str()) {
                        self.partial_calls[index].call_type.push_str(call_type);
                    }
                    if let Some(func) = tc_delta.get("function") {
                        if let Some(name) = func.get("name").and_then(|n| n.as_str()) {
                            self.partial_calls[index].name.push_str(name);
                        }
                        if let Some(args) = func.get("arguments").and_then(|a| a.as_str()) {
                            self.partial_calls[index].arguments.push_str(args);
                        }
                    }
                }
            }
        }

        // Attach partial state to the last output delta.
        if let Some(last) = out.last_mut() {
            last.partial_tool_calls = self.partial_snapshot();
        }

        out
    }

    /// Flush the last partially-built tool call, if it is complete enough.
    pub fn flush(&mut self) -> Option<StreamDelta> {
        self.flush_last().map(|call| StreamDelta {
            content: None,
            reasoning_content: None,
            tool_calls: vec![call],
            ..Default::default()
        })
    }

    fn flush_last(&mut self) -> Option<ToolCall> {
        let idx = self.last_seen_index?;
        let ptc = self.partial_calls.get(idx)?;
        let call = assemble(ptc);
        if call.id.is_empty() || call.function.name.is_empty() {
            None
        } else {
            Some(call)
        }
    }
}

fn assemble(ptc: &PartialToolCall) -> ToolCall {
    ToolCall {
        id: ptc.id.clone(),
        call_type: if ptc.call_type.is_empty() {
            "function".to_string()
        } else {
            ptc.call_type.clone()
        },
        function: clarity_contract::FunctionCall {
            name: ptc.name.clone(),
            arguments: ptc.arguments.clone(),
        },
    }
}
