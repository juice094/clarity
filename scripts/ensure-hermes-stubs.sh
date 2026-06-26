#!/usr/bin/env bash
# ponytail: minimal shim so Cargo can load optional hermes path deps without the
# private hermes-memory repository. When the `hermes` feature is off these stubs
# are not compiled; when it is on, the real repository is required.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HERMES_ROOT="${REPO_ROOT}/../hermes-memory"

if [ -f "${HERMES_ROOT}/crates/hermes-memory-core/Cargo.toml" ]; then
  exit 0
fi

mkdir -p "${HERMES_ROOT}/crates"
for NAME in hermes-memory-core hermes-memory-store hermes-memory-search; do
  CRATE_DIR="${HERMES_ROOT}/crates/${NAME}"
  mkdir -p "${CRATE_DIR}/src"
  cat > "${CRATE_DIR}/Cargo.toml" <<EOF
[package]
name = "${NAME}"
version = "0.1.0-dev"
edition = "2021"
EOF
  printf '// ponytail: stub only; real implementation lives in the private hermes-memory repo.\n' \
    > "${CRATE_DIR}/src/lib.rs"
done
