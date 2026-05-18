# Agent 指引 — clarity-tools

## 构建

```bash
cargo build -p clarity-tools
```

## 测试

```bash
cargo test -p clarity-tools --lib
```

## 关键文件

- `src/lib.rs` — 入口、工具重导出、`clarity_data_dir()`
- `src/file.rs` — `FileReadTool`, `FileEditTool`, `FileWriteTool`
- `src/shell.rs` — `BashTool`, `PowerShellTool`，含超时与安全检测
- `src/search.rs` — `GlobTool`, `GrepTool`
- `src/web.rs` / `src/web_browser.rs` — `WebFetchTool`, `WebSearchTool`, `WebBrowserTool`
- `src/diff.rs` — 文本 diff 与 patch 应用
- `src/ask_user.rs` — `AskUserTool` 用户确认交互
