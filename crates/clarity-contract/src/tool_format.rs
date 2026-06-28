//! Tool format constants for prompt-guided tool calling.
//!
//! Shared between `clarity-llm::tool_payload` (XML generation) and
//! `clarity-core::agent::tool_parser` (XML parsing), eliminating the
//! implicit coupling where both sides independently agree on the format.

/// XML dialect used for prompt-guided tool calling.
///
/// The `tool_parser` module in clarity-core expects tool calls in this
/// format, and `tool_payload` in clarity-llm generates tool descriptions
/// in this format.
pub const TOOL_FORMAT_XML_INSTRUCTION: &str = "\n\nYou have access to the following tools. \
     When you need to use a tool, output exactly one XML block on its own line \
     and then stop. Wait for the tool result before continuing.\n\n\
     Output format (you MUST use <arg key=...> tags for every parameter):\n\
     <tool name=\"tool_name\">\n\
       <arg key=\"arg_name\">arg_value</arg>\n\
     </tool>";

/// Tag name used in the XML dialect.
pub const XML_TOOL_TAG: &str = "tool";
/// Attribute for the tool name.
pub const XML_TOOL_NAME_ATTR: &str = "name";
/// Tag name for individual arguments.
pub const XML_ARG_TAG: &str = "arg";
/// Attribute for the argument key.
pub const XML_ARG_KEY_ATTR: &str = "key";
/// Tag name for tool descriptions (injected into system prompt).
pub const XML_TOOL_DESC_TAG: &str = "tool_description";
/// Tag name for parameter descriptions.
pub const XML_PARAM_TAG: &str = "parameter";
