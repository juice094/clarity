# Clarity Threat Model

> 版本：v0.3.1+ | 关联：[`ARCHITECTURE.md`](ARCHITECTURE.md) · [`AGENTS.md`](../AGENTS.md)

---

## 1. 信任边界

- **User OS**（操作系统用户空间）← 边界 → **Clarity Process**（单进程多 crate）
- **Clarity Process** ← 边界 → **MCP Server Process**（外部子进程 / HTTP 服务）
- **Clarity Process** ← 边界 → **Cloud LLM API**（OpenAI / Anthropic / 自托管 HTTP）
- **Browser/Network** ← 边界 → **clarity-gateway**（HTTP / WebSocket）
- **Local GGUF** ← 边界 → **clarity-core**（用户提供的模型文件）

---

## 2. 资产清单

| 资产 | 存储位置 | 敏感等级 |
|------|---------|---------|
| LLM API keys / tokens | `mcp.json`, 环境变量, 内存 | Critical |
| Session 数据 / 对话历史 | `clarity-memory` SQLite | High |
| 本地文件系统内容 | `resolve_path()` 限定的 CWD | High |
| Tool 执行权限 | `ToolRegistry` + `requires_approval()` | High |
| 用户 prompt / 系统指令 | 内存 / wire 传输 | Medium |
| MCP server 命令行 | `mcp.json` 配置 | High |

---

## 3. STRIDE 分析

### 3.1 Spoofing（身份伪造）

| 威胁 | 组件 | 风险等级 | 缓解措施 | 状态 |
|------|------|---------|---------|------|
| Gateway admin port 身份伪造 | `clarity-gateway` | High | Admin port (18800) 绑定 `127.0.0.1` + Bearer token 校验 | ⚠️ 部分 |
| WebSocket 消息伪造 | `clarity-gateway` | Medium | Session ID 随机生成；连接与 session 绑定 | ⚠️ 部分 |
| OS notification 伪造 | `clarity-claw` | Low | 依赖 OS 通知中心源验证；Clarity 不校验通知签名 | ❌ 未处理 |

### 3.2 Tampering（篡改）

| 威胁 | 组件 | 风险等级 | 缓解措施 | 状态 |
|------|------|---------|---------|------|
| MCP command injection | `clarity-mcp` | High | `validate_mcp_command()` 拒绝 metacharacters、相对路径、不存在的绝对路径 | ⚠️ 部分 |
| Path traversal（文件路径篡改） | `clarity-core` / `clarity-gateway` | High | `resolve_path()` 强制 CWD 前缀；`sanitize_path()` 在 `canonicalize()` 后再次校验 | ⚠️ 部分 |
| LLM prompt injection | `clarity-core` | Medium | `scrub_credentials()` 减少侧信道；无结构化 prompt 边界或输出 schema 锁定 | ❌ 未处理 |
| SSE reconnection 流篡改 | `clarity-mcp` | Low | SSE transport 使用 HTTP；localhost 场景下无 TLS pinning | ❌ 未处理 |

### 3.3 Repudiation（抵赖）

| 威胁 | 组件 | 风险等级 | 缓解措施 | 状态 |
|------|------|---------|---------|------|
| Tool execution 抵赖 | `clarity-core` | Low | `ApprovalRecord` 以 JSON 持久化到 SQLite（tags: `["approval", "record"]`） | ⚠️ 部分 |
| Subagent action 抵赖 | `clarity-core` | Low | Session store 记录子代理调用链；无密码学签名 | ⚠️ 部分 |

### 3.4 Information Disclosure（信息泄露）

| 威胁 | 组件 | 风险等级 | 缓解措施 | 状态 |
|------|------|---------|---------|------|
| Credential leakage in logs | `clarity-core` / `clarity-mcp` | Medium | MCP 结果：`scrub_credentials()` 脱敏；tracing 输出：`RedactingWriter` 在 subscriber 层统一脱敏 | ✅ 已缓解 |
| Sensitive file exfiltration via tools | `clarity-core` | High | 自动检测 `.env`、SSH keys、kubeconfig 等敏感文件；`requires_approval()` 强制人工审批 | ⚠️ 部分 |
| Local GGUF model supply chain | `clarity-core` | Medium | 用户自行提供模型路径；无签名验证或哈希校验 | ❌ 未处理 |

### 3.5 Denial of Service（拒绝服务）

| 威胁 | 组件 | 风险等级 | 缓解措施 | 状态 |
|------|------|---------|---------|------|
| Subagent budget exhaustion | `clarity-core` | Medium | Session 级 token budget 追踪；超限告警，但无硬中断机制 | ⚠️ 部分 |
| Background task resource exhaustion | `clarity-core` | Medium | `CancellationToken` 级联取消；无 cgroups / rlimit 硬限制 | ⚠️ 部分 |
| SSE reconnection storm | `clarity-mcp` | Low | 指数退避重连；未设置最大重试次数上限 | ⚠️ 部分 |

### 3.6 Elevation of Privilege（权限提升）

| 威胁 | 组件 | 风险等级 | 缓解措施 | 状态 |
|------|------|---------|---------|------|
| Path traversal → 任意文件读取 | `clarity-core` / `clarity-gateway` | High | `resolve_path()` + `sanitize_path()` 双重 CWD 校验；TOCTOU 未完全消除 | ⚠️ 部分 |
| MCP command injection → 任意代码执行 | `clarity-mcp` | High | `validate_mcp_command()` 限制命令路径；参数注入风险仍存在 | ⚠️ 部分 |
| Gateway admin bypass → admin API 访问 | `clarity-gateway` | High | 本地绑定 + Bearer token；缺少 mTLS / 请求签名 | ⚠️ 部分 |
| LLM prompt injection → tool 滥用 | `clarity-core` | Medium | `requires_approval()` 对 ComputerUse / WebBrowser 等强制审批 | ⚠️ 部分 |

---

## 4. 攻击树摘要

1. **本地攻击者 → Gateway admin bypass → Session 劫持 → 敏感文件读取**
   - 利用 admin port 仅本地绑定但 token 可能通过进程内存泄露，接管 session 后通过 tool call 读取敏感文件。
2. **恶意 MCP server → Command injection → 子进程代码执行 → Credential 窃取**
   - 若 `validate_mcp_command()` 被绕过或参数注入成功，攻击者控制 MCP server 子进程并读取 Clarity 内存中的 API key。
3. **恶意网页 → DNS rebinding → Admin port 访问 → Tool 批量执行**
   - 浏览器端脚本通过 DNS rebinding 访问 `127.0.0.1:18800`，伪造审批请求触发批量 tool execution。
4. **被篡改的 LLM 响应 → Prompt injection → 诱导自动审批 → 数据外泄**
   - 恶意工具返回构造数据，经 LLM 上下文注入诱导 agent 发起未经充分审批的文件读取请求。

---

## 5. 安全测试矩阵

| 威胁 | 对应测试 / 校验点 |
|------|------------------|
| MCP command injection | `validate_mcp_command()` 单元测试（拒绝 `;`, `\|`、相对路径、不存在绝对路径） |
| Path traversal | `resolve_path()` 边界测试；`sanitize_path()` CWD 前缀断言 |
| Credential leakage | `scrub_credentials()` 集成测试（API key / token / password 脱敏）；`RedactingWriter` 单元测试（跨 write 边界脱敏） |
| Sensitive file exfiltration | `requires_approval()` 对敏感路径的自动拦截测试 |
| Gateway admin bypass | Gateway 启动测试（端口绑定 `127.0.0.1`）；Bearer token 拒绝非法请求 |
| Subagent budget exhaustion | `test_budget_day_limit` 等 token budget 超限测试 |
| Background task资源耗尽 | `CancellationToken` 级联取消测试（`ParallelExecutor::execute`） |
| WebSocket 消息伪造 | Session store 随机 ID 生成与绑定测试 |

---

## 6. 未解决风险与路线图

| 风险 | 计划版本 | 缓解方向 |
|------|---------|---------|
| LLM prompt injection | v0.4.x | 引入结构化输出约束（JSON schema）与 prompt 边界符 |
| Local GGUF supply chain | v0.4.x | 模型文件 SHA-256 校验与用户确认提示 |
| SSE reconnection hijacking | Backlog | SSE over TLS 强制或本地 Unix domain socket 替代 |
| OS notification spoofing | Backlog | 若通知带操作按钮，增加交互令牌校验（依赖 OS API 支持） |
| Log 层 credential 脱敏 | ✅ v0.3.x | 在 tracing subscriber 层统一应用 `RedactingWriter`（`clarity_core::logging`） |
| Admin port mTLS | Backlog | 127.0.0.1 场景下评估自签名证书必要性 |
