# ADR-003: Extract `clarity-contract` as a Standalone Crate for Trait-Centric Tool Interoperability

- Status: Accepted
- Deciders: juice094
- Date: 2026-05-01

## Context

`clarity-core` had grown to ~27,000 lines with deep internal coupling between modules. Sprint 13 (Week 4) identified that cycles such as `agent ↔ approval` and `agent ↔ llm` were partially caused by trait definitions living inside the god crate, forcing downstream modules to depend on the entire core surface.

The specific trigger was the need for a **zero-dependency foundation** for tool interoperability:
- `mcp-sdk-rs` is transport-centric (stdio/SSE); it forces consumers to commit to a wire protocol.
- `rig-core` couples tools to its own executor and provider abstractions.
- Clarity needed a minimal trait layer (`Tool`, capability tokens, structured errors) that third-party crates could implement without pulling in the full Clarity engine (LLM providers, memory systems, background tasks, etc.).

Extracting this layer into its own crate also serves the "架构健康纪律" (distribution-as-coupling-check): if `clarity-contract` cannot be compiled independently and described in 50 words, the interface boundary is too messy.

## Decision

Extract `clarity-contract` as a new workspace crate containing the minimal trait contracts for AI agent tools:

- `Tool` trait (name, description, parameters schema, execute method)
- `ToolCall` / `FunctionCall` structs
- Capability tokens and structured error types (`ToolError`, `AgentError` subset)
- `Message`, `MessageRole`, `StreamDelta` (extracted in commit `5f16263d`)

`clarity-core` re-exports these types to maintain backward compatibility for existing callers. Full downstream migration to direct `clarity-contract` dependencies is deferred until the contract surface stabilizes.

The crate is positioned as:
> "Minimal trait contracts for AI agent tools. Extract this crate when you need a zero-dependency foundation for tool interoperability without pulling in the full Clarity engine."

## Consequences

### Positive
- Breaks cyclic dependencies between `agent`, `approval`, `llm`, and `tools` modules by uplifting shared types into a leaf crate.
- Enables external consumers (plugins, MCP servers, third-party tool authors) to implement `Tool` without depending on `clarity-core`'s ~27k lines and heavy dependency tree.
- Serves as a compile-speed win: changes to `clarity-core` internals no longer invalidate crates that only need the contract types.
- Satisfies the architecture health discipline: the crate compiles independently (`cargo check -p clarity-contract`) and has a 50-word README explaining its purpose.

### Negative
- Adds a ninth crate to the workspace, increasing workspace coordination overhead.
- Re-export shim in `clarity-core` is temporary technical debt; eventual full migration requires updating all `use` statements across 6+ crates.
- Some types (e.g., `AgentError`) straddle the boundary between "contract" and "implementation"; the current split is pragmatic but not theoretically pure.

### Neutral
- `clarity-contract` has no features or optional dependencies; it is a leaf crate with minimal compile time.
- CI coverage: `cargo test -p clarity-contract --lib` runs 41+ contract-level tests (added in Sprint 15.5).

## Alternatives Considered

| Alternative | Evaluation | Outcome |
|---|---|---|
| **Keep traits inside `clarity-core`** | Would avoid a new crate but perpetuate the coupling cycles and block external tool interoperability. | Rejected |
| **Adopt `mcp-sdk-rs` as the contract layer** | MCP is transport-centric (stdio/SSE). It forces a wire-protocol commitment and is heavier than a simple trait boundary. | Rejected |
| **Adopt `rig-core` traits** | Rig couples tools to its own executor and provider abstractions; incompatible with Clarity's multi-provider, multi-frontend architecture. | Rejected |
| **Extract a larger `clarity-types` crate** | Would include too many implementation details (session types, view models, etc.), violating the "minimal contract" goal. | Rejected |
| **Extract `clarity-contract` with only `Tool` + errors** | Matches the "trait-centric, zero-dep" requirement exactly. Scales by adding more types only when they are proven stable. | Accepted |

## References

- Commit: `5f16263d` (docs: Phase 0 完成 + contract: Message/MessageRole/StreamDelta 提取)
- Commit: `e86a64e8` (feat: generic OAuth provider architecture + contract layer extraction)
- Related docs: `crates/clarity-contract/README.md`
- Related docs: `docs/ARCHITECTURE.md` (Crate Topology, Reusability rating)
- Related docs: `docs/AGENTS.md` (Sprint 13 Week 4 — `clarity-contract` crate PoC)
