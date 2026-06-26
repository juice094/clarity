#!/usr/bin/env python3
"""Clarity environment doctor.

Checks the local development environment and reports whether the project can
be built, tested, and run. Python is a dev-only convenience, NOT a runtime
dependency of Clarity.

Usage:
    python scripts/doctor.py
    python scripts/doctor.py --json
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import time
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Callable

ROOT = Path(__file__).resolve().parents[1]
MSRV = "1.85"


@dataclass
class Check:
    name: str
    status: str  # OK, WARN, FAIL, SKIP
    message: str
    suggestion: str = ""


@dataclass
class Report:
    generated_at: str
    root: str
    checks: list[Check] = field(default_factory=list)

    def ok_count(self) -> int:
        return sum(1 for c in self.checks if c.status == "OK")

    def warn_count(self) -> int:
        return sum(1 for c in self.checks if c.status == "WARN")

    def fail_count(self) -> int:
        return sum(1 for c in self.checks if c.status == "FAIL")


def _run(args: list[str], cwd: Path = ROOT) -> tuple[int, str, str]:
    proc = subprocess.run(
        args,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return proc.returncode, proc.stdout.strip(), proc.stderr.strip()


def check_executable(name: str, suggestion: str) -> Check:
    path = shutil.which(name)
    if path:
        return Check(name, "OK", f"found at {path}")
    return Check(name, "FAIL", f"not found in PATH", suggestion)


def check_rust_version() -> Check:
    rc, out, err = _run(["rustc", "--version"])
    if rc != 0:
        return Check("rustc", "FAIL", "could not run rustc", "Install Rust via rustup")
    match = re.search(r"rustc\s+(\d+\.\d+(?:\.\d+)?)", out)
    if not match:
        return Check("rustc", "WARN", f"unexpected output: {out}")
    version = match.group(1)
    # Compare major.minor only
    current = tuple(int(x) for x in version.split(".")[:2])
    required = tuple(int(x) for x in MSRV.split(".")[:2])
    if current >= required:
        return Check("rustc", "OK", f"{version} (>= {MSRV})")
    return Check(
        "rustc",
        "FAIL",
        f"{version} < MSRV {MSRV}",
        f"Update Rust to >= {MSRV}",
    )


def check_cargo_workspace() -> Check:
    rc, out, err = _run(["cargo", "metadata", "--format-version", "1"])
    if rc == 0:
        return Check("cargo workspace", "OK", "metadata resolves")
    return Check(
        "cargo workspace",
        "FAIL",
        f"metadata failed: {err}",
        "Run cargo check to see errors",
    )


def check_git_lfs() -> Check:
    rc, out, err = _run(["git", "lfs", "version"])
    if rc == 0:
        return Check("git-lfs", "OK", out.splitlines()[0])
    return Check(
        "git-lfs",
        "WARN",
        "not installed or not configured",
        "Install git-lfs if your clone uses LFS assets",
    )


def check_local_model() -> Check:
    path_str = os.environ.get("CLARITY_LOCAL_MODEL_PATH", "")
    if path_str and Path(path_str).exists():
        return Check("CLARITY_LOCAL_MODEL_PATH", "OK", f"{path_str} exists")
    if path_str:
        return Check(
            "CLARITY_LOCAL_MODEL_PATH",
            "WARN",
            f"set but not found: {path_str}",
            "Verify path or unset to skip local-llm tests",
        )
    return Check(
        "CLARITY_LOCAL_MODEL_PATH",
        "SKIP",
        "not set; local-llm tests that need a model will be skipped",
    )


def check_hermes_repo() -> Check:
    # AGENTS.md says hermes-memory should be at ../../../hermes-memory/
    # relative to crates/clarity-memory (i.e. <workspace-root>/../hermes-memory).
    hermes = ROOT.parent / "hermes-memory"
    if hermes.exists():
        return Check("hermes-memory", "OK", f"found at {hermes}")
    return Check(
        "hermes-memory",
        "SKIP",
        f"not found at {hermes}; hermes feature tests skipped",
        "Clone hermes-memory to <workspace-root>/../hermes-memory/ to enable hermes feature",
    )


def check_clippy() -> Check:
    rc, out, err = _run(
        [
            "cargo",
            "clippy",
            "--workspace",
            "--lib",
            "--bins",
            "--tests",
            "--exclude",
            "clarity-slint",
            "--",
            "-D",
            "warnings",
        ]
    )
    if rc == 0:
        return Check("clippy", "OK", "zero warnings")
    return Check(
        "clippy",
        "WARN",
        f"clippy exited {rc}; check output",
        "Run cargo clippy to see details",
    )


def run_all_checks() -> Report:
    report = Report(
        generated_at=time.strftime("%Y-%m-%dT%H:%M:%S"),
        root=str(ROOT),
    )
    checks: list[Callable[[], Check]] = [
        lambda: check_executable("cargo", "Install Rust via rustup"),
        check_rust_version,
        check_cargo_workspace,
        lambda: check_executable("python", "Python is only needed for dev scripts"),
        check_git_lfs,
        check_local_model,
        check_hermes_repo,
        check_clippy,
    ]
    for fn in checks:
        report.checks.append(fn())
    return report


def markdown_report(report: Report) -> str:
    lines = [
        "# Clarity Environment Doctor Report",
        "",
        f"- Generated: {report.generated_at}",
        f"- Project root: {report.root}",
        f"- Checks: {report.ok_count()} OK, {report.warn_count()} WARN, {report.fail_count()} FAIL",
        "",
        "| Check | Status | Message | Suggestion |",
        "|-------|--------|---------|------------|",
    ]
    for c in report.checks:
        lines.append(f"| {c.name} | {c.status} | {c.message} | {c.suggestion} |")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description="Clarity environment doctor")
    parser.add_argument(
        "--json",
        type=Path,
        help="Write JSON report to this path",
    )
    parser.add_argument(
        "--markdown",
        type=Path,
        default=Path("target/doctor-report.md"),
        help="Write Markdown report to this path (default: target/doctor-report.md)",
    )
    args = parser.parse_args()

    report = run_all_checks()

    print(f"Clarity Doctor: {report.ok_count()} OK, {report.warn_count()} WARN, {report.fail_count()} FAIL")
    for c in report.checks:
        print(f"  [{c.status}] {c.name}: {c.message}")

    args.markdown.parent.mkdir(parents=True, exist_ok=True)
    args.markdown.write_text(markdown_report(report), encoding="utf-8")
    print(f"Report: {args.markdown.resolve()}")

    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(
            json.dumps(asdict(report), indent=2), encoding="utf-8"
        )
        print(f"JSON: {args.json.resolve()}")

    return 1 if report.fail_count() > 0 else 0


if __name__ == "__main__":
    sys.exit(main())
