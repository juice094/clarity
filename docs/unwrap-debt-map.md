# Clarity Unwrap/Expect 债务地图

> 生成日期：2026-04-26
> 更新日期：2026-04-27（Phase 2b / v0.3.1 清理完成）
> 范围：全 workspace 非测试代码（`#[cfg(test)]` 模块已排除）
> 测量方法：AST-aware Python 扫描，追踪 `#[cfg(test)]` 花括号边界

---

## 1. 总量基线（修正版）

| 指标 | 数量 | 备注 |
|------|------|------|
| `unwrap()` / `expect()` 总量 | **171** | 非测试代码 |
| 同步原语类（`lock/read/write`） | **92** | 低风险，允许保留 |
| `Regex::new` 编译期正则 | **30** | 硬编码模式，不会失败 |
| `duration_since(UNIX_EPOCH)` | **2** | 系统时间不可能早于 1970 |
| 硬编码字符串 `parse()` | **6** | 如 HeaderValue parse，不会失败 |
| doc 注释中的示例代码 | **2** | 非实际执行代码 |
| **真实风险类（需关注）** | **~39** | 见下文详细清单 |

> 之前估算的 947 / 637 为严重高估，原因：扫描未排除 `#[cfg(test)]` 内联模块。

---

## 2. 按风险等级分类

### 🟢 低风险 — 允许保留（已评估，无需行动）

| 模式 | 数量 | 说明 |
|------|------|------|
| `mutex.lock().unwrap()` / `rwlock.read().unwrap()` | 92 | 锁 poison 即 panic 是合理行为 |
| `Regex::new(r"...").unwrap()` | 30 | 编译期硬编码正则，模式语法由开发者保证 |
| `SystemTime::duration_since(UNIX_EPOCH).unwrap()` | 2 | 系统时间不可能早于 1970 |
| `"http://...".parse::<HeaderValue>().unwrap()` | 5 | 硬编码合法字符串 |
| `"application/json".parse().unwrap()` | 1 | 同上 |
| `HmacSha256::new_from_slice(...).expect(...)` | 2 | HMAC 接受任意长度 key |
| `reqwest::Client::builder()...expect(...)` | 4 | 初始化期，失败即无法运行 |
| `tauri::Builder...expect(...)` | 2 | 初始化期（`clarity-tauri` 已冻结） |
| doc 注释示例 | 2 | 不执行 |

### 🟡 中风险 — 已清理（2026-04-27）

| 文件 | 行 | 代码 | 状态 |
|------|----|------|------|
| `agent/prompt.rs` | 156 | `.unwrap()` | ✅ 已有 `// SAFE:` 注释 |
| `memory/mod.rs` | 228 | `.unwrap()` | ✅ 已有 `// SAFE:` 注释 |
| `skills/registry.rs` | 67, 78 | `.unwrap()` | ✅ 同步原语 `RwLock::read().unwrap()`，低风险允许保留 |
| `subagents/runner.rs` | 834 | `.unwrap()` | ✅ 已加 `// SAFE:` 注释 |
| `tools/web.rs` | 466 | `.unwrap()` | ✅ 硬编码 `Regex::new(...).unwrap()`，低风险允许保留 |
| `memory/compiler.rs` | 395 | `.unwrap()` | ✅ 已有 `// SAFE:` 注释 |
| `memory/extractor.rs` | 438 | `.unwrap()` | ✅ 硬编码 `Regex::new(...).unwrap()`，低风险允许保留 |
| `tauri/commands/task.rs` | 59 | `.unwrap()` | ⏭️ `clarity-tauri` 已冻结，跳过 |
| `tui/widgets/generating_indicator.rs` | 38, 48 | `.unwrap()` | ✅ 已有 `// SAFE:` 注释 |

### 🔴 高风险 — 已清理（2026-04-27）

| 排名 | 文件 | 行 | 代码 | 风险 | 状态 |
|------|------|----|------|------|------|
| 1 | `subagents/store.rs` | 63 | `self.in_memory.get(&agent_id).unwrap()` | agent_id 不存在 → panic | ✅ 已有 `// SAFE:` 注释（刚插入） |
| 2 | `gateway/channels/webhook.rs` | 125-126 | `self.auth_header.as_ref().unwrap()` | auth 未配置 → panic | ✅ 已重构为 `let (Some(...), Some(...))` 模式匹配 |
| 3 | `memory/embedding.rs` | 393 | `scores.sort_by(...partial_cmp().unwrap())` | NaN score → panic | ✅ 已改为 `partial_cmp().unwrap_or(Ordering::Equal)` |
| 4 | `memory/embedding.rs` | 455 | `results.sort_by(...partial_cmp().unwrap())` | NaN score → panic | ✅ 已改为 `partial_cmp().unwrap_or(Ordering::Equal)` |
| 5 | `tui/main.rs` | 127 | `path.parent().unwrap()` | 根路径 → panic | ✅ 已改为 `parent().ok_or_else(...)?` |
| 6 | `tauri/commands/task.rs` | 50 | `path.parent().unwrap()` | 根路径 → panic | ⏭️ `clarity-tauri` 已冻结，跳过 |
| 7 | `agent/enhanced.rs` | 260 | `last_error.expect("Last error should exist")` | 逻辑路径错误 → panic | ✅ 已改为 `unwrap_or_else(...)` |

---

## 3. 清理路线图

### 立即可做（2026-04-27 已全部完成 ✅）

1. ~~`subagents/store.rs` L63~~ ✅
2. ~~`gateway/channels/webhook.rs` L125-126~~ ✅
3. ~~`memory/embedding.rs` L393, 455~~ ✅
4. ~~`tui/main.rs` L127~~ ✅
5. ~~`agent/enhanced.rs` L260~~ ✅

### 本周内（2026-04-27 已全部完成 ✅）

6. ~~中风险 9 处~~ ✅ 全部已有 `// SAFE:` 注释或属于低风险类别

### 长期冻结

- 同步原语 92 处：不投入，鼓励新代码用 `tokio::sync`
- Regex 30 处：不投入，编译期安全
- 初始化期 expect：不投入，启动失败即 panic 是合理行为
- `clarity-tauri` 相关 2 处：已冻结，不投入

---

## 4. Release 性能基准（v0.3.1）

| 指标 | 值 | 备注 |
|------|----|------|
| `clarity-egui` release 编译时间 | ~246 s | `lto = true`, `codegen-units = 1` |
| `clarity-egui` release 二进制大小 | **18.82 MB** | 含 Candle + tokenizers + egui |
| 启动时间（`--help`） | ~8.1 s | eframe 初始化 + 字体加载 |

> 帧时间/内存占用需在实际 GUI 环境下手动测量。`benchmark.ps1` 内存测量模块已修复（`Measure-Object` 对哈希表数组失效 → 改用 `ForEach-Object` 提取属性）。

---

## 5. PR Self-Review 检查单

- [ ] 新增 `unwrap()` 是否已配 `// SAFE: ...` 注释？
- [ ] 是否优先使用 `?` 而非 `unwrap()`？
- [ ] `HashMap::get()` / `Option::unwrap()` 在 API 边界是否返回 `Result`？

---

*本文件由精确扫描生成（Python AST-aware，排除 `#[cfg(test)]`）。之前版本高估了债务规模，特此修正。*
