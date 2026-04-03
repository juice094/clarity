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

```bash
# 验收单个 crate
./scripts/verify.sh clarity-core

# 验收全部
./scripts/verify.sh --all

# 生成报告
./scripts/verify.sh --report
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
```bash
cd Desktop/clarity
cargo test -p clarity-xxx
cargo tarpaulin -p clarity-xxx --out Stdout
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
| clarity-core | ??% | ?? | 待测量 |
| clarity-memory | ??% | 33 | 待测量 |
| clarity-gateway | ??% | 5 | 待测量 |
| clarity-tui | 0% | 0 | 需补充 |

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

cargo fmt -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace

echo "=== All checks passed ==="
```

---

**生效日期**：立即生效  
**最后更新**：2026-04-03  
**负责人**：主控会话（人类监督）
