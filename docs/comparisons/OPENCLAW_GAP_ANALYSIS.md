# Clarity vs OpenClaw — 功能替代可行性分析

> 日期：2026-04-23 | 版本：clarity v0.1.1 | openclaw (third_party 最新)

---

## 一、OpenClaw 核心功能清单

| 模块 | 功能 | 技术实现 |
|------|------|----------|
| **Gateway** | HTTP/WebSocket 控制平面 | Node.js + Express/Fastify |
| **Channels** | 20+ 消息平台收件箱 | WhatsApp, Telegram, Slack, Discord, Signal, iMessage, Teams, Matrix, Feishu, LINE, Zalo, WeChat, QQ, WebChat... |
| **Multi-agent** | 工作空间隔离 + 每代理会话 | 内置路由 |
| **Voice** | 语音唤醒 + 连续对话 | macOS/iOS/Android native + ElevenLabs TTS |
| **Live Canvas** | Agent 驱动的可视化工作区 | macOS native A2UI |
| **Tools** | 浏览器、Canvas、节点、Cron、会话操作 | 内置 + 插件扩展 |
| **Skills** | 技能系统 + ClawHub 市场 | Markdown+YAML |
| **Companion Apps** | macOS 菜单栏、iOS/Android nodes | Swift/Kotlin native |
| **Security** | Docker 沙箱、DM 配对、allowlist | Docker + 本地存储 |
| **Plugins** | Provider/Channel/Extension SDK | 完整插件体系 |
| **Cron/Automation** | 定时任务、Webhook、Gmail Pub/Sub | node-cron + webhook |
| **Onboarding** | 向导式 CLI 设置 | `openclaw onboard` |

---

## 二、Clarity 当前实现状态

| 模块 | 状态 | 说明 |
|------|------|------|
| **Gateway** | ✅ 完整 | HTTP API (axum) + Web UI + WebSocket |
| **Channels** | ❌ 无 | 仅 Web UI，无消息平台集成 |
| **Multi-agent** | ✅ 完整 | 并行子代理 + 后台任务调度 |
| **Voice** | ❌ 无 | 无语音输入/输出 |
| **Live Canvas** | ❌ 无 | 无可视化工作区 |
| **Tools** | ⚠️ 部分 | 文件读写、Shell、Web 搜索、MCP（缺浏览器、Cron） |
| **Skills** | ✅ 完整 | Markdown+YAML 技能系统 |
| **Companion Apps** | ⚠️ 部分 | 仅系统托盘（claw），无原生 macOS/iOS/Android 应用 |
| **Security** | ⚠️ 部分 | 路径遍历修复、MCP 命令验证、敏感文件检测（缺 Docker 沙箱） |
| **Plugins** | ❌ 无 | 无插件 SDK |
| **Cron/Automation** | ❌ 无 | 无定时任务、Webhook 接收端 |
| **Onboarding** | ❌ 无 | 无向导式设置 |

---

## 三、替代可行性结论

### 已可替代的场景（v0.1.1）

| 场景 | 说明 |
|------|------|
| **本地开发助手** | 代码编辑、文件操作、Shell 执行、Web 搜索、Plan Mode 规划执行 |
| **Web 管理界面** | 通过浏览器管理 Agent、查看任务、切换配置 |
| **后台任务** | 长时间运行的 Agent 任务 + 系统托盘通知 |
| **多 LLM 切换** | Gateway Provider API 支持多模型切换 |

### 不可替代的场景（差距较大）

| 场景 | 差距 | 工作量预估 |
|------|------|----------|
| **消息平台助手** | 20+ Channels 无一个实现 | 2-3 个月/频道 |
| **语音交互** | 无语音 I/O | 1-2 个月 |
| **可视化 Canvas** | 无 A2UI | 2-3 个月 |
| **移动端 Companion** | 无 iOS/Android 应用 | 3-6 个月 |
| **自动化 Cron** | 无定时任务 | 2-4 周 |
| **插件生态** | 无插件 SDK | 2-3 个月 |

### 战略判断

> **Clarity 当前可替代 OpenClaw 的「开发者/代码助手」子集，但无法替代其「个人 AI 助手」完整定位。**

OpenClaw 的核心护城河是 **Channels（消息平台全覆盖）+ Voice + Canvas + Companion Apps**，这些是面向普通消费者的「个人助手」体验。Clarity 的当前定位更偏向 **「开发者的 AI 运行时」**，两者目标用户不同。

如果目标是「替代 OpenClaw 的全部功能」，需要追加投资 **6-12 个月** 的 Channels、Voice、Canvas、移动端开发。

如果目标是「在开发者场景中比 OpenClaw 更好用」，Clarity 已经具备竞争力（Rust 性能、Plan Mode、并行子代理、三层运行时）。
