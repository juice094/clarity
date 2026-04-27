# Clarity 子代理串并行执行计划

> 版本：v1.0 | 日期：2026-04-25 | 基于依赖关系的子代理编排 + Git 安全机制

---

## 一、依赖关系总图

```
                    ┌─────────────────────────────────────────────┐
                    │  Phase 1: Session persistence ✅            │
                    │  (Subagent-G ✅ 已完成)                      │
                    │  分支: subagent/session-persist-2026-0425    │
                    └──────────────────┬──────────────────────────┘
                                       │
                    ┌──────────────────▼──────────────────────────┐
                    │  里程碑: AppState 重构完成 + SessionStore    │
                    │  合并到 main 后解锁 Phase 2                  │
                    └──────────────────┬──────────────────────────┘
                                       │
        ┌──────────────────────────────┼──────────────────────────────┐
        │                              │                              │
        ▼                              ▼                              ▼
┌─────────────────────┐    ┌─────────────────────┐    ┌─────────────────────┐
│ Phase 2A 🔄         │    │ Phase 2B 🔄         │    │ Phase 2C (可选)     │
│ TaskPanel 真实对接  │    │ Diff view           │    │ Headless mode       │
│ (Subagent-I)        │    │ (Subagent-H)        │    │ (Subagent-J)        │
│                     │    │                     │    │                     │
│ 依赖: AppState 稳定 │    │ 依赖: 无            │    │ 依赖: 无            │
│ 阻塞: Phase 3A      │    │ 阻塞: 无            │    │ 阻塞: 无            │
└─────────────────────┘    └─────────────────────┘    └─────────────────────┘
        │                              │                              │
        └──────────────────────────────┼──────────────────────────────┘
                                       │
                    ┌──────────────────▼──────────────────────────┐
                    │  里程碑: Phase 2 全部合并                    │
                    └──────────────────┬──────────────────────────┘
                                       │
        ┌──────────────────────────────┼──────────────────────────────┐
        │                              │                              │
        ▼                              ▼                              ▼
┌─────────────────────┐    ┌─────────────────────┐    ┌─────────────────────┐
│ Phase 3A            │    │ Phase 3B            │    │ Phase 3C            │
│ Computer Use        │    │ 设置面板增强        │    │ Gateway 增强        │
│ 调研 & 实现         │    │ (模型热切换等)      │    │ (Web IDE 完善)      │
│ (Subagent-K)        │    │                     │    │                     │
│                     │    │                     │    │                     │
│ 依赖: 无            │    │ 依赖: 无            │    │ 依赖: 无            │
│ 阻塞: Phase 4       │    │ 阻塞: 无            │    │ 阻塞: 无            │
└─────────────────────┘    └─────────────────────┘    └─────────────────────┘
                                       │
                    ┌──────────────────▼──────────────────────────┐
                    │  里程碑: Phase 3 全部合并                    │
                    └──────────────────┬──────────────────────────┘
                                       │
                                       ▼
                    ┌─────────────────────────────────────────────┐
                    │  Phase 4: 长期规划                            │
                    │  • LSP integration (Subagent-L)              │
                    │  • Mobile app (Subagent-M)                   │
                    │  • Voice / 语音交互                          │
                    │  • 插件市场 / WASM 扩展                      │
                    └─────────────────────────────────────────────┘
```

---

## 二、Phase 详细编排

### Phase 1 ✅ — Session persistence

| 属性 | 值 |
|------|-----|
| **模式** | 串行（单轨） |
| **原因** | 所有涉及 AppState 的后续任务都依赖其完成 |
| **子代理** | Subagent-G |
| **分支** | `subagent/session-persist-2026-0425` |
| **范围** | `commands/session.rs` + `lib.rs` + `App.tsx` + `Cargo.toml` |
| **验收标准** | 刷新页面后会话不丢失；`cargo test --workspace --lib` 全绿；clippy zero warning |
| **实际提交** | `95bf6fb` |

**为什么串行？**
- Session persistence 需要扩展 `AppState`（引入 `SessionStore`）
- TaskPanel 真实对接也需要扩展 `AppState`（引入 `BackgroundTaskManager`）
- 两者同时修改 `AppState` 会导致合并冲突和架构混乱
- 先完成 Session persistence，确立 AppState 扩展模式，后续任务直接复用

---

### Phase 2 ✅ — 双轨并行（已完成）

#### 2A: TaskPanel 真实对接

| 属性 | 值 |
|------|-----|
| **模式** | 并行 |
| **子代理** | Subagent-I |
| **分支** | `subagent/task-real-2026-0425` |
| **范围** | `commands/task.rs` + `AppState` + `lib.rs` |
| **关键改动** | `AppState` 增加 `BackgroundTaskManager`；`list_tasks`/`cancel_task` 对接真实数据 |

#### 2B: Diff view

**实际提交**: `66aa2d3`

| 属性 | 值 |
|------|-----|
| **模式** | 并行 |
| **子代理** | Subagent-H |
| **分支** | `subagent/diff-view-2026-0425` |
| **范围** | 新建 `DiffPanel.tsx` + `App.tsx` 集成 + CSS |
| **参考** | TUI 已有 `diff.rs` + `diff_popup.rs`（`similar` crate） |
| **交互** | Agent 执行 `file_edit` 后在聊天区显示 diff 预览 |

#### 2C: Headless mode（可选，可延至 Phase 3）

| 属性 | 值 |
|------|-----|
| **模式** | 并行（与 2A/2B 无依赖） |
| **子代理** | Subagent-J |
| **分支** | `subagent/headless-2026-04XX` |
| **范围** | `clarity-tui` CLI 参数解析 + `--print` 输出模式 |
| **参考** | cc-haha 的 `--print` 模式 |

---

### Phase 3（Phase 2 合并后）— 三轨并行

#### 3A: Computer Use 调研 & 实现

| 属性 | 值 |
|------|-----|
| **模式** | 并行 |
| **子代理** | Subagent-K |
| **分支** | `subagent/computer-use-2026-04XX` |
| **范围** | 截屏 + 鼠标 + 键盘控制 |
| **调研参考** | cc-haha 的 Computer Use 实现（`docs/features/computer-use.md`） |
| **技术选型** | Windows: `windows` crate API；macOS: `core-graphics`；或跨平台 `enigo` |

#### 3B: 设置面板增强

| 属性 | 值 |
|------|-----|
| **模式** | 并行 |
| **子代理** | Subagent-L |
| **分支** | `subagent/settings-enhance-2026-04XX` |
| **范围** | 模型/provider 热切换（无需重建 Agent）；API Key 安全存储 |

#### 3C: Gateway Web IDE 完善

| 属性 | 值 |
|------|-----|
| **模式** | 并行 |
| **子代理** | 主会话直接开发 |
| **范围** | `chat.html` 会话管理；文件树浏览器；Monaco 编辑器增强 |

---

### Phase 4（长期规划）

| 功能 | 优先级 | 依赖 | 备注 |
|------|--------|------|------|
| LSP integration | P3 | 无 | 语言服务器协议，参考 rust-analyzer |
| Mobile app (Tauri 2) | P3 | Tauri mobile 配置 | iOS/Android 适配 |
| Voice / 语音交互 | P3 | 无 | Whisper + TTS 集成 |
| 插件市场 / WASM | P4 | 无 | 长期架构演进 |

---

## 三、Git 安全机制（强制规范）

### 3.1 分支隔离

```
main                           ← 永远可编译、可测试
  └── subagent/<feature>-YYYY-MM-DD  ← 子代理唯一工作区
```

- **每个子代理必须在独立分支工作**
- **分支命名**：`subagent/<kebab-case-feature>-YYYY-MM-DD`
- **禁止在 main 上直接开发**
- **禁止子代理切换/操作其他子代理的分支**

### 3.2 启动检查清单（主会话强制执行）

```bash
# 1. 创建分支前确认 main 最新
git checkout main && git pull origin main

# 2. 创建独立分支
git checkout -b subagent/<feature>-YYYY-MM-DD

# 3. 子代理执行 git status 确认
git status  # 应为空或仅有预期文件
```

### 3.3 验收门槛（合并前必须通过）

```bash
# Rust 侧
cargo test --workspace --lib        # 必须: 全绿通过
cargo clippy --workspace --lib -- -D warnings  # 必须: zero warning
cargo audit                          # 允许: 已知 Tauri 上游警告

# 前端侧
cd crates/clarity-tauri/frontend && npm run build  # 必须: 通过
```

### 3.4 合并策略

```bash
# 主会话执行（子代理禁止自行 merge）
git checkout main
git merge --no-ff subagent/<feature>-YYYY-MM-DD -m "feat(...): ..."
git push origin main
```

- 使用 `--no-ff` 保留分支历史
- 主会话负责解决合并冲突
- 合并后立即运行完整测试验证

### 3.5 意外提交应急处理

**场景 A：子代理混入未实现引用**（如 Subagent-E 混入 `commands::file`）
```bash
# 主会话发现后立即修复
git checkout main
git commit --amend  # 或新建 fix commit
git push origin main
```

**场景 B：子代理提交额外文件**（dev 日志、skill 文件）
```bash
# 清理
git rm -f <unwanted-files>
git commit -m "chore: remove accidentally committed files"
# 更新 .gitignore 防止复发
```

**场景 C：子代理在错误分支上工作**
```bash
# 抢救
git stash push -u -m "rescue: <feature>"
git checkout correct-branch
git stash pop
```

### 3.6 历史教训（已发生）

| 事件 | 原因 | 修复措施 |
|------|------|----------|
| Subagent-B v1 污染 taskpanel 分支 | 未 checkout 到正确分支 | kill 重建 v2 + 强制启动检查 |
| Subagent-F 文件出现在 main | stash/分支切换混乱 | stash rescue + 重新 checkout |
| Subagent-E 混入 file commands | `lib.rs` 上有残留修改 | 主会话修复 commit `351c020` |

---

## 四、子代理任务规范模板

每个子代理启动时必须接收以下完整上下文：

```
## 任务：<功能名>

**目标**：<一句话描述>
**工作分支**：`subagent/<feature>-YYYY-MM-DD`（已 checkout）
**项目路径**：`C:\Users\22414\dev\third_party\clarity`

### 修改清单
1. `<文件路径>` — <变更描述>
2. ...

### 验收标准
1. `cargo test --workspace --lib` 通过
2. `cargo clippy --workspace --lib -- -D warnings` 通过
3. `npm run build` 通过
4. 提交在指定分支上
5. 提交前 `git status` 确认无额外文件

### 重要约束
- 不要修改测试文件
- 不要提交 dev 日志 / skill 文件
- 保持现有代码风格
```

---

## 五、当前状态看板

| Phase | 任务 | 子代理 | 分支 | 状态 | 阻塞 |
|-------|------|--------|------|------|------|
| **Phase 1** | Session persistence | Subagent-G | `subagent/session-persist-2026-0425` | ✅ Done (`95bf6fb`) | — |
| **Phase 2A** | **TaskPanel 真实对接** | **Subagent-I** | **`subagent/task-real-2026-0425`** | ✅ **Done** (`6b8ffec`) | Phase 1 |
| **Phase 2B** | **Diff view** | **Subagent-H** | **`subagent/diff-view-2026-0425`** | ✅ **Done** (`66aa2d3`) | Phase 1 |
| Phase 2C | Headless mode | — | — | ⏸️ Pending | — |
| Phase 3A | Computer Use | — | — | ⏸️ Pending | Phase 2 |
| Phase 3B | 设置面板增强 | — | — | ⏸️ Pending | — |
| Phase 3C | Gateway Web IDE | — | — | ⏸️ Pending | — |
| Phase 4 | LSP / Mobile | — | — | 📋 Backlog | Phase 3 |

---

**更新记录**

| 日期 | 版本 | 变更 |
|------|------|------|
| 2026-04-25 | v1.0 | 初始编排，基于 Sprint 1-2 完成后的依赖梳理 |
