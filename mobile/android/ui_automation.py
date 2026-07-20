#!/usr/bin/env python3
"""Minimal UI automation helper for the Clarity Android app."""
import shutil
import subprocess
import sys
import time
import xml.etree.ElementTree as ET

DUMP_FILE = "mobile/android/window_dump.xml"


def run(cmd: list[str], check: bool = True) -> str:
    return subprocess.run(cmd, capture_output=True, text=True, check=check).stdout


def dump() -> ET.Element:
    run(["adb", "shell", "uiautomator", "dump", "//sdcard/window_dump.xml"])
    run(["adb", "pull", "/sdcard/window_dump.xml", DUMP_FILE])
    return ET.parse(DUMP_FILE).getroot()


def bounds_center(bounds: str) -> tuple[int, int]:
    parts = bounds.strip("[]").replace("][", ",").split(",")
    x1, y1, x2, y2 = map(int, parts)
    return (x1 + x2) // 2, (y1 + y2) // 2


def find(node: ET.Element, text: str | None = None, desc: str | None = None) -> ET.Element | None:
    for n in node.iter("node"):
        if text is not None and n.get("text") == text:
            return n
        if desc is not None and n.get("content-desc") == desc:
            return n
    return None


def tap(node: ET.Element) -> None:
    x, y = bounds_center(node.get("bounds"))
    run(["adb", "shell", "input", "tap", str(x), str(y)])


def input_text(text: str) -> None:
    # adb shell input text treats spaces specially; use %s for spaces.
    run(["adb", "shell", "input", "text", text.replace(" ", "%s")])


def wait_for(text: str | None = None, desc: str | None = None, timeout: float = 30.0) -> ET.Element:
    deadline = time.time() + timeout
    while time.time() < deadline:
        root = dump()
        n = find(root, text=text, desc=desc)
        if n is not None:
            return n
        time.sleep(0.5)
    raise RuntimeError(f"Timed out waiting for text={text!r} desc={desc!r}")


def tap_by_text(text: str) -> None:
    n = wait_for(text=text)
    tap(n)


def main() -> None:
    # Start fresh.
    run(["adb", "shell", "am", "force-stop", "com.juice094.clarity.mobile"])
    run(["adb", "shell", "am", "start", "-n", "com.juice094.clarity.mobile/.MainActivity"])
    time.sleep(3)

    # Open provider setup.
    fab = wait_for(desc="New chat", timeout=10)
    tap(fab)
    time.sleep(1)

    # Select DeepSeek device-login provider.
    tap_by_text("Provider")
    time.sleep(1)
    tap_by_text("DEEPSEEK_DEVICE")
    time.sleep(1)

    # Fill mobile.
    tap_by_text("Mobile number")
    time.sleep(0.3)
    input_text("13626566112")

    # Fill password.
    tap_by_text("Password")
    time.sleep(0.3)
    input_text("zjx040507")

    # Hide soft keyboard so the Connect button is not covered.
    run(["adb", "shell", "input", "keyevent", "4"])
    time.sleep(0.5)

    # Connect.
    btn = wait_for(text="Connect Local Agent", timeout=10)
    tap(btn)
    print("Connecting...")

    # Wait for thread list.
    wait_for(text="Claw", timeout=120)
    print("Connected; on thread list")

    # Create a new chat.
    fab = wait_for(desc="New chat", timeout=10)
    tap(fab)
    time.sleep(1)

    # Wait for chat input and send a message.
    msg = wait_for(text="Message", timeout=10)
    tap(msg)
    time.sleep(0.3)
    input_text("hello")
    send = wait_for(desc="Send", timeout=10)
    tap(send)
    print("Sent message; waiting for response...")

    # Wait a bit and capture a final dump for verification.
    time.sleep(30)
    dump()
    shutil.copy(DUMP_FILE, "mobile/android/final_dump.xml")
    print("Final dump saved to mobile/android/final_dump.xml")


if __name__ == "__main__":
    main()
