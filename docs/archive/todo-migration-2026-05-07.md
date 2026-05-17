# TODO/FIXME 迁移记录 — 2026-05-07

> Sprint: Phase 2 工程纪律清理
> 原则：代码中不留 TODO/FIXME，架构债务迁移至本文档或 GitHub Issues。

---

## 1. i18n JSON 加载（原 `clarity-egui/src/i18n.rs:21`）

**原标记**：`TODO-WEEK2: Load translation maps from JSON files so locale packs can be added without recompilation.`

**状态**：功能路线图，非阻塞。

**上下文**：当前 i18n 使用硬编码的 `HashMap<&str, &str>`（`ZH_CN`）。要支持 JSON 加载，需要：
1. 在 `clarity_data_dir()` 下创建 `locales/` 目录结构
2. 启动时扫描 `locales/*.json`，合并到 `HashMap<String, String>`
3. 提供运行时切换 locale 的 UI（当前只有 ZhCN/En 两种硬编码枚举）

**优先级**：P3 — 当前只有中英两种语言，硬编码足够。

---

## 2. Chat Handlers App 解耦（原 `clarity-egui/src/handlers/chat.rs:8,26`）

**原标记**：`TODO: decompose App dependency`

**状态**：架构债务，P2。

**上下文**：`on_done` 和 `on_error` 直接接收 `&mut crate::App`，耦合了 5+ 个 store 操作。应拆分为：
- `ChatStore` 状态变更（is_loading, agent_status, stopping）
- `SessionStore` 保存
- `Agent` 重置 / 快照获取
- `UiStore` toast 推送
- 输入队列回退逻辑

**建议**：引入 `ChatTurnResult` 事件类型，由 App 层统一消费，handler 只产生事件不直接操作 App。

---

## 3. Widgets 模块启用（原 `clarity-egui/src/widgets/mod.rs:11,13`）

**原标记**：
- `// pub use settings_row::settings_row; // TODO: enable when integrated into tabs`
- `// pub use toggle::toggle; // TODO: enable when used in UI`

**状态**：非债务，正常开发占位。

**上下文**：`settings_row` 和 `toggle` 模块已存在但未被使用。删除 TODO 标记即可，保留注释说明未启用原因。

---

## 4. model_download 迁出 clarity-core（原 `clarity-core/src/model_download.rs:7`）

**原标记**：`TODO(Sprint-31-debt): migrate to clarity-infrastructure crate.`

**状态**：架构债务，P2。

**上下文**：`model_download` 是首次运行引导功能，依赖 `reqwest`（已在 core 中）和 `tokio::sync::mpsc`。当前位于 `clarity-core` 使其膨胀。但 `clarity-infrastructure` crate 目前不存在，迁出需要：
1. 新建 `clarity-infrastructure` crate（或复用 `clarity-gateway`？）
2. 将 `ModelDownloadProgress` / `ModelDownloadManager` 移出
3. 处理 `clarity-core` 中的引用（`view_models::settings` 等）

**决策**：暂缓迁出。当前 `model_download` 仅 259 行，耦合度低（只被 settings view model 引用），不值得新建 crate。

---

## 统计

| 项目 | 清理前 | 清理后 |
|------|--------|--------|
| TODO/FIXME 代码标记 | 6 | 0 |
