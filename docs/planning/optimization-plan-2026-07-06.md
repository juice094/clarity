# Clarity 跨维度优化计划

> 生成时间：2026-07-06  
> 范围：`clarity-egui` / `clarity-ui` / `clarity-apps` / `clarity-shell` / `clarity-chrome` 前端栈  
> 状态：第一、二批已落地

## 目标

在 **UI 美化**、**现代化交互体验**、**资源占用** 三个维度上持续推进优化：

1. 完成现状审计并输出可执行的优先级清单。
2. 落地 3–5 个可验证的具体改进。
3. 保持 `cargo test --workspace --lib/bins/doc` 与 `cargo clippy --workspace --lib --bins --tests -- -D warnings` 全绿。

---

## 审计结论

| # | 优先级 | 类别 | 关键文件 | 问题 | 建议方案 | 规模/风险 |
|---|--------|------|----------|------|----------|-----------|
| 1 | P0 | 资源 | `panels/chat/message_list.rs` | 虚拟列表每帧遍历全部 turn 重算高度，流式最后一 turn 每帧重建 | 缓存 `(session, max_width)` 总高度，仅消息/宽度变化时失效；最后一 turn 增量更新 | 大/中 |
| 2 | P0 | 资源 | `pretext.rs` | `Font`→`egui::FontId` 映射每帧做 `to_lowercase` 与多次 `contains` | 加 `HashMap<Font, FontId>` 缓存 | 小/低 |
| 3 | P0 | 交互 | `apps/chat.rs` | 滚动到底为瞬时跳转，无回底按钮 | 浮动回底按钮 + `End` 快捷键 | 中/低 |
| 4 | P1 | 资源 | `apps/chat.rs`、`message_list.rs` | 热路径每帧 `theme.clone()` / `active_id.clone()` / `status_message.clone()` | 按需传递引用；`get_mut` 避免 `entry(...).or_default()` 的 key clone | 中/低 |
| 5 | P1 | 资源 | `widgets/rich_paragraph.rs` | 每帧重算 span/theme hash 构造 `LayoutKey` | `Message` 预存 span fingerprint，theme 仅在切换时失效缓存 | 小/低 |
| 6 | P1 | 交互 | `message_list.rs`、`shortcuts/mod.rs` | 消息操作仅 hover，无键盘导航 | ↑/↓ 选消息，`Ctrl+C` 复制、`E` 编辑、`R` 重生成 | 中/中 |
| 7 | P1 | UI | `apps/chat.rs`、`ui/render.rs`、`input/tui_style.rs` 等 | 多处 `Frame::new()`、`add_space(4.0)`、`CornerRadius::same(10)` 等 raw egui 字面量 | 路由到 `clarity_ui::design_system` token/组件或加 `// LAYOUT-EXEMPT` | 中/低 |
| 8 | P1 | UI | `ui/render.rs` | typing indicator 是静态 `● ● ●` | 按时间做脉冲 alpha 动画 | 小/低 |
| 9 | P2 | 资源/架构 | `ui/types.rs`、`components/agent_turn.rs` | `Message` 同时保留 `content`、`blocks`、`parsed`、`lines`；`AgentTurn` clone Message | 归一化为一份 prepared 表示；布局缓存内聚到 Message | 大/高 |
| 10 | P2 | 可维护 | `main.rs`、`app_logic.rs`、`ui/render.rs`、`message_list.rs` | 文件远超项目自定的 300 行面板规范 | 按域拆分：`virtual_window.rs`、`turn_renderer.rs`、update/render/commands 分离 | 大/中 |

---

## 已落地（第一批）

详见 `docs/planning/architecture-audit-2026-07-06.md` 第三批。

### 1. `pretext.rs` — `FontId` 映射缓存

- 实现：`EguiFontMetrics` 内部增加 `Arc<Mutex<HashMap<Font, egui::FontId>>>`。
- 收益：消除每次 `measure`/`supports_char`/`row_height` 中的 `to_lowercase()` 与字符串包含检查。
- 文件：`crates/clarity-egui/src/pretext.rs`

### 2. `ui/render.rs` — typing indicator 脉冲动画

- 实现：三个圆点基于 `ui.input(|i| i.time)` 做相位错开的 alpha 波动。
- 收益：加载态从静态文字变为动态反馈，符合现代化交互预期。
- 文件：`crates/clarity-egui/src/ui/render.rs`

### 3. 聊天回底按钮 + `End` 快捷键

- 实现：
  - `apps/chat.rs`：当内容超出视口且未贴底时，右下角绘制浮动回底按钮。
  - `shortcuts/mod.rs`：新增 `ShortcutAction::ScrollToBottom`，绑定 `End` 键。
  - `clarity-core/src/ui/commands.rs`：新增 `ids::SCROLL_TO_BOTTOM`。
  - `main.rs`：`dispatch_command` 处理该命令并设置 `stick_to_bottom = true`。
- 收益：解决之前“滚轮无法上移/一直锁底”的交互痛点，提供显式回底入口。
- 文件：`crates/clarity-egui/src/apps/chat.rs`、`crates/clarity-egui/src/shortcuts/mod.rs`、`crates/clarity-egui/src/main.rs`、`crates/clarity-core/src/ui/commands.rs`

### 4. 聊天热路径克隆削减

- 实现：
  - `panels/chat/message_list.rs`：`render_message_list` 与 `estimate_total_height` 改为接收 `&Theme`，内部不再 `theme.clone()`；`status_message` 与 `draft_status` 改为引用传递；`pretext_metrics` 改为 `Arc` 友好克隆。
  - `apps/chat.rs`：移除非必要 `theme.clone()`，通过参数把主题引用传入消息列表。
- 收益：每帧减少一次完整的 `Theme` 克隆和若干 `String`/`DraftStatus` 克隆，降低长会话时的分配压力。
- 文件：`crates/clarity-egui/src/panels/chat/message_list.rs`、`crates/clarity-egui/src/apps/chat.rs`

---

## 推荐下一步（第三批）

按 **性价比** 排序：

1. **P0 #1 虚拟列表高度缓存**：对 1000+ 条长会话的帧率影响最大；建议先加 `(session_id, max_width)` dirty flag，再逐步引入前缀和。
2. **P1 #6 消息键盘导航**：继续推进现代化交互，适合与 accessibility 测试一起落地。
3. **P1 #7 DESIGN_PROTOCOL 字面量清理**：风险低，建议按文件分批执行，避免一次改动过大。

---

## 验证基线

提交前必须保证：

```bash
cargo fmt --all -- --check
cargo test --workspace --lib --bins -- --test-threads=2
cargo test --workspace --doc -- --test-threads=2
cargo test -p clarity-integration-tests --lib
cargo clippy --workspace --lib --bins --tests -- -D warnings
```

当前第一批落地后上述命令全部通过。
