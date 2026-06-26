---
id: clarity-mobile-core
name: clarity-mobile-core
type: mobile-core
layer: presentation
depends_on:
- clarity-core
- clarity-wire
- clarity-memory
- clarity-contract
- clarity-llm
consumed_by: []
title: clarity-mobile-core
description: Mobile FFI core for Android/iOS.
tags:
- clarity
- mobile-core
- presentation
timestamp: '2026-06-26T11:28:50Z'
---

# clarity-mobile-core

Mobile FFI core for Android/iOS.

## Responsibilities

- UniFFI bridge
- Runtime/events/config/memory APIs
- Kotlin/Swift bindings

## Notes

Full Android/iOS UI is still in roadmap; `local-llm` disabled by default for mobile ABI.
