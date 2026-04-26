# Clarity Unwrap/Expect 债务地图

> 生成日期：2026-04-26  
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
| `tauri::Builder...expect(...)` | 2 | 初始化期 |
| doc 注释示例 | 2 | 不执行 |

### 🟡 中风险 — 需加 `// SAFE: ...` 注释

| 文件 | 行 | 代码 | 说明 |
|------|----|------|------|
| `agent/prompt.rs` | 156 | `.unwrap()` | 需查看上下文 |
| `memory/mod.rs` | 228 | `.unwrap()` | 需查看上下文 |
| `skills/registry.rs` | 67, 78 | `.unwrap()` | 需查看上下文 |
| `subagents/runner.rs` | 834 | `.unwrap()` | 需查看上下文 |
| `tools/web.rs` | 466 | `.unwrap()` | 需查看上下文 |
| `memory/compiler.rs` | 395 | `.unwrap()` | 需查看上下文 |
| `memory/extractor.rs` | 438 | `.unwrap()` | 需查看上下文 |
| `tauri/commands/task.rs` | 59 | `.unwrap()` | 需查看上下文 |
| `tui/widgets/generating_indicator.rs` | 38, 48 | `.unwrap()` | 需查看上下文 |

### 🔴 高风险 — 优先 `?` 化或改为 safe 模式

| 排名 | 文件 | 行 | 代码 | 风险 |
|------|------|----|------|------|
| 1 | `subagents/store.rs` | 63 | `self.in_memory.get(&agent_id).unwrap()` | agent_id 不存在 → panic |
| 2 | `gateway/channels/webhook.rs` | 125-126 | `self.auth_header.as_ref().unwrap()` | auth 未配置 → panic |
| 3 | `memory/embedding.rs` | 393 | `scores.sort_by(...partial_cmp().unwrap())` | NaN score → panic |
| 4 | `memory/embedding.rs` | 455 | `results.sort_by(...partial_cmp().unwrap())` | NaN score → panic |
| 5 | `tui/main.rs` | 127 | `memory_db_path.parent().unwrap()` | 根路径 → panic |
| 6 | `tauri/commands/task.rs` | 50 | `path.parent().unwrap()` | 根路径 → panic |
| 7 | `agent/enhanced.rs` | 260 | `last_error.expect("Last error should exist")` | 逻辑路径错误 → panic |

---

## 3. 清理路线图（修正版）

### 立即可做（≤ 30 分钟，6 处）

1. **`subagents/store.rs` L63**：`get(&agent_id).unwrap()` → `ok_or(SubagentError::NotFound)?`
2. **`gateway/channels/webhook.rs` L125-126**：`as_ref().unwrap()` → `if let Some(...) = ...`
3. **`memory/embedding.rs` L393, 455**：`partial_cmp().unwrap()` → `partial_cmp().unwrap_or(Ordering::Equal)`
4. **`tauri/commands/task.rs` L50**：`path.parent().unwrap()` → `ok_or("invalid path")?`
5. **`tui/main.rs` L127**：`path.parent().unwrap()` → `ok_or("invalid path")?`

### 本周内（需理解上下文）

6. **`agent/enhanced.rs` L260**：`last_error.expect(...)` → 改为 `match last_error` 避免 panic
7. **中风险 9 处**：逐个查看上下文，加 `// SAFE: ...` 或 `?` 化

### 长期冻结

- 同步原语 92 处：不投入，鼓励新代码用 `tokio::sync`
- Regex 30 处：不投入，编译期安全
- 初始化期 expect：不投入，启动失败即 panic 是合理行为

---

## 4. PR Self-Review 检查单（精简版）

- [ ] 新增 `unwrap()` 是否已配 `// SAFE: ...` 注释？
- [ ] 是否优先使用 `?` 而非 `unwrap()`？
- [ ] `HashMap::get()` / `Option::unwrap()` 在 API 边界是否返回 `Result`？

---

*本文件由精确扫描生成（Python AST-aware，排除 `#[cfg(test)]`）。之前版本高估了债务规模，特此修正。*
