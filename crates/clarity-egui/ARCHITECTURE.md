# clarity-egui Architecture — Pretext Alignment

> 本文档约束所有后续代码变更。任何改动若破坏以下原则，必须在 PR 中显式说明理由。

---

## 一、冷热路径分离（核心原则）

Pretext 公式：**确定性布局 = 一次性采样（冷路径）+ 无限次纯算术（热路径）**

| 路径 | 触发时机 | 允许的操作 | 禁止的操作 |
|------|----------|-----------|-----------|
| **冷路径** | 内容变更、初始化、文件加载 | 字符串解析、正则、I/O、JSON、Markdown 分段、字体加载 | 无 |
| **热路径** | 每帧 `update()` | 遍历预计算结构、纯算术、egui 命令发射 | **字符串解析**、**正则**、**I/O**、**堆分配**、**JSON 序列化** |

### 1.1 Markdown — 冷路径唯一入口

```
Message::content 变更
        ↓
Message::prepare()  ← 唯一合法入口
        ↓
parse_markdown() → Vec<RenderBlock>
        ↓
Message::parsed 缓存
```

**铁律**：`ui/render.rs` 和 `ui/markdown.rs` 中的 `render_blocks()` **只能读取** `msg.parsed`，**禁止**对 `msg.content` 做任何解析操作。

**违规示例**（已删除）：
```rust
// ❌ 禁止：热路径中调用 text.lines().collect()
ui.label(RichText::new(&msg.content));
```

**正确示例**：
```rust
// ✅ 热路径只遍历预解析 blocks
crate::ui::markdown::render_blocks(ui, &msg.parsed, theme, text_color);
```

### 1.2 消息高度 — 缓存一致性

```
Message::prepare()          // 内容变更时
        ↓
msg.cached_height = None    // 强制失效

message_bubble()            // 热路径渲染时
        ↓
msg.cached_height = Some(actual_height)  // 写入实测值
```

**铁律**：
- `cached_height` 只能在 `prepare()` 中被置为 `None`
- `cached_height` 只能在 `message_bubble()` 返回后被写入实测值
- 新增 `RenderBlock` 变体时，必须同步更新 `estimate_height()`

---

## 二、虚拟列表约束

### 2.1 可见范围计算

使用上一帧 `ScrollAreaState::offset.y` + `available_height` + 估算高度确定 `start_idx..end_idx`。

**缓冲区**：±3 条消息（`saturating_sub(3)` / `+3`），防止滚动闪烁。

**铁律**：
- 禁止在热路径中遍历全部消息
- 禁止移除 `ui.allocate_space()` 顶部/底部 spacer
- 估算误差由缓冲区吸收，无需精确

### 2.2 新增消息类型的 checklist

若新增 `RenderBlock` 变体（如 `Table`、`Image`）：
1. [ ] 在 `parse_markdown()` 中添加解析逻辑
2. [ ] 在 `render_blocks()` 中添加渲染逻辑
3. [ ] 在 `estimate_height()` 中添加高度估算
4. [ ] 在 `ui/types.rs` 中定义新 block 类型

---

## 三、错误处理规范

### 3.1 禁止静默忽略

所有 `Result<T, E>` 必须显式处理。禁止：
```rust
// ❌ 禁止
let _ = save_session_internal(session);
let _ = tx.send(event);
```

允许：
```rust
// ✅ 允许：记录到 tracing
if let Err(e) = save_session_internal(session) {
    tracing::warn!("Failed to save session: {}", e);
}

// ✅ 允许：用户可见的 Toast
self.push_toast(&format!("Save failed: {}", e), ToastLevel::Error);
```

### 3.2 分级策略

| 场景 | 处理方式 |
|------|----------|
| mpsc channel 发送失败（应用关闭时） | `tracing::warn!` |
| 文件 I/O 失败（session、设置、附件） | `tracing::warn!` + `Toast::Error` |
| 网络/LLM 错误 | `Toast::Error` + 消息气泡 |
| panic hook 内部失败 | `eprintln!`（tracing 可能不可用） |

---

## 四、Design Token 使用规范

所有颜色/间距/圆角必须通过 `Theme` token 引用，禁止硬编码常量。

```rust
// ❌ 禁止
const COLOR_BG: Color32 = Color32::from_rgb(26, 26, 26);
ui.label(RichText::new("text").color(Color32::WHITE));

// ✅ 允许
self.theme.bg
self.theme.text
self.theme.accent
```

---

## 五、模块职责边界

| 模块 | 职责 | 禁止 |
|------|------|------|
| `main.rs` | 事件循环、状态管理、窗口布局、生命周期 | 渲染细节、字符串解析 |
| `ui/markdown.rs` | **冷路径**：`parse_markdown()`；**热路径**：`render_blocks()` | I/O、文件操作 |
| `ui/render.rs` | 气泡/工具/指示器的纯 layout 代码 | 任何解析逻辑 |
| `ui/types.rs` | 类型定义、冷路径 `prepare()` | 渲染代码 |
| `theme.rs` | Design Token、主题应用辅助方法 | 业务逻辑 |
| `app_state.rs` | Agent/LLM/TaskStore 状态、异步初始化 | UI 渲染 |

---

## 六、变更审批 checklist

任何涉及以下内容的 PR 必须对照本文档自查：

- [ ] 是否在热路径中引入了字符串解析/正则/I/O？
- [ ] 是否新增了 `let _ =` 静默错误？
- [ ] 是否修改了 `RenderBlock` 但未更新 `estimate_height()`？
- [ ] 是否引入了硬编码颜色/间距？
- [ ] 是否破坏了 `Message::prepare()` → `parsed` → `render_blocks()` 的数据流？
