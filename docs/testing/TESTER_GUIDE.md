---
title: Clarity Tester Guide
category: Testing
date: 2026-06-25
tags: [testing, qa, guide]
---

# Clarity 测试人员快速上手指南

> 本文档面向**测试人员 / QA / 新加入的开发者**，说明如何在不深入 Rust 代码的前提下，验证 Clarity 的功能正确性、环境健康与发布基线。
>
> 权威运行上下文与测试基线见 [`AGENTS.md`](../../AGENTS.md) §6。

---

## 1. 测试哲学

Clarity 是一个**本地优先、单二进制、零外部运行时依赖**的 Rust AI 运行时。测试的核心目标是：

1. **保证每个入口可独立构建并运行**（egui / tui / gateway / headless / claw / mobile-core）。
2. **保证核心功能在代码变更后零回归**：单元、二进制、文档、集成四层测试全绿。
3. **保证发布产物无需 Python / Node.js / Ollama 即可运行**。

Python 脚本仅用于**开发与测试编排**，不是 Clarity 的运行时依赖。

---

## 2. 环境检查（Doctor）

### 2.1 运行环境医生

```powershell
python scripts/doctor.py
```

输出示例：

```text
Clarity Doctor: 6 OK, 0 WARN, 0 FAIL
  [OK] cargo: found at C:\Users\22414\.cargo\bin\cargo.EXE
  [OK] rustc: 1.95.0 (>= 1.85)
  [OK] cargo workspace: metadata resolves
  [OK] python: found at ...
  [OK] git-lfs: ...
  [SKIP] CLARITY_LOCAL_MODEL_PATH: not set
  [SKIP] hermes-memory: not found ...
  [OK] clippy: zero warnings
```

### 2.2 Doctor 检查项说明

| 检查项 | 通过标准 | 失败处理 |
|--------|---------|---------|
| `cargo` | 在 PATH 中 | 安装 Rust |
| `rustc` | >= 1.85 | 升级 Rust |
| `cargo workspace` | `cargo metadata` 成功 | 查看 `Cargo.toml` 错误 |
| `python` | 在 PATH 中 | 仅影响 dev 脚本 |
| `git-lfs` | 已安装 | 非必须，安装可避免大文件问题 |
| `CLARITY_LOCAL_MODEL_PATH` | 指向存在的 `.gguf` | 不设则跳过本地 LLM 测试 |
| `hermes-memory` | 位于 `<workspace>/../hermes-memory/` | 不设则跳过 `hermes` feature 测试 |
| `clippy` | 零 warning | 按提示修复 |

报告会生成在 `target/doctor-report.md`。

---

## 3. 自动化测试矩阵

### 3.1 一键跑全量

```powershell
python scripts/test_runner.py
```

默认执行以下四层测试：

| 层级 | 命令 | 说明 |
|------|------|------|
| lib | `cargo test --workspace --lib --exclude clarity-slint` | 各 crate 内部单元测试 |
| bins | `cargo test --workspace --bins --exclude clarity-slint -- --test-threads=2` | bin target 逻辑测试 |
| doc | `cargo test --workspace --doc --exclude clarity-slint -- --test-threads=2` | rustdoc 示例测试 |
| integration | `cargo test -p clarity-integration-tests --lib` | 跨 crate 集成测试 |

### 3.2 跳过某些层级

```powershell
# 跳过集成测试（最快反馈）
python scripts/test_runner.py --skip integration

# 跳过 doc + bins
python scripts/test_runner.py --skip doc bins
```

### 3.3 输出报告

```powershell
python scripts/test_runner.py --markdown target/test-report.md --json target/test-report.json
```

- Markdown 报告：人类可读摘要 + 每个 suite 的输出 tail
- JSON 报告：CI / 数据面板可消费

### 3.4 与 CI 对齐

CI 入口在 `.github/workflows/ci.yml`，共 12 个 job。本地最简等价验证：

```powershell
python scripts/doctor.py
python scripts/test_runner.py
cargo fmt --all -- --check
```

---

## 4. Feature / 外部资源矩阵

部分测试需要外部资源或特定 feature，本地默认跳过或禁用：

| Feature / 资源 | 如何启用 | 不启用的影响 |
|---------------|---------|-------------|
| `local-llm` | 默认开启；需本地 `.gguf` 模型跑完整测试 | 相关测试被忽略 |
| `local-llm-cuda` | `--features local-llm-cuda` | 无 NVIDIA CUDA 时跳过 |
| `hermes` | 需 `CLARITY_MEMORY_BACKEND=hermes` + `<workspace>/../hermes-memory/` | 默认关闭 |
| Discord / Telegram | 上游 `rustls-webpki` 问题未解决 | 通道测试默认禁用 |
| Webhook | 默认启用 | 无需额外配置 |

---

## 5. 手动验收场景

以下场景建议发布前手动验证（目前无 UI 自动化）：

### 5.1 egui 桌面端

```powershell
cargo run -p clarity-egui
```

- [ ] 窗口默认尺寸 1280×800 正常显示
- [ ] 左 icon rail + 中主舞台 + 右工具 rail 三栏布局无重叠
- [ ] 空状态时 Composer 居中显示
- [ ] 发送消息后输入栏恢复底部固定
- [ ] 右 rail Status / Tools / Subagents / Memory / Knowledge 卡片可切换
- [ ] Knowledge 面板：在搜索框输入关键词并点击 Search，结果列表出现且均为文件节点（无 `tag:` 条目）
- [ ] Knowledge 面板：点击 Top active，列表按激活度排序，点击结果可在下方查看路径与摘要
- [ ] 发送含 `[[某笔记]]` 或 `某笔记.md` 的消息后，Knowledge 面板 Top active 中该笔记激活度上升
- [ ] `Ctrl+Shift+L` 可调出布局诊断覆盖层

### 5.2 TUI

```powershell
cargo run -p clarity-tui
```

- [ ] `/` 命令列表可呼出
- [ ] Esc 可关闭弹窗
- [ ] 聊天消息渲染无乱码

### 5.3 Gateway + Web IDE

```powershell
cargo run -p clarity-gateway
# 浏览器访问 http://127.0.0.1:18800/
```

- [ ] Admin UI 可加载
- [ ] WebSocket 连接可建立
- [ ] 创建 session 后可发送消息

### 5.4 Headless CLI

```powershell
cargo run -p clarity-headless -- --prompt "hello" --provider local --output json
```

- [ ] 单条 prompt 可执行
- [ ] `--file` 模式可读取文件
- [ ] JSON 输出格式正确

### 5.5 Claw 系统托盘

```powershell
cargo run -p clarity-claw
```

- [ ] 托盘图标出现
- [ ] 可连接本地 Gateway

---

## 6. 常见失败排查

### `cargo test` 编译失败

```powershell
cargo check --workspace --lib --bins --exclude clarity-slint
```

### 测试忽过忽不过

- 检查 `CLARITY_LOCAL_MODEL_PATH` 是否指向有效 `.gguf`
- 检查是否有其他进程占用 Gateway 端口 18790 / 18800
- 使用 `--test-threads=2` 减少资源竞争

### Clippy 失败

```powershell
cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings
```

### 格式化失败

```powershell
cargo fmt --all
```

---

## 7. 提交 Bug

请提供：

1. 运行命令与完整输出（或 `target/test-report.md`）
2. `python scripts/doctor.py` 输出
3. 复现步骤
4. 期望结果 vs 实际结果
5. 若涉及 UI，请附截图或 `Ctrl+Shift+L` 诊断层截图

---

## 8. 相关文件

- 测试基线：[`AGENTS.md`](../../AGENTS.md) §6.2
- 测试策略：[`docs/testing/TEST_STRATEGY.md`](./TEST_STRATEGY.md)
- 开发环境：[`docs/development/setup.md`](../../docs/development/setup.md)
- Provider 配置：[`docs/development/provider-config.md`](../../docs/development/provider-config.md)
- 架构健康：[`docs/development/ARCHITECTURE_HEALTH.md`](../../docs/development/ARCHITECTURE_HEALTH.md)
- 测试编排脚本：[`scripts/test_runner.py`](../../scripts/test_runner.py)
- 环境检查脚本：[`scripts/doctor.py`](../../scripts/doctor.py)
- 架构健康脚本：[`scripts/arch_health.py`](../../scripts/arch_health.py)
