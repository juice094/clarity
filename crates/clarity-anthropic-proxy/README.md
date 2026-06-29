# clarity-anthropic-proxy

Anthropic Messages API → DeepSeek App private API reverse proxy.

## Purpose

This crate provides a small standalone binary (`cc-proxy`) that lets clients
speaking the Anthropic Messages API (such as Claude Code) route requests to the
DeepSeek mobile/device API instead of the official Anthropic endpoint.

It translates Anthropic-formatted `POST /v1/messages` requests into DeepSeek
device API calls, parses XML tool calls returned by DeepSeek, and returns
Anthropic-formatted responses.

## Usage

Set credentials via environment variables:

```bash
# Option 1: device token
export DEEPSEEK_DEVICE_TOKEN="your-mmkv-token"

# Option 2: mobile + password
export DEEPSEEK_DEVICE_MOBILE="13800138000"
export DEEPSEEK_DEVICE_PASSWORD="your_password"

# Optional
export CC_PROXY_PORT=18791  # default
```

Run the proxy:

```bash
cargo run -p clarity-anthropic-proxy --release
```

Point Claude Code at it:

```bash
export ANTHROPIC_BASE_URL="http://127.0.0.1:18791/v1/messages"
```

## Architecture

- `main.rs` — thin axum server: routing, logging middleware, and DeepSeek device
  credential loading.
- Anthropic protocol conversion lives in `clarity_llm::anthropic::AnthropicAdapter`.
  The proxy simply deserializes the request, calls the adapter, and returns the
  response.
- Tool call parsing is provided by `clarity_contract::tool_parser`.

This crate is intentionally thin: the Anthropic facade is now a first-class
adapter inside `clarity-llm`, and this binary is one runtime consumer of that
adapter (targeting the DeepSeek device provider by default).

## License

AGPL-3.0-or-later. See the workspace `LICENSE` file.
