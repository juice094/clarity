#!/usr/bin/env python3
"""End-to-end loop test for the native Gateway WebSocket /ws chat endpoint.

Connects to ws://127.0.0.1:18790/ws, sends a chat message, and asserts that
streaming chat chunks plus a final `{"type":"done"}` frame are received.

Requires `websockets` (pip install websockets) and a running clarity-gateway.
"""

import asyncio
import json
import sys

import websockets

GATEWAY_WS = "ws://127.0.0.1:18790/ws"
TIMEOUT_SECONDS = 60


async def main() -> int:
    print(f"Connecting to {GATEWAY_WS}...")
    async with websockets.connect(GATEWAY_WS) as ws:
        welcome = json.loads(await asyncio.wait_for(ws.recv(), timeout=5))
        if welcome.get("type") != "welcome":
            print(f"[FAIL] unexpected welcome: {welcome}")
            return 1
        print(f"[OK] welcome: {welcome}")

        message = "Hello from Gateway WS loop test"
        await ws.send(
            json.dumps(
                {
                    "type": "chat",
                    "message": message,
                    "context": None,
                    "use_wire": True,
                }
            )
        )
        print(f"[OK] sent: {message}")

        seen_chunks = 0
        seen_done = False
        try:
            while True:
                raw = await asyncio.wait_for(ws.recv(), timeout=TIMEOUT_SECONDS)
                ev = json.loads(raw)
                print(f"event: {ev}")
                if ev.get("type") == "chat":
                    seen_chunks += 1
                elif ev.get("type") == "done":
                    seen_done = True
                    break
                elif ev.get("type") == "error":
                    print(f"[FAIL] received error event: {ev}")
                    return 1
        except asyncio.TimeoutError:
            print(f"[FAIL] no done/error event within {TIMEOUT_SECONDS}s")
            return 1

    if not seen_done:
        print("[FAIL] never received done frame")
        return 1

    print(f"[PASS] received {seen_chunks} chat chunk(s) and done frame")
    return 0


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
