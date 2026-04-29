# Sprint 11 — 超越 Kimi CLI（Surpass Kimi CLI）

> 制定日期：2026-04-30
> 基线 commit：`phase2/protocol-pilot` @ `6ef6ed5`
> 来源：Sprint 10 附录视角路线（用户批准纳入）
> 核心目标：**将分散在 Subagent/Headless/TUI 各层的能力统一注入主 Agent 的默认编码工作流**

---

## 执行摘要

Sprint 10 审计发现 Clarity 的底层能力（Git 上下文、Plan 模式、并发执行、Skill 系统、MCP、记忆）已经超过或持平 Kimi CLI。差距不是"有没有"，而是"分散在哪一层、有没有统一注入主 Agent"。

本 Sprint 通过三阶段整合，将 Clarity 从"个人 AI 运行时"升级为"终端编码伴侣"，直接对标并超越 Kimi CLI。

---

## 一、Phase A：上下文注入（Week 1）

### 1.1 目标

将 Subagent 层已有的 `GitContext::collect` 和轻量项目扫描提升到主 Agent 的 `SystemPromptBuilder`，使主 Agent 开箱即即具备项目上下文感知能力。

### 1.2 文件变更

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/clarity-core/src/agent/prompt.rs` | 修改 | `SystemPromptBuilder` 新增 `auto_context()` 方法 |
| `crates/clarity-core/src/agent/prompt.rs` | 修改 | `build()` 中追加 `auto_context()` 输出到 system prompt |
| `crates/clarity-core/src/subagents/runner.rs` | 导出 | 将 `GitContext` / `collect_git_context` 提升为 `pub`（若尚未） |
| `crates/clarity-core/src/agent/prompt.rs` (tests) | 新增 | ≥3 个测试：Git 注入、Cargo.toml 注入、文件树注入 |

### 1.3 设计细节

```rust
// agent/prompt.rs
impl SystemPromptBuilder {
    async fn auto_context(&self) -> String {
        let mut ctx = String::new();
        let wd = &self.config.working_dir;

        // 1. Git 上下文（已有实现，从 subagents/runner.rs 迁移）
        if let Some(git) = crate::subagents::GitContext::collect(wd).await {
            ctx.push_str(&git.to_prompt_string());
            ctx.push('\n');
        }

        // 2. 项目元数据（轻量扫描，前 1KB）
        for manifest in ["Cargo.toml", "package.json", "pyproject.toml"] {
            let path = wd.join(manifest);
            if let Ok(content) = std::fs::read_to_string(&path) {
                let truncated = &content[..content.len().min(1024)];
                ctx.push_str(&format!("\n# {}\n```\n{}\n```\n", manifest, truncated));
            }
        }

        // 3. 文件树（浅层 2 级，排除 .git/target/node_modules）
        ctx.push_str("\n# File Tree (depth 2)\n");
        ctx.push_str(&shallow_tree(wd, 2));

        ctx
    }
}

fn shallow_tree(dir: &Path, max_depth: usize) -> String {
    // 递归生成目录树，max_depth 限制深度
    // 排除: .git, target, node_modules, .clarity, .vscode
}
```

**性能约束**：`auto_context()` 在每次 `SystemPromptBuilder::build()` 时执行（即每轮 Agent 对话开始时）。文件树扫描必须控制在 <10ms，否则影响交互体验。采用缓存：若工作目录 mtime 未变，复用上一次的 `auto_context` 结果。

### 1.4 验收标准

- `test_git_context_injected_into_prompt` — 主 Agent system prompt 包含 Git 分支信息
- `test_cargo_toml_injected_into_prompt` — system prompt 包含 Cargo.toml 内容
- `test_shallow_tree_respects_depth` — 文件树深度不超过 2 级
- `test_context_cached_across_turns` — 同一工作目录连续两轮复用缓存

---

## 二、Phase B：编辑精度升级（Week 1–2）

### 2.1 目标

将 `file_edit` 从纯字符串替换升级为支持批量替换和 unified diff 预览，使代码编辑精度达到 Kimi CLI 水平。

### 2.2 文件变更

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/clarity-core/src/tools/file.rs` | 修改 | `file_edit` Schema 扩展 `replacements: Vec<{old, new}>` |
| `crates/clarity-core/src/tools/file.rs` | 修改 | 批量替换逻辑 + 原子回滚（单文件内） |
| `crates/clarity-core/src/tools/file.rs` | 修改 | `_diff_preview` 改为 unified diff 格式 |
| `crates/clarity-core/src/tools/file.rs` (tests) | 新增 | ≥5 个测试：批量替换、部分失败回滚、unified diff 格式 |

### 2.3 设计细节

**Schema 扩展**（向后兼容）：

```json
{
  "path": "string",
  "old_string": "string",
  "new_string": "string",
  "replacements": [
    {"old": "str1", "new": "str2"},
    {"old": "str3", "new": "str4"}
  ]
}
```

- `old_string`/`new_string` 与 `replacements` 互斥。若两者都提供，优先 `replacements`。
- `replacements` 按数组顺序依次应用。若某一步失败（old 未找到），**整批回滚**（文件恢复到原始内容），返回错误。

**Unified diff 预览**：

```rust
fn unified_diff(old: &str, new: &str, path: &str) -> String {
    // 使用 similar 或 diff  crate 生成标准 unified diff
    // 行级对比，带 +/- 前缀和 @@ 头
}
```

**为什么不直接做 AST-aware**：
- AST 解析需要 per-language parser（tree-sitter/syn/swiftc 等），引入成本高
- Unified diff + 批量替换已覆盖 90% 的代码编辑场景
- AST-aware 可作为 Sprint 12 的 Phase B2（可选 feature）

### 2.4 验收标准

- `test_batch_replacement_all_succeed` — 3 个 replacements 全部成功，文件正确更新
- `test_batch_replacement_partial_failure_rollback` — 第 2 个 replacement 失败，文件回滚到原始内容
- `test_unified_diff_format` — 返回标准 unified diff，含 `---`/`+++`/`@@` 头
- `test_backward_compat_single_replace` — 仅提供 `old_string`/`new_string` 时行为与 Sprint 10 一致

---

## 三、Phase C：终端体验补齐（Week 2）

### 3.1 目标

补齐 TUI 和 Headless 的终端交互缺口，使 Clarity 成为真正的 shell 伴侣工具。

### 3.2 文件变更

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/clarity-tui/src/commands.rs` | 修改 | 新增 `/yolo`、`/interactive`、`/planmode` 命令 |
| `crates/clarity-tui/src/app.rs` | 修改 | 命令 handler 切换 `AgentInner.approval_mode` |
| `crates/clarity-headless/src/main.rs` | 修改 | `read_prompt()` 支持 `std::io::stdin()` 读取 |
| `crates/clarity-headless/src/main.rs` (tests) | 新增 | stdin 管道测试 |

### 3.3 设计细节

**TUI 模式切换**：

```rust
// commands.rs
"/yolo" => {
    app.agent.set_approval_mode(ApprovalMode::Yolo);
    app.system_message("Switched to YOLO mode — all tools auto-approved".into());
}
"/interactive" => {
    app.agent.set_approval_mode(ApprovalMode::Interactive);
    app.system_message("Switched to Interactive mode — medium/high risk tools require approval".into());
}
```

**Headless stdin 管道**：

```rust
// headless/src/main.rs
fn read_prompt(args: &Args) -> Result<String> {
    match (&args.prompt, &args.file) {
        (Some(p), _) => Ok(p.clone()),
        (None, Some(path)) => std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read prompt file: {}", path)),
        (None, None) => {
            // NEW: read from stdin if not a tty (pipe mode)
            use std::io::{self, Read};
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            if buffer.trim().is_empty() {
                anyhow::bail!("Either --prompt, --file, or stdin must be provided");
            }
            Ok(buffer)
        }
    }
}
```

### 3.4 验收标准

- `test_tui_yolo_command` — `/yolo` 后调用 `file_edit` 不弹出 DiffPopup
- `test_tui_interactive_command` — `/interactive` 后调用 `file_edit` 弹出 DiffPopup
- `test_headless_stdin_pipe` — `echo "prompt" | cargo run -p clarity-headless` 成功执行
- `test_headless_stdin_empty_fallback` — stdin 为空且 `--prompt` 未提供时返回错误

---

## 四、跨 Phase 依赖与执行顺序

```
Phase A ──→ Phase B ──→ Phase C
 (Week 1)    (Week 1-2)   (Week 2)
     │           │            │
     └─ 零依赖   └─ 依赖 A 完成  └─ 依赖 B 完成（file_edit Schema 变更需稳定）
```

**回退策略**：
- 若 Phase B 的 unified diff 引入编译失败，回退到仅批量替换（不加 diff crate），diff 预览延至 Sprint 12
- 若 Phase A 的 `auto_context()` 性能 >10ms，先禁用文件树扫描，仅保留 Git + manifest 注入

---

## 五、与 Kimi CLI 的最终对比目标（Sprint 11 结束后）

| 维度 | 当前差距 | Sprint 11 交付后 |
|------|---------|-----------------|
| **代码编辑** | 字符串替换 | 批量替换 + unified diff 预览 |
| **上下文提取** | 无自动注入 | Git + Cargo.toml/package.json + 文件树自动注入 |
| **审批交互** | TUI 硬编码 Interactive | `/yolo`/`/interactive` 运行时切换 |
| **管道友好** | 不支持 stdin | `echo "prompt" | clarity-headless` 可用 |
| **Plan 执行** | 顺序执行 | 保持顺序（复杂度高，不阻塞） |
| **离线可用** | ✅ 已超越 | ✅ 保持 |
| **多模型** | ✅ 已超越 | ✅ 保持 |
| **MCP 生态** | ✅ 已超越 | ✅ 保持 |
| **记忆持久化** | ✅ 已超越 | ✅ 保持 |

---

## 六、验收总表

| # | 标准 | 验证方式 |
|---|------|---------|
| 1 | 主 Agent prompt 自动包含 Git 上下文 | 打印 system prompt，确认含分支名和未提交文件数 |
| 2 | 主 Agent prompt 自动包含 Cargo.toml | 在 Rust 项目目录运行，打印 system prompt 确认 |
| 3 | file_edit 支持 3 处批量替换且原子回滚 | 单元测试：2 成功 1 失败 → 文件恢复原状 |
| 4 | file_edit 返回 unified diff | 单元测试：确认输出含 `---`/`+++`/`@@` |
| 5 | TUI `/yolo` 免审批 | 手动：/yolo → file_edit → 无 DiffPopup |
| 6 | Headless 管道输入 | `echo "hi" | cargo run -p clarity-headless` 成功 |
| 7 | 测试增量 | ≥12 个新增测试；覆盖率不下降 |
| 8 | clippy 零警告 | `cargo clippy --workspace --lib --tests -- -D warnings` |

---

*本计划由 Sprint 10 附录视角路线触发，用户批准纳入 Sprint 11。任何与代码实态的冲突以代码为准。*
