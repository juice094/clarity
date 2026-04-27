# Plan: clarity-egui Pretext 健康度审查与运维硬化

> **协议**：Pretext 思想（确定性布局 = 冷路径采样 + 热路径纯算术）作为代码审查元框架。
> **范围**：`crates/clarity-egui` 单 crate，不新增依赖，不扩张功能边界。
> **目标**：将 egui 从"快速迭代原型"硬化为"可维护的每日使用栈"。

---

## 一、Pretext 四层映射框架

Pretext 的核心不是"优化"，而是**重构计算的时空结构**——将不可计算转化为可计算。将其四层设计模式映射到代码健康审查：

| Pretext 层级 | 审查问题 | egui 当前不合格信号 |
|-------------|---------|-------------------|
| **Truth-Source 隔离** | 热路径是否通过只读观测获取真相，而非触发副作用？ | `App::update()` 550 行混合了渲染、I/O（settings 读写）、网络（model 下载进度）、文件拖拽、MCP 调用。热路径直接触及外部系统。 |
| **度量前置化** | I/O / 分配 / 外部调用成本是否被移到初始化/冷路径？ | `Theme::dark()`/`light()` 每处 UI 构造都重新分配颜色值；字体大小、圆角半径无缓存；虚拟列表高度无确定性模型。 |
| **模型外推** | 是否用数学代理/纯函数替代对黑盒系统的推理？ | `Result<(), String>` 丢失错误结构；`AgentStatus` 状态机无显式转移图；零测试意味着行为无形式化验证。 |
| **确定性输出** | 核心计算是否为纯函数接口（同输入→同输出，可测试，可并行，可缓存）？ | 12 处 `Mutex::lock().unwrap()` 引入 poison 非确定性；`App` 结构体 40 字段导致状态空间爆炸；零单元测试。 |

**一句话诊断**：clarity-egui 当前是**渲染依赖型计算**（每帧重新决策、重新分配、重新 I/O），而非 **数据依赖型计算**。Pretext 思想的引入目标，是将其重构为"初始化期一次性采样 + 运行期纯算术组合"。

---

## 二、健康度审计结果（定量基线）

| 指标 | 当前值 | 目标值 | 测量命令 |
|------|--------|--------|---------|
| 代码行数 | ~2,912 行 | 控制增长 | `tokei crates/clarity-egui` |
| `unwrap()` / `expect()`（非同步原语） | 1 处（`tokio::runtime::new().expect`） | 0 新增 | `grep -rn "\.unwrap()\|\.expect(" crates/clarity-egui --include="*.rs" \| grep -v "lock().unwrap()"` |
| `Mutex::lock().unwrap()` | 12 处 | ≤ 6（减半） | `grep -rn "lock().unwrap()" crates/clarity-egui` |
| `pub fn` doc 覆盖率 | ~45%（14/31） | ≥ 90% | `cargo doc --no-deps -p clarity-egui 2>&1 \| grep "missing"` |
| clippy warning | 0 | 0 | `cargo clippy -p clarity-egui --lib --bins --tests -D warnings` |
| 单元测试数 | 0 | ≥ 20 | `cargo test -p clarity-egui --lib` |
| 死依赖 | `anyhow`（声明未使用） | 0 | `cargo udeps -p clarity-egui` |
| `unsafe` | 0 | 禁止新增 | `grep -rn "unsafe" crates/clarity-egui --include="*.rs"` |

**关键不合格项**：
1. 🔴 `App::update()` 550 行 — 复杂度黑洞，所有 Pretext 层级均不合格。
2. 🔴 零测试 — 违反"确定性输出"原则，无法验证行为稳定性。
3. 🔴 `Mutex::lock().unwrap()` ×12 — 热路径中的 poison 风险，非确定性 panic。
4. 🟡 `Result<(), String>` — 错误处理反模式，违反"模型外推"。
5. 🟡 doc 覆盖率 45% — 低于 AGENTS.md 基线（≥90%）。

---

## 三、运维 Plan（三阶段，6 周）

**原则**：
- 不新增外部依赖（`anyhow` 死依赖移除除外）。
- 不新增功能（Settings/TUI 功能冻结至本 plan 完成）。
- 每阶段结束必须满足"验收命令全绿"才进入下一阶段。
- 每阶段产出独立 commit，禁止跨阶段混提。

---

### Phase 1：热路径清剿（Truth-Source 隔离 + 度量前置化）— 2 周

**目标**：将 I/O、锁操作、状态突变从 `App::update()` 热路径中剥离。

#### 1.1 `App::update()` 拆分（P0）

将 `main.rs:537-1088` 的 550 行巨型函数拆分为 5 个纯渲染子函数：

```rust
// 新文件：src/render/sidebar.rs
fn render_sidebar(app: &mut App, ui: &mut egui::Ui) { ... }

// 新文件：src/render/chat.rs
fn render_chat_area(app: &mut App, ui: &mut egui::Ui) { ... }

// 新文件：src/render/input_bar.rs
fn render_input_bar(app: &mut App, ui: &mut egui::Ui) { ... }

// 新文件：src/render/panels.rs
fn render_panels(app: &mut App, ctx: &egui::Context) { ... }

// 新文件：src/render/toasts.rs
fn render_toasts(app: &mut App, ctx: &egui::Context) { ... }
```

**约束**：
- 子函数**禁止**在 `ui` 作用域外进行 I/O（文件读写、网络请求、锁获取）。
- 所有副作用（如 `self.state.cached_settings.lock()`）在 `update()` 入口处一次性完成，结果通过 `&mut App` 字段传递。
- 拆分后 `update()` 本体控制在 80 行以内。

#### 1.2 `std::sync::Mutex` → `parking_lot::Mutex`（P0）

**理由**：`parking_lot::Mutex` 无 poison 状态，`lock()` 不返回 `Result`，直接消除 12 处 `unwrap()`。

**范围**：
- `AppState.cached_settings: std::sync::Mutex<GuiSettings>` → `parking_lot::Mutex<GuiSettings>`
- `AppState.llm_binding: std::sync::Mutex<...>` → `parking_lot::Mutex<...>`
- `AppState.prewarm_error: std::sync::Mutex<...>` → `parking_lot::Mutex<...>`

**验证**：替换后 `grep -rn "lock().unwrap()" crates/clarity-egui` 返回空。

#### 1.3 `Theme` 度量前置化（P1）

**当前问题**：`theme.dark()` / `theme.light()` 每次调用都重新构造 30+ 个 `Color32`，产生大量堆分配。

**修复**：
```rust
// Theme 结构体改为缓存实例
pub struct ThemeCache {
    pub dark: Theme,
    pub light: Theme,
}

impl ThemeCache {
    pub fn new() -> Self {
        // 冷路径：一次性计算所有颜色
        Self { dark: Theme::compute_dark(), light: Theme::compute_light() }
    }
}
```

`App` 初始化时构造 `ThemeCache`，运行时仅做 `theme_cache.dark` 指针拷贝。

#### 1.4 移除 `anyhow` 死依赖（P2）

`Cargo.toml:19` 声明 `anyhow` 但代码中零引用。直接移除。

**Phase 1 验收命令**：
```bash
cd crates/clarity-egui
cargo clippy --lib --bins --tests -D warnings
cargo test --lib
cargo doc --no-deps
grep -rn "lock().unwrap()" src/   # 必须为空
grep -rn "anyhow" src/ Cargo.toml # 必须为空（除可能的历史迁移注释）
```

---

### Phase 2：确定性输出硬化（模型外推 + 测试注入）— 2 周

**目标**：建立纯函数边界、结构化错误、单元测试基线。

#### 2.1 错误类型结构化（P0）

替换所有 `Result<(), String>` 为内部错误枚举：

```rust
// src/error.rs（新建）
#[derive(Debug, thiserror::Error)]
pub enum EguiError {
    #[error("IO failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Config parse failed: {0}")]
    ConfigParse(String),
    #[error("LLM not configured")]
    LlmNotConfigured,
    #[error("Network offline")]
    NetworkOffline,
}
```

**范围**：`settings.rs`、`app_state.rs`、`mcp_panel.rs` 中的 `String` 错误。

#### 2.2 `markdown.rs` 纯函数测试（P0）

`parse_markdown` 是纯函数（输入字符串 → 输出 `Vec<RenderBlock>`），最易测试且零依赖 egui 运行时。

**测试矩阵**：
| 场景 | 输入 | 期望输出 |
|------|------|---------|
| 空输入 | `""` | `vec![]` |
| 纯段落 | `"hello world"` | `vec![Paragraph([Text("hello world")])]` |
| 三级标题 | `"### Title"` | `vec![Heading(3, [Text("Title")])]` |
| 混合 | `"# H1\n\npara\n\n> quote"` | `Heading(1, ...)`, `Paragraph(...)`, `Blockquote(...)` |
| 边界：非法 UTF-8 边界 | `"# \xF0\x90\x80"` | 不 panic（容错） |

#### 2.3 `settings.rs` 序列化测试（P1）

测试 `GuiSettings` 的 load/save round-trip：
- 默认配置写入磁盘 → 重新读取 → 字段全等。
- 旧版本配置（缺字段）读取 → 正确 fallback 到 default。
- 非法路径（`\0` 字符）→ 返回 `EguiError::Io` 而非 panic。

#### 2.4 `theme.rs` hex 解析测试（P1）

`hex_to_color32` 是纯函数，测试边界：
- `"#FF5733"` → `Color32::from_rgb(255, 87, 51)`
- `"FF5733"`（缺 `#`）→ 容错或错误
- `"#GGG"`（非法 hex）→ fallback 到 `Color32::BLACK`（当前行为需文档化）
- `""` → fallback

#### 2.5 `App` 状态机契约文档（P2）

`AgentStatus`（`Offline`/`Online`/`Loading`/`Error`）的转移图以 doc comment 形式写入代码：

```rust
/// Agent 运行状态机。
///
/// # 状态转移图
/// ```text
/// Offline --(ensure_llm 成功)--> Online
/// Online  --(send 调用)-------> Loading
/// Loading --(UiEvent::Done)----> Online
/// Loading --(UiEvent::Error)---> Error
/// Error   --(用户重试)---------> Loading
/// ```
pub enum AgentStatus { ... }
```

**Phase 2 验收命令**：
```bash
cargo test -p clarity-egui --lib           # ≥ 20 测试通过
cargo clippy -p clarity-egui --lib --bins --tests -D warnings
grep -rn "Result<.*String>" src/           # 必须为 0（允许测试中的 assert message）
grep -rn "pub fn" src/ | wc -l            # pub fn 总数
grep -rn "///" src/ | grep "pub fn" | wc -l  # 有 doc 的 pub fn 数，覆盖率 ≥ 90%
```

---

### Phase 3：长期维护契约（文档 + 监控 + 防退化）— 2 周

**目标**：建立工程规范，防止健康度退化。

#### 3.1 `crates/clarity-egui/AGENTS.md`（P0）

新建 crate 级 AGENTS.md，固化以下约束：
- `App::update()` 不得超过 100 行；新增面板必须通过独立 `render_*` 函数。
- 禁止在 `render_*` 函数中调用 `std::fs` / `reqwest` / `tokio::spawn`。
- 新增 `pub fn` 必须配 `///` doc；必须含至少一个单元测试（纯逻辑）或集成测试（UI 流程）。
- `Mutex` 统一使用 `parking_lot::Mutex`；禁止引入 `std::sync::Mutex`。
- clippy warning 零容忍；新增 warning 即阻断 merge。

#### 3.2 CI workflow 增补（P1）

`.github/workflows/check.yml` 中增加 egui 专用 job：

```yaml
  egui:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-cache@v2
      - run: cargo clippy -p clarity-egui --lib --bins --tests -D warnings
      - run: cargo test -p clarity-egui --lib
      - run: cargo doc --no-deps -p clarity-egui
```

#### 3.3 unwrap 债务看板自动化（P1）

扩展 `scripts/verify.ps1`，增加 egui 专项扫描：

```powershell
# 新增段
echo "=== clarity-egui health scan ==="
$unwraps = (grep -rn "\.unwrap()\|\.expect(" crates/clarity-egui/src --include="*.rs" | grep -v "test" | Measure-Object).Count
$docs = (grep -rn "pub fn" crates/clarity-egui/src --include="*.rs" | Measure-Object).Count
$docd = (grep -rn "///" crates/clarity-egui/src --include="*.rs" | grep "pub fn" | Measure-Object).Count
$tests = (grep -rn "#\[test\]" crates/clarity-egui/src --include="*.rs" | Measure-Object).Count
echo "unwrap/expect (non-test): $unwraps"
echo "doc coverage: $docd / $docs"
echo "tests: $tests"
```

#### 3.4 `docs/ai-protocol.md` 健康度锚点更新（P2）

在 ai-protocol.md 中新增 egui 健康度快照节，每次发布前更新。

**Phase 3 验收命令**：
```bash
./scripts/verify.ps1 --egui    # 输出 JSON，全绿
# 或手动等效：
cargo clippy -p clarity-egui --lib --bins --tests -D warnings
cargo test -p clarity-egui --lib
cargo doc --no-deps -p clarity-egui
```

---

## 四、任务优先级与时间表

| 阶段 | 周次 | P0 任务 | P1 任务 | P2 任务 | 阻塞解除条件 |
|------|------|---------|---------|---------|-------------|
| **Phase 1** | W1-W2 | update() 拆分、Mutex 替换 | Theme 缓存 | anyhow 移除 | 无 |
| **Phase 2** | W3-W4 | 错误枚举、markdown 测试 | settings 测试、theme 测试 | 状态机文档 | Phase 1 验收通过 |
| **Phase 3** | W5-W6 | AGENTS.md | CI job | unwrap 看板 | Phase 2 验收通过 |

**每日使用保障**：
- 所有改动必须在 `cargo run -p clarity-egui` 下肉眼验证 UI 无回归。
- 每阶段 PR 需附带截图：拆分前后的 UI 对比（应完全无视觉差异）。

---

## 五、风险与冻结项

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|---------|
| `parking_lot` 与 `tokio::sync` 混用导致死锁 | 中 | 高 | 统一策略：`AppState` 内部全用 `parking_lot`；与 `clarity-core` 的 async 边界通过 channel 隔离，不跨 crate 传锁。 |
| `update()` 拆分引入 UI 状态同步 bug | 中 | 高 | 拆分前用 `git diff --patience` 逐行审计；拆分后跑 30 分钟手动交互测试（新建会话、发送消息、切换设置、拖拽文件）。 |
| 测试依赖 egui context 难以构造 | 低 | 中 | 纯逻辑（markdown/settings/theme）零依赖 egui；UI 交互测试留到 Phase 3 后评估 `egui_kittest`。 |
| 人力不足，6 周计划无法完成 | 中 | 中 | **硬截止**：Phase 1 必须在 2 周内完成；若超期，冻结 Phase 2/3，将剩余工作拆分为独立 Issue 归档。 |

**冻结项（本 plan 期间不触碰）**：
- 不新增 egui 插件/第三方 widget。
- 不重构 `clarity-core` 侧接口（`AppState` 的字段增减需最小化）。
- 不改动 Tauri/TUI 代码（零交叉影响）。
- 不引入 `egui_kittest` 或 snapshot 测试（Phase 3 后评估）。

---

## 六、Pretext 思想在本 plan 中的元认知声明

> **性质**：本节为工程启发式，非学术理论。

Pretext 的"冷热路径分离"在 UI 工程中的映射：**渲染帧 = 热路径，初始化 = 冷路径**。

本 plan 的所有动作可归结为一句话——**将 `App::update()` 从"每帧重新做决策"转化为"每帧只做算术组合"**。

- 拆分 `update()` = 将决策树扁平化，减少每帧的条件分支数。
- `parking_lot::Mutex` = 消除锁获取的故障模式，使锁操作从"可能 panic 的计算"变为"确定性算术"。
- `ThemeCache` = 将颜色构造从热路径移到冷路径。
- 单元测试 = 验证纯函数的确定性输出，使行为可计算、可缓存、可回归验证。

**反叙事槽位**（对立面证据）：
- `parking_lot` 比 `std::sync` 多一个外部依赖（但已在 Rust 生态中被 `tokio`、`rayon` 等广泛验证，且 binary size 增加 < 50KB）。
- 拆分函数可能增加栈深度（但在 egui 的即时模式渲染中，栈深度本就在 10-20 层，影响可忽略）。
- 单元测试对 UI 代码的覆盖有限（故本 plan 仅要求纯逻辑模块测试，UI 测试留待后续评估）。

---

*Plan created by agent on 2026-04-27*
*生效条件：人类开发者确认后执行*
*审查周期：每 2 周对照本 plan 的验收命令执行一次健康度扫描*
