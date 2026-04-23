# Agent 指引 — clarity-claw

## 构建

```bash
cargo build -p clarity-claw
```

## 测试

```bash
cargo test -p clarity-claw --lib
```

## 关键文件

- `src/main.rs` — 托盘初始化、后台轮询、Tao 事件循环

## 约定

- 错误处理使用 `anyhow`
- 异步使用 `tokio`
- 托盘事件通过 `MenuEvent::receiver()` 与 `TrayIconEvent::receiver()` 接收
- Gateway 地址优先从环境变量 `CLARITY_GATEWAY_URL` 读取，默认 `http://127.0.0.1:18790`
- 文件系统监听加速任务刷新，监听路径为 `.clarity/tasks`
