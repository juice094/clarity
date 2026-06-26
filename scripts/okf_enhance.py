#!/usr/bin/env python3
"""Enhance an OKF bundle to better conform to OKF v0.1 recommendations.

Adds title, description, tags, and timestamp to concept frontmatter,
cleans up empty string arrays, and creates a root log.md if missing.
"""

from __future__ import annotations

import re
import sys
from datetime import datetime, timezone
from pathlib import Path

import yaml

BUNDLE_DIR = Path("docs/okf/clarity-worktree")
TIMESTAMP = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def parse_frontmatter(text: str) -> tuple[dict | None, str]:
    """Extract YAML frontmatter and body from markdown text."""
    if not text.startswith("---\n"):
        return None, text
    parts = text.split("---\n", 2)
    if len(parts) < 3:
        return None, text
    try:
        front = yaml.safe_load(parts[1]) or {}
    except yaml.YAMLError as e:
        print(f"YAML parse error: {e}", file=sys.stderr)
        return None, text
    return front, parts[2]


def dump_frontmatter(front: dict) -> str:
    """Dump frontmatter with a stable key order and line spacing."""
    # Preserve custom keys by dumping the whole dict; yaml does a reasonable job.
    return yaml.safe_dump(
        front,
        sort_keys=False,
        allow_unicode=True,
        default_flow_style=False,
        width=120,
    )


def clean_array(value):
    """Replace [''] with [] and drop empty strings from dependency arrays."""
    if not isinstance(value, list):
        return value
    cleaned = [item for item in value if item != ""]
    return cleaned


def extract_description(body: str) -> str:
    """Extract a one-line description from the markdown body."""
    lines = [line.strip() for line in body.splitlines() if line.strip()]
    for line in lines:
        if line.startswith("#"):
            continue
        # Remove markdown links and bold/italic markers for plain description.
        plain = re.sub(r"\[([^\]]+)\]\([^)]+\)", r"\1", line)
        plain = re.sub(r"[*_`]|\[|\]", "", plain)
        if plain:
            return plain.rstrip(". ") + "."
    return ""


def derive_tags(front: dict) -> list[str]:
    """Derive tags from existing fields."""
    tags = {"clarity"}
    layer = front.get("layer")
    if layer:
        tags.add(str(layer))
    concept_type = front.get("type")
    if concept_type and concept_type != "concept":
        tags.add(str(concept_type))
    return sorted(tags)


def enhance_file(path: Path) -> bool:
    """Enhance a single OKF concept file. Returns True if modified."""
    text = path.read_text(encoding="utf-8")
    front, body = parse_frontmatter(text)
    if front is None:
        print(f"Skipping {path}: no frontmatter")
        return False

    original = front.copy()

    # Clean empty arrays in known relation fields.
    for key in ("depends_on", "consumed_by"):
        if key in front:
            front[key] = clean_array(front[key])

    # Add recommended OKF fields if missing.
    if "title" not in front and "name" in front:
        front["title"] = front["name"]
    if "description" not in front:
        desc = extract_description(body)
        if desc:
            front["description"] = desc
    if "tags" not in front:
        front["tags"] = derive_tags(front)
    if "timestamp" not in front:
        front["timestamp"] = TIMESTAMP

    if front == original:
        print(f"No changes needed: {path}")
        return False

    new_text = "---\n" + dump_frontmatter(front) + "---\n" + body
    path.write_text(new_text, encoding="utf-8")
    print(f"Enhanced: {path}")
    return True


def enhance_index(path: Path) -> bool:
    """Enhance the bundle root index.md."""
    text = path.read_text(encoding="utf-8")
    front, body = parse_frontmatter(text)
    if front is None:
        return False

    original = front.copy()
    if "title" not in front:
        front["title"] = front.get("name", "Clarity Project Worktree")
    if "type" not in front:
        front["type"] = "index"
    if "okf_version" not in front:
        front["okf_version"] = "0.1"
    if "timestamp" not in front:
        front["timestamp"] = TIMESTAMP

    if front == original:
        print(f"No changes needed: {path}")
        return False

    new_text = "---\n" + dump_frontmatter(front) + "---\n" + body
    path.write_text(new_text, encoding="utf-8")
    print(f"Enhanced: {path}")
    return True


def create_log(path: Path) -> bool:
    """Create a root log.md if it does not exist."""
    if path.exists():
        return False
    content = f"""---
type: log
title: Changelog
description: Change history for the Clarity OKF worktree.
timestamp: {TIMESTAMP}
---

# Changelog

## {TIMESTAMP[:10]}

- Enhanced all concept files with OKF v0.1 recommended fields (title, description, tags, timestamp).
- Cleaned empty dependency arrays.
- Declared bundle conformance to OKF v0.1.
"""
    path.write_text(content, encoding="utf-8")
    print(f"Created: {path}")
    return True


def main() -> int:
    if not BUNDLE_DIR.is_dir():
        print(f"Bundle directory not found: {BUNDLE_DIR}", file=sys.stderr)
        return 1

    modified = 0
    for concept in sorted(BUNDLE_DIR.glob("concepts/*.md")):
        if enhance_file(concept):
            modified += 1

    index = BUNDLE_DIR / "index.md"
    if enhance_index(index):
        modified += 1

    log = BUNDLE_DIR / "log.md"
    if create_log(log):
        modified += 1

    print(f"\nTotal modified/created: {modified}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
