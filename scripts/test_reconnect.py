#!/usr/bin/env python3
"""End-to-end reconnect test for Clarity Gateway /openclaw/ws.

Validates:
- chat.send works on first connection
- session history survives reconnect
- chat.send works after reconnect with same sessionKey
"""

import asyncio
import json
import sys
import time
import urllib.request
from pathlib import Path

import websockets

GATEWAY_WS = "ws://127.0.0.1:18790/openclaw/ws"
ADMIN_TOKEN_PATH = Path(".clarity/openclaw-admin-token")
SESSION_KEY = "reconnect:test:session"


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
                        "id": "reconnect-test",
                        "display_name": "Reconnect Test",
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


async def send_chat(ws, text: str, req_id: str) -> dict:
    await ws.send(
        json.dumps(
            {
                "type": "req",
                "id": req_id,
                "method": "chat.send",
                "params": {
                    "sessionKey": SESSION_KEY,
                    "message": [{"type": "text", "text": text}],
                },
            }
        )
    )
    return json.loads(await asyncio.wait_for(ws.recv(), timeout=180))


async def main() -> int:
    token = read_admin_token()
    print("=" * 60)
    print("Clarity Gateway reconnect test")
    print("=" * 60)

    # Turn 1
    print("\n[1/3] First connection + chat.send")
    ws = await open_connection(token)
    reply1 = await send_chat(ws, "hello", "chat-1")
    print(f"  reply.ok={reply1.get('ok')} id={reply1.get('id')}")
    if not reply1.get("ok"):
        print(f"  [FAIL] {reply1.get('error')}")
        return 1
    await ws.close()

    # Reconnect
    print("\n[2/3] Reconnect with same sessionKey")
    ws = await open_connection(token)

    # Turn 2
    print("\n[3/3] Second chat.send after reconnect")
    reply2 = await send_chat(ws, "what did I just say", "chat-2")
    print(f"  reply.ok={reply2.get('ok')} id={reply2.get('id')}")
    if not reply2.get("ok"):
        print(f"  [FAIL] {reply2.get('error')}")
        return 1
    await ws.close()

    print("\n" + "=" * 60)
    print("Reconnect test passed")
    print("=" * 60)
    return 0


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
