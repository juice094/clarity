---
title: 架构地图 · 验证层
category: Architecture
date: 2026-05-16
tags: [architecture]
---

# 架构地图 · 验证层

> 用途：改完代码后知道跑什么命令验证，崩了知道回滚到哪
> 更新触发：新增测试、CI 流程变更、测试基线变化

---

## 全局基线（每次修改后必跑）

```bash
cd C:\Users\22414\dev\third_party\clarity

# 1. 单元测试（577 passed / 0 failed / 6 ignored 为当前基线）
cargo test --workspace --lib

# 2. Clippy 零 warning
cargo clippy --workspace --lib --bins --tests -- -D warnings

# 3. 格式检查
cargo fmt --all -- --check

# 4. Doc 零 warning
cargo doc --no-deps

# 5. 安全审计（非阻断，仅参考）
cargo audit --deny unsound --deny yanked
```

**当前基线 commit**：`32bbd457`（phase2/protocol-pilot）

---

## 分层验证命令

### 核心层（clarity-core）

```bash
# 全 core 测试
cargo test --workspace --lib -p clarity-core

# 子模块精准测试
cargo test --workspace --lib agent::
cargo test --workspace --lib approval::
cargo test --workspace --lib tools::
cargo test --workspace --lib llm::
cargo test --workspace --lib subagents::
cargo test --workspace --lib background::
cargo test --workspace --lib skills::
cargo test --workspace --lib mcp::
cargo test --workspace --lib memory::
cargo test --workspace --lib compaction::
cargo test --workspace --lib notifications::
cargo test --workspace --lib view_models::
```

### 协议层（clarity-wire）

```bash
cargo test --workspace --lib -p clarity-wire
```

### 基础设施层（clarity-memory）

```bash
cargo test --workspace --lib -p clarity-memory
```

### 前端层（clarity-egui）

```bash
# 编译检查（egui 无 lib tests，只有编译验证）
cargo check -p clarity-egui

# 纯逻辑测试（分布在 core 中）
cargo test --workspace --lib llm_policy::
cargo test --workspace --lib view_models::settings::
```

**注意**：clarity-egui **零 UI 渲染测试**。任何 UI 改动必须手动验证。

### Gateway 层（clarity-gateway）

```bash
cargo test --workspace --lib -p clarity-gateway

# 启动验证
cargo run -p clarity-gateway
# 然后 curl http://localhost:PORT/health
```

### 集成测试

```bash
cargo test --test integration
```

---

## 手动验证清单（UI 改动必做）

| 改动范围 | 验证步骤 |
|---------|---------|
| Chat 输入 | 启动 egui → 输入多行文本 → Shift+Enter 换行 → Enter 发送 → 检查 draft persistence（切 session 再切回） |
| 审批弹窗 | 设置 Interactive/Smart 模式 → 触发 file_write → 检查弹窗渲染 → Approve / Reject / ApproveForSession |
| Plan 可视化 | 触发 plan 工具 → 检查步骤状态图标（⏳/▶️/✅/❌）→ 检查步骤详情 |
| Settings | 打开 Settings → 切换 provider → 检查 model 下拉框联动 → 修改 api_key → 保存 → 重启验证持久化 |
| Streaming | 发送长消息 → 检查流式渲染无闪烁 → 点击 Stop → 检查中断后状态 |
| Steer Mode | streaming 时发送第二条消息 → 检查 pending_send 队列 → 检查第一条取消后第二条自动发送 |
| Theme | 切换 dark/light → 检查所有面板颜色一致 |
| Onboarding | 删除配置 → 启动 → 检查 onboarding 流程 |

---

## 回滚锚点

### 方法 1 — Git stash（推荐，未提交时）

```bash
git stash push -m "WIP: <模块名>"
# 验证基线通过后
git stash pop
```

### 方法 2 — 回滚到最近 green commit

```bash
# 查看最近提交
git log --oneline -10

# 找到最近一个 "test pass" 或 "clippy clean" 的 commit
# 例如：32bbd457 docs: SSPL licensing memo...

# 软回滚（保留改动到 working tree）
git reset --soft HEAD~1

# 硬回滚（丢弃所有改动，危险！）
git reset --hard <green-commit-hash>
```

### 方法 3 — 分支逃逸

```bash
# 改代码前切新分支
git checkout -b experiment/<功能名>

# 改崩了直接丢弃分支
git checkout phase2/protocol-pilot
git branch -D experiment/<功能名>
```

---

## CI 检查单（提交前自检）

- [ ] `cargo test --workspace --lib` → 577 passed, 0 failed
- [ ] `cargo clippy --workspace --lib --bins --tests -- -D warnings` → 0 errors
- [ ] `cargo fmt --all -- --check` → 0 diff
- [ ] `cargo doc --no-deps` → 0 warning
- [ ] 手动验证通过（如有 UI 改动）
- [ ] commit message 符合规范：`type(scope): description`

---

*本文件由 AI 会话维护。测试基线变更时需更新 passed/failed/ignored 计数。*
