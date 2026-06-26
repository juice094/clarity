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

- `main.rs` — axum server, request/response routing, and middleware.
- Anthropic request types are deserialized, flattened into a prompt string, and
  forwarded through `clarity_llm::deepseek_device::DeepSeekDeviceProvider`.
- Tool calls are parsed with `clarity_core::agent::tool_parser` and emitted as
  Anthropic `tool_use` content blocks.

## License

AGPL-3.0-or-later. See the workspace `LICENSE` file.
