# Clarity v0.2.0 改进计划

> 基线：`d6407e9` (v0.1.0 tag) | 122 .rs files | 352 tests passed

---

## 一、扫描发现汇总

### 🔴 P0 — 安全漏洞（2项）

| 编号 | 问题 | 位置 | 攻击面 |
|------|------|------|--------|
| S1 | `resolve_path` 对绝对路径完全放行 | `core/src/tools/mod.rs:254` | Agent 可被诱导读写任意文件 |
| S2 | Gateway `sanitize_path` 未限制在工作目录内 | `gateway/src/handlers.rs:1094` | 通过 `../` 或绝对路径访问服务端任意文件 |

### 🟡 P1 — 基础设施缺口（7项）

| 编号 | 问题 | 影响 |
|------|------|------|
| D1 | 5 个 crates 缺 README | 新用户无法快速理解各 crate 职责 |
| D2 | `clarity-gateway/src/lib.rs` 零注释 | pub API 无文档 |
| D3 | pub API 文档覆盖率 < 30%（tui/claw） | 内部协作困难 |
| D4 | AGENTS.md 未下沉到 crates | 子代理缺少 crate 级上下文 |
| T1 | gateway handlers 0 单元测试 | HTTP API 变更无自动化保护 |
| T2 | claw/tui 几乎无 lib 测试 | 二进制 crate 质量不可控 |
| T3 | 1 个 ignored 测试（embedding_integration timeout） | memory 后端存在隐患 |

### 🟢 P2 — 技术债与增强（5项）

| 编号 | 问题 | 现状 |
|------|------|------|
| TD1 | `std::sync::RwLock` → `tokio::sync::RwLock` | 已知，当前无 bug，长期隐患 |
| TD2 | MCP HTTP transport 未 E2E 验证 | 有实现，无真实 server 测试 |
| TD3 | CI 缺少安全扫描（cargo audit） | 依赖漏洞被动发现 |
| TD4 | CI 缺少覆盖率报告 | 无法量化测试缺口 |
| C1 | 1 个 clippy warning（ptr_arg） | 轻微代码质量 |

---

## 二、改进路线图

### Phase 0：安全热修复（立即，不批量子代理）
- [ ] **S1** `resolve_path` 增加 `starts_with(allowed_base)` 校验
- [ ] **S2** Gateway `sanitize_path` 增加工作目录前缀限制
- [ ] 为 S1/S2 各写 2-3 个针对性单元测试

### Phase 1：基础设施补全（可并行分发子代理）

**子代理 A — 文档补全**
- [ ] D1: 为 clarity-core / clarity-gateway / clarity-wire / clarity-tui / clarity-claw 写 crate README
- [ ] D2: 为 gateway/src/lib.rs 写 crate-level doc comment
- [ ] D3: 为 tui/claw 核心 pub API 补 doc comment
- [ ] D4: 在 crates/* 下放 AGENTS.md（继承根目录 + 补充 crate 约定）

**子代理 B — 测试补全**
- [ ] T1: gateway handlers mock 测试（axum::Server + tower::ServiceExt）
- [ ] T2: claw 核心逻辑拆分出 lib + 写测试
- [ ] T2: tui 核心逻辑拆分出 lib + 写测试
- [ ] T3: 修复 embedding_integration timeout 并恢复启用

**子代理 C — CI 与质量**
- [ ] TD3: CI 增加 `cargo audit` 步骤（安全扫描）
- [ ] TD4: CI 增加 `cargo tarpaulin` 步骤（覆盖率报告）
- [ ] C1: 修复 clippy ptr_arg warning
- [ ] 清理未跟踪文件（hello.rs / test_output.txt / subagent_context/）

### Phase 2：架构演进（需主会话把控）
- [ ] TD1: RwLock 迁移评估 + 分 crate 实施
- [ ] TD2: MCP HTTP transport E2E 验证（寻找/搭建真实 HTTP MCP server）

---

## 三、子代理任务分配方案

主会话（当前）负责 **Phase 0 安全修复** 和 **进度同步**。

确认后并行启动：

| 子代理 | 任务 | 预计产出 |
|--------|------|----------|
| **Alpha** | Phase 1A 文档补全 | 5 个 README + doc comments + 5 个 AGENTS.md |
| **Beta** | Phase 1B 测试补全 | gateway handlers mock 测试 + claw/tui lib 拆分 + 测试 |
| **Gamma** | Phase 1C CI/质量 | ci.yml 增强 + clippy fix + 文件清理 |

Phase 2 待 Phase 1 全部合并后再启动。

---

## 四、验收标准

- [ ] `cargo test --workspace --lib` = 0 failed, 0 ignored
- [ ] `cargo clippy --workspace --lib --bins --tests -- -D warnings` = 0 warnings
- [ ] `cargo audit` = 0 vulnerabilities (unmaintained 除外)
- [ ] 各 crate README 完整
- [ ] Gateway handlers 有 mock 测试覆盖
- [ ] CI 全绿（check/test/clippy/fmt/audit/coverage）
