# Clarity Roadmap

> 策略：本地优先 + 零依赖 + 开源  
> 当前：v0.2.0（已完成）→ 目标：v0.3.0（本地优先标杆）

---

## 阶段一：最小可用发布（已完成 ✅）

**目标**：切断历史关联，建立独立品牌，实现"下载即用"。

| 里程碑 | 说明 |
|--------|------|
| 法律清理 | README 重写，零泄露源码关联表述 |
| 核心闭环 | `cargo install --git` 可用，TUI + Gateway + Headless CLI 独立运行 |
| 多提供商 | Kimi / OpenAI / Anthropic / DeepSeek / Ollama / Local (Candle GGUF) |
| 记忆验证 | SQLite + BM25 + 向量混合搜索，跨会话持久化 |

**阶段一的核心交付物**：`cargo install --git https://github.com/juice094/clarity --bin clarity-tui` 即可运行一个功能完整的 AI Agent。

---

## 阶段二：本地优先标杆（当前重点 🎯）

**目标**：在"个人 AI 运行时"品类中，成为离线/本地场景的默认选项。

| 里程碑 | 交付物 | 优先级 |
|--------|--------|--------|
| 本地 LLM 深度集成 | ✅ Candle 原生 GGUF 支持（Qwen2/DeepSeek-R1-Distill） | **P0** |
| 零依赖发行 | 单二进制 + 嵌入式模型（用户无需安装 Rust/Ollama/Python） | **P0** |
| 协议草案 | 开源 Agent 通信协议（供其他 Runtime 参考实现） | **P2** |
| 企业/团队版 | Multi-user session 支持 | **P3** |

### 当前可执行动作

- ✅ Settings Panel 中本地模型路径配置 + 自动扫描
- 离线模式检测（无网络时自动 fallback 到 LocalGgufProvider）
- 单二进制打包调研（cargo-bundle / tauri-bundler）
- `clarity-tauri` 默认启用 `local-llm` feature（当前默认构建不包含 LocalGgufProvider，GUI 配置无法作用于运行时）

### 风险对冲

若 v0.2.0 发布后 **30 天内无实质性社区反馈**（GitHub Star ≥ 50 / Issue+PR ≥ 3），阶段二冻结，资源回拨至 devbase。

---

## 详细功能路线图

### Phase 0：基础夯实（已完成 ✅）

Agent ReAct 循环、Plan Mode、三层审批、MCP 三协议、Memory 系统、Background Tasks、Lazy Master。

### Phase 1：GUI 奠基（Sprint 1-2 已完成 ✅）

`clarity-tauri` 已可用：Chat Panel、Session Sidebar、Task Panel、Settings Panel、Theme System (Dark/Light/Auto)。

### Phase 2：核心补齐（部分已完成 ✅）

| 工作项 | 状态 | 说明 |
|--------|------|------|
| 审批系统增强 | 🔄 进行中 | AI 分类器 + 规则引擎 |
| 文件浏览器集成 | ✅ 已完成 | 工作目录树 + `@path` 引用 |
| LSP 支持 | ✅ 已完成 | LSP proxy layer + GUI panel |
| WebBrowserTool | ✅ 已完成 | reqwest+scraper 轻量实现 |
| 快捷键系统 | ⏸️ 未启动 | 全局快捷键 + Vim 键位引擎 |
| 搜索增强 | ⏸️ 未启动 | Command Palette 风格 |
| 性能优化 | ⏸️ 未启动 | 虚拟滚动、懒加载 |
| 桌面端打包 | ⏸️ 未启动 | CI/CD 自动构建 |

### Phase 3：Mobile 适配（3 周）

iOS/Android 构建链、移动端 UI 适配、推送通知、生物识别审批。

### Phase 4：生态扩展（6 周）

Bridge 远程控制、Vector Search (`sqlite-vec`)、Sandbox (`landlock`)、Plugin SDK (Rust dylib / WASM)、Voice 集成、Canvas 支持。

---

## 技术债务

| 债务项 | 状态 | 处理策略 |
|--------|------|---------|
| cargo audit 9 warnings | ⚠️ Tauri 上游间接依赖 | 等待上游更新，不主动投入 |
| Discord/Telegram CVE | ❌ 已禁用 | 等上游修复 |
| Mobile app | ⏸️ 未启动 | Phase 3 考虑 |

---

## 质量标准

```bash
cargo test --workspace --lib      # 全绿
cargo clippy --workspace --lib --bins --tests -- -D warnings  # 零 warning
cargo fmt --all -- --check        # 格式检查通过
cargo audit                       # 无高危漏洞
```

---

## 里程碑时间线

```
2026-04 ── v0.2.0      基础功能完备（当前）
    │
2026-05 ── v0.3.0-alpha  本地 LLM 完善 + Desktop GUI 功能完整
    │
2026-06 ── v0.4.0-beta   LSP + 性能优化 + 打包
    │
2026-07 ── v0.5.0-beta   Mobile iOS/Android 适配
    │
2026-08 ── v0.6.0-rc     Sandbox + Plugin SDK
    │
2026-09 ── v0.7.0-rc     Bridge + Voice + Canvas
    │
2026-10 ── v1.0.0        稳定版发布
```

---

*本文件随开发进度持续更新。每次重大决策或方向调整时同步修订。*
