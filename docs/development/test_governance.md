---
title: Clarity 项目测试治理框架
category: Document
date: 2026-06-25
tags: [document]
---

# Clarity 项目测试治理框架

> 原则：**测试通过 = 唯一交付标准**。不接受"代码已写"、"应该能工作"等口头承诺。

---

## 1. 不信任原则

### 1.1 子代理交付验收流程

```
子代理报告完成
       ↓
   [自动验证] ←── 运行测试脚本，不看报告看结果
       ↓
   ┌─────────┐
   │ 通过？   │
   └────┬────┘
   是 /   \ 否
     /     \
 [验收通过] [退回返工]
 更新看板   + 详细失败日志
```

### 1.2 禁止事项

- ❌ **禁止口头报告**："已完成"、"应该没问题"
- ❌ **禁止部分交付**："功能实现了，测试稍后补"
- ❌ **禁止绕过验证**：未经自动化测试直接合并

### 1.3 强制检查点

| 检查点 | 验证方式 | 失败处理 |
|--------|---------|---------|
| 代码存在性 | `ls -la` 确认文件存在 | 立即退回 |
| 编译检查 | `cargo check` 零错误 | 立即退回 |
| 单元测试 | `cargo test` 100% 通过 | 立即退回 |
| Clippy | `cargo clippy` 零警告 | 必须修复 |
| 文档测试 | `cargo test --doc` 通过 | 必须修复 |

---

## 2. 测试标准矩阵

### 2.1 按模块类型的测试要求

| 模块类型 | 单元测试 | 集成测试 | 文档测试 | 覆盖率目标 |
|---------|---------|---------|---------|-----------|
| **Core 工具** | 必须 | 推荐 | 必须 | 80%+ |
| **LLM 提供商** | Mock 测试 | 可选（标记 ignore） | 必须 | 70%+ |
| **渠道集成** | Mock 测试 | 手动测试文档 | 必须 | 60%+ |
| **配置系统** | 必须 | 必须 | 必须 | 85%+ |
| **Memory** | 必须 | 必须 | 必须 | 85%+ |

### 2.2 测试命名规范

```rust
// 单元测试
#[test]
fn test_<模块>_<功能>_<场景>() {}

// 示例
#[test]
fn test_websearch_empty_query_error() {}
#[test]
fn test_config_load_env_override() {}

// 集成测试（需要外部资源）
#[tokio::test]
#[ignore = "Requires network access"]
async fn test_deepseek_live_api() {}
```

### 2.3 必须测试的边界条件

每个功能必须测试：
- [ ] **正常路径**：标准输入，期望输出
- [ ] **空输入**：空字符串、空数组、None
- [ ] **超大输入**：超出限制的大小
- [ ] **错误处理**：无效参数、网络失败、权限拒绝
- [ ] **并发安全**：多线程/异步环境下的行为

---

## 3. 自动化验收脚本

### 3.1 使用方式

```powershell
# 验收单个 crate
.\scripts\verify.ps1 clarity-core

# 验收全部
.\scripts\verify.ps1 --all

# 生成报告
.\scripts\verify.ps1 --all -Report
```

### 3.2 验收脚本标准（必须实现）

脚本必须输出：
```json
{
  "crate": "clarity-core",
  "timestamp": "2026-04-03T14:30:00Z",
  "checks": {
    "compile": { "status": "PASS", "duration_ms": 1200 },
    "test": { "status": "PASS", "passed": 45, "failed": 0, "ignored": 2 },
    "clippy": { "status": "PASS", "warnings": 0 },
    "fmt": { "status": "PASS" }
  },
  "overall": "PASS"
}
```

---

## 4. 子代理任务模板（强制使用）

### 4.1 任务分配模板

```markdown
## 任务：XXX

### 交付物清单
- [ ] `path/to/file1.rs` （必须包含单元测试）
- [ ] `path/to/file2.rs` （必须包含文档测试）

### 验收标准（必须全部满足）
- [ ] `cargo test -p clarity-xxx` 100% 通过
- [ ] `cargo clippy -p clarity-xxx` 零警告
- [ ] 新增代码覆盖率 >= 70%

### 验证命令
```powershell
cd C:\Users\22414\dev\clarity
cargo test -p clarity-xxx
cargo clippy -p clarity-xxx -- -D warnings
```

### 报告格式
完成后必须提供：
1. 测试输出截图/日志
2. 覆盖率报告
3. 任何跳过测试的理由说明
```

### 4.2 子代理禁止行为

- 不得修改本治理文件
- 不得降低测试标准
- 不得标记测试为 `#[ignore]` 而不提供理由

---

## 5. 测试覆盖率追踪

### 5.1 当前基线（2026-04-03）

| Crate | 行覆盖率 | 测试数 | 状态 |
|-------|---------|--------|------|
| clarity-core | ??% | 603+ | 待测量 |
| clarity-memory | ??% | 97 | 待测量 |
| clarity-gateway | ??% | 116 | 待测量 |
| clarity-tui | ??% | 46 | 待测量 |

### 5.2 目标

- 2周内：核心 crate 覆盖率达到 70%
- 1个月内：全 workspace 达到 80%

---

## 6. 违规处理

| 违规次数 | 处理措施 |
|---------|---------|
| 第1次 | 警告，要求补测试 |
| 第2次 | 暂停分配新任务，必须先补测试 |
| 第3次 | 永久禁用该子代理，仅使用其他可靠代理 |

---

## 7. 工具链

### 7.1 必需工具

```bash
# 测试
cargo test

# 覆盖率（可选但推荐）
cargo install cargo-tarpaulin
cargo tarpaulin --out Html

# 代码检查
cargo clippy -- -D warnings
cargo fmt -- --check
```

### 7.2 CI 预检（本地）

```bash
#!/bin/bash
# .githooks/pre-push

set -e

echo "=== Running pre-push checks ==="

cargo fmt --all -- --check
cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings
cargo test --workspace --lib --exclude clarity-slint

echo "=== All checks passed ==="
```

---

## 8. 代码健康红线（Code Health Baseline）

> 本节基于 v0.3.0 代码健康评估注入，作为增量开发的硬性约束。

### 8.1 定量基线（不可退化）

| 指标 | v0.3.0 基线 | 天花板 / 目标 | 测量命令 |
|------|------------|--------------|---------|
| `unwrap()` / `expect()` 密度（非测试） | ~1,069 处 | **不新增**；存量逐步 `?` 化 | `grep -rn "\.unwrap()\|\.expect(" crates/ --include="*.rs" \| grep -v "test" \| grep -v "\.smoke\."` |
| `pub fn` doc 覆盖率 | ~92% | **≥90%** | `cargo doc --no-deps 2>&1 \| grep "missing"` |
| clippy warning | 0 | **0** | `cargo clippy --workspace --lib --bins --tests -D warnings` |
| `unsafe` 数量 | 1 处 | **禁止新增** | `grep -rn "unsafe" crates/ --include="*.rs" \| grep -v "test"` |
| 测试通过率 | 1554/0 | **100%** | `cargo test --workspace --lib --exclude clarity-slint` |
| 二进制测试 | 275/0 | **100%** | `cargo test --workspace --bins --exclude clarity-slint -- --test-threads=2` |

### 8.2 `unwrap()` / `expect()` 分类策略

存量 1,069 处按风险等级分类处理：

| 类别 | 示例 | 策略 |
|------|------|------|
| **同步原语** | `lock().unwrap()`, `read().unwrap()`, `write().unwrap()` | 允许保留；鼓励新代码用 `tokio::sync` |
| **初始化期** | `config.parse().unwrap()`（启动时一次） | 允许保留，但必须配 `// SAFE: 仅启动期调用，失败即 panic 是合理行为` |
| **解析/IO 风险** | `serde_json::from_str().unwrap()`, `PathBuf` 操作 | **禁止新增**；存量逐步改为 `?` + `AgentError` |
| **测试代码** | `#[test]` / `*.smoke.test.*` 中的 unwrap | 不受限制 |

### 8.3 验收命令（每次提交前必跑）

```bash
# Rust 侧
cargo test --workspace --lib --exclude clarity-slint
cargo test --workspace --bins --exclude clarity-slint -- --test-threads=2
cargo test --workspace --doc --exclude clarity-slint -- --test-threads=2
cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings
cargo fmt --all -- --check
cargo doc --workspace --no-deps --exclude clarity-slint

# 安全检查（本地预检）
cargo audit --deny unsound --deny yanked
```

### 8.4 违规处理

与测试违规同等对待：

| 违规类型 | 第 1 次 | 第 2 次 | 第 3 次 |
|---------|---------|---------|---------|
| 新增无注释 unwrap | 警告，要求补注释或改为 `?` | 暂停分配新任务，先修复 | 永久禁用该子 Agent |
| 新增 pub fn 无 doc | 警告，要求补文档 | 暂停分配 | 永久禁用 |
| 引入 clippy warning | 必须修复，否则不合并 | — | — |
| 新增 unsafe | **立即阻断**，需人工审批 | — | — |

### 8.5 存量债务追踪

- `clarity-core` 27k 行 god crate 拆分：见 `ENGINEERING_PLAN.md` Phase D，冻结至 v0.5.0 后评估。
- unwrap 存量清理：不设定硬 deadline，但每个功能 PR 应附带 "此 PR 减少 X 处风险类 unwrap" 的 self-review。

---

**生效日期**：立即生效  
**最后更新**：2026-06-25  
**负责人**：主控会话（人类监督）
