# Third-Party Notices

## hermes-memory

Clarity integrates code from the `hermes-memory` project, used under the
MIT OR Apache-2.0 license.

- Source: `C:/Users/22414/dev/hermes-memory` (path dependency)
- License: MIT OR Apache-2.0
- Components used in this repository:
  - `hermes-memory-core` (traits, data models, errors)
  - `hermes-memory-store` (SQLite-backed memory backend)
  - `hermes-memory-search` (BM25 / vector / hybrid recall)

The integration is gated by the optional `hermes` feature in
`crates/clarity-memory/Cargo.toml` and is not enabled by default.
