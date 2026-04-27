# Clarity 工程长程计划 · v0.3.0+

> 制定依据：成熟工程理论（康威定律、技术债务四象限、持续交付、测试金字塔）
> + 开发者理论（哥德尔不完备定理 → 保留扩展接口；MODE-O → 人类是问题生成器；
> 四层主权 → 本地优先；集群即单机 → 先验证后穿透）
> + 当前架构契约（AGENTS.md V3.1-EP-O）

---

## 一、当前状态锚点（2026-04-26）

| 维度 | 状态 |
|------|------|
| 版本 | v0.3.0 已发布（tag pushed） |
| Rust 测试 | 515 passed / 0 failed / 0 warning |
| 前端测试 | 31 passed / 11 test files（smoke + 1 interaction） |
| Gateway 集成测试 | 8 tests passing |
| CI/CD | Release workflow tag-triggered，`.msi` / `.exe` / `.nsis` 产出验证通过 |
| 本地构建 | `cargo build --release` 产出确认 |
| 文档健康 | `cargo doc --no-deps` 零 warning |
| 分支卫生 | 13 个已合并 subagent 分支已清理 |

---

## 二、未完成事项全面盘点

### P0 · 工程闭合（本轮必须完成）

| 项 | 说明 | 理论依据 |
|---|------|---------|
| CI Release 验证 | v0.3.0 tag 已 push，需确认 GitHub Actions 成功产出 signed bundles | 持续交付：每次 tag 必须是可部署的 |
| 前端测试矩阵 | ~~仅 1 个 smoke test~~ → 11 组件 / 30 tests 已覆盖 | ✅ 已完成 |
| 版本号一致性 | tauri.conf.json 已同步 0.3.0；需确认所有 Cargo.lock 衍生 crate 版本正确 | 配置即代码：版本漂移是发布事故根因 |

### P1 · 质量加固（v0.3.1，1-2 周）

| 项 | 说明 | 理论依据 |
|---|------|---------|
| 前端组件测试 | 11 组件 smoke tests 全覆盖 + SettingsPanel cancel 交互测试 | ✅ 已完成（2026-04-26） |
| Gateway 集成测试 | HTTP chat completions 边界场景 | ✅ 已完成（8 tests，2026-04-26） |
| 性能基准 | 启动时间、内存占用基准脚本 | ✅ 已交付（dev 基准已采集，release 待编译，2026-04-26） |
| 错误处理审计 | `invoke().catch(console.error)` 向前端暴露 | ✅ 已完成（6 组件修复 + 审计报告，2026-04-26） |
| `cargo doc` warnings | 13 处 → 0 | ✅ 已清零（2026-04-26） |
| unwrap 清理 | 11 处高风险 `?` 化 + 8 处 SAFETY 注释 | ✅ 已完成（2026-04-26） |

### P2 · 功能推进（v0.4.0-alpha，2-4 周）

| 项 | 说明 | 理论依据 |
|---|------|---------|
| 嵌入式模型自动下载 | 首次启动检测无模型 → 引导下载 Qwen2.5-3B / DeepSeek-R1-Distill-1.5B；`hf-hub` 已集成 | 零依赖发行：用户不应手动找模型文件 |
| T_APPROVAL V2 设计 | AI 分类器 + 规则引擎混合审批；V1 已完成规则引擎，V2 需 LLM-based 风险分类 | 哥德尔不完备：规则引擎无法覆盖所有场景，需 LLM 补全 |
| 单机跨窗口协作原型 | 同一台机器上多个 Clarity 窗口共享 Hub-Worker 状态；验证 Wire 消息边界 | 集群即单机：先在单机验证分布式语义 |
| egui 聊天原型验证 | 在 `clarity-tui` 中验证 egui 即时模式 GUI 替代 ratatui：聊天窗口 + Wire 事件绑定 | 零外部运行时依赖：egui 与 core 同一内存空间，毫秒级启动 |

### P3 · 架构演进（v0.5.0+，长期冻结至约束解除）

| 项 | 冻结原因 | 解除条件 |
|---|---------|---------|
| Mobile iOS/Android | 项目广度 > 5 核心工具；Tauri mobile 编译链重型 | v0.4.0 社区反馈 ≥ 50 stars |
| Plugin SDK / WASM | 安全边界未定；landlock 沙箱未调研 | Sandbox 技术选型完成 |
| Syncthing-Rust P2P 桥接 | 需先完成单机跨窗口协作验证 | 单机协作原型验收通过 |
| Voice / Canvas | 非核心路径；增加外部依赖 | 本地 Whisper/TTS 方案验证 |

### 技术债务（已知，不阻塞）

| 债务 | 状态 | 策略 |
|------|------|------|
| cargo audit 20+ upstream unmaintained | 已忽略 | 等待 Tauri 生态更新，不主动投入 |
| Discord/Telegram CVE (rustls-webpki) | 已禁用 | 等上游 serenity 0.12.6+ |
| `std::sync::RwLock` in `Agent.inner` |  intentional | 短临界区设计，非债务 |
| `unwrap()` / `expect()` 密度（~171 精确值） | 已测绘 + 部分清理 | 见 `docs/unwrap-debt-map.md`；11 处已 `?` 化/重构，8 处已 SAFETY 注释，冻结新增 |
| `cargo doc` warnings | ✅ 已清零 | 13 处已修复，建立零 warning 基线 |
| `clarity-tauri` 冻结 | 停止新功能开发，仅维护现有代码 | 待 egui 方向验证通过后评估废弃或归档；沉没成本极低（~1000 行前端） |

---

## 三、执行路线图

```
Week 1-2: v0.3.1 — 质量硬化
  ├─ Day 1-2: 前端组件测试矩阵（Sidebar + SettingsPanel + ErrorBoundary）
  ├─ Day 3-4: Gateway 集成测试扩展 + 性能基准脚本
  └─ Day 5:   CI Release 端到端验证 + 文档更新

Week 3-4: v0.3.2 — 体验优化
  ├─ 嵌入式模型首次启动引导（auto-detect + download UI）
  ├─ 错误处理审计：所有 silent catch 改为 Wire 事件或 Toast
  └─ Settings Panel 模型下载进度持久化（断点续传）

Week 5-6: v0.4.0-alpha — 架构扩展
  ├─ T_APPROVAL V2 设计文档 + 原型实现
  ├─ 单机跨窗口协作：SharedWorker / Tauri IPC 广播
  ├─ 性能优化：虚拟滚动（messages > 100 条时）
  └─ egui 方向验证：聊天窗口原型 + Wire 绑定（替代 ratatui 可行性）

Month 3+: v0.5.0-beta — 生态准备
  ├─ 条件触发：v0.4.0 发布 30 天后评估社区反馈
  ├─ 集群语义验证验收（多窗口/IPC/AgentPool）→ 见 [`FUTURE_DIRECTION.md`](FUTURE_DIRECTION.md)
  └─ Plugin SDK 技术选型（WASM vs Rust dylib）
```

---

## 四、验收标准

```bash
# 每次提交到 main 前必须全绿
cargo test --workspace --lib
cargo clippy --workspace --lib --bins --tests -- -D warnings
cargo fmt --all -- --check
cargo audit --deny unsound --deny yanked  # CI 实际配置；本地可追加 `--deny warnings` 做严格检查
cargo doc --no-deps

# 前端
cd crates/clarity-tauri/frontend && npm test

# 发布前额外验证
cargo tauri build  # 本地验证 bundler
git tag -a vx.x.x  # CI 自动触发 release workflow
```

---

## 五、Hard Veto 边界（不可逾越）

| 约束 | 说明 |
|------|------|
| 本地 LLM 优先 | 任何新功能必须支持离线模式；云端是可选增强 |
| 禁止数据外泄 | API key 不离开本机；Session 数据本地持久化 |
| 禁止 Docker | 无容器化依赖 |
| 禁止 RAG(Qdrant) | SQLite + BM25 + CosineIndex 已足够 |
| 项目广度 ≤ 5 核心工具 | 新增功能需裁减旧功能，或放入冻结区 |
| Rust 核心不外包 | 子 Agent 可辅助调研，但核心模块代码必须由本机 Agent 审查 |

---

*本计划由 AI Agent 维护，人类开发者可直接编辑。每次重大方向调整时同步修订。*
