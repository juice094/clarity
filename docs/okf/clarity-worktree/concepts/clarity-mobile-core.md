---
id: clarity-mobile-core
name: clarity-mobile-core
type: mobile-core
layer: presentation
depends_on: ["clarity-core", "clarity-wire", "clarity-memory", "clarity-contract", "clarity-llm"]
consumed_by: [""]
---

# clarity-mobile-core

Mobile FFI core for Android/iOS.

## Responsibilities

- UniFFI bridge
- Runtime/events/config/memory APIs
- Kotlin/Swift bindings

## Notes

Full Android/iOS UI is still in roadmap; `local-llm` disabled by default for mobile ABI.
