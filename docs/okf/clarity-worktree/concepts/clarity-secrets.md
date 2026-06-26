---
id: clarity-secrets
name: clarity-secrets
type: secrets
layer: infrastructure
depends_on:
- clarity-contract
consumed_by:
- clarity-llm
- clarity-core
title: clarity-secrets
description: Encrypted secret storage using ChaCha20-Poly1305.
tags:
- clarity
- infrastructure
- secrets
timestamp: '2026-06-26T11:28:50Z'
---

# clarity-secrets

Encrypted secret storage using ChaCha20-Poly1305.

## Responsibilities

- `enc2:` key encryption/decryption
- Local keyring integration

## Notes

Used by `models.toml` per-alias encrypted keys.
