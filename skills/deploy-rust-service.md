---
id: deploy-rust-service
name: Deploy Rust Service
version: "1.0.0"
description: Safe deployment workflow for Rust services with health checks and rollback
tools:
  - bash
  - file_read
  - file_write
tags:
  - deploy
  - rust
  - production
---

## Prerequisites

Before deploying, ensure:
1. The service compiles successfully (`cargo build --release`)
2. All tests pass (`cargo test`)
3. The `Dockerfile` and `docker-compose.yml` are present

## Deployment Steps

### 1. Pre-flight Checks
- Read the current `Cargo.toml` to verify version
- Check `git status` to ensure no uncommitted changes
- Run `cargo test` to validate correctness

### 2. Build & Package
- Build release binary: `cargo build --release`
- Build Docker image: `docker build -t <service>:<version> .`

### 3. Deploy
- Update `docker-compose.yml` image tag if needed
- Run `docker-compose up -d`
- Verify health endpoint returns 200 OK

### 4. Rollback Plan
If health check fails:
- `docker-compose down`
- Revert to previous image tag
- `docker-compose up -d`
- Alert on-call if rollback also fails

## Output Format

Summarize the deployment in this format:
```
Deployed: <service> v<version>
Status: <success|failed|rolled-back>
Health: <endpoint> → <status>
Duration: <seconds>s
```
