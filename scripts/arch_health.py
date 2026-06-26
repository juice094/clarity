#!/usr/bin/env python3
"""Clarity architecture health metrics collector.

Collects code-size, crate-dependency, and topology-invariant metrics. The
output feeds architecture-health iteration documentation and can be diffed
over time. Python is dev-only, NOT a runtime dependency of Clarity.

Usage:
    python scripts/arch_health.py
    python scripts/arch_health.py --json arch-health.json
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def _parse_workspace_members() -> list[str]:
    """Read workspace members from root Cargo.toml."""
    import tomllib

    cargo_toml = ROOT / "Cargo.toml"
    data = tomllib.loads(cargo_toml.read_text(encoding="utf-8"))
    members = data.get("workspace", {}).get("members", [])
    # Expand globs roughly: crates/* and tests/integration
    result: list[str] = []
    for pattern in members:
        if pattern.endswith("/*"):
            result.extend(p.name for p in (ROOT / pattern[:-2]).iterdir() if p.is_dir())
        else:
            result.append(Path(pattern).name)
    return result


def _count_rust_code() -> tuple[int, int]:
    """Count Rust source files and total non-blank lines."""
    files = list(ROOT.rglob("crates/**/*.rs")) + list(ROOT.rglob("tests/**/*.rs"))
    total_lines = 0
    for f in files:
        content = f.read_text(encoding="utf-8", errors="ignore")
        total_lines += sum(1 for line in content.splitlines() if line.strip())
    return len(files), total_lines


def _extract_crate_info(cargo_toml: Path) -> dict[str, object]:
    import tomllib

    data = tomllib.loads(cargo_toml.read_text(encoding="utf-8"))
    deps = data.get("dependencies", {})
    internal_deps = [d for d in deps if d.startswith("clarity-")]
    return {
        "name": data.get("package", {}).get("name", cargo_toml.parent.name),
        "version": data.get("package", {}).get("version", "unknown"),
        "internal_deps": internal_deps,
        "internal_dep_count": len(internal_deps),
    }


def collect_crate_metrics() -> list[dict[str, object]]:
    metrics: list[dict[str, object]] = []
    for crate_dir in sorted((ROOT / "crates").iterdir()):
        cargo_toml = crate_dir / "Cargo.toml"
        if not cargo_toml.exists():
            continue
        info = _extract_crate_info(cargo_toml)
        src_dir = crate_dir / "src"
        rs_files = list(src_dir.rglob("*.rs")) if src_dir.exists() else []
        info["rs_files"] = len(rs_files)
        metrics.append(info)
    return metrics


def build_report() -> dict[str, object]:
    file_count, total_lines = _count_rust_code()
    crate_metrics = collect_crate_metrics()
    workspace_members = _parse_workspace_members()
    return {
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "project_root": str(ROOT),
        "workspace_members": len(workspace_members),
        "crate_directories_in_crates_folder": len(crate_metrics),
        "rust_source_files": file_count,
        "non_blank_rust_lines": total_lines,
        "crate_metrics": crate_metrics,
        "topology_rules": [
            "clarity-contract has zero internal dependencies",
            "clarity-core has zero frontend/network crate dependencies",
            "frontend crates never import each other",
        ],
    }


def markdown_report(report: dict[str, object]) -> str:
    lines = [
        "# Clarity Architecture Health Report",
        "",
        f"- Generated: {report['generated_at']}",
        f"- Workspace members: {report['workspace_members']}",
        f"- Crate directories: {report['crate_directories_in_crates_folder']}",
        f"- Rust source files: {report['rust_source_files']}",
        f"- Non-blank Rust lines: {report['non_blank_rust_lines']:,}",
        "",
        "## Crate Metrics",
        "",
        "| Crate | Version | Internal Deps | .rs Files |",
        "|-------|---------|---------------|-----------|",
    ]
    for m in report["crate_metrics"]:
        deps = ", ".join(m["internal_deps"]) or "—"
        lines.append(
            f"| {m['name']} | {m['version']} | {deps} | {m['rs_files']} |"
        )
    lines.extend([
        "",
        "## Topology Invariants",
        "",
    ])
    for rule in report["topology_rules"]:
        lines.append(f"- {rule}")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description="Clarity architecture health metrics")
    parser.add_argument(
        "--json",
        type=Path,
        help="Write JSON report to this path",
    )
    parser.add_argument(
        "--markdown",
        type=Path,
        default=Path("target/arch-health-report.md"),
        help="Write Markdown report to this path (default: target/arch-health-report.md)",
    )
    args = parser.parse_args()

    report = build_report()

    print(f"Architecture Health:")
    print(f"  Workspace members: {report['workspace_members']}")
    print(f"  Crate directories: {report['crate_directories_in_crates_folder']}")
    print(f"  Rust files: {report['rust_source_files']}")
    print(f"  Non-blank lines: {report['non_blank_rust_lines']:,}")

    args.markdown.parent.mkdir(parents=True, exist_ok=True)
    args.markdown.write_text(markdown_report(report), encoding="utf-8")
    print(f"Report: {args.markdown.resolve()}")

    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(report, indent=2), encoding="utf-8")
        print(f"JSON: {args.json.resolve()}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
