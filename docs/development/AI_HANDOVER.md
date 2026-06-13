---
title: AI 实例交接手册 · Clarity
category: Handover
date: 2026-05-16
tags: [handover]
---

# AI 实例交接手册 · Clarity

> 用途：新 AI 会话启动时快速恢复项目上下文。
> 协议版本：V3.1-EP-O
> 最后更新：2026-05-03

---

## 一、项目速览

**定位**：集群协作原语的单机验证运行时（非本地聊天工具）。
**技术栈**：Rust workspace, egui 0.31, tokio, eframe, axum 0.7, ratatui 0.24
**当前分支**：`phase2/protocol-pilot` @ `01990446`

```text
Workspace 结构
├── crates/clarity-core      # Agent 引擎 + LLM 运行时 + 审批系统
├── crates/clarity-egui      # 主力 GUI（egui + eframe）★ 当前主战场
├── crates/clarity-tui       # 终端 UI（ratatui）
├── crates/clarity-gateway   # HTTP API + WebSocket + MCP 网关
├── crates/clarity-memory    # SQLite + BM25 + Cosine 向量存储
├── crates/clarity-wire      # 事件总线（跨 crate 消息通道）
└── crates/clarity-claw      # Headless CLI（归档维护）
```

---

## 二、环境准备

```powershell
# 工作目录
cd C:\Users\22414\dev\third_party\clarity

# 验证基线（任何修改前必须执行）
cargo test --workspace --lib        # 期望：438 passed, 0 failed, 6 ignored
cargo check --workspace             # 期望：零错误
cargo clippy --workspace --lib --tests -- -D warnings  # 期望：零 warning

# 运行 egui 桌面端
cargo run -p clarity-egui

# 运行 TUI
cargo run -p clarity-tui
```

### 环境变量（按需设置）

```powershell
$env:KIMI_CODE_API_KEY="sk-kimi-..."        # Kimi Code（编程 plan）
$env:KIMI_API_KEY="sk-..."                  # Moonshot Open Platform
$env:ANTHROPIC_AUTH_TOKEN="..."             # Claude
$env:DEEPSEEK_API_KEY="..."                 # DeepSeek
$env:OPENAI_API_KEY="..."                   # OpenAI
$env:CLARITY_LOCAL_MODEL_PATH="C:\path\to\model.gguf"
```

---

## 三、关键文件地图

### 3.1 egui 前端（当前主战场）

| 文件 | 职责 |
|------|------|
| `crates/clarity-egui/src/main.rs` | 应用入口 + 自定义标题栏 + 窗口 resize 手柄 |
| `crates/clarity-egui/src/app.rs` | `App` 结构体定义 + `update()` 主循环 + 面板分发 |
| `crates/clarity-egui/src/app_logic.rs` | 业务逻辑：new_session/save_current_session/switch_category |
| `crates/clarity-egui/src/app_state.rs` | `AppState`：LLM 管理 + Session 管理 + 工具注册 + 任务管理 |
| `crates/clarity-egui/src/ui/render.rs` | 主渲染分发器（`render_main_ui`）— 三栏布局总控 |
| `crates/clarity-egui/src/panels/chat/mod.rs` | CentralPanel：header + message_list + input + preview + plan |
| `crates/clarity-egui/src/panels/chat/header.rs` | 浏览器式 tabs + 右侧状态栏（Online/Busy + Token 用量 + Settings） |
| `crates/clarity-egui/src/panels/chat/message_list.rs` | 消息滚动区 + AB 混合气泡策略 |
| `crates/clarity-egui/src/panels/chat/input.rs` | 底部输入栏 + attachment chips + send button |
| `crates/clarity-egui/src/panels/sidebar.rs` | 左侧 Sidebar：分类导航 + 可折叠 Tools |
| `crates/clarity-egui/src/panels/workspace.rs` | 右侧 Workspace：文件树 + 预览 |
| `crates/clarity-egui/src/components/settings/` | Settings 面板（provider/interface/about） |
| `crates/clarity-egui/src/theme.rs` | 设计系统：OLED Black 主题 + 语义化 token |
| `crates/clarity-egui/src/services/` | 业务服务：agent_runner/gateway_poller/task_service |

### 3.2 共享类型

| 文件 | 职责 |
|------|------|
| `crates/clarity-egui/src/ui/types.rs` | `AgentStatus`, `PreviewItem`, `ToastLevel` 等 UI 类型 |
| `crates/clarity-egui/src/stores/` | Zustand-style Store：SessionStore/ChatStore/SettingsStore/UiStore... |
| `crates/clarity-core/src/types.rs` | 跨 crate 核心类型：`Message`, `ToolCall`, `Plan`, `PlanStep` |

### 3.3 配置与持久化

| 路径 | 内容 |
|------|------|
| `~/.config/clarity/gui-settings.json` | egui 设置（provider/model/window_size/content_max_width） |
| `~/.config/clarity/providers/*.toml` | 自定义 Provider 配置（builtin=false） |
| `~/.config/clarity/models.toml` | ModelRegistry 动态 provider 列表 |
| `~/.config/clarity/sessions/*.json` | Session 持久化 |
| `~/.config/clarity/skills/` | Skill 自动发现目录 |

---

## 四、当前活跃问题（精简版）

| 问题 | 位置 | 状态 | 备注 |
|------|------|------|------|
| Provider list 仅显示 2 项 | `provider_tab.rs` | 🔍 待诊断 | 用户环境仅显示 Local/OpenAI；builtin providers（5个）未完整加载 |
| `builtin` flag 编辑后丢失 | `ProviderRegistry` | ⚠️ 已知 | 编辑 builtin provider 保存为 custom TOML（`builtin=false`），reload 后变为可删除 |
| `settings_active_tab: u8` | `SettingsStore` | ⚠️ 类型债 | 应使用 `SettingsTab` enum（已定义但未使用） |
| Dead code 未集成 | `widgets/` | ⚠️ 技术债 | `card.rs`, `badge.rs`, `settings_row.rs`, `toggle.rs` 及 `render_toolbar` 已写但未接入 UI |
| `toolbar.rs` 残留 | `panels/toolbar.rs` | ⚠️ 编译 warning | 不影响功能，待清理 |
| 响应式自动收缩 | 全局 | ⏸️ 遗留 | 无 `CHAT_MIN_WIDTH` 自动折叠 sidebar/workspace 逻辑 |

> 完整问题清单见 [`AGENTS.md`](../../crates/clarity-wire/AGENTS.md) Known Issues 章节。

---

## 五、常见任务执行模板

### 5.1 修改 UI 布局

```
1. 定位目标面板 → `crates/clarity-egui/src/panels/{name}.rs`
2. 修改前执行 `cargo test --workspace --lib` 确认基线
3. 修改后执行 `cargo check` 快速验证编译
4. UI 回归测试：手动运行 `cargo run -p clarity-egui` 验证视觉效果
5. 提交前再次执行完整测试套件
```

### 5.2 新增 Provider 支持

```
1. ① crates/clarity-core/src/llm/model_registry.rs
   → ProtocolType match 分支（如需要新协议）
2. ② crates/clarity-core/src/view_models/settings.rs
   → get_available_models() 硬编码 fallback 补充 provider + model
3. ③ crates/clarity-core/src/llm/model_registry.rs
   → build_provider_from_registry/build_provider_from_registry_with_key 补充构建逻辑
4. ④ cargo test --workspace --lib + cargo clippy --workspace --lib --tests -- -D warnings
```

### 5.3 修改 Agent 核心逻辑

```
1. 修改前必须运行完整测试：cargo test --workspace --lib
2. 涉及 AgentController/Op 时，检查三处调用方：
   - clarity-tui 事件处理与渲染逻辑
   - clarity-gateway HTTP API / WebSocket 序列化
   - tests/integration 断言匹配
3. unwrap()/expect() 新增必须配 // SAFE: <不变量说明> 注释
```

---

## 六、Hard Veto（不可触碰）

| 约束 | 说明 |
|------|------|
| 本地 LLM 优先 | `LocalGgufProvider` 已验证；`ensure_llm` 自动 fallback |
| 禁止数据外泄 | API key 仅存储本地；云端 Provider 由用户显式选择 |
| 禁止 Docker | 无容器化依赖 |
| 禁止 RAG(Qdrant) | clarity-memory 使用 SQLite + BM25 + CosineIndex |
| 禁止 Electron | Tauri 2 已替代（已归档） |
| 项目广度 ≤ 5 核心工具 | 当前已达上限，新增功能需裁减 |
| 不入赘 Kimi 生态 | 学习但独立，四层主权不可让渡 |

---

## 七、关键决策速查

| 决策 | 文件 | 要点 |
|------|------|------|
| Settings 增量保存 | `settings.rs` | `merge_json` 递归合并，保留未知字段，参考 OpenClaw 教训 |
| API Key 安全 | `provider.rs` | 支持 `${env:VAR}` 语法；禁止明文落盘；中期目标 OS Keychain |
| LlmFactory 冻结 | `llm/mod.rs` | `#[deprecated]`；ModelRegistry 为唯一真相源 |
| Approval 持久化 | `approval/mod.rs` | `PersistingApprovalRuntime` 写入 clarity-memory（tags: approval, record） |
| 错误边界 | `ui/render.rs` | `render_safe()` + `catch_unwind`；单 panel panic 不崩溃整应用 |

---

## 八、元协议

- **决策先于叙事**：工程参数（内存、延迟、binary size、测试通过率）优先于任何身份/战略叙事
- **可剥离测试**：剥离叙事后，决策仍成立
- **定期审计**：每 3–6 个月检查活跃叙事是否退化为约束

---

*本文件由 AI 会话维护，人类开发者可直接编辑。重大架构变更需同步更新。*
