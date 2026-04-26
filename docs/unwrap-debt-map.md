# Clarity Unwrap/Expect 债务地图

> 生成日期：2026-04-26  
> 范围：`clarity-core` + `clarity-gateway` + `clarity-memory` 非测试代码  
> 测量命令：`grep -rn "\.unwrap()\|\.expect(" crates/*/src --include="*.rs" | grep -v "test"`

---

## 1. 总量基线

| 指标 | 数量 | 备注 |
|------|------|------|
| `unwrap()` / `expect()` 总量 | **947** | 非测试代码 |
| 同步原语类（`lock/read/write`） | ~44 | 低风险，允许保留 |
| `await` / `handle.await` 类 | ~59 | 中低风险，async 边界 |
| **非同步原语、非 await 类（高风险）** | **~637** | 需逐步清理 |

---

## 2. 按风险等级分类

### 🟢 低风险 — 允许保留（已评估）

| 模式 | 示例 | 说明 |
|------|------|------|
| 同步原语 | `mutex.lock().unwrap()` | 锁 poison 即 panic 是合理行为 |
| 同步原语 | `rwlock.read().unwrap()` | 同上 |
| 同步原语 | `rwlock.write().unwrap()` | 同上 |
| async join | `handle.await.unwrap()` | Tokio JoinHandle 的取消语义 |

### 🟡 中风险 — 需配 `// SAFE: ...` 注释

| 模式 | 示例 | 说明 |
|------|------|------|
| 初始化期配置 | `config.parse().unwrap()` | 仅在启动期调用一次 |
| 测试辅助 | `test_data.to_string().unwrap()` | 非生产路径 |
| 内部状态转换 | `enum_from_str(s).unwrap()` | 输入来自内部生成，非外部 |

### 🔴 高风险 — 优先 `?` 化

| 模式 | 示例 | 典型文件 | 风险 |
|------|------|---------|------|
| JSON 解析 | `serde_json::from_str(x).unwrap()` | `handlers.rs`, `webhook.rs`, `ollama.rs` | 无效 JSON → panic |
| 字符串解析 | `s.parse::<T>().unwrap()` | `config.rs`, `model_registry.rs` | 格式错误 → panic |
| 文件 IO | `fs::read_to_string(p).unwrap()` | `compiler.rs`, `registry.rs` | 路径不存在 → panic |
| 路径操作 | `PathBuf::from(s).canonicalize().unwrap()` | `tools/file.rs` | 权限拒绝 → panic |
| 网络响应 | `response.json().await.unwrap()` | `handlers.rs`, `web.rs` | 非 JSON 响应 → panic |
| 数据库操作 | `conn.execute(sql).unwrap()` | `session_store.rs`, `lib.rs` | 约束冲突 → panic |

---

## 3. Top 15 高风险文件（非 lock/await 类 unwrap 密度）

| 排名 | 文件 | unwrap/expect 数 | 主要风险模式 |
|------|------|-----------------|-------------|
| 1 | `clarity-memory/src/session_store.rs` | 32 | 数据库 execute/query |
| 2 | `clarity-core/src/tools/web.rs` | 32 | 网络响应解析、HTML 提取 |
| 3 | `clarity-memory/src/lib.rs` | 27 | 数据库连接、BM25 索引 |
| 4 | `clarity-core/src/approval/mod.rs` | 27 | 规则引擎求值 |
| 5 | `clarity-gateway/src/handlers.rs` | 23 | JSON 序列化、请求体解析 |
| 6 | `clarity-core/src/tools/web_browser.rs` | 23 | CDP 消息解析 |
| 7 | `clarity-core/src/background/store.rs` | 23 | Session 存储序列化 |
| 8 | `clarity-core/src/background/mod.rs` | 22 | Task 状态转换 |
| 9 | `clarity-core/src/subagents/runner.rs` | 18 | 子代理输出解析 |
| 10 | `clarity-core/src/llm/ollama.rs` | 16 | Ollama 响应 JSON 解析 |
| 11 | `clarity-core/src/mcp/config.rs` | 16 | MCP 配置解析 |
| 12 | `clarity-memory/src/extractor.rs` | 16 | 文本提取 |
| 13 | `clarity-core/src/mcp/enhanced.rs` | 16 | MCP 消息解析 |
| 14 | `clarity-core/src/notifications/mod.rs` | 15 | 通知序列化 |
| 15 | `clarity-core/src/skills/registry.rs` | 14 | Skill 元数据解析 |

---

## 4. 清理路线图

### Phase 1（每次 PR 附带，无独立 deadline）

**规则**：任何修改上述 15 个文件的 PR，必须附带 "此 PR 减少 X 处风险类 unwrap" 的 self-review。

**优先级**：
1. `handlers.rs`（Gateway 对外接口，panic = 服务崩溃）
2. `web.rs` / `web_browser.rs`（工具调用，panic = 用户请求失败）
3. `session_store.rs` / `lib.rs`（Memory 层，panic = 数据丢失）
4. `ollama.rs` / `mcp/enhanced.rs`（Provider 层，panic = 模型不可用）

### Phase 2（v0.4.0 前，集中清理）

目标：上述 Top 15 文件的风险类 unwrap 减少 **50%**（从 ~320 降至 ~160）。

策略：
- 将 `unwrap()` 替换为 `?` + `AgentError` / `anyhow::Error`
- 在 API 边界（Gateway handlers、Tool execute）使用 `Result` 传播
- 在内部模块（Memory store、Background store）使用 `thiserror` 定义精确错误类型

### Phase 3（v0.5.0+，长期维护）

目标：全 workspace 非 lock/await 类 unwrap 降至 **<300**。

---

## 5. PR Self-Review 检查单

提交代码前，若修改了本列表中的文件，请检查：

- [ ] 新增 `unwrap()` 是否已配 `// SAFE: <不变量说明>` 注释？
- [ ] 是否优先使用 `?` 而非 `unwrap()`？
- [ ] JSON/字符串/路径解析是否返回 `Result` 而非 panic？
- [ ] 本 PR 净减少了几处风险类 unwrap？（期望 ≥0，鼓励 >0）

---

*本文件由代码健康评估生成，每次重大版本发布时更新基线。*
