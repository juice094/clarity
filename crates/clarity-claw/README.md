# clarity-claw

Clarity 系统托盘守护进程：常驻后台，监控任务状态，提供快速入口与 OS 通知。

## 职责

- **系统托盘** — 基于 `tao` + `tray-icon` 实现 Windows 系统托盘常驻
- **快速输入** — 左键点击托盘图标唤起输入框，直接发送消息到 Gateway
- **任务监控** — 轮询 Gateway `/v1/tasks`，实时显示运行中 / 待处理任务数
- **OS 通知** — 任务完成、失败或取消时推送桌面通知（`notify-rust`）
- **文件监听** — 监听 `.clarity/tasks` 目录变化，加速任务状态刷新
- **Wire 集成** — 预留 `clarity-wire` 通道，未来可直接接收 Soul 端推送

## 关键类型

- `UserEvent` — 自定义事件枚举，用于 Tao 事件循环的跨线程通信
- `TaskSummary` / `TaskListPayload` — Gateway 任务列表的极简反序列化结构
- `main` — 初始化托盘、启动后台轮询与事件循环的入口

## 测试

```bash
cargo test -p clarity-claw --lib
```
