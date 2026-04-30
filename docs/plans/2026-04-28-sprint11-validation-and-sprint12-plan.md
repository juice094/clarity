# Sprint 11 验证与 Sprint 12 规划

> 状态: Plan 模式
> 日期: 2026-04-28
> 前置: Sprint 11 A/B/C 全阶段代码已完成并推送 (`684cbca`)
> 测试基线: 407 passed, 0 failed, 6 ignored | Clippy 0 warnings

---

## 一、当前状态快照

Sprint 11 目标"超越 Kimi CLI"的代码层面已交付：

| 阶段 | 交付物 | 验证状态 |
|------|--------|---------|
| A | SystemPromptBuilder 汇流 GitContext + ActiveFiles + ProjectMetadata | ❌ 未经验证 |
| B | file_edit 批量替换 + unified diff 预览 | ❌ 未经验证 |
| C | TUI /yolo/interactive/planmode + Headless stdin | ❌ 未经验证 |

**关键缺口**: Layer 3 工作流穿透（能力汇流审计协议）未执行——没有真实编码任务验证上下文注入是否真正提升了 Agent 的编码精度。

---

## 二、阶段 V1: Sprint 11 风险点清偿（必须先于验证）

### V1.1 批量替换原子性（P0，数据安全）

**问题**: `replacements` 数组第 N 条失败时，前 N-1 条已落盘，文件处于半修改状态。

**方案**:
```rust
// 当前：逐条替换，逐条落盘
// 目标：全部在内存中替换，验证全部命中后一次性写入

let mut current_content = content.clone();
for (idx, (old, new)) in replacements.iter().enumerate() {
    if !current_content.contains(old) {
        return Err(...); // 此时磁盘仍是原始内容 ✅
    }
    current_content = current_content.replacen(old, new, 1);
}
fs::write(&path, &current_content).await?; // 一次性写入
```

**工作量**: ~30 分钟
**验证**: `test_file_edit_batch_replacement_missing_pattern` 应仍通过（失败时磁盘未修改）

### V1.2 active_file_paths 保留目录结构（P1，上下文质量）

**问题**: `build_active_files_context()` 只取 `file_name()`，`src/main.rs` 和 `tests/main.rs` 都显示为 `main.rs`。

**方案**: 改用相对于 `working_dir` 的路径：
```rust
let lines: Vec<String> = paths
    .iter()
    .filter_map(|p| {
        p.strip_prefix(working_dir)
            .unwrap_or(p)
            .to_string_lossy()
            .to_string()
    })
    .collect();
```

**工作量**: ~30 分钟
**验证**: 新增测试 `test_build_active_files_context_preserves_dir_structure`

### V1.3 TUI approval_mode 状态指示器（P2，体验）

**问题**: 用户切换模式后，界面上没有任何地方显示当前模式，容易遗忘。

**方案**: 在 TUI 标题栏或底部状态栏追加 `[Interactive]` / `[YOLO]` / `[Plan]` 标记。

**工作量**: ~1 小时（需了解 TUI 渲染布局）
**优先级**: 可选，验证阶段可跳过

---

## 三、阶段 V2: 端到端验证方案

### 验证目标
确认"超越 Kimi CLI"不是叙事幻觉，而是可观测的能力提升。

### 验证场景设计

**场景 1: 上下文感知验证**
```
输入: "在这个仓库里，给我概述当前项目的结构和最近的变更"
预期: Agent 的响应中应引用当前分支名、未提交文件数、Cargo.toml 中的 package name
观察: System Prompt 中是否正确注入了 GitContext / ProjectMetadata
```

**场景 2: 批量编辑验证**
```
输入: "把 src/lib.rs 中所有的 'foo' 替换为 'bar'，'baz' 替换为 'qux'"
预期: file_edit 一次性完成两条替换，返回 unified diff patch，TUI diff popup 正确渲染
观察: replacements 数组是否正确路由；diff patch 格式 TUI 是否能解析
```

**场景 3: 模式切换验证**
```
输入: /yolo
      "删除 target/ 目录下的所有文件"
预期: 工具自动执行，无 diff popup 拦截
观察: approval_mode 是否正确切换；Yolo 模式下 _diff_preview 是否不出现
```

### 验证执行步骤

1. `cargo run -p clarity-tui` 启动
2. 输入场景 1，观察响应质量（截图/复制文本）
3. 输入场景 2，观察 diff popup 渲染
4. 输入 `/yolo`，再输入场景 3，观察是否自动执行
5. 记录偏差：预期行为 vs 实际行为

### 验收标准

| 检查项 | 通过标准 |
|--------|---------|
| 上下文注入 | Agent 能在响应中引用分支名和项目元数据 |
| 批量替换 | 单条 file_edit 调用完成多条替换，返回正确 diff |
| 模式切换 | `/yolo` 后敏感工具无审批弹窗，直接执行 |
| 回归 | 原有功能（Skill 激活、Plan 生成、记忆检索）不受影响 |

### 验证风险

| 风险 | 概率 | 缓解 |
|------|------|------|
| 无 API Key / 网络不可用 | 中 | 提前确认 `KIMI_API_KEY` 或切换到 local provider |
| LLM 响应不符合预期（幻觉） | 高 | 不依赖 LLM 质量，只验证上下文是否注入（检查 System Prompt） |
| TUI 渲染异常 | 低 | 若 TUI 崩溃，切换到 `cargo run -p clarity-headless` 验证 |

---

## 四、阶段 V3: Sprint 12 方向决策（验证后）

Sprint 12 的方向取决于 V2 验证结果：

### 决策矩阵

| 验证结果 | Sprint 12 方向 | 理由 |
|---------|---------------|------|
| 上下文注入效果显著 | **egui 功能补齐** | core 能力已完备，最大缺口是 egui 功能暴露（审批弹窗、Plan 可视化、Skill UI） |
| 上下文注入效果一般 | **Prompt 工程优化** | 需要调整上下文注入策略（深度、过滤规则、优先级） |
| 批量替换/diff 有问题 | **编辑精度回修** | 修复 Phase B 遗留问题后再扩展 |
| 全部验证通过 | **egui 功能补齐 + Phase 4 记忆推送** | 两条线并行 |

### Sprint 12 候选方向详述

**方向 A: egui 功能补齐（推荐，如果验证通过）**
- 审批弹窗 UI（Interactive/Plan 模式可用）
- Plan 步骤可视化面板
- Skill 列表、激活/切换界面
- Token 用量显示

**方向 B: Prompt 工程与上下文优化**
- 文件树过滤规则配置化（排除 target/、node_modules/）
- 元数据读取策略优化（保留 [package] 和 [dependencies]）
- 上下文注入的 token 预算控制（避免 System Prompt 过长）

**方向 C: 记忆主动推送（Phase 4）**
- 连续多轮围绕同一文件 → 自动注入历史编辑
- Tool Call 失败 → 检索同类错误的历史解决方案
- Plan 模式 → 检索过去同类 Plan 的执行记录

---

## 五、全周期风险预测矩阵

| 风险 | 阶段 | 概率 | 影响 | 缓解方案 |
|------|------|------|------|---------|
| 原子性修复引入新 bug | V1 | 低 | 高 | 增加边界测试（空替换、重叠替换） |
| 端到端验证阻塞（API/网络） | V2 | 中 | 中 | 准备 offline 验证路径（local provider） |
| egui 审批 UI 复杂度超预期 | V3-A | 中 | 高 | 先用 protocol_renderer.rs 的 VStack/HStack 做原型 |
| 秋招时间压力导致功能砍半 | V3 | 高 | 高 | Sprint 12 采用"最小可用闭环"策略，不做完美主义 |
| Kimi CLI 模型迭代拉大差距 | V3 | 中 | 低 | Clarity 的差异化（离线、多模型、记忆）是结构性优势，非功能对标 |

---

## 六、时间线

```
Day 1 (4.28): V1.1 + V1.2 风险点清偿 → 提交
Day 2 (4.29): V2 端到端验证 → 记录验证报告
Day 3 (4.30): V3 Sprint 12 决策 → 编写 Sprint 12 计划文档
Week 2-3:    Sprint 12 执行
```

---

## 七、停止条件

- V1 停止: 两个 P0/P1 风险点修复 + 测试全绿
- V2 停止: 三个验证场景全部通过，或记录不可修复的偏差
- V3 停止: Sprint 12 计划文档通过评审（用户确认方向）

---

> 本计划受能力汇流审计协议 v1.0 统辖。验证结论是双向扰动的产物，用户保留对"是否超越 Kimi CLI"的最终裁定权。
