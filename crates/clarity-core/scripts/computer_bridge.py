#!/usr/bin/env python3
"""Computer Use Bridge — GUI automation via pyautogui and mss.

Called by Clarity's ComputerUseTool via std::process::Command.
Receives a JSON argument via sys.argv[1] and returns JSON via stdout.

Supported actions:
  - screenshot: Capture full screen, return base64 PNG
  - click: Click at (x, y) coordinates
  - type: Type text string
  - scroll: Scroll at (x, y) by amount

Abort: if ~/.clarity/.computer_use_abort exists, exit immediately.
"""

import sys
import json
import base64
import os
from pathlib import Path


def check_abort() -> bool:
    """Check if abort signal file exists."""
    abort_file = Path.home() / ".clarity" / ".computer_use_abort"
    return abort_file.exists()


def remove_abort_file():
    """Remove abort signal file if it exists."""
    abort_file = Path.home() / ".clarity" / ".computer_use_abort"
    if abort_file.exists():
        abort_file.unlink()


def action_screenshot() -> dict:
    """Capture full screen and return base64-encoded PNG."""
    try:
        import mss
        with mss.mss() as sct:
            monitor = sct.monitors[0]  # full screen
            screenshot = sct.grab(monitor)
            import mss.tools
            png_bytes = mss.tools.to_png(screenshot.rgb, screenshot.size)
            b64 = base64.b64encode(png_bytes).decode("utf-8")
            return {"success": True, "data": b64, "format": "png_base64"}
    except Exception as e:
        return {"success": False, "error": str(e)}


def action_click(x: int, y: int) -> dict:
    """Click at screen coordinates."""
    try:
        import pyautogui
        pyautogui.click(x, y)
        return {"success": True, "action": "click", "x": x, "y": y}
    except Exception as e:
        return {"success": False, "error": str(e)}


def action_type(text: str) -> dict:
    """Type text at current cursor position."""
    try:
        import pyautogui
        pyautogui.typewrite(text, interval=0.01)
        return {"success": True, "action": "type", "text": text}
    except Exception as e:
        return {"success": False, "error": str(e)}


def action_scroll(x: int, y: int, amount: int) -> dict:
    """Scroll at coordinates."""
    try:
        import pyautogui
        pyautogui.scroll(amount, x, y)
        return {"success": True, "action": "scroll", "x": x, "y": y, "amount": amount}
    except Exception as e:
        return {"success": False, "error": str(e)}


def main():
    if len(sys.argv) < 2:
        print(json.dumps({"success": False, "error": "Missing JSON argument"}), flush=True)
        sys.exit(1)

    if check_abort():
        print(json.dumps({"success": False, "error": "Aborted by signal file"}), flush=True)
        sys.exit(1)

    try:
        payload = json.loads(sys.argv[1])
    except json.JSONDecodeError as e:
        print(json.dumps({"success": False, "error": f"Invalid JSON: {e}"}), flush=True)
        sys.exit(1)

    action = payload.get("action")
    args = payload.get("args", {})

    result = {}
    if action == "screenshot":
        result = action_screenshot()
    elif action == "click":
        x = args.get("x", 0)
        y = args.get("y", 0)
        result = action_click(x, y)
    elif action == "type":
        text = args.get("text", "")
        result = action_type(text)
    elif action == "scroll":
        x = args.get("x", 0)
        y = args.get("y", 0)
        amount = args.get("amount", 0)
        result = action_scroll(x, y, amount)
    else:
        result = {"success": False, "error": f"Unknown action: {action}"}

    print(json.dumps(result), flush=True)


if __name__ == "__main__":
    main()
