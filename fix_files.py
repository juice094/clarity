import re

# Fix agent/mod.rs
agent_path = 'crates/clarity-core/src/agent/mod.rs'
with open(agent_path, 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace(
    ') -> Result<tokio::sync::mpsc::Receiver<Result<String, AgentError>>, AgentError>;',
    ') -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError>;'
)

with open(agent_path, 'w', encoding='utf-8') as f:
    f.write(content)
print('Fixed agent/mod.rs')

# Fix llm/mod.rs borrow issues
llm_path = 'crates/clarity-core/src/llm/mod.rs'
with open(llm_path, 'r', encoding='utf-8') as f:
    content = f.read()

old_block = '''            let assemble = |ptc: &PartialToolCall| -> crate::agent::ToolCall {
                crate::agent::ToolCall {
                    id: ptc.id.clone(),
                    call_type: if ptc.call_type.is_empty() {
                        "function".to_string()
                    } else {
                        ptc.call_type.clone()
                    },
                    function: crate::agent::FunctionCall {
                        name: ptc.name.clone(),
                        arguments: ptc.arguments.clone(),
                    },
                }
            };

            let mut partial_calls: Vec<PartialToolCall> = Vec::new();
            let mut last_seen_index: Option<usize> = None;

            let flush_last = || -> Option<crate::agent::ToolCall> {
                let idx = last_seen_index?;
                let ptc = partial_calls.get(idx)?;
                let call = assemble(ptc);
                if call.id.is_empty() || call.function.name.is_empty() {
                    None
                } else {
                    Some(call)
                }
            };'''

new_block = '''            let assemble = |ptc: &PartialToolCall| -> crate::agent::ToolCall {
                crate::agent::ToolCall {
                    id: ptc.id.clone(),
                    call_type: if ptc.call_type.is_empty() {
                        "function".to_string()
                    } else {
                        ptc.call_type.clone()
                    },
                    function: crate::agent::FunctionCall {
                        name: ptc.name.clone(),
                        arguments: ptc.arguments.clone(),
                    },
                }
            };

            let flush_last = |pc: &[PartialToolCall], lsi: Option<usize>| -> Option<crate::agent::ToolCall> {
                let idx = lsi?;
                let ptc = pc.get(idx)?;
                let call = assemble(ptc);
                if call.id.is_empty() || call.function.name.is_empty() {
                    None
                } else {
                    Some(call)
                }
            };

            let mut partial_calls: Vec<PartialToolCall> = Vec::new();
            let mut last_seen_index: Option<usize> = None;'''

content = content.replace(old_block, new_block)

# Replace all flush_last() calls with flush_last(&partial_calls, last_seen_index)
content = content.replace('if let Some(call) = flush_last() {',
                          'if let Some(call) = flush_last(&partial_calls, last_seen_index) {')

with open(llm_path, 'w', encoding='utf-8') as f:
    f.write(content)
print('Fixed llm/mod.rs')
