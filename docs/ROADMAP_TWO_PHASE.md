# Clarity 两步走路线图

> 定位：下一代 AI 基础设施的标杆  
> 策略：本地优先 + 零依赖 + 开源  
> 版本：v0.2.0（当前）→ v0.3.0（阶段二目标）

---

## 阶段一：最小可用发布（已完成 ✅）

**目标**：切断历史关联，建立独立品牌，实现"下载即用"。

### 已完成里程碑

| 里程碑 | 状态 | 说明 |
|--------|------|------|
| 法律清理 | ✅ | README 重写，零泄露源码关联表述 |
| 核心闭环 | ✅ | cargo install --git 可用，TUI + Gateway + Headless CLI 独立运行 |
| 多提供商 | ✅ | Kimi / OpenAI / Anthropic / DeepSeek / Ollama 全部可用 |
| 记忆验证 | ✅ | SQLite + BM25 + 向量混合搜索，跨会话持久化 |

### 作为"增量价值"保留的功能（不宣传为主打）

以下功能已完成，但不作为阶段一的核心卖点。它们增加了产品的完整性，但不做为重点推广：

- Desktop GUI（Tauri 2）— 含 Chat、Session、Task、Settings、File Browser
- Computer Use Panel — 截图/点击/打字/滚动
- LSP Proxy Layer — LSP Server 进程管理 + JSON-RPC 调试
- Diff Viewer — 代码 diff 展示
- WebBrowserTool — 网页内容提取
- Headless CLI — 脚本/CI 自动化

**阶段一的核心交付物是**：`cargo install --git https://github.com/juice094/clarity --bin clarity-tui` 即可运行一个功能完整的 AI Agent。

---

## 阶段二：本地优先标杆（当前重点 🎯）

**目标**：在"个人 AI 运行时"品类中，成为离线/本地场景的默认选项。

### 里程碑

| 里程碑 | 交付物 | 验收标准 | 优先级 |
|--------|--------|---------|--------|
| 本地 LLM 深度集成 | kalosm / Candle / llama.cpp 原生支持 | 8G 显存可跑 7B 级模型，无需外部 API | **P0** |
| 零依赖发行 | 单二进制 + 嵌入式模型 | 用户无需安装 Rust/Ollama/Python | **P0** |
| Ollama 体验优化 | 模型列表自动发现 + 一键切换 | GUI 中自动显示本地已安装模型 | **P1** |
| 协议输出 | 开源 Agent 通信协议草案 | 供其他 Runtime 参考实现 | **P2** |
| 企业/团队版 | Multi-user session 支持 | 验证从小众个人工具向团队基础设施扩展 | **P3** |

### 风险对冲

若 v0.2.0 发布后 **30 天内无实质性社区反馈**，阶段二冻结，资源回拨至 devbase。

**"实质性社区反馈"的定义**：
- GitHub Star ≥ 50
- Issue 或 PR ≥ 3
- 或任何外部使用者主动联系

---

## 技术债务与当前状态

| 债务项 | 状态 | 处理策略 |
|--------|------|---------|
| cargo audit 9 warnings | ⚠️ Tauri 上游间接依赖 | 等待上游更新，不主动投入 |
| Discord/Telegram CVE | ❌ 已禁用 | 阶段二不恢复，等上游修复 |
| GUI Monaco 编辑器 | ⏸️ 未启动 | 阶段二暂不投入，聚焦本地 LLM |
| Mobile app | ⏸️ 未启动 | 阶段三考虑 |

---

## 当前可执行动作

### 立即执行（今天）
1. ✅ `git push origin main --tags` — 发布 v0.2.0
2. 创建 GitHub Release，附上 v0.2.0 release notes
3. 在 V2EX / Rust 中文社区 / 相关 Discord 发布介绍帖

### 本周执行
1. Ollama 模型列表自动发现（`GET /api/tags`）
2. Settings Panel 中本地模型选择器优化
3. 离线模式检测（无网络时自动提示切换 Ollama）

### 本月执行
1. kalosm 本地 Provider 接入（等待 benchmark 数据）
2. 单二进制打包调研（cargo-bundle / tauri-bundler）
3. 协议草案初稿

---

## 资源分配原则

> 个人项目必须"先发布，再迭代"。

- **已完成功能**：不删除，但不追加投入
- **新功能开发**：只接受与"本地优先"直接相关的功能
- **维护工作**：测试 + clippy + 安全审计保持零警告
- **文档工作**：README + ROADMAP + 快速入门指南优先于功能开发
