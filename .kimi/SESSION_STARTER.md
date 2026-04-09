# Clarity 快速启动指南

## 1. 项目状态 (一句话)
Clarity 是 Rust AI Agent 框架，**279 测试通过**，新增 MCP/Skill/通知/Worker 模块。

## 2. 关键命令
```bash
cd Desktop/clarity
cargo test --workspace --lib  # 279 测试
cargo check --workspace       # 0 错误
```

## 3. 对照组
- **Kimi CLI**: `Desktop/kimi-cli-main/`
- **用途**: 代码参考源，稳定对照组

## 4. 文件映射 (Clarity → Kimi CLI)
```
mcp/enhanced.rs       →  acp/mcp.py
skill/                →  skill/
notifications/        →  notifications/
background/worker.rs  →  background/worker.py
tools/policy.rs       →  subagents/models.py
```

## 5. 上下文清理
- 读取 `PROJECT_STATUS.md` 获取完整状态
- 不要重复读取已稳定的模块代码
- 新增功能时对照 Kimi CLI 实现

## 6. 禁止事项
- ❌ 不修改现有测试
- ❌ 不破坏编译通过的代码
- ❌ 不添加 unsafe
- ❌ 不改变架构设计
