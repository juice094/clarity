#!/usr/bin/env python3
"""Regression script for clarity-claw / clarity-openclaw merge boundaries.

Verifies that after the merge:
1. No `clarity-openclaw` crate remains in the workspace.
2. `clarity-claw` library (default features) does not depend on `clarity-core`
   or `clarity-wire`.
3. `clarity-egui` depends on `clarity-claw` and not on `clarity-openclaw`.
4. `clarity-claw`'s `tray` feature is only pulled in by the `clarity-claw`
   binary target, not by library consumers.
"""

from __future__ import annotations

import json
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CLAW_CARGO_TOML = ROOT / "crates" / "clarity-claw" / "Cargo.toml"


def run(cmd: list[str]) -> dict:
    result = subprocess.run(
        cmd,
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        print(f"command failed: {' '.join(cmd)}", file=sys.stderr)
        print(result.stderr, file=sys.stderr)
        sys.exit(1)
    return json.loads(result.stdout)


def get_package(metadata: dict, name: str) -> dict | None:
    for pkg in metadata.get("packages", []):
        if pkg["name"] == name:
            return pkg
    return None


def get_dependency_names(pkg: dict) -> set[str]:
    """Return direct dependency crate names for a package (all targets)."""
    names: set[str] = set()
    for dep in pkg.get("dependencies", []):
        names.add(dep["name"])
    return names


def get_enabled_features(pkg: dict, dep_name: str) -> list[str]:
    """Return explicitly enabled features for a dependency."""
    for dep in pkg.get("dependencies", []):
        if dep["name"] == dep_name:
            return dep.get("features", [])
    return []


def load_cargo_toml(path: Path) -> dict:
    with path.open("rb") as f:
        return tomllib.load(f)


def is_optional_dependency(manifest: dict, name: str) -> bool:
    """Check whether a dependency is declared as optional in Cargo.toml."""
    deps = manifest.get("dependencies", {})
    spec = deps.get(name)
    if isinstance(spec, dict):
        return spec.get("optional", False)
    return False


def default_features(manifest: dict) -> list[str]:
    return manifest.get("features", {}).get("default", [])


def main() -> int:
    metadata = run(["cargo", "metadata", "--format-version", "1"])

    errors: list[str] = []

    # 1. No clarity-openclaw in workspace.
    if get_package(metadata, "clarity-openclaw") is not None:
        errors.append("clarity-openclaw still exists in workspace")

    claw_pkg = get_package(metadata, "clarity-claw")
    if claw_pkg is None:
        errors.append("clarity-claw not found in workspace")
        print_errors(errors)
        return 1

    egui_pkg = get_package(metadata, "clarity-egui")
    if egui_pkg is None:
        errors.append("clarity-egui not found in workspace")
        print_errors(errors)
        return 1

    # 2. clarity-claw lib default features do not depend on clarity-core/clarity-wire.
    # Optional dependencies are allowed only if they are gated behind non-default features.
    manifest = load_cargo_toml(CLAW_CARGO_TOML)
    forbidden_lib_deps = {"clarity-core", "clarity-wire"}
    defaults = default_features(manifest)
    for dep_name in forbidden_lib_deps:
        if not is_optional_dependency(manifest, dep_name):
            errors.append(
                f"clarity-claw dependency {dep_name} must be declared optional"
            )
        else:
            # Optional deps become enabled only via features. Verify they are not
            # in the default feature set.
            for feat, enabled in manifest.get("features", {}).items():
                if feat == "default":
                    continue
                if dep_name in enabled or f"dep:{dep_name}" in enabled:
                    if feat in defaults:
                        errors.append(
                            f"clarity-claw optional dependency {dep_name} is enabled by default feature '{feat}'"
                        )

    # 3. clarity-egui depends on clarity-claw, not clarity-openclaw.
    egui_deps = get_dependency_names(egui_pkg)
    if "clarity-claw" not in egui_deps:
        errors.append("clarity-egui does not depend on clarity-claw")
    if "clarity-openclaw" in egui_deps:
        errors.append("clarity-egui still depends on clarity-openclaw")

    # 4. clarity-egui does not enable the tray feature of clarity-claw.
    enabled = get_enabled_features(egui_pkg, "clarity-claw")
    if "tray" in enabled:
        errors.append(
            "clarity-egui enables clarity-claw/tray feature (should only be used by binary)"
        )

    if errors:
        print("FAILED: clarity-claw boundary checks", file=sys.stderr)
        for err in errors:
            print(f"  - {err}", file=sys.stderr)
        return 1

    print("OK: clarity-claw boundary checks passed")
    return 0


def print_errors(errors: list[str]) -> None:
    print("FAILED: clarity-claw boundary checks", file=sys.stderr)
    for err in errors:
        print(f"  - {err}", file=sys.stderr)


if __name__ == "__main__":
    sys.exit(main())
