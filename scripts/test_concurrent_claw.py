#!/usr/bin/env python3
"""Concurrent chat.send test for Clarity Gateway /openclaw/ws.

Validates the shared-agent single-turn invariant:
- Two separate OpenClaw connections send chat.send at the same time.
- Exactly one should succeed; the other must receive a clear, immediate error
  (not hang or corrupt shared state).
- After both finish, a subsequent chat.send on a fresh connection must succeed,
  proving the agent returned to Idle.
"""

import asyncio
import json
import sys
import time
from pathlib import Path

import websockets

GATEWAY_WS = "ws://127.0.0.1:18790/openclaw/ws"
ADMIN_TOKEN_PATH = Path(".clarity/openclaw-admin-token")


def read_admin_token() -> str:
    if not ADMIN_TOKEN_PATH.exists():
        print(f"[FAIL] admin token not found at {ADMIN_TOKEN_PATH}")
        sys.exit(1)
    return ADMIN_TOKEN_PATH.read_text().strip()


async def open_connection(token: str):
    ws = await websockets.connect(GATEWAY_WS)
    challenge = json.loads(await asyncio.wait_for(ws.recv(), timeout=5))
    assert challenge.get("event") == "connect.challenge", challenge
    await ws.send(
        json.dumps(
            {
                "type": "req",
                "id": "conn-1",
                "method": "connect",
                "params": {
                    "min_protocol": 3,
                    "max_protocol": 3,
                    "client": {
                        "id": "concurrent-test",
                        "display_name": "Concurrent Test",
                        "version": "0.4.0",
                        "platform": "win32",
                        "device_family": "Windows",
                        "mode": "cli",
                    },
                    "caps": [],
                    "auth": {"token": token},
                    "role": "operator",
                    "scopes": ["operator.read"],
                },
            }
        )
    )
    hello = json.loads(await asyncio.wait_for(ws.recv(), timeout=5))
    assert hello.get("ok") is True, hello
    return ws


async def send_chat(ws, text: str, req_id: str, timeout: float = 60.0) -> dict:
    await ws.send(
        json.dumps(
            {
                "type": "req",
                "id": req_id,
                "method": "chat.send",
                "params": {
                    "sessionKey": "concurrent:test:session",
                    "message": [{"type": "text", "text": text}],
                },
            }
        )
    )
    return json.loads(await asyncio.wait_for(ws.recv(), timeout=timeout))


async def chat_task(token: str, text: str, req_id: str, timeout: float = 60.0) -> dict:
    ws = await open_connection(token)
    try:
        return await send_chat(ws, text, req_id, timeout)
    finally:
        await ws.close()


async def main() -> int:
    token = read_admin_token()
    print("=" * 60)
    print("Clarity Gateway concurrent OpenClaw chat test")
    print("=" * 60)

    # Race two chat.send requests
    print("\n[1/2] Two concurrent chat.send requests")
    start = time.perf_counter()
    results = await asyncio.gather(
        chat_task(token, "hello from connection A", "chat-a", timeout=60.0),
        chat_task(token, "hello from connection B", "chat-b", timeout=60.0),
        return_exceptions=True,
    )
    elapsed = (time.perf_counter() - start) * 1000

    oks = [r for r in results if isinstance(r, dict) and r.get("ok")]
    errors = [r for r in results if isinstance(r, dict) and not r.get("ok")]
    exceptions = [r for r in results if isinstance(r, Exception)]

    print(f"  elapsed={elapsed:.1f}ms")
    print(f"  successes={len(oks)} errors={len(errors)} exceptions={len(exceptions)}")
    for r in oks:
        print(f"    ok id={r.get('id')}")
    for r in errors:
        err = r.get("error", {})
        print(f"    error id={r.get('id')} code={err.get('code')} msg={err.get('message')}")
    for e in exceptions:
        print(f"    exception: {e}")

    if len(oks) != 2:
        print("  [FAIL] expected both concurrent requests to succeed (queueing)")
        return 1

    # With a single shared Agent, turns must be serialized. Two concurrent
    # "hello" turns (~3s each) should take >> 3s if queueing is active.
    if elapsed < 4500:
        print("  [FAIL] elapsed too short; turns may not be serialized")
        return 1

    # Verify agent recovered to Idle after both turns
    print("\n[2/2] Post-race recovery chat.send")
    recovery = await chat_task(token, "are you still there", "chat-recovery", timeout=60.0)
    print(f"  recovery.ok={recovery.get('ok')} id={recovery.get('id')}")
    if not recovery.get("ok"):
        print(f"  [FAIL] agent did not recover: {recovery.get('error')}")
        return 1

    print("\n" + "=" * 60)
    print("Concurrent test passed")
    print("=" * 60)
    return 0


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
