# OpenClaw Gateway Protocol Notes

This document captures the WebSocket JSON-RPC protocol used by KimiClaw /
OpenClaw Gateways, as observed while wiring `clarity-claw` to remote
OpenClaw instances. It is **not** an official protocol specification — it is a
running record of the messages and invariants that Clarity relies on.

## Connection Overview

```text
Client                              Gateway
  | --- WebSocket upgrade -----------> |
  | <---- connect.challenge (event) -- |
  | --- connect (req, with device) ---> |
  | <---- hello-ok (res) -------------- |
  | --- sessions.send (req) -----------> |
  | <---- session.message/chat/agent -- |
```

All messages are JSON text frames.

## Authentication Modes

`clarity-claw` supports three `ClawAuth` variants:

| Mode | Use Case | Sends `device` block? |
|------|----------|----------------------|
| `TokenOnly` | Local/unpaired Gateways that do not enforce device identity. | No |
| `DevicePaired` | Local KimiClaw where the device has already been paired and received a device token. | Yes, signed challenge |
| `TokenWithDevice` | Remote Gateways that require device attestation even when the client presents an admin or device token. | Yes, signed challenge |

## The `connect.challenge` Handshake

After the WebSocket upgrade the Gateway emits a routine challenge **before**
authentication is finalized:

```json
{
  "type": "event",
  "event": "connect.challenge",
  "payload": {
    "nonce": "<uuid>",
    "ts": 1781849226427
  }
}
```

The client must sign the following v2 payload with its Ed25519 device key:

```text
v2|{deviceId}|{clientId}|{clientMode}|{role}|{scopes}|{signedAtMs}|{token}|{nonce}
```

where `{scopes}` is the comma-joined list of requested scopes in the same order
as the `connect` request.

Then send a single `connect` request:

```json
{
  "type": "req",
  "id": "1",
  "method": "connect",
  "params": {
    "minProtocol": 3,
    "maxProtocol": 3,
    "client": {
      "id": "gateway-client",
      "version": "0.3.0",
      "platform": "windows",
      "mode": "cli"
    },
    "role": "operator",
    "scopes": [
      "operator.admin",
      "operator.read",
      "operator.write",
      "operator.approvals",
      "operator.pairing",
      "operator.talk.secrets"
    ],
    "auth": { "token": "<admin-or-device-token>" },
    "device": {
      "id": "<sha256-of-ed25519-public-key>",
      "publicKey": "<base64url-ed25519-public-key>",
      "signature": "<base64url-ed25519-signature>",
      "signedAt": 1781849220019,
      "nonce": "<challenge-nonce>"
    }
  }
}
```

### Critical Invariant

**Do not send a `connect` request without the `device` block first.** If the
Gateway receives a token-only `connect` for a non-loopback client it will
finalize the session with **no scopes** and return `hello-ok` immediately. A
subsequent authenticated `connect` will then be treated as an ordinary RPC and
fail with `missing scope: operator.admin`.

The correct sequence is:

1. Open WebSocket.
2. Wait for the `connect.challenge` event.
3. Send **one** `connect` request that already contains the signed `device`
   block.

## Origin Header

Local KimiClaw expects:

```text
Origin: app://kimi-desktop
```

Remote Gateways observed so far reject `app://kimi-desktop` from non-local
addresses with `CONTROL_UI_ORIGIN_NOT_ALLOWED`, but accept a missing `Origin`
header. `clarity-claw` therefore sends:

- `Origin: app://kimi-desktop` for `127.0.0.1` / `localhost` hosts.
- no `Origin` header for all other hosts.

## `sessions.send`

To send a message into a session:

```json
{
  "type": "req",
  "id": "2",
  "method": "sessions.send",
  "params": {
    "key": "agent:main:main",
    "message": "Hello from Clarity"
  }
}
```

Note that the parameter name is **`key`**, not `sessionKey`.

## Subscriptions

To receive messages and session-level events for `agent:main:main`:

```json
{
  "type": "req",
  "id": "3",
  "method": "sessions.subscribe",
  "params": { "key": "agent:main:main" }
}
```

```json
{
  "type": "req",
  "id": "4",
  "method": "sessions.messages.subscribe",
  "params": { "key": "agent:main:main" }
}
```

After subscribing the Gateway pushes events such as `session.message`, `chat`,
and `agent` streaming events.

## Streaming Assistant Events

OpenClaw/KimiClaw assistant streams send the **full cumulative assistant text**
in every chunk. `clarity-claw` computes the incremental delta so the UI
does not duplicate content. See `compute_stream_delta` in `src/client.rs`.

## Error Codes Observed

| Code / Message | Meaning |
|----------------|---------|
| `CONTROL_UI_ORIGIN_NOT_ALLOWED` | Wrong or missing `Origin` header for remote host. |
| `missing scope: operator.write` | Token-only remote connect succeeded but scopes were cleared. |
| `NOT_PAIRED` / `pairing required` | Device is not known to the Gateway or has no active token. |
| `role-upgrade` | Device is known but not authorized for the requested `operator` role. |

## References

- `src/client.rs` — connection loop, handshake, and delta computation.
- `src/device.rs` — Ed25519 device identity and token persistence.
- `examples/remote_chat.rs` — interactive bidirectional chat.
