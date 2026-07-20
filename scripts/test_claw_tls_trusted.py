#!/usr/bin/env python3
"""Trusted-CA verification for Clarity TLS Claw endpoints.

Validates that a client loading .clarity/local-ca.crt can connect to the
HTTPS/WSS reverse proxy without disabling certificate verification.

Tests:
- HTTPS /health with CA-verified TLS
- WSS /openclaw/ws handshake + chat.send with CA-verified TLS

Usage:
    python scripts/test_claw_tls_trusted.py
"""

import asyncio
import json
import ssl
import sys
import time
import urllib.request
from pathlib import Path

import websockets

CLARITY_HOME = Path(".clarity")
CA_CERT = CLARITY_HOME / "local-ca.crt"
ADMIN_TOKEN_PATH = CLARITY_HOME / "openclaw-admin-token"
TLS_HTTP = "https://localhost:8443"
TLS_WS = "wss://localhost:8443"


def read_admin_token() -> str:
    if not ADMIN_TOKEN_PATH.exists():
        print(f"[FAIL] admin token not found at {ADMIN_TOKEN_PATH}")
        sys.exit(1)
    return ADMIN_TOKEN_PATH.read_text().strip()


def ensure_ca() -> ssl.SSLContext:
    if not CA_CERT.exists():
        print(f"[FAIL] CA certificate not found at {CA_CERT}")
        print("Generate it first: python scripts/generate_local_ca.py")
        sys.exit(1)
    ctx = ssl.create_default_context(cafile=str(CA_CERT))
    return ctx


def https_get(path: str, ctx: ssl.SSLContext) -> dict:
    url = f"{TLS_HTTP}{path}"
    start = time.perf_counter()
    req = urllib.request.Request(url)
    with urllib.request.urlopen(req, context=ctx, timeout=10) as resp:
        body = resp.read().decode()
    elapsed_ms = (time.perf_counter() - start) * 1000
    return {"status": resp.status, "body": body, "elapsed_ms": elapsed_ms}


async def wss_openclaw_check(admin_token: str, ctx: ssl.SSLContext) -> dict:
    uri = f"{TLS_WS}/openclaw/ws"
    start = time.perf_counter()
    async with websockets.connect(uri, ssl=ctx) as ws:
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
                            "id": "test-claw-tls-1",
                            "display_name": "Test Claw TLS",
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
                        "message": [{"type": "text", "text": "hello over trusted TLS"}],
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
    print("Clarity TLS Claw trusted-CA verification")
    print("=" * 60)

    ctx = ensure_ca()

    # 1. HTTPS health with verification enabled
    print("\n[1/2] HTTPS /health (CA verification enabled)")
    try:
        health = https_get("/health", ctx)
        print(f"  status={health['status']} elapsed={health['elapsed_ms']:.1f}ms")
        print(f"  body={health['body'][:200]}")
    except Exception as e:
        print(f"  [FAIL] {e}")
        return 1

    # 2. WSS OpenClaw with verification enabled
    print("\n[2/2] WSS /openclaw/ws (CA verification enabled)")
    try:
        admin_token = read_admin_token()
        oc = await wss_openclaw_check(admin_token, ctx)
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
    print("Trusted TLS verification passed")
    print("=" * 60)
    return 0


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
