#!/usr/bin/env python3
"""Performance baseline collector for the Clarity Android app.

Measures two metrics on a connected emulator/device:

- Cold start: ``am force-stop`` + ``am start -W`` TotalTime, plus the
  ActivityManager "Displayed" first-frame line from logcat.
- First token: the app's built-in ``ClarityLatency/first_token_latency_ms``
  logcat probe (sendMessage() timestamp -> first UiEvent.ContentPart).

Usage:
    python mobile/android/perf_baseline.py coldstart [N]
    python mobile/android/perf_baseline.py setup <api_key>
    python mobile/android/perf_baseline.py firsttoken [N]
"""
import os
import re
import statistics
import subprocess
import sys
import time
import xml.etree.ElementTree as ET

PKG = "com.juice094.clarity.mobile"
ACTIVITY = f"{PKG}/.MainActivity"
DUMP_FILE = os.path.join(os.path.dirname(__file__), "perf_dump.xml")


def run(cmd: list[str], check: bool = True) -> str:
    return subprocess.run(cmd, capture_output=True, text=True, check=check).stdout


def dump() -> ET.Element:
    run(["adb", "shell", "uiautomator", "dump", "//sdcard/window_dump.xml"])
    run(["adb", "pull", "/sdcard/window_dump.xml", DUMP_FILE])
    return ET.parse(DUMP_FILE).getroot()


def bounds_center(bounds: str) -> tuple[int, int]:
    x1, y1, x2, y2 = map(int, bounds.strip("[]").replace("][", ",").split(","))
    return (x1 + x2) // 2, (y1 + y2) // 2


def find(node: ET.Element, text: str | None = None, desc: str | None = None) -> ET.Element | None:
    for n in node.iter("node"):
        if text is not None and n.get("text") == text:
            return n
        if desc is not None and n.get("content-desc") == desc:
            return n
    return None


def find_prefix(node: ET.Element, prefix: str) -> ET.Element | None:
    for n in node.iter("node"):
        t = n.get("text")
        if t is not None and t.startswith(prefix):
            return n
    return None


def tap(node: ET.Element) -> None:
    x, y = bounds_center(node.get("bounds"))
    run(["adb", "shell", "input", "tap", str(x), str(y)])


def input_text(text: str) -> None:
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


# ---------------------------------------------------------------- cold start

def cold_start_once() -> tuple[int, int | None]:
    """Return (TotalTime ms from am start -W, Displayed ms from logcat)."""
    run(["adb", "shell", "am", "force-stop", PKG])
    run(["adb", "logcat", "-c"])
    time.sleep(1.0)
    out = run(["adb", "shell", "am", "start", "-W", "-n", ACTIVITY])
    total = None
    for line in out.splitlines():
        m = re.search(r"TotalTime:\s*(\d+)", line)
        if m:
            total = int(m.group(1))
    if total is None:
        raise RuntimeError(f"am start -W output missing TotalTime:\n{out}")
    time.sleep(2.0)  # let the Displayed line land in logcat
    displayed = None
    log = run(["adb", "logcat", "-d", "-s", "ActivityTaskManager:I", "ActivityManager:I"], check=False)
    for line in log.splitlines():
        m = re.search(r"Displayed.*?:\s*\+(?:(\d+)s)?(\d+)ms", line)
        if m and PKG in line:
            displayed = int(m.group(1) or 0) * 1000 + int(m.group(2))
    return total, displayed


def cold_start(n: int) -> None:
    totals, displayeds = [], []
    for i in range(n):
        total, displayed = cold_start_once()
        totals.append(total)
        if displayed is not None:
            displayeds.append(displayed)
        print(f"[coldstart {i + 1}/{n}] TotalTime={total}ms Displayed={displayed}ms")
    print(f"coldstart TotalTime: n={len(totals)} median={statistics.median(totals):.0f}ms "
          f"min={min(totals)}ms max={max(totals)}ms all={totals}")
    if displayeds:
        print(f"coldstart Displayed: n={len(displayeds)} median={statistics.median(displayeds):.0f}ms all={displayeds}")
    # Confirm the app is actually interactive after the last run. Depending on
    # the persisted launch mode this is the thread list ("New chat" FAB) or a
    # restored chat screen ("Send"/"Back").
    deadline = time.time() + 15
    while time.time() < deadline:
        root = dump()
        if (find(root, desc="New chat") is not None or find(root, desc="Send") is not None
                or find(root, desc="Back") is not None
                or find(root, text="Connect to Clarity") is not None):
            print("interactive check: main UI present")
            return
        time.sleep(0.5)
    raise RuntimeError("App did not reach an interactive screen after cold start")


# ------------------------------------------------------------- provider setup

def on_setup_screen(root: ET.Element) -> bool:
    # Label depends on the currently selected provider.
    return find(root, text="API Key") is not None or find(root, text="Device token (optional)") is not None


def provider_setup(api_key: str, provider: str = "DEEPSEEK") -> None:
    """One-time local-agent setup with an API-key provider (no device login)."""
    run(["adb", "shell", "am", "force-stop", PKG])
    run(["adb", "shell", "am", "start", "-n", ACTIVITY])
    time.sleep(4)
    root = dump()
    if not on_setup_screen(root):
        # Not on the setup screen: try the thread-list FAB.
        fab = find(root, desc="New chat")
        if fab is None:
            raise RuntimeError("Neither ProviderSetup nor thread list visible")
        tap(fab)
        time.sleep(1.5)
        root = dump()
    if not on_setup_screen(root):
        raise RuntimeError("ProviderSetup screen not reached (runtime already initialized?)")
    sel = wait_for(desc="Select provider", timeout=10)
    tap(sel)
    time.sleep(1)
    root = dump()
    item = find(root, text=provider)
    if item is None:
        raise RuntimeError(f"{provider} not found in provider dropdown")
    tap(item)
    time.sleep(1)
    key = wait_for(text="API Key", timeout=10)
    tap(key)
    time.sleep(0.5)
    input_text(api_key)
    run(["adb", "shell", "input", "keyevent", "4"])  # hide keyboard
    time.sleep(0.5)
    btn = wait_for(text="Connect Local Agent", timeout=10)
    tap(btn)
    print(f"Connecting local agent ({provider})...")
    # On success the app lands either on the thread list or directly in a new
    # chat (auto-login behaviour), both are fine.
    deadline = time.time() + 60
    while time.time() < deadline:
        root = dump()
        if find(root, text="Claw") is not None or find(root, desc="Send") is not None:
            print("Connected; main UI visible")
            return
        time.sleep(1.0)
    raise RuntimeError("Connect did not reach thread list or chat screen")


# ------------------------------------------------------------- first token

def read_first_token_latency(timeout: float = 95.0) -> int:
    deadline = time.time() + timeout
    while time.time() < deadline:
        log = run(["adb", "logcat", "-d", "-s", "ClarityLatency:D"], check=False)
        m = re.findall(r"first_token_latency_ms=(\d+)", log)
        if m:
            return int(m[-1])
        time.sleep(1.0)
    raise RuntimeError("Timed out waiting for ClarityLatency first_token_latency_ms")


def wait_turn_end(timeout: float = 95.0) -> None:
    deadline = time.time() + timeout
    while time.time() < deadline:
        log = run(["adb", "logcat", "-d", "-s", "ClarityEvent:D"], check=False)
        if "TurnEnd" in log or "TurnError" in log or "Error" in log:
            return
        time.sleep(1.0)
    raise RuntimeError("Timed out waiting for turn end")


def first_token(n: int, message: str = "hi") -> None:
    # Local auto-login lands directly on a chat screen with an active thread.
    run(["adb", "shell", "am", "force-stop", PKG])
    run(["adb", "shell", "am", "start", "-n", ACTIVITY])
    time.sleep(5)
    deadline = time.time() + 60
    field = None
    while time.time() < deadline:
        root = dump()
        if find(root, text="Claw") is not None:
            # Landed on the thread list: open a fresh local chat.
            tap(find(root, desc="New chat"))
            time.sleep(1.5)
            continue
        field = find_prefix(root, "Message")
        if field is not None and find(root, desc="Send") is not None:
            break
        time.sleep(1.0)
    if field is None:
        raise RuntimeError("Chat screen not reached")
    latencies = []
    for i in range(n):
        root = dump()
        field = find_prefix(root, "Message")
        if field is None:
            raise RuntimeError("Message input not found")
        tap(field)
        time.sleep(0.3)
        input_text(message)
        run(["adb", "shell", "input", "keyevent", "4"])  # hide keyboard
        time.sleep(0.3)
        run(["adb", "logcat", "-c"])
        send = wait_for(desc="Send", timeout=10)
        tap(send)
        latency = read_first_token_latency()
        latencies.append(latency)
        print(f"[firsttoken {i + 1}/{n}] first_token_latency={latency}ms")
        wait_turn_end()
        time.sleep(1.0)
    print(f"firsttoken: n={len(latencies)} median={statistics.median(latencies):.0f}ms "
          f"min={min(latencies)}ms max={max(latencies)}ms all={latencies}")


def main() -> None:
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(1)
    cmd = sys.argv[1]
    if cmd == "coldstart":
        cold_start(int(sys.argv[2]) if len(sys.argv) > 2 else 5)
    elif cmd == "setup":
        provider_setup(sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else "DEEPSEEK")
    elif cmd == "firsttoken":
        first_token(int(sys.argv[2]) if len(sys.argv) > 2 else 5)
    else:
        print(__doc__)
        sys.exit(1)


if __name__ == "__main__":
    main()
