#!/usr/bin/env python3
"""Standalone test for OpenClaw device.pair.request against Gray-Cloud Gateway.

This script bypasses the Android UI and talks directly to the Gateway through
the local TCP proxy (or directly if reachable) to verify the pairing protocol
and discover the exact error returned for device.pair.request.
"""

import json
import socket
import threading
import time
from base64 import b64encode
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

GATEWAY_URL = "ws://127.0.0.1:18790"
GATEWAY_TOKEN = "13ef094cc169523a711d4e508362bcc5192b7310"
TARGET_HOST = "100.69.11.71"
TARGET_PORT = 18789


def create_websocket_frame(payload: bytes, opcode: int = 0x1) -> bytes:
    """Build a minimal WebSocket text frame (no masking, client->server must mask)."""
    frame = bytearray()
    frame.append(0x80 | opcode)
    length = len(payload)
    if length < 126:
        frame.append(0x80 | length)
    elif length < 65536:
        frame.append(0x80 | 126)
        frame.extend(length.to_bytes(2, "big"))
    else:
        frame.append(0x80 | 127)
        frame.extend(length.to_bytes(8, "big"))
    mask = b"\x00\x00\x00\x00"
    frame.extend(mask)
    frame.extend(payload)
    return bytes(frame)


def parse_frames(buf: bytes):
    """Parse complete WebSocket frames from buf; return (consumed, [(opcode, payload), ...])."""
    frames = []
    consumed = 0
    while True:
        if len(buf) - consumed < 2:
            break
        p = consumed
        opcode = buf[p] & 0x0F
        masked = (buf[p + 1] & 0x80) != 0
        length = buf[p + 1] & 0x7F
        offset = p + 2
        if length == 126:
            if len(buf) - consumed < 4:
                break
            length = int.from_bytes(buf[p + 2 : p + 4], "big")
            offset = p + 4
        elif length == 127:
            if len(buf) - consumed < 10:
                break
            length = int.from_bytes(buf[p + 2 : p + 10], "big")
            offset = p + 10
        mask = b""
        if masked:
            if len(buf) - offset < 4:
                break
            mask = buf[offset : offset + 4]
            offset += 4
        if len(buf) - offset < length:
            break
        payload = buf[offset : offset + length]
        if masked:
            payload = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
        consumed = offset + length
        if opcode == 0x1:
            try:
                frames.append((opcode, payload.decode("utf-8")))
            except UnicodeDecodeError:
                frames.append((opcode, payload))
        else:
            frames.append((opcode, payload))
    return consumed, frames


def send(sock: socket.socket, obj: dict) -> None:
    text = json.dumps(obj, separators=(",", ":"))
    print(f"[C->S] {text[:500]}")
    sock.sendall(create_websocket_frame(text.encode("utf-8")))


def recv_any(sock: socket.socket, timeout: float = 10.0):
    sock.settimeout(timeout)
    buf = b""
    while True:
        consumed, frames = parse_frames(buf)
        buf = buf[consumed:]
        for opcode, payload in frames:
            if opcode == 0x1:
                print(f"[S->C] {str(payload)[:500]}")
                return json.loads(payload)
            elif opcode == 0x8:
                raise ConnectionError(f"server closed: {payload!r}")
            elif opcode == 0x9:
                # ping
                sock.sendall(create_websocket_frame(payload, opcode=0xA))
        data = sock.recv(8192)
        if not data:
            raise ConnectionError("socket closed")
        buf += data


def recv_json(sock: socket.socket, timeout: float = 10.0) -> dict:
    return recv_any(sock, timeout)


def main():
    private_key = Ed25519PrivateKey.generate()
    public_key = private_key.public_key()
    public_key_b64 = b64encode(public_key.public_bytes_raw()).decode("ascii")
    device_id = public_key_b64[:32]

    print(f"Connecting to {TARGET_HOST}:{TARGET_PORT} via local proxy 127.0.0.1:18790 ...")
    sock = socket.create_connection(("127.0.0.1", 18790), timeout=10)

    # Perform WebSocket handshake.
    handshake = (
        f"GET /ws HTTP/1.1\r\n"
        f"Host: 127.0.0.1:18790\r\n"
        f"Upgrade: websocket\r\n"
        f"Connection: Upgrade\r\n"
        f"Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n"
        f"Sec-WebSocket-Version: 13\r\n"
        f"\r\n"
    )
    sock.sendall(handshake.encode("ascii"))
    response = sock.recv(4096).decode("ascii", errors="replace")
    print(f"[handshake] {response.splitlines()[0]}")
    if "101" not in response.splitlines()[0]:
        raise ConnectionError(f"handshake failed: {response[:500]}")

    # Send connect.
    send(sock, {
        "type": "req",
        "id": "1",
        "method": "connect",
        "params": {
            "minProtocol": 3,
            "maxProtocol": 3,
            "client": {
                "id": "gateway-client",
                "version": "0.4.0",
                "platform": "windows",
                "mode": "cli",
            },
            "role": "operator",
            "scopes": ["operator.admin", "operator.read", "operator.write", "operator.approvals", "operator.pairing", "operator.talk.secrets"],
            "auth": {"token": GATEWAY_TOKEN},
            "caps": ["tool-events"],
        },
    })
    # Wait for the actual connect response (skip connect.challenge event).
    connect_resp = None
    for _ in range(5):
        msg = recv_any(sock, timeout=10.0)
        if msg.get("type") == "res" and msg.get("id") == "1":
            connect_resp = msg
            break
    if connect_resp is None:
        raise ConnectionError("did not receive connect response")
    payload = connect_resp.get('payload', {})
    print(f"[connect] ok={connect_resp.get('ok')} payload keys={list(payload.keys()) if isinstance(payload, dict) else 'n/a'}")
    if isinstance(payload, dict):
        if 'policy' in payload:
            print(f"[connect] policy scopes={payload['policy'].get('scopes')}")
        if 'snapshot' in payload and isinstance(payload['snapshot'], dict):
            print(f"[connect] snapshot scopes={payload['snapshot'].get('scopes')}")

    # Send device.pair.request.
    send(sock, {
        "type": "req",
        "id": "2",
        "method": "device.pair.request",
        "params": {
            "deviceId": device_id,
            "publicKey": public_key_b64,
            "clientId": "pairing-test",
            "clientMode": "test",
            "platform": "python",
            "role": "operator",
            "scopes": ["operator.admin", "operator.read", "operator.write", "operator.approvals", "operator.pairing", "operator.talk.secrets"],
        },
    })
    # Wait for the direct response to device.pair.request (id="2") and any
    # follow-up events for up to 15 seconds.
    sock.settimeout(15.0)
    deadline = time.time() + 15.0
    while time.time() < deadline:
        try:
            msg = recv_any(sock, timeout=max(0.1, deadline - time.time()))
        except socket.timeout:
            break
        mid = msg.get("id")
        event = msg.get("event")
        if mid == "2":
            print(f"[pair] direct response ok={msg.get('ok')} result={msg.get('result')} error={msg.get('error')}")
        elif event in ("device.pair.requested", "device.pair.resolved", "device.paired"):
            print(f"[pair] event {event}: {msg}")
        else:
            print(f"[pair] ignoring {msg.get('type')}/{event or mid}")

    sock.close()


if __name__ == "__main__":
    main()
