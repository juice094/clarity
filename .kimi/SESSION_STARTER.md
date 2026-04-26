# Clarity 会话启动检查清单

## 0. 代码健康红线（每次会话必读 ⚠️）

**基线数据（v0.3.0）**：`unwrap()` ~1,069 / `pub fn` doc ~92% / clippy 0 warning / unsafe 1 处

新增代码必须遵守：
- [ ] `unwrap()` / `expect()` 新增必须配 `// SAFE: <不变量说明>` 注释（`lock().unwrap()` 除外）
- [ ] `pub fn` / `pub struct` / `pub enum` 必须含 `///` doc 注释
- [ ] clippy 零 warning（`cargo clippy --workspace --lib --bins --tests -D warnings`）
- [ ] **禁止新增 `unsafe`**（现有 1 处已白名单）
- [ ] 修改 `AgentController` / `Op` / `WireMessage` 必须检查三处调用方：`clarity-tui`、`clarity-gateway`、集成测试
- [ ] 禁止代码中遗留 `TODO` / `FIXME` / `XXX`（转 GitHub Issue）

---

## 1. 项目状态（一句话）

Clarity 是 Rust AI Agent 框架，**515 测试通过 / 0 failed / 0 warning**，新增 MCP/Skill/通知/Worker 模块。

---

## 2. 关键命令

```bash
cd C:\Users\22414\dev\third_party\clarity

# Rust 验证（每次修改后必跑）
cargo test --workspace --lib
cargo clippy --workspace --lib --bins --tests -D warnings
cargo fmt --all -- --check

# 前端验证
cd crates/clarity-tauri/frontend && npm test

# 运行各入口
cargo run -p clarity-tui               # TUI
cargo run -p clarity-gateway           # Gateway
cargo tauri dev                        # Desktop GUI

# Tauri with CUDA（可选）
$env:NVCC_CCBIN = "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Tools\MSVC\14.50.35717\bin\Hostx64\x64\cl.exe"
$env:CUDA_HOME = "C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.6"
cargo tauri build --features cuda
```

---

## 3. 环境变量

```powershell
$env:KIMI_CODE_API_KEY="sk-kimi-..."
$env:ANTHROPIC_AUTH_TOKEN="..."
$env:DEEPSEEK_API_KEY="..."
$env:OPENAI_API_KEY="..."
$env:CLARITY_LOCAL_MODEL_PATH="C:\path\to\model.gguf"
$env:CLARITY_MCP_ALLOWLIST="C:\tools\mcp-server.exe"
```

---

## 4. 对照组

- **Kimi CLI**: `C:\Users\22414\Desktop\kimi-cli-main\`
- **用途**: 代码参考源，稳定对照组

---

## 5. 文件映射（Clarity → Kimi CLI）

```
mcp/enhanced.rs       →  acp/mcp.py
skill/                →  skill/
notifications/        →  notifications/
background/worker.rs  →  background/worker.py
tools/policy.rs       →  subagents/models.py
```

---

## 6. 上下文清理

- 读取 `PROJECT_STATUS.md` 获取完整状态
- 读取 `docs/ai-protocol.md` 获取架构决策与 Hard Veto
- 不要重复读取已稳定的模块代码
- 新增功能时对照 Kimi CLI 实现

---

## 7. 禁止事项

- ❌ 不修改现有测试（除非适配接口变更）
- ❌ 不破坏编译通过的代码
- ❌ 不添加 `unsafe`
- ❌ 不改变架构设计（`AgentController` / `Op` / `WireMessage` 变更需检查三处调用方）
- ❌ 代码中不留 `TODO` / `FIXME` / `XXX`
