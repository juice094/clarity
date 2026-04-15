# Clarity 实测计划

> 版本：v1.0
> 日期：2026-04-03
> 状态：待执行

---

## 1. 实测目标

验证以下已完成代码的功能在真实环境中的实际表现：

1. **TUI 真实 LLM 调用**（当前无单元测试）
2. **Gateway HTTP API** 端到端能力
3. **MCP Client** 与真实 Server 联调
4. **记忆系统** 多轮对话闭环
5. **流式响应** 长时间运行稳定性

---

## 2. 环境准备

### 2.1 基础环境

```powershell
# 检查 Rust 版本
rustc --version  # >= 1.75
cargo --version

# 检查 Node.js (MCP Server 需要)
node --version   # >= 18
npm --version
```

### 2.2 LLM API 配置（二选一）

**方案 A: Kimi Code**
```powershell
$env:ANTHROPIC_BASE_URL="https://api.kimi.com/coding/"
$env:ANTHROPIC_AUTH_TOKEN="your-kimi-api-key"
```

**方案 B: OpenAI 兼容**
```powershell
$env:OPENAI_BASE_URL="https://api.openai.com/v1"
$env:OPENAI_API_KEY="your-openai-key"
# 或自定义端点
$env:OPENAI_BASE_URL="http://localhost:11434/v1"  # Ollama
```

### 2.3 MCP Server 准备

```powershell
# 安装参考 MCP Server (文件系统)
npm install -g @modelcontextprotocol/server-filesystem

# 或直接使用 npx (推荐)
# npx -y @modelcontextprotocol/server-filesystem C:\Users\<user>\Desktop
```

---

## 3. 实测项目

### 测试 1: TUI 基础功能

**目的**: 验证 TUI 能正常启动并进行基本交互

```powershell
cd C:\Users\<user>\Desktop\clarity

# 设置环境变量
$env:ANTHROPIC_BASE_URL="https://api.kimi.com/coding/"
$env:ANTHROPIC_AUTH_TOKEN="your-key"

# 运行 TUI
cargo run -p clarity-tui
```

**测试步骤**:
1. [ ] TUI 启动无 panic
2. [ ] 界面渲染正常（输入框、聊天区域、状态栏）
3. [ ] 能输入文本并发送
4. [ ] 状态栏显示模型名称

**预期结果**: TUI 正常显示，无渲染错误

---

### 测试 2: TUI 真实 LLM 对话

**目的**: 验证 TUI 能调用真实 LLM 并返回响应

```powershell
cargo run -p clarity-tui
```

**测试步骤**:
1. [ ] 输入简单问候："你好"
2. [ ] 观察"Generating..."指示器
3. [ ] 等待响应出现在聊天区域
4. [ ] 验证响应内容与输入相关

**检查点**:
- [ ] 响应时间在合理范围内（< 30s）
- [ ] 响应内容不是硬编码/模拟数据
- [ ] 无超时或连接错误

---

### 测试 3: TUI 流式响应

**目的**: 验证 SSE 流式响应正常工作

```powershell
cargo run -p clarity-tui
```

**测试步骤**:
1. [ ] 输入请求："请写一段 500 字的文章介绍 Rust"
2. [ ] 观察文本是否逐字/逐段出现
3. [ ] 检查光标位置和滚动行为

**检查点**:
- [ ] 文本增量出现（非一次性显示）
- [ ] 流式过程中 UI 保持响应
- [ ] 流式结束后状态正确重置

---

### 测试 4: TUI 工具调用

**目的**: 验证 Agent 能通过 TUI 执行工具

```powershell
cargo run -p clarity-tui
```

**测试步骤**:
1. [ ] 输入："列出当前目录的所有文件"
2. [ ] 观察 Agent 是否调用 glob 工具
3. [ ] 验证返回结果与实际目录一致

**检查点**:
- [ ] Agent 正确识别需要使用工具
- [ ] 工具执行结果正确
- [ ] 错误处理（如无权限） graceful

---

### 测试 5: Gateway HTTP API

**目的**: 验证 Gateway 能处理真实 HTTP 请求

```powershell
# 终端 1: 启动 Gateway
cargo run -p clarity-gateway
# 监听 http://127.0.0.1:3000
```

```powershell
# 终端 2: 发送测试请求
$headers = @{
    "Content-Type" = "application/json"
}

$body = @{
    model = "kimi-k2"
    messages = @(
        @{ role = "user"; content = "你好，请介绍一下自己" }
    )
    stream = $false
} | ConvertTo-Json -Depth 3

Invoke-RestMethod -Uri "http://127.0.0.1:3000/v1/chat/completions" -Method Post -Headers $headers -Body $body
```

**检查点**:
- [ ] HTTP 200 响应
- [ ] 返回 JSON 格式正确
- [ ] `choices[0].message.content` 包含 LLM 响应
- [ ] `usage` 字段包含 token 统计

---

### 测试 6: Gateway Admin API

**目的**: 验证管理接口返回真实数据

```powershell
# 健康检查
Invoke-RestMethod -Uri "http://127.0.0.1:3000/health"

# 统计信息
Invoke-RestMethod -Uri "http://127.0.0.1:3000/admin/stats"

# 工具列表
Invoke-RestMethod -Uri "http://127.0.0.1:3000/admin/tools"
```

**检查点**:
- [ ] `/health` 返回 healthy 状态
- [ ] `/admin/stats` 返回活跃 session 数、请求数
- [ ] `/admin/tools` 返回 8 个工具的真实信息

---

### 测试 7: MCP Client 联调

**目的**: 验证 MCP Client 能连接真实 Server

```powershell
# 使用示例程序测试 MCP
cargo run --example mcp_demo
# 或
cargo run --example mcp_filesystem_demo
```

**手动测试步骤**:
1. [ ] 启动 MCP filesystem server
2. [ ] 通过 MCP Client 连接
3. [ ] 调用 `tools/list` 获取工具列表
4. [ ] 调用 `tools/call` 执行文件读取

**检查点**:
- [ ] 成功建立 stdio 连接
- [ ] JSON-RPC 2.0 握手成功
- [ ] 能列出 MCP Server 提供的工具
- [ ] 能调用 MCP 工具并获取结果

---

### 测试 8: 记忆系统闭环

**目的**: 验证记忆在多轮对话中工作

```powershell
cargo run -p clarity-tui
```

**测试步骤**:
1. [ ] 第一轮："请记住我叫张三"
2. [ ] 第二轮："我叫什么名字？"
3. [ ] 验证 Agent 能回答"张三"
4. [ ] 检查 SQLite DB 文件是否生成

**数据库验证**:
```powershell
# 检查记忆数据库
cd C:\Users\<user>\AppData\Local\Clarity  # 或其他数据目录
sqlite3 memories.db "SELECT * FROM memories LIMIT 5;"
```

**检查点**:
- [ ] 记忆被正确存储到 SQLite
- [ ] 后续对话能检索到相关记忆
- [ ] memory_ticker 触发编译流水线

---

### 测试 9: 长对话压力测试

**目的**: 验证系统在长对话下的稳定性

```powershell
cargo run -p clarity-tui
```

**测试步骤**:
1. [ ] 进行 20+ 轮对话
2. [ ] 每轮包含工具调用和流式响应
3. [ ] 观察内存使用（Task Manager）

**检查点**:
- [ ] 无内存泄漏（内存使用稳定）
- [ ] 响应时间不随对话长度显著增加
- [ ] 无 panic 或崩溃

---

### 测试 10: 边界情况

**目的**: 验证错误处理和边界情况

| 测试场景 | 输入 | 预期行为 |
|----------|------|----------|
| 超长输入 | 10,000 字符 | 正确处理或截断 |
| 特殊字符 | emoji、中文、代码块 | 正确渲染 |
| 网络断开 | 关闭网络 | graceful 错误提示 |
| 无效 API Key | 设置错误 key | 清晰错误信息 |
| 大文件读取 | 读取 10MB 文件 | 流式处理或拒绝 |

---

## 4. 测试结果记录模板

```markdown
### 测试 X: [名称]
- 执行时间: YYYY-MM-DD HH:MM
- 执行者: [姓名/ID]
- 环境: Windows 11 / Rust 1.XX
- LLM: Kimi Code / OpenAI / Ollama

**结果**: ✅ 通过 / ❌ 失败 / ⚠️ 部分通过

**详细记录**:
- [步骤 1 结果]
- [步骤 2 结果]
- ...

**问题记录**:
- [如有问题，记录现象和复现步骤]

**截图/日志**:
- [附上相关截图或日志片段]
```

---

## 5. 测试完成标准

### 5.1 通过标准

- [ ] 测试 1-4 (TUI 基础) 全部通过
- [ ] 测试 5-6 (Gateway) 全部通过
- [ ] 测试 7 (MCP) 至少基础连接通过
- [ ] 测试 8 (记忆) 数据持久化验证通过
- [ ] 测试 9 (压力) 无明显内存泄漏
- [ ] 测试 10 (边界) 错误处理 graceful

### 5.2 测试报告

测试完成后需生成报告，包含：

1. 执行的测试列表和结果
2. 发现的问题和严重级别
3. 修复建议
4. 是否具备进入 Phase 4 的条件

---

## 6. 附录

### 6.1 快速命令参考

```powershell
# 构建所有 crate
cargo build --workspace --release

# 运行所有测试
cargo test --workspace

# 运行特定 crate
cargo run -p clarity-tui
cargo run -p clarity-gateway

# 日志级别
$env:RUST_LOG="debug"  # trace, debug, info, warn, error
cargo run -p clarity-tui
```

### 6.2 常见问题

**Q: TUI 启动后显示乱码**
A: 确保使用支持 Unicode 的终端（Windows Terminal 推荐）

**Q: Gateway 端口被占用**
A: 修改 `clarity-gateway/src/main.rs` 中的端口配置

**Q: MCP Server 无法启动**
A: 确保 Node.js 版本 >= 18，尝试全局安装而非 npx

---

**待测试标记**: 🔴 未开始 / 🟡 进行中 / 🟢 已完成
