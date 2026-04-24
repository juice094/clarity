# Clarity 项目进度快照

> 生成时间：2026-04-20  
> 用途：上下文压缩防护，供后续会话快速恢复状态  
> 项目路径：`C:\Users\22414\dev\third_party\clarity`  
> GitHub：`juice094/clarity`  

---

## 1. 基础元数据

| 项 | 值 |
|---|---|
| 版本 | v0.1.2 → v0.2.0-dev（开发中） |
| HEAD | `3ec2fa8` |
| 分支 | main |
| 测试 | `cargo test --workspace --lib` = **443 passed, 0 failed, 2 ignored** |
| Clippy | 零 warning（`-D warnings`） |
| 技术栈 | Rust 1.85, edition 2021, Tokio, Axum, ratatui, SQLite |

---

## 2. 已完成功能（按 Commit 顺序）

### Phase 1 — P0 核心补强（已完成）

| Commit | 功能 | 文件 |
|--------|------|------|
| `0c39a08` | **CapabilityToken** + **Dynamic Skill Discovery** + **Cron Scheduler** | `subagents/token.rs`, `skills/discovery.rs`, `background/cron.rs`, `tools/cron.rs` |
| `dbb04bf` | **P0-2A SessionNotes** 类型 + SQLite `session_notes` 表 | `clarity-memory/src/types.rs`, `sqlite.rs`, `store.rs`, `lib.rs` |
| `8e58697` | **P0-2B TurnMemoryExtractor** | `clarity-core/src/memory/extraction.rs` |
| `974d6e7` | **P0-2C Agent::run() 集成** extractMemories | `agent/config.rs`, `agent/mod.rs`, `agent/run.rs` |
| `1052939` | **P0-5A ComputerUseTool** | `tools/computer.rs` |

**Phase 1 功能详情**：
- **CapabilityToken**：子代理权限隔离（allowed_tools + sandbox_dir + read_only），`ToolRegistry::execute()` 拦截校验
- **Dynamic Skill Discovery**：运行时扫描 `.clarity/skills/` 和 `.claude/skills/`，`paths` frontmatter 条件匹配，SkillRegistry 热更新
- **Cron Scheduler**：`cron` crate 解析，`BackgroundTaskManager` 扩展，`ScheduleCronTool`/`ListCronTool`/`CancelCronTool`
- **extractMemories**：每轮 turn 后 `tokio::spawn` 轻量子代理提取 `SessionNotes`（current_state/errors/learnings/key_results），默认关闭
- **ComputerUseTool**：screenshot/click/type/scroll，通过 `std::process::Command` 调用 Python 桥 `computer_bridge.py`（mss/pyautogui）

---

## 3. 串行流水线（进行中）

```
✅ P0-1 动态 Skill
✅ P0-3 CapabilityToken
✅ P0-4 Cron 调度
✅ P0-2A SessionNotes 类型
✅ P0-2B TurnMemoryExtractor
✅ P0-2C Agent::run() 集成
✅ P0-5A ComputerUseTool
✅ P0-5B computer_bridge.py（Python 脚本）
⏳ P0-5C 审批标记（强制审批）
⏳ P1-6A Hook trait 定义
⏳ P1-6B HookRegistry
⏳ P1-6C Agent 集成
⏳ P1-7 P0-P3 交付分级
⏳ P1-9 Lazy Master
⏳ P1-8 Agent Teams + Mailbox
⏳ P2-10 Gateway 多标签
⏳ P2-11 Bridge 远程控制
⏳ P2-12 AutoDream
```

**执行策略**：串行子代理（每次 1 个），完成后主代理审核 → commit → 启动下一个。避免工作区冲突。

---

## 4. 关键架构决策

### 4.1 三端策略
- **TUI（ratatui）**：保留，开发者核心场景
- **Web UI（Gateway）**：升级为响应式 PWA，覆盖 PC 云 + Mobile
- **Desktop**：未来用 Tauri + Sidecar（Axum Gateway 作为 Sidecar）
- **Mobile**：不开发独立 App，走 PWA + Push Notification

### 4.2 云端部署（Kimi 云）
- claw 托盘 → **claw-server**（无 UI 守护进程）
- OS 通知 → **Webhook / Web Push / FCM-APNs**
- 本地 SQLite → 外挂卷 / S3 同步 / 远程 PG
- Computer Use → 无头浏览器（Playwright）或 VNC 桥接

### 4.3 子代理执行策略（经验教训）
- **第一批**：3 并行 → 成功但超时，编译通过
- **第二批**：3 并行 → 全部失败，工作区冲突 + target 损坏
- **当前**：**串行执行**（1 个/次），成功率 100%（P0-2A~C、P0-5A 全部成功）

---

## 5. 参考项目

| 项目 | 路径 | 用途 |
|------|------|------|
| cc-haha | `C:\Users\22414\dev\third_party\cc-haha` | Claude Code 魔改版，功能借鉴源 |

**cc-haha 关键借鉴点**：
- Memory：forked agent 提取、AutoDream 夜间整合
- Skills：动态发现、条件激活（paths frontmatter）
- Teams：TeamFile + Mailbox + Leader 集中审批
- Desktop：Tauri 2 + Sidecar 模式
- Computer Use：Python 桥（pyautogui/mss）+ MCP 封装

---

## 6. 环境信息

| 项 | 值 |
|---|---|
| OS | Windows |
| Shell | PowerShell |
| Rust | 1.85 |
| Git 用户名 | juice094 |
| Git 邮箱 | 160722440+juice094@users.noreply.github.com |

---

## 7. 快速恢复命令

```powershell
cd C:\Users\22414\dev\third_party\clarity
git log --oneline -5
cargo test --workspace --lib
cargo clippy --workspace --lib --bins --tests -- -D warnings
```

---

*本文件在每次重大进度更新时应同步更新。*
