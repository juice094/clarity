# Clarity 长程路线图（6 个月）— v0.3.0 → v0.5.0

> 制定日期：2026-04-27
> 基线 commit：`main` @ `4ee5504`
> 整合来源：Pretext Health Plan / Parity Gap Plan / cluster-as-single-node / ROADMAP / FUTURE_DIRECTION
> 工程理论锚点：Cynefin 框架（复杂域→明晰域迁移）、Martin Fowler 技术债务预算、防御性编程（Rust 类型系统即证明）、持续交付门控（Jez Humble）

---

## 一、目标声明

**总目标**：在 6 个月内将 `clarity-egui` 从"复杂域原型"迁移到"明晰域日常工具"（Cynefin 框架），同时验证"集群即单节点"核心架构。每个 Phase 以**健康度门控**而非功能清单为验收标准 —— 功能可以裁剪，工程健康不可妥协。

**分阶段目标**：
- **v0.3.1–v0.3.2**（0–6 周）：**健康基线** — egui 达到可维护、可测试、可审计的状态，P0 阻塞解除
- **v0.4.0-beta**（6–14 周）：**功能收敛** — 在健康基线上补齐 Parity 差距，功能集冻结，不再扩展
- **v0.5.0-beta**（14–26 周）：**架构验证** — 集群语义验证，同时保持代码健康度不衰退

**工程健康定义**（不可谈判）：
1. `cargo test/clippy/fmt/doc/audit` 五维全绿
2. 新增代码的测试覆盖率 ≥ 60%（逻辑分支）
3. 零 `unsafe`；`unwrap()` 密度不增加（当前 171 总量，目标 ≤ 150）
4. 每 Phase 结束后强制 24h "冷却期" —— 只读代码审查，不写新功能
5. 安全审计：除 `cargo audit` 外，每 4 周执行一次依赖树人工审查（检查新引入的间接依赖）

---

## 二、时间线总览

```
Week  1  2  3  4  5  6  7  8  9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26
      ├──────── Pretext Phase 1 ────────┤
      ├─────── Parity P0 ───────┤
              ├────── Parity P1/P2 ──────┤
                                          ├────── Gateway Phase A/B ──────┤
                                          ├────── v0.4.0 集中窗口 ────────┤
                                                                          ├──── v0.5.0 ────┤
```

| 版本 | 时间 | 主题 | 关键交付 |
|------|------|------|----------|
| v0.3.1 | W1–W2 | 审批 UI + 紧急修复 | Approval 模态、Yolo/Interactive/Plan 模式可用 |
| v0.3.2 | W3–W6 | 测试基线 + Parity P1 | ≥20 单元测试、Task 创建/取消、Token 用量、Plan 可视化 |
| v0.4.0-beta | W7–W14 | 功能完备 + Gateway 增强 | 子代理 UI、技能 UI、记忆 UI、WebSocket MCP、BTM 集成 |
| v0.5.0-beta | W15–W26 | 集群语义验证 | 多窗口协作、AgentPool、IPC 传输、Syncthing P2P 同步 |

---

## 三、Phase 详细规划

### Phase 1: 健康基线（W1–W6）— v0.3.1 → v0.3.2

**目标**：解决 P0 阻塞问题，建立可维护的工程基线。**技术债务预算：每 2 周预留 20% 时间（即 2 天）用于债务偿还而非新功能**。

#### W1–W2: P0 审批交互 UI（硬 deadline）
- **任务**：将 `clarity-core` 的 `ApprovalRuntime` 接入 egui
- **交付**：
  - `ApprovalModal` — Approve / Reject / Approve-for-Session 三按钮
  - 规则引擎可视化（显示匹配的规则和 RiskLevel）
  - `Interactive`/`Plan`/`Yolo` 模式切换在 Settings 中生效
- **验收标准**：
  - `Interactive` 模式下调用 `file_write` 时弹出模态，不阻塞 egui 事件循环
  - **FMEA 验证**：模拟 ApprovalRuntime panic / 模态关闭异常 / 规则引擎死循环，确保有 fallback 到 Yolo 模式的路径
- **安全要求**：
  - 审批弹窗必须显示**完整工具调用参数**（防止参数篡改攻击）
  - "Approve for Session" 的范围限制在当前会话 ID，不可跨会话生效
- **风险**：若 W2 未完成，冻结所有 P2/P3 工作，全力攻此项
- **债务偿还（预留 2 天）**：清理审批相关代码中的 `unwrap()`，补充 `AgentError` 映射

#### W3–W4: 测试基线注入 + 防御性编程硬化
- **任务**：为 egui 建立最小可维护的测试骨架，同时强化错误处理
- **交付**：
  - `EguiError` 枚举（替代所有裸 `String` 错误）
  - `settings.rs` 单元测试（≥5 个，覆盖损坏文件、环境变量互斥、provider 切换）
  - `theme.rs` 单元测试（≥3 个）
  - `app_state.rs` 逻辑测试（LLM fallback 路径、网络探测 mock）
  - **新增**：`file_browser.rs` 路径遍历防护测试（验证 `resolve_path` 不可逃逸工作目录）
- **约束**：
  - 不测试 UI 渲染（egui 即时模式难以单元测试），只测纯逻辑
  - 每个 `pub fn` 必须携带 `#[doc = "..."]` 和 `// SAFETY:` 或 `// INVARIANT:` 注释
- **债务偿还（预留 2 天）**：将现有 `String` 错误迁移到 `EguiError`，清理 5-10 处 `unwrap()`

#### W5–W6: Parity P1 核心功能 + 安全审计
- **任务**：补齐 core 已有但 egui 缺失的交互功能
- **交付**：
  - Task Panel 工具栏：Spawn / Cancel / Cron 创建
  - Token 用量微标签（Chat Area 右下角显示 input/output tokens）
  - Plan 模式可视化：步骤列表 + 执行状态 + 展开查看每步结果
- **安全要求**：
  - Task Spawn 的 shell 命令必须经过 `validate_mcp_command()` 同等校验（防止命令注入）
  - Cron 表达式解析使用白名单（禁止 `* * * * *` 等过于频繁的调度）
- **Week 6 末尾：强制 24h 冷却期** —— 只读审查，不写代码。输出：《Phase 1 健康度报告》
- **Week 6 末尾：依赖树人工审查** —— 检查 W1-W6 期间 Cargo.lock 的变动，确认无新增高风险间接依赖

### Phase 2: 功能收敛（W7–W14）— v0.4.0-beta

**目标**：egui 功能与已归档的 tauri 版本持平，同时建立**安全纵深防御**和**开发者体验基线**。**技术债务预算：每 2 周预留 20% 时间**。

#### W7–W8: 子代理 + 技能系统 UI
- **交付**：
  - 并行执行面板：子代理卡片列表 + 实时进度 + 结果汇总
  - Skills 设置标签页：技能列表、启用/禁用、关键词搜索
- **安全要求**：
  - 子代理的 `file_write` 操作必须继承父代理的审批模式（不可子代理用 Yolo 绕过父代理的 Interactive 设置）
  - Skill 的 Markdown 文件解析必须限制模板变量深度（防止递归爆炸）
- **DX 优化**：
  - 编译时间监控：若 `cargo check -p clarity-egui` 超过 30 秒，识别并拆分臃肿模块

#### W9–W10: 记忆 + 日志 + 输入验证硬化
- **交付**：
  - 记忆提取面板：跨会话搜索、BM25 结果高亮、向量相似度排序
  - Log/Console 面板：tracing 日志订阅、级别过滤、按会话筛选
- **安全要求**：
  - 记忆搜索输入必须经过长度限制（≤ 1000 字符）和 Unicode 规范化（NFKC），防止 ReDoS 和视觉欺骗攻击
  - 日志面板不得显示 `api_key`、`token`、`password` 等敏感字段（正则匹配 + 掩码处理）
- **开发者体验**：
  - 新增 `justfile` 或 `Makefile` 封装常用命令（`test`, `clippy`, `doc`, `audit`）

#### W11–W12: 模型下载 + Gateway 增强
- **交付**：
  - 模型下载对话框：HuggingFace repo ID 输入 + 文件名 + `egui::ProgressBar` 进度
  - Gateway WebSocket MCP transport（`McpTransport::WebSocket`）
  - Gateway ↔ BackgroundTaskManager 集成（HTTP API 操作后台任务）
- **安全要求**：
  - HuggingFace repo ID 必须白名单验证（只允许 `^[a-zA-Z0-9_/-]+$`），防止路径遍历和 SSRF
  - 下载文件的校验：SHA256 校验（若 HuggingFace 提供）或至少文件大小验证
  - Gateway 的 WebSocket 端点必须启用 Origin 校验，防止 CSWSH 攻击
- **债务偿还（预留 2 天）**：Gateway handler 的输入验证全面审计，补充 `#[validate]` 或手工校验

#### W13–W14: v0.4.0 验收窗口 + 安全审查
- **任务**：
  - 集成测试、性能回归、文档同步
  - **人工安全审查**：模拟攻击者视角，检查所有新增 API 端点和 UI 输入点
  - **回滚策略文档化**：若 v0.4.0-beta 发现严重缺陷，如何快速回退到 v0.3.2（git tag + 二进制备份）
- **验收**：
  - `cargo test --workspace --lib` 全绿
  - `cargo clippy` 零警告
  - egui `unwrap()` 数量不增加（目标 ≤ 150，当前 171）
  - **新增**：`cargo geiger` 或手动审计确认零 `unsafe`

### Phase 3: 架构验证（W15–W26）— v0.5.0-beta

**目标**：验证"集群即单节点"架构在单机上的可行性，同时保证**分布式场景下的安全边界**。**技术债务预算：每 2 周预留 25% 时间**（架构重构的债务成本更高）。

#### W15–W18: Phase C — 运行时重构 + 安全边界定义
- **交付**：
  - `AgentPool` + `AgentInstance`（包装而非替换 `AgentController`，向后兼容）
  - `Identity` 枚举 + 身份路由
  - Agent 间 Wire 消息扩展（`AgentMessage`）
  - IPC 传输层（`Transport::Ipc`，本机多进程通信）
  - 多窗口状态模型（单窗口 `AppState` → 多窗口共享状态）
- **安全要求**：
  - IPC 通道必须认证：使用 OS 级别的进程认证（Windows: 命名管道 ACL / Unix: socket 文件权限），防止恶意进程冒充 Agent
  - `AgentMessage` 必须包含 HMAC-SHA256 签名（共享密钥派生自会话 ID），防止中间人篡改
  - 多窗口状态共享必须通过 `parking_lot::RwLock` 而非 `std::sync::Mutex`，避免优先级反转导致 UI 卡顿
- **故障模式分析（FMEA）**：
  - IPC 连接断开 → 自动降级为单窗口模式，不 panic
  - AgentPool 中某个 Agent 实例 panic → 隔离该实例，不影响其他实例和 UI
  - 状态同步冲突 → 最后写入者胜（Last-Write-Wins）+ 冲突日志，不自旋等待

#### W19–W22: Phase D — 跨设备同步 + 零信任设计
- **交付**：
  - Syncthing-Rust 设备注册表集成
  - 会话 CRDT 同步（Loro 或自研轻量方案）
  - Agent 状态迁移（窗口 A → 窗口 B 的会话 handoff）
  - P2P Wire 协议原型
- **安全要求**：
  - 跨设备同步必须端到端加密（Syncthing 原生加密或额外 AES-GCM 层）
  - 设备身份验证：基于 Ed25519 设备密钥对，拒绝未授权设备加入同步集群
  - 会话数据在传输前脱敏：移除 `api_key`、局部路径（`C:\Users\...`）等敏感信息
- **零信任原则**：即使在本机 IPC 中，也不假设对端是可信的 —— 所有消息均需校验签名和时戳（防止重放攻击）

#### W23–W26: v0.5.0 验收 + 社区门控 + 安全审计
- **任务**：
  - 完整集成测试（多窗口 + IPC + P2P 模拟）
  - **第三方安全审计**：邀请外部审计者（或自我审计清单）检查 IPC/P2P 攻击面
  - **灾难恢复演练**：模拟 `.clarity/` 目录损坏、同步冲突爆炸、网络分区，验证系统 graceful degradation
- **30 天社区反馈门控**：v0.4.0 发布后观察 stars/issues/PRs
  - **通过**（≥50 stars + ≥3 issues/PRs）：继续 v0.5.0 路线
  - **未通过**：冻结 Phase 3，资源重新分配至 **devbase** 知识库项目
- **验收**：
  - 新增 `cargo test --workspace --lib` 全绿（新增测试 ≥ 50 个）
  - **性能基线**：`clarity-egui` 冷启动 ≤ 3 秒，内存占用 ≤ 150 MB（发布模式）
  - **安全基线**：通过自研《安全审查清单》（见下方）

---

## 四、资源分配原则

| 原则 | 说明 |
|------|------|
| **串行优先** | Phase 1 内部串行（审批 UI → 测试 → Parity P1），不可并行 |
| **前后端解耦** | Gateway Phase A/B 可与 egui Parity 并行（不同开发者/不同时段） |
| **集中窗口** | Phase C/D 需要连续 4–6 周不受干扰的窗口，不可与 patch release 穿插 |
| **零新增 crate** | 6 个活跃 crate 是硬天花板，所有功能在现有结构内实现 |
| **Rust 核心不外包** | `clarity-core` / `clarity-memory` / `clarity-wire` 的代码必须由人类或主 Agent 直接编写 |
| **技术债务预算** | 每 2 周预留 20% 时间（Phase 3 为 25%）用于债务偿还：unwrap 清理、文档补齐、重构 |
| **冷却期制度** | 每 Phase 结束强制 24h 只读审查期，不写新功能 |
| **开发者体验（DX）基线** | `cargo check -p clarity-egui` ≤ 30s；`cargo test --workspace --lib` ≤ 60s；构建失败时必须提供可执行的复现步骤 |

---

## 五、风险与门控

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| 审批 UI 延期 | 中 | 🔴 阻塞所有后续工作 | W2 未完成则冻结 Parity P2/P3，全力攻坚 |
| egui 测试难以落地 | 中 | 🟡 技术债务持续累积 | 只测纯逻辑（settings/state/theme），不测渲染；若仍困难则放宽至 ≥10 测试 |
| Syncthing-Rust 数据未到达 | 高 | 🟡 Phase D 无法验证 P2P | 用本地 IPC + 文件系统模拟代替真实 P2P；不等待 agri-paper 7B |
| 社区反馈未达标 | 中 | 🟡 v0.5.0 冻结 | 预设 fallback：资源转向 devbase，Clarity 进入维护模式 |
| 项目广度超限 | 低 | 🔴 违反 Hard Veto | 每 Phase 开始前审计 crate 数量；任何新增功能必须伴随等量的裁剪/归档 |
| **安全事件（漏洞利用）** | 低 | 🔴 项目声誉损毁 | 建立《安全事件响应手册》：发现漏洞 → 24h 内评估 → 72h 内修复或发布缓解措施 → 公开披露 |
| **编译时间退化** | 中 | 🟡 DX 恶化 | 每周监控 `cargo check` 时间；超过 30s 则识别并拆分臃肿模块 |
| **关键开发者 burnout** | 中 | 🔴 项目停滞 | 每 6 周强制休息 3 天；Phase 之间设置 1 周缓冲期；不接受紧急插单 |

---

## 六、关键依赖关系

```
Pretext Phase 1 完成 (Mutex, update() 拆分)
            ↓
    Parity P0 审批 UI
            ↓
    Parity P1 测试基线
            ↓
    Parity P2 子代理/技能/记忆
            ↓
    v0.4.0-beta 发布 ─────────┬──────── Gateway Phase A/B (可并行)
            ↓                  │
    30 天社区门控              │
            ↓                  │
    Phase C AgentPool/IPC      │
            ↓                  │
    Phase D Syncthing P2P      │
            ↓                  │
    v0.5.0-beta 发布 ◄─────────┘
```

---

## 七、冻结项（6 个月内不启动）

| ID | 事项 | 冻结原因 | 解除条件 |
|----|------|----------|----------|
| T_APPROVAL_V2 | AI 分类器混合审批 | 需大模型标注数据，ROI 不明确 | v0.5.0 后且有标注数据集时 |
| T_SHORTCUTS | 全局快捷键系统 | egui 跨平台快捷键支持不成熟 | egui 官方快捷键 API 稳定后 |
| T_MOBILE | Mobile 适配 | 违反 Hard Veto | 项目广度约束解除且 v1.0 发布后 |
| T_PLUGIN_SDK | Plugin SDK / Sandbox | 需要 WASM 或 IPC 沙箱基础设施 | v0.6.0 后 |
| T_VOICE | 语音输入/输出 | 依赖外部语音识别引擎，与"零依赖"冲突 | 本地语音识别方案（Whisper.cpp 集成）验证通过后 |
| T_KIMICLI_REF | 借鉴 Kimi CLI settings 设计 | 仅作设计参考，不推进实现 | 永不解除，仅作设计参考 |

---

## 八、验收命令（每 Phase 结束必执行）

### 自动化门控（CI 执行）
```bash
cargo test --workspace --lib          # 全绿，覆盖率 ≥ 60%（新增代码）
cargo clippy --workspace --lib --bins --tests -- -D warnings  # 0 warnings
cargo fmt --all -- --check            # 0 diff
cargo doc --no-deps                   # 0 doc warnings
cargo audit --deny unsound --deny yanked  # 0 RUSTSEC
cargo build --release -p clarity-egui   # 发布模式编译成功，binary size 记录存档
```

### 人工门控（24h 冷却期内执行）
| 检查项 | 执行者 | 通过标准 |
|--------|--------|----------|
| 代码审查 | 自我审查 + AI 辅助 | 无新增 `unwrap()`；所有 `pub fn` 有文档；输入验证完整 |
| 安全清单 | 自我审查 | 通过下方《安全审查清单》 |
| 依赖审计 | 人工 | Cargo.lock 变动 ≤ 10 个间接依赖；无新增 yanked/unmaintained |
| 性能基线 | 自动化 | `clarity-egui` 冷启动 ≤ 3s；内存 ≤ 150MB |
| DX 检查 | 自我体验 | `cargo check -p clarity-egui` ≤ 30s；IDE 代码补全正常 |

---

## 九、附录 A：安全审查清单（每 Phase 结束执行）

```
□ 输入验证：所有用户输入（文件路径、URL、API key、模型 ID）均有长度限制和格式校验
□ 路径安全：所有文件操作使用 canonicalize() + 前缀校验，防止目录遍历
□  secrets 管理：API key 仅内存存储，不写入日志，不传输到子进程
□ 网络边界：本地 Provider 不发起外部网络请求；云端 Provider 使用 TLS 1.3
□ 并发安全：所有共享状态使用 parking_lot::Mutex/RwLock，无非同步访问
□ 错误处理：无裸 panic/unwrap() 新增；所有错误路径返回 Result 而非 abort
□ 依赖审计：cargo audit 零 RUSTSEC；无新增 yanked crate
□ 日志脱敏：tracing 日志不包含 api_key、token、密码等敏感信息
□ IPC/P2P 认证（Phase 3）：消息签名验证 + 设备身份认证 + 重放攻击防护
```

---

## 十、附录 B：开发者体验监控指标

| 指标 | 目标 | 监控方式 |
|------|------|----------|
| `cargo check -p clarity-egui` | ≤ 30s | 每周手动计时 |
| `cargo test --workspace --lib` | ≤ 60s | CI 计时 |
| `clarity-egui.exe` 冷启动 | ≤ 3s | 发布模式手动测试 |
| 运行时内存（idle） | ≤ 150MB | Windows 任务管理器 / `cargo flamegraph` |
| `unwrap()` 数量 | ≤ 150（当前 171，净减少） | `grep -r "unwrap()" crates/clarity-egui/src/ | wc -l` |
| 文档覆盖率 | 所有 `pub fn` 有 doc comment | `cargo doc --no-deps` 无 warning |

---

*本计划随版本发布同步更新。下次全面审视：v0.3.2 发布时（W6 末）。*
