#!/usr/bin/env python3
"""Clarity unified test runner.

Orchestrates the cargo test matrix, parses results, and emits a unified
Markdown/JSON report. Intended for developers and QA; Python is NOT a runtime
dependency of Clarity itself.

Usage:
    python scripts/test_runner.py
    python scripts/test_runner.py --json report.json
    python scripts/test_runner.py --skip integration
    python scripts/test_runner.py --exclude clarity-anthropic-proxy
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import time
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Iterable

ROOT = Path(__file__).resolve().parents[1]


@dataclass(frozen=True)
class TestCommand:
    name: str
    cmd: list[str]
    optional: bool = False


@dataclass
class TestResult:
    name: str
    cmd: str
    returncode: int
    passed: int
    failed: int
    ignored: int
    measured: int
    filtered: int
    duration_sec: float
    stdout_tail: str
    stderr_tail: str


DEFAULT_MATRIX: list[TestCommand] = [
    TestCommand(
        "lib",
        [
            "cargo",
            "test",
            "--workspace",
            "--lib",
            "--exclude",
            "clarity-slint",
        ],
    ),
    TestCommand(
        "bins",
        [
            "cargo",
            "test",
            "--workspace",
            "--bins",
            "--exclude",
            "clarity-slint",
            "--",
            "--test-threads=2",
        ],
    ),
    TestCommand(
        "doc",
        [
            "cargo",
            "test",
            "--workspace",
            "--doc",
            "--exclude",
            "clarity-slint",
            "--",
            "--test-threads=2",
        ],
    ),
    TestCommand(
        "integration",
        ["cargo", "test", "-p", "clarity-integration-tests", "--lib"],
    ),
]


def _parse_test_result(output: str) -> dict[str, int]:
    """Parse 'test result: ok. X passed; Y failed; Z ignored; ...'."""
    result_line_re = re.compile(
        r"test result:\s*(ok|FAILED)\.\s*"
        r"(\d+)\s+passed;\s*"
        r"(\d+)\s+failed;\s*"
        r"(\d+)\s+ignored;\s*"
        r"(\d+)\s+measured;\s*"
        r"(\d+)\s+filtered out"
    )
    totals = {
        "passed": 0,
        "failed": 0,
        "ignored": 0,
        "measured": 0,
        "filtered": 0,
    }
    for match in result_line_re.finditer(output):
        totals["passed"] += int(match.group(2))
        totals["failed"] += int(match.group(3))
        totals["ignored"] += int(match.group(4))
        totals["measured"] += int(match.group(5))
        totals["filtered"] += int(match.group(6))
    return totals


def _tail(text: str, lines: int = 20) -> str:
    return "\n".join(text.splitlines()[-lines:])


def run_command(tc: TestCommand, cwd: Path) -> TestResult:
    start = time.monotonic()
    proc = subprocess.run(
        tc.cmd,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    duration = time.monotonic() - start
    parsed = _parse_test_result(proc.stdout + proc.stderr)
    return TestResult(
        name=tc.name,
        cmd=" ".join(tc.cmd),
        returncode=proc.returncode,
        passed=parsed["passed"],
        failed=parsed["failed"],
        ignored=parsed["ignored"],
        measured=parsed["measured"],
        filtered=parsed["filtered"],
        duration_sec=round(duration, 2),
        stdout_tail=_tail(proc.stdout, 30),
        stderr_tail=_tail(proc.stderr, 30),
    )


def build_matrix(skip: Iterable[str], exclude: str | None) -> list[TestCommand]:
    matrix = list(DEFAULT_MATRIX)
    if exclude:
        for tc in matrix:
            if "--exclude" in tc.cmd:
                tc.cmd[tc.cmd.index("--exclude") + 1] = exclude
    if "integration" in skip:
        matrix = [tc for tc in matrix if tc.name != "integration"]
    if "doc" in skip:
        matrix = [tc for tc in matrix if tc.name != "doc"]
    if "bins" in skip:
        matrix = [tc for tc in matrix if tc.name != "bins"]
    return matrix


def markdown_report(results: list[TestResult], started_at: str) -> str:
    total_passed = sum(r.passed for r in results)
    total_failed = sum(r.failed for r in results)
    total_ignored = sum(r.ignored for r in results)
    total_duration = round(sum(r.duration_sec for r in results), 2)
    overall = "✅ PASS" if total_failed == 0 else "❌ FAIL"

    lines = [
        "# Clarity Test Report",
        "",
        f"- Generated: {started_at}",
        f"- Overall: {overall}",
        f"- Total: {total_passed} passed, {total_failed} failed, {total_ignored} ignored",
        f"- Duration: {total_duration}s",
        "",
        "| Suite | Passed | Failed | Ignored | Duration | Status |",
        "|-------|--------|--------|---------|----------|--------|",
    ]
    for r in results:
        status = "✅" if r.returncode == 0 and r.failed == 0 else "❌"
        lines.append(
            f"| {r.name} | {r.passed} | {r.failed} | {r.ignored} | {r.duration_sec}s | {status} |"
        )
    lines.extend([
        "",
        "## Details",
        "",
    ])
    for r in results:
        lines.extend([
            f"### {r.name}",
            "",
            f"Command: `{r.cmd}`",
            "",
            f"Return code: {r.returncode}",
            "",
            "```text",
            r.stdout_tail if r.stdout_tail else "(no stdout)",
            "```",
            "",
        ])
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Clarity unified test runner and reporter"
    )
    parser.add_argument(
        "--json",
        type=Path,
        help="Write machine-readable JSON report to this path",
    )
    parser.add_argument(
        "--markdown",
        type=Path,
        default=Path("target/test-report.md"),
        help="Write Markdown report to this path (default: target/test-report.md)",
    )
    parser.add_argument(
        "--skip",
        nargs="+",
        choices=["integration", "doc", "bins"],
        default=[],
        help="Skip one or more test suites",
    )
    parser.add_argument(
        "--exclude",
        default="clarity-slint",
        help="Crate to exclude from workspace tests (default: clarity-slint)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print the test matrix without running",
    )
    args = parser.parse_args()

    matrix = build_matrix(args.skip, args.exclude)
    if args.dry_run:
        for tc in matrix:
            print(" ".join(tc.cmd))
        return 0

    started_at = time.strftime("%Y-%m-%dT%H:%M:%S")
    results: list[TestResult] = []
    for tc in matrix:
        print(f"[test_runner] Running {tc.name}: {' '.join(tc.cmd)}", flush=True)
        result = run_command(tc, ROOT)
        results.append(result)
        print(
            f"[test_runner] {tc.name}: {result.passed} passed, "
            f"{result.failed} failed, {result.ignored} ignored "
            f"({result.duration_sec}s)",
            flush=True,
        )

    total_failed = sum(r.failed for r in results)

    args.markdown.parent.mkdir(parents=True, exist_ok=True)
    args.markdown.write_text(markdown_report(results, started_at), encoding="utf-8")
    print(f"[test_runner] Markdown report: {args.markdown.resolve()}")

    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        payload = {
            "started_at": started_at,
            "results": [asdict(r) for r in results],
            "total_passed": sum(r.passed for r in results),
            "total_failed": total_failed,
            "total_ignored": sum(r.ignored for r in results),
            "total_duration_sec": round(sum(r.duration_sec for r in results), 2),
        }
        args.json.write_text(json.dumps(payload, indent=2), encoding="utf-8")
        print(f"[test_runner] JSON report: {args.json.resolve()}")

    return 1 if total_failed > 0 else 0


if __name__ == "__main__":
    sys.exit(main())
