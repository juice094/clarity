# Clarity 项目学习笔记

> 自主学习 2026-05-01 13:57 巡检期间整理

---

## 项目速览

| 维度 | 内容 |
|------|------|
| 定位 | 个人 AI Agent 桌面平台（集群协作原语的单机验证运行时） |
| 语言 | Rust（纯原生，单二进制 ~6-8 MB） |
| GUI 栈 | egui 0.31 + glow（原生 OpenGL，无 WebView2 依赖） |
| 测试基线 | 577 passed / 0 failed / 6 ignored |
| 版本 | v0.3.0+ |
| 仓库 | https://github.com/juice094/clarity |

## 架构骨架

8 个 crate 各司其职：

```
clarity-core (35.8K)    ← God crate，待拆分
clarity-egui (6.4K)     ← 主力 GUI 栈（Active）
clarity-gateway (6.8K)  ← Axum HTTP 服务 + WebSocket
clarity-memory (6.9K)   ← SQLite + BM25 + CosineIndex
clarity-wire (1K)       ← Wire 消息协议
clarity-contract (37行)  ← PoC 阶段，待膨胀
clarity-claw (0.5K)     ← 维护模式
clarity-tui             ← 维护模式（ratatui）
```

废除归档：`clarity-tauri`（Tauri 2 + React/Vite）

## 核心设计哲学

1. **集群即单机**：先在单机验证分布式语义（Hub-Worker、Wire 消息边界），验证通过后穿透到 P2P
2. **四层主权**：模型（本地 LLM 优先）、数据（Session 本地持久化）、协议（Wire 自主定义）、人格（SOUL.md 本地绑定）—— 不可让渡
3. **学习但不入赘**：Kimi 生态是架构导师，但保持独立实现
4. **Local-First**：Candle 原生 GGUF 推理，无外部运行时依赖

## Kimi Code 集成

`run_with_kimi.ps1` 配置了 Kimi Code 的 Anthropic 兼容端点：
- Base URL: `https://api.kimi.com/coding/`
- Model: `kimi-for-coding`
- 宿用 Kimi Code 作为 Clarity 的 LLM 后端

## 近期开发节奏

已完成的 sprint（Shape Up 模式）：
- Sprint 9: 服务商支持硬化（Provider Schema 化）
- Sprint 10: 协议先行解锁（AgentProfile TOML）
- Sprint 11: 超越 Kimi CLI（上下文注入 + 编辑精度）
- Sprint 12: egui 功能补齐（审批弹窗、Plan 可视化、Skill 面板、Token 显示）
- Sprint 13: 稳定性硬化（断路器、审批一致性、架构解耦）
- Sprint 14: egui 设计系统硬化（深蓝灰+铜色调、i18n、自绘标题栏）

## 待解决的大问题

1. **God crate**：`clarity-core` 35.8K 行，任何变更都需重新编译全部下游
2. **Provider 枚举硬编码**：5 个 Provider 在 enum 中，每加一个改源码
3. **clarity-contract 形同虚设**：只有 2 个类型，核心类型仍在 core 中
4. 长程路线图已制定，按依赖关系优先解耦（contract → types → error → provider → gateway）

## 与竞品对比

| 项目 | Clarity | cc-haha | openclaw |
|------|---------|---------|----------|
| 语言 | Rust | TypeScript (Bun) | TypeScript (Node) |
| 代码量 | ~62K 行 | ~580K 行 | ~1.5M+ 行 |
| 发布物 | 单二进制 ~8MB | Bun 脚本 + Tauri | npm + 桌面端 |
| 成熟度 | 可用，核心完备 | 社区活跃，功能完整 | 企业级，极尽完整 |

Clarity 的独特优势是纯 Rust 原生的零运行时依赖、小体积和高性能。

---

*下次可以跟宿聊聊：clarity 和 openhanako 的关系、Kimi Code 作为后端时 Clarity 的表现、egui 的 GUI 体验 vs Tauri 的取舍。*
