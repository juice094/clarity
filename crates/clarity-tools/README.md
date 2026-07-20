# clarity-tools

Tool implementations for Project Clarity.

## 职责

- **内置工具集** — 文件操作、Shell 执行、搜索、Web 浏览、通知等 15+ 工具
- **`Tool` trait** — 统一接口：`execute()` 接收 `ToolContext` 返回 `ToolResult`
- **跨平台兼容** — Windows 使用 PowerShell，Unix 使用 Bash
- **安全沙箱** — 路径脱敏、敏感文件检测、命令白名单
- **契约重导出** — `Tool`、`ToolContext`、`ToolError` 等核心类型定义在 `clarity-contract`

## 工具清单

| 类别 | 工具 |
|------|------|
| 文件 | `FileReadTool`, `FileEditTool`, `FileWriteTool` |
| Shell | `BashTool` (Unix), `PowerShellTool` (Windows) |
| 搜索 | `GlobTool`, `GrepTool` |
| Web | `WebFetchTool`, `WebSearchTool`, `WebBrowserTool` |
| 协作 | `AskUserTool`, `NotifyTool`, `ChannelSendTool` |
| 规划 | `PlanTool`, `TodoTool`, `ThinkTool` |
| 知识 | `KnowledgeSearchTool` |
| 其他 | `ComputerUseTool`, `ReadMediaFileTool`, `TeamCreateTool` |

## 测试

```bash
cargo test -p clarity-tools --lib
```
