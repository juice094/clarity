# Create minimal stub crates for hermes-memory when the real repository is not
# checked out. This allows Cargo to load manifests for the optional `hermes`
# dependencies without actually compiling them (the feature is off by default).
#
# The real hermes-memory repository is expected at:
#   <clarity-repo>/../hermes-memory

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$hermesRoot = Join-Path $repoRoot ".." "hermes-memory"

$coreToml = Join-Path $hermesRoot "crates" "hermes-memory-core" "Cargo.toml"
if (Test-Path $coreToml) {
    Write-Host "hermes-memory already present at $hermesRoot; skipping stub creation."
    exit 0
}

Write-Host "Creating hermes-memory stubs at $hermesRoot..."

New-Item -ItemType Directory -Force -Path (Join-Path $hermesRoot "crates" "hermes-memory-core" "src") | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $hermesRoot "crates" "hermes-memory-store" "src") | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $hermesRoot "crates" "hermes-memory-search" "src") | Out-Null

@"
[package]
name = `"hermes-memory-core`"
version = `"0.1.0-dev`"
edition = `"2021`"
license = `"AGPL-3.0-or-later`"
description = `"Stub crate for optional hermes-memory integration.`"
"@ | Set-Content -Path (Join-Path $hermesRoot "crates" "hermes-memory-core" "Cargo.toml") -Encoding UTF8

@"
//! Stub implementation. The real hermes-memory-core lives in the private
//! hermes-memory repository; this stub exists only so Cargo can load the
//! optional dependency manifest when hermes is disabled.
"@ | Set-Content -Path (Join-Path $hermesRoot "crates" "hermes-memory-core" "src" "lib.rs") -Encoding UTF8

@"
[package]
name = `"hermes-memory-store`"
version = `"0.1.0-dev`"
edition = `"2021`"
license = `"AGPL-3.0-or-later`"
description = `"Stub crate for optional hermes-memory integration.`"
"@ | Set-Content -Path (Join-Path $hermesRoot "crates" "hermes-memory-store" "Cargo.toml") -Encoding UTF8

@"
//! Stub implementation. See hermes-memory-core for context.
"@ | Set-Content -Path (Join-Path $hermesRoot "crates" "hermes-memory-store" "src" "lib.rs") -Encoding UTF8

@"
[package]
name = `"hermes-memory-search`"
version = `"0.1.0-dev`"
edition = `"2021`"
license = `"AGPL-3.0-or-later`"
description = `"Stub crate for optional hermes-memory integration.`"
"@ | Set-Content -Path (Join-Path $hermesRoot "crates" "hermes-memory-search" "Cargo.toml") -Encoding UTF8

@"
//! Stub implementation. See hermes-memory-core for context.
"@ | Set-Content -Path (Join-Path $hermesRoot "crates" "hermes-memory-search" "src" "lib.rs") -Encoding UTF8

Write-Host "hermes-memory stubs created."
