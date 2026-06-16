> **注意**：egui 前端当前正在进行 Pretext 三栏布局重构。本文件作为**集成标记与编排说明**，不在 egui 源码中遗留 `TODO/FIXME/XXX`，避免与重构冲突。待重构稳定后，再按本文件落地实现。

# egui Thread 集成标记

## 目标
让 egui 桌面端能够：
1. 显示最近线程列表（左/右 rail）。
2. 点击线程后加载该线程的历史并进入对话视图。
3. 新对话自动创建线程，并在 URL/状态中记录 `thread_id`。
4. 通过 `clarity-wire` 接收 thread 事件，避免直接访问 `clarity-thread-store`。

## 集成点（按依赖顺序）

### 1. `clarity-wire` 协议扩展
- 文件：`crates/clarity-wire/src/lib.rs`
- 动作：在 `WireMessage` 中增加 `ThreadList` / `ThreadActive` / `ThreadCreated` / `ThreadHistoryLoaded` 等变体。
- 阻塞：无；由后端/Agent 推进。

### 2. `clarity-core::thread::ThreadManager` 事件发射
- 文件：`crates/clarity-core/src/thread/manager.rs`
- 动作：`ThreadManager` 持有可选的 `Wire`，在 `create_thread` / `list_threads` / `update_metadata` / `append_turn` 后广播 `WireMessage::ThreadList` / `ThreadActive`。
- 阻塞：依赖第 1 步。

### 3. egui 线程列表面板
- 文件：`crates/clarity-egui/src/panels/sidebar/` 或 `crates/clarity-egui/src/panels/workspace/thread_list.rs`（新建）
- 动作：
  - 渲染最近线程列表（标题 + 更新时间）。
  - 支持点击切换：通过 `Wire` 发送 `UserAction::SwitchThread`。
  - 支持 “New Thread” 按钮。
- 阻塞：**待 Pretext 重构稳定后**再新建面板，避免布局冲突。

### 4. egui 对话视图识别 `thread_id`
- 文件：`crates/clarity-egui/src/agent/` 或 `crates/clarity-egui/src/panels/chat/`（根据重构后目录）
- 动作：
  - 在 AppState / ViewState 中增加 `current_thread_id: Option<ThreadId>`。
  - 发送消息时若存在 `current_thread_id`，通过 Gateway 或 ThreadManager 走 thread 路径；否则保持现有无状态流程。
- 阻塞：依赖第 3 步与重构后状态机设计。

### 5. 与 Claw 托盘对齐
- Claw 托盘已能列出线程并打开 `chat.html?thread_id=...`。
- egui 启动时应解析命令行/URI 中的 `thread_id`，直接加载对应线程。

## 不应急于做的事
- 在 egui 中直接 import `clarity-thread-store` 或 `clarity-rollout`；前端只消费 `clarity-wire`。
- 在 egui 源码中写 `TODO: thread integration`；所有临时标记统一放到本文件。

## 建议落地顺序
1. 后端完成 `clarity-wire` Thread 事件扩展。
2. `ThreadManager` 接入事件广播。
3. 任意一个非 egui 前端（如 TUI）先跑通列表+切换，验证协议。
4. Pretext 重构完成后，egui 按本文件第 3、4 步实现。
