# Sprint 40 Plan — 运行时健壮性深化 + 集成测试补全

**规划日期**: 2026-05-08  
**基线提交**: `3c9907cc` (docs: archive Sprint 33-39)  
**预计工时**: 6–8 小时  
**交付目标**: 降低生产代码 unwrap 密度 ~65%，新增 MCP 端到端集成测试，跟进 dependabot 安全告警。

---

## 一、调研结论（子代理输出汇总）

### 1.1 unwrap 分布
- 生产代码共 **178 个 unwrap**（非此前估算的 209）
- `agent/construct.rs` 42 个 unwrap **全部为 `RwLock`/`Mutex` 锁获取**，运行时 panic 风险极低
- 约 **115 个 unwrap**（占生产代码 65%）来自标准库同步原语的 `LockResult::unwrap()`
- `parking_lot` 已在 `clarity-egui` 中作为依赖（`parking_lot = "0.12"`），项目已接受该库

### 1.2 集成测试现状
- `tests/integration/` 已有 **10 个测试**（core_wire ×2, gateway_http ×7, memory_persistence ×1）
- **未覆盖**: claw 协调、MCP 端到端、TUI 交互、egui
- `clarity-egui` 为 bin-only，强行添加 lib target 涉及面过广，本次 Sprint 不触及

### 1.3 安全漏洞
- `cargo audit` 仅报告 **14 个 unmaintained warnings**，未检出 high severity
- GitHub `security/dependabot/22` 具体 CVE 待定位，可能涉及 `Cargo.lock` 中未更新的传递依赖

---

## 二、Phase 拆分

### Phase 1 — parking_lot 迁移（~2–3h）

**目标**: 用 `parking_lot::RwLock`/`Mutex` 替换 `std::sync` 版本，消除 ~115 个锁 unwrap。

**涉及 crate**:
| Crate | 预估 unwrap 数 | 优先级 |
|-------|---------------|--------|
| `clarity-core` | ~60 | P0 |
| `clarity-memory` | ~35 | P0 |
| `clarity-gateway` | ~20 | P1 |

**操作步骤**:
1. 在 `clarity-core`、`clarity-memory`、`clarity-gateway` 的 `Cargo.toml` 中添加 `parking_lot = "0.12"`
2. 全局搜索 `std::sync::(RwLock|Mutex)` → 替换为 `parking_lot::(RwLock|Mutex)`
3. 移除所有 `.write().unwrap()` → 改为 `.write()`（parking_lot 不返回 `LockResult`）
4. 移除所有 `.read().unwrap()` → 改为 `.read()`
5. 移除所有 `.lock().unwrap()` → 改为 `.lock()`
6. 编译修复：`cargo check --workspace`
7. 测试：`cargo test --workspace --lib`

**风险点**:
- `parking_lot` 不实现 `LockResult` 的 poison 语义；若某线程在持有锁时 panic，锁不会 poison。Clarity 的锁 panic 风险极低（构造路径），此行为变化可接受。
- `parking_lot::RwLock` 不支持 `into_inner()`（需确认是否使用）

### Phase 2 — MCP 端到端集成测试（~2–3h）

**目标**: 在 `tests/integration/tests/` 新增 `mcp_end_to_end.rs`，覆盖 MCP 工具注册 → 执行 → 结果回传。

**测试场景**:
1. 启动 `StdioMcpClient` 连接到一个 mock MCP server（可用 `cat` 或简单 Python 脚本模拟 JSON-RPC）
2. 注册一个工具（`tools/list` → `tools/call`）
3. Agent 通过 `ToolRegistry` 调用该工具
4. 验证结果正确回传

**参考模式**:
- `tests/integration/tests/core_wire.rs` — Agent + Wire Mock
- `tests/integration/tests/common/mod.rs` — `SequentialMockLlm`

**实现策略**:
- 若 MCP server mock 搭建成本高，可降级为单元测试级别的端到端（在 `clarity-mcp` 中测试 `StdioMcpClient` 与 mock 子进程的交互）
- 优先保证测试**可运行**、**可维护**，不追求真实子进程

### Phase 3 — dependabot/22 跟进（~1h）

**目标**: 定位并处理 GitHub 高严重性漏洞。

**步骤**:
1. 访问 `https://github.com/juice094/clarity/security/dependabot/22` 获取具体 CVE
2. 若涉及 `Cargo.lock` 中的传递依赖，执行 `cargo update -p <crate>`
3. 若涉及 break changes，评估升级范围；若过大，记录为已知限制
4. 更新 `AGENTS.md` 安全章节

---

## 三、验收标准

- [ ] `cargo test --workspace --lib` = **全部通过 / 0 failed**
- [ ] `cargo clippy --workspace` = **0 warning**（允许已存在的 egui i18n dead_code）
- [ ] 生产代码 unwrap 数量从 **178 → <80**（目标 65% 降幅）
- [ ] 新增至少 **1 个 MCP 端到端测试**，且能在 CI 中运行
- [ ] dependabot/22 状态明确（已修复 或 已记录为已知限制）
- [ ] `AGENTS.md` 更新 Sprint 40 记录

---

## 四、备选方案

若 parking_lot 迁移过程中发现未预期的问题（如某些 crate 的 `RwLock` 被 `std::sync::Arc` 包装导致类型冲突），可降级为：
- **方案 B**: 仅对 `agent/construct.rs` 做 `parking_lot` 局部迁移（42 个 unwrap → 0），其余保持现状。
- **方案 C**: 若 parking_lot 完全不适用，改用 `unwrap_or_else(|e| panic!("..."))` 语义化，将 115 个 unwrap 转为带上下文的 expect。

---

## 五、相关文件

- `crates/clarity-core/src/agent/construct.rs`
- `crates/clarity-core/src/background/store.rs`
- `crates/clarity-memory/src/session_store.rs`
- `crates/clarity-memory/src/lib.rs`
- `crates/clarity-gateway/src/session_store.rs`
- `tests/integration/tests/mcp_end_to_end.rs` (新增)
- `tests/integration/tests/common/mod.rs`
- `AGENTS.md`
