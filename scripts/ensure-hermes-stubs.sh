#!/usr/bin/env bash
# Create minimal stub crates for hermes-memory when the real repository is not
# checked out. This allows Cargo to load manifests for the optional `hermes`
# dependencies without actually compiling them (the feature is off by default).
#
# The real hermes-memory repository is expected at:
#   <clarity-repo>/../hermes-memory

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HERMES_ROOT="${REPO_ROOT}/../hermes-memory"

if [ -f "${HERMES_ROOT}/crates/hermes-memory-core/Cargo.toml" ]; then
  echo "hermes-memory already present at ${HERMES_ROOT}; skipping stub creation."
  exit 0
fi

echo "Creating hermes-memory stubs at ${HERMES_ROOT}..."

mkdir -p "${HERMES_ROOT}/crates/hermes-memory-core/src"
mkdir -p "${HERMES_ROOT}/crates/hermes-memory-store/src"
mkdir -p "${HERMES_ROOT}/crates/hermes-memory-search/src"

cat > "${HERMES_ROOT}/crates/hermes-memory-core/Cargo.toml" <<'EOF'
[package]
name = "hermes-memory-core"
version = "0.1.0-dev"
edition = "2021"
license = "AGPL-3.0-or-later"
description = "Stub crate for optional hermes-memory integration."
EOF

cat > "${HERMES_ROOT}/crates/hermes-memory-core/src/lib.rs" <<'EOF'
//! Stub implementation. The real hermes-memory-core lives in the private
//! hermes-memory repository; this stub exists only so Cargo can load the
//! optional dependency manifest when hermes is disabled.
EOF

cat > "${HERMES_ROOT}/crates/hermes-memory-store/Cargo.toml" <<'EOF'
[package]
name = "hermes-memory-store"
version = "0.1.0-dev"
edition = "2021"
license = "AGPL-3.0-or-later"
description = "Stub crate for optional hermes-memory integration."
EOF

cat > "${HERMES_ROOT}/crates/hermes-memory-store/src/lib.rs" <<'EOF'
//! Stub implementation. See hermes-memory-core for context.
EOF

cat > "${HERMES_ROOT}/crates/hermes-memory-search/Cargo.toml" <<'EOF'
[package]
name = "hermes-memory-search"
version = "0.1.0-dev"
edition = "2021"
license = "AGPL-3.0-or-later"
description = "Stub crate for optional hermes-memory integration."
EOF

cat > "${HERMES_ROOT}/crates/hermes-memory-search/src/lib.rs" <<'EOF'
//! Stub implementation. See hermes-memory-core for context.
EOF

echo "hermes-memory stubs created."
