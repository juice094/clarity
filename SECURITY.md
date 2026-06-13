# Security Policy

> 中文版本见下方 [安全策略（中文）](#安全策略中文)

## Supported Versions

Security updates are provided for the current minor release series and the previous one.

| Version | Supported          |
| ------- | ------------------ |
| 0.3.x   | :white_check_mark: |
| 0.2.x   | :white_check_mark: |
| < 0.2   | :x:                |

## Reporting a Vulnerability

We take security issues seriously. If you discover a vulnerability in Clarity, please report it responsibly.

**Preferred channel**: Open a [GitHub Security Advisory](https://github.com/juice094/clarity/security/advisories/new) (private, visible only to maintainers).

**Alternative**: Email `juice094@users.noreply.github.com` with the subject `[Clarity Security] <brief description>`. Please include:
- Affected version(s) and commit hash if known
- Steps to reproduce
- Potential impact assessment
- Suggested fix (if any)

### Response Timeline

| Phase | Target |
|-------|--------|
| Initial acknowledgment | Within 48 hours |
| Triage & severity assessment | Within 7 days |
| Patch release (critical) | Within 14 days |
| Patch release (high) | Within 30 days |
| Public disclosure (coordinated) | After patch is available + 7 days user grace period |

We follow [coordinated vulnerability disclosure](https://en.wikipedia.org/wiki/Coordinated_vulnerability_disclosure). We will not take legal action against researchers who act in good faith.

## Security Design Principles

Clarity is designed with the following security-first constraints:

1. **Local-first execution**: The core agent engine runs entirely on your machine. No remote telemetry, no cloud dependency for basic operation.
2. **No forced data exfiltration**: User data (sessions, memory, API keys) never leaves the local system unless the user explicitly configures an external provider or channel.
3. **API Key hygiene**: Clarity supports `${env:VAR}` syntax for API keys to avoid plaintext storage on disk. We strongly recommend using this over hardcoded keys in config files.
4. **Tool sandboxing**: File-system tools enforce path resolution with base-directory containment (`resolve_path`). Parent-directory traversal (`../`) and deep symlinks are rejected by default.
5. **Approval gating**: High-risk tools (`shell`, `file_write`) require explicit user approval in Interactive and Plan modes. YOLO mode can be enabled only via explicit opt-in.
6. **Snapshot isolation**: The side-Git snapshot feature (Sprint 38-C+) uses a **separate bare repository** (`~/.clarity/snapshots/`) to avoid polluting or exposing the user's main project history.

## Known Security Boundaries & Caveats

| Area | Boundary | User Action |
|------|----------|-------------|
| LLM Output | **Untrusted by default**. An LLM may emit malicious shell commands or file paths. | Always review tool calls in Interactive/Plan mode before approving. |
| Environment Variables | API keys in `${env:VAR}` are resolved at runtime and may leak in process listings. | Use OS-level secret management (Windows Credential Manager, macOS Keychain, Linux keyring) where possible. |
| MCP Servers | Third-party MCP servers execute arbitrary code. Clarity validates commands against an allowlist, but servers run as the host user. | Only enable MCP servers from trusted sources. Review `mcp_config.toml` before starting. |
| Subagents | Parallel subagents share the same working directory and registry. File-write race conditions are possible. | Use read-only mode (`read_only: true`) for untrusted subagent tasks. |
| Memory Store | `clarity-memory` persists conversation extracts to local SQLite/JSONL. Data is not encrypted at rest. | Encrypt your home directory or user profile at the OS level if device theft is a concern. |
| Side-Git Snapshots | Snapshots capture the full working tree. Sensitive files (`.env`, private keys) in the working tree will be snapshotted. | Add sensitive files to `.gitignore`; the snapshot engine respects Git ignore rules where possible. |

## Supply Chain Security

- `cargo audit` is run regularly to track RUSTSEC advisories.
- Unmaintained dependencies are tracked in `Cargo.toml` comments and resolved on a rolling basis.
- Binary releases are built via GitHub Actions with reproducible workflow definitions (`.github/workflows/`).

## Security-Related Configuration

```toml
# Example: harden a Clarity agent profile
[agent]
approval_mode = "interactive"   # Never run YOLO on sensitive projects
read_only = false               # Set true for audit/dry-run scenarios
max_iterations = 20             # Limit unbounded agent loops
extract_memories = true         # Persist learnings locally (never remote)

[snapshot]
enabled = true
max_snapshots = 10              # Limit side-repo growth
```

---

## 安全策略（中文）

### 支持的版本

安全更新仅提供给当前 minor 版本及其前一个 minor 版本。

| 版本    | 支持状态 |
| ------- | -------- |
| 0.3.x   | ✅ 支持  |
| 0.2.x   | ✅ 支持  |
| < 0.2   | ❌ 不支持 |

### 报告漏洞

**首选渠道**：[GitHub Security Advisory](https://github.com/juice094/clarity/security/advisories/new)（私密，仅维护者可见）。

**备用渠道**：发送邮件至 `juice094@users.noreply.github.com`，主题格式 `[Clarity Security] <简述>`。请包含：
- 受影响版本及 commit hash（如已知）
- 复现步骤
- 潜在影响评估
- 建议修复方案（如有）

### 响应时间线

| 阶段 | 目标时间 |
|------|---------|
| 首次确认 | 48 小时内 |
| 分类与严重性评估 | 7 天内 |
| 补丁发布（Critical） | 14 天内 |
| 补丁发布（High） | 30 天内 |
| 公开披露（协调式） | 补丁可用 + 7 天用户缓冲期后 |

我们遵循[协调式漏洞披露](https://en.wikipedia.org/wiki/Coordinated_vulnerability_disclosure)原则。对于善意研究者，我们不会采取法律行动。

### 安全设计原则

1. **本地优先执行**：核心 Agent 引擎完全在本地运行，无远程遥测，基本操作不依赖云端。
2. **禁止强制数据外泄**：用户数据（会话、记忆、API key）不会离开本地系统，除非用户显式配置了外部服务商或通道。
3. **API Key 卫生**：支持 `${env:VAR}` 语法避免明文落盘。强烈建议在配置文件中使用此语法，而非硬编码密钥。
4. **工具沙箱**：文件系统工具强制执行基于工作目录的路径解析（`resolve_path`），默认拒绝父目录穿越（`../`）和深层符号链接。
5. **审批门控**：高风险工具（`shell`、`file_write`）在 Interactive 和 Plan 模式下需要用户显式审批。YOLO 模式只能通过显式 opt-in 启用。
6. **快照隔离**：Side-Git 快照功能使用**独立的 bare 仓库**（`~/.clarity/snapshots/`），避免污染或暴露用户主项目历史。

### 已知安全边界与注意事项

| 领域 | 边界 | 用户建议 |
|------|------|---------|
| LLM 输出 | **默认不可信**。LLM 可能生成恶意 shell 命令或文件路径。 | 在 Interactive/Plan 模式下审批前始终检查工具调用内容。 |
| 环境变量 | `${env:VAR}` 形式的 API key 在运行时解析，可能在进程列表中泄露。 | 尽可能使用操作系统级密钥管理（Windows Credential Manager、macOS Keychain、Linux keyring）。 |
| MCP 服务器 | 第三方 MCP 服务器执行任意代码。Clarity 通过 allowlist 校验命令，但服务器以宿主用户身份运行。 | 仅启用来自可信来源的 MCP 服务器。启动前审阅 `mcp_config.toml`。 |
| 子代理 | 并行子代理共享同一工作目录和注册表，可能出现文件写入竞态。 | 对不可信的子代理任务启用只读模式（`read_only: true`）。 |
| 记忆存储 | `clarity-memory` 将会话摘要持久化到本地 SQLite/JSONL，数据**静止态未加密**。 | 如担心设备失窃，请在操作系统层面加密用户主目录或配置文件目录。 |
| Side-Git 快照 | 快照捕获完整工作树。工作树中的敏感文件（`.env`、私钥）也会被快照。 | 将敏感文件加入 `.gitignore`；快照引擎在可能范围内尊重 Git ignore 规则。 |

### 供应链安全

- 定期运行 `cargo audit` 跟踪 RUSTSEC 安全公告。
- `Cargo.toml` 中标记了未维护依赖，按滚动计划解决。
- 二进制发布通过 GitHub Actions 构建，工作流定义可复现（`.github/workflows/`）。

### 更多安全文档

| 文档 | 说明 |
|------|------|
| [`docs/security/THREAT_MODEL.md`](docs/security/THREAT_MODEL.md) | STRIDE 威胁模型 |
| [`docs/security/risk-assessment.md`](docs/security/risk-assessment.md) | 技术风险评估 |
| [`docs/security/PRIVACY_REVIEW.md`](docs/security/PRIVACY_REVIEW.md) | 隐私整改记录 |
| [`docs/security/operations.md`](docs/security/operations.md) | 安全与运维细则 |
