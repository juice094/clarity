#!/usr/bin/env python3
"""Quantitative connectivity check for Clarity Gateway Claw endpoints.

Tests:
- HTTP /health
- HTTP /api/v1/claw/devices
- Native Gateway WebSocket /ws (welcome, ping, register_device)
- OpenClaw JSON-RPC WebSocket /openclaw/ws (handshake, chat.send error shape)
"""

import asyncio
import json
import sys
import time
import urllib.request
from pathlib import Path

import websockets

GATEWAY_HTTP = "http://127.0.0.1:18790"
GATEWAY_WS = "ws://127.0.0.1:18790"
CLARITY_HOME = Path(".clarity")
ADMIN_TOKEN_PATH = CLARITY_HOME / "openclaw-admin-token"


def read_admin_token() -> str:
    if not ADMIN_TOKEN_PATH.exists():
        print(f"[FAIL] admin token not found at {ADMIN_TOKEN_PATH}")
        sys.exit(1)
    return ADMIN_TOKEN_PATH.read_text().strip()


def http_get(path: str) -> dict:
    url = f"{GATEWAY_HTTP}{path}"
    start = time.perf_counter()
    with urllib.request.urlopen(url, timeout=10) as resp:
        body = resp.read().decode()
    elapsed_ms = (time.perf_counter() - start) * 1000
    return {"status": resp.status, "body": body, "elapsed_ms": elapsed_ms}


async def native_ws_check() -> dict:
    uri = f"{GATEWAY_WS}/ws"
    start = time.perf_counter()
    async with websockets.connect(uri) as ws:
        welcome = json.loads(await asyncio.wait_for(ws.recv(), timeout=5))
        await ws.send(json.dumps({"type": "ping"}))
        pong = json.loads(await asyncio.wait_for(ws.recv(), timeout=5))
        await ws.send(
            json.dumps(
                {
                    "type": "register_device",
                    "id": "test-claw-1",
                    "name": "test-claw-1",
                    "host": "127.0.0.1",
                    "version": "0.4.0",
                }
            )
        )
        ack = json.loads(await asyncio.wait_for(ws.recv(), timeout=5))
        elapsed_ms = (time.perf_counter() - start) * 1000
        return {
            "welcome": welcome,
            "pong": pong,
            "device_ack": ack,
            "elapsed_ms": elapsed_ms,
        }


async def openclaw_ws_check(admin_token: str) -> dict:
    uri = f"{GATEWAY_WS}/openclaw/ws"
    start = time.perf_counter()
    async with websockets.connect(uri) as ws:
        challenge = json.loads(await asyncio.wait_for(ws.recv(), timeout=5))
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
                            "id": "test-claw-1",
                            "display_name": "Test Claw",
                            "version": "0.4.0",
                            "platform": "win32",
                            "device_family": "Windows",
                            "mode": "cli",
                        },
                        "caps": [],
                        "auth": {"token": admin_token},
                        "role": "operator",
                        "scopes": ["operator.read"],
                    },
                }
            )
        )
        hello = json.loads(await asyncio.wait_for(ws.recv(), timeout=5))
        await ws.send(
            json.dumps(
                {
                    "type": "req",
                    "id": "chat-1",
                    "method": "chat.send",
                    "params": {
                        "sessionKey": "agent:main:main",
                        "message": [{"type": "text", "text": "hello"}],
                    },
                }
            )
        )
        reply = json.loads(await asyncio.wait_for(ws.recv(), timeout=30))
        elapsed_ms = (time.perf_counter() - start) * 1000
        return {
            "challenge": challenge,
            "hello": hello,
            "chat_reply": reply,
            "elapsed_ms": elapsed_ms,
        }


async def main() -> int:
    print("=" * 60)
    print("Clarity Gateway Claw connectivity check")
    print("=" * 60)

    # 1. HTTP health
    print("\n[1/4] HTTP /health")
    try:
        health = http_get("/health")
        print(f"  status={health['status']} elapsed={health['elapsed_ms']:.1f}ms")
        print(f"  body={health['body'][:200]}")
    except Exception as e:
        print(f"  [FAIL] {e}")
        return 1

    # 2. HTTP device list
    print("\n[2/4] HTTP /api/v1/claw/devices")
    try:
        devices = http_get("/api/v1/claw/devices")
        print(f"  status={devices['status']} elapsed={devices['elapsed_ms']:.1f}ms")
        print(f"  body={devices['body'][:200]}")
    except Exception as e:
        print(f"  [FAIL] {e}")
        return 1

    # 3. Native WS
    print("\n[3/4] Native Gateway WebSocket /ws")
    try:
        native = await native_ws_check()
        print(f"  elapsed={native['elapsed_ms']:.1f}ms")
        print(f"  welcome.type={native['welcome'].get('type')}")
        print(f"  pong.type={native['pong'].get('type')}")
        print(f"  device_ack.type={native['device_ack'].get('type')}")
    except Exception as e:
        print(f"  [FAIL] {e}")
        return 1

    # 4. OpenClaw WS
    print("\n[4/4] OpenClaw WebSocket /openclaw/ws")
    try:
        admin_token = read_admin_token()
        oc = await openclaw_ws_check(admin_token)
        print(f"  elapsed={oc['elapsed_ms']:.1f}ms")
        print(f"  challenge.event={oc['challenge'].get('event')}")
        print(f"  hello.ok={oc['hello'].get('ok')}")
        print(f"  chat_reply.ok={oc['chat_reply'].get('ok')} id={oc['chat_reply'].get('id')}")
        if oc["chat_reply"].get("error"):
            print(f"  chat_reply.error={oc['chat_reply']['error']}")
    except Exception as e:
        print(f"  [FAIL] {e}")
        return 1

    print("\n" + "=" * 60)
    print("All checks passed")
    print("=" * 60)
    return 0


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
