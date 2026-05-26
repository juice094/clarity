---
title: 前端错误处理审计报告
category: Document
date: 2026-05-16
tags: [document]
---

# 前端错误处理审计报告

> 审计日期：2026-04-26  
> 范围：`crates/clarity-tauri/frontend/src/**/*.tsx`  
> 依据：ENGINEERING_PLAN.md P1 — "错误处理审计：所有 `invoke().catch(console.error)` 是否应向前端用户暴露"

---

## 1. 审计方法

```bash
cd crates/clarity-tauri/frontend/src
grep -rn "catch (" --include="*.tsx" .
grep -rn "console\.error\|console\.warn" --include="*.tsx" .
grep -rn "invoke(" --include="*.tsx" . | grep -B2 -A2 "catch\|catch(console"
```

---

## 2. 当前状态总览

| 维度 | 状态 |
|------|------|
| `invoke()` 调用总数 | ~30+ 处 |
| `try/catch` 包裹 | 18 处 |
| `.catch(console.error)` 静默处理 | 12 处 |
| 已有错误 UI 暴露（setError） | ComputerUsePanel, LspPanel |
| 已有日志面板劫持 | App.tsx（Console hijack → 可折叠面板） |
| **无错误暴露** | SettingsPanel, TaskPanel, FileBrowser, OnboardingModal |

---

## 3. 按组件详细审计

### 3.1 App.tsx（日志面板已劫持 console）

| 位置 | 代码 | 当前处理 | 应否暴露 | 建议 |
|------|------|---------|---------|------|
| L121-132 | `get_launch_status` catch | `console.error` + 注释"silently ignore" | ⚠️ 部分 | launch 状态获取失败应显示 banner（但 updater 检查失败可静默） |
| L172 | `load_sessions` catch | `console.error` | ✅ 是 | 应显示 Toast "无法加载会话列表" |
| L196 | `save_session` .catch | `console.error` | ✅ 是 | 应显示 Toast "会话保存失败" |
| L278 | `complete_task` .catch | `.catch(console.error)` | ❌ 否 | 后台任务完成是内部状态，可日志记录 |
| L356 | `refreshStatus` after download | `console.error` | ⚠️ 部分 | 下载后刷新失败，可静默（用户已看到下载完成） |
| L373-374 | `installUpdate` catch | `console.error` | ✅ 是 | 应显示 Toast "更新安装失败" |
| L398 | `complete_task` (interrupt) | `.catch(console.error)` | ❌ 否 | 中断时的清理，可静默 |
| L415 | `complete_task` (new session) | `.catch(console.error)` | ❌ 否 | 创建会话时的清理，可静默 |
| L431 | `save_session` (new session) | `.catch(console.error)` | ✅ 是 | 应显示 Toast "新会话创建失败" |
| L440 | `complete_task` (delete) | `.catch(console.error)` | ❌ 否 | 删除时的清理，可静默 |
| L458 | `save_session` (switch) | `.catch(console.error)` | ✅ 是 | 应显示 Toast "会话切换保存失败" |
| L467-468 | `delete_session` .catch | `console.error` | ✅ 是 | 应显示 Toast "会话删除失败" |
| L494 | `create_task` catch | `console.error` | ✅ 是 | 应显示 Toast "任务创建失败" |
| L502-503 | `agent_run_streaming` catch | 注释"Fallback: show error" | ✅ 是 | **已处理** — 通过 setMessages 显示错误到聊天面板 |
| L521 | `complete_task` (stream error) | `.catch(console.error)` | ❌ 否 | 流错误时的清理，可静默 |

** verdict **：App.tsx 有 Console 劫持机制（L91-108），所有 `console.error` 已自动重定向到前端日志面板。但关键用户操作（save/delete session、create task）仍缺少 **主动 Toast 通知**。

### 3.2 SettingsPanel.tsx（无错误暴露）

| 位置 | 操作 | 当前处理 | 应否暴露 | 建议 |
|------|------|---------|---------|------|
| L66-67 | `get_settings` | `console.error` | ✅ 是 | 应显示 error banner 或 Toast |
| L79-80 | `get_available_models` / `get_approval_modes` / `get_local_models` | `console.error` | ✅ 是 | 应显示 settings error banner |
| L185 | `set_approval_mode` | `console.error` | ⚠️ 部分 | 可在 save Toast 中附带失败提示 |
| L190 | `reload_llm` | `console.error` | ⚠️ 部分 | 可在 save Toast 中附带失败提示 |
| L192-193 | `save_settings` | `console.error` | ✅ 是 | **已部分处理** — 有 `setToast(t("settings.saved"))`，但 catch 分支无 Toast |
| L342-343 | `download_model` | `console.error` | ✅ 是 | 应更新 download status 为 error 并显示提示 |

**verdict**：SettingsPanel 有 `setToast` 机制但仅在成功路径使用。所有 catch 分支应补充 `setToast(t("settings.error"))` 或设置 `error` 状态。

### 3.3 TaskPanel.tsx（无错误暴露）

| 位置 | 操作 | 当前处理 | 应否暴露 | 建议 |
|------|------|---------|---------|------|
| L25-26 | `list_tasks` | `console.error` | ✅ 是 | 应显示 error banner |
| L41-42 | `cancel_task` | `console.error` | ✅ 是 | 应显示 Toast "取消失败" |

### 3.4 FileBrowser.tsx（无错误暴露）

| 位置 | 操作 | 当前处理 | 应否暴露 | 建议 |
|------|------|---------|---------|------|
| L36 | `file_tree` | `console.error` | ✅ 是 | 应显示 error 状态替代空树 |

### 3.5 OnboardingModal.tsx（无错误暴露）

| 位置 | 操作 | 当前处理 | 应否暴露 | 建议 |
|------|------|---------|---------|------|
| L54-55 | `download_model` | `console.error` | ✅ 是 | 应更新 `downloadStatus` 为 "error" 并显示提示 |

### 3.6 ComputerUsePanel.tsx / LspPanel.tsx（已有 setError）

| 组件 | 模式 | 状态 |
|------|------|------|
| ComputerUsePanel | `setError(String(e))` | ✅ 已暴露 |
| LspPanel | `setError(String(e))` | ✅ 已暴露 |

**verdict**：这两个组件模式正确，但错误信息仅为 `String(e)`，缺少用户友好的本地化消息。

---

## 4. 修复优先级

### 🔴 高优先级（用户核心操作）

1. **SettingsPanel.tsx**：`save_settings` catch 分支补充 `setToast(t("settings.saveFailed"))`
2. **App.tsx**：`save_session` / `delete_session` / `load_sessions` 补充 Toast
3. **TaskPanel.tsx**：`list_tasks` / `cancel_task` 补充 error banner

### 🟡 中优先级（辅助功能）

4. **FileBrowser.tsx**：`file_tree` 错误显示替代空状态
5. **OnboardingModal.tsx**：下载失败更新 UI 状态
6. **App.tsx**：updater 检查失败保持静默（已有注释说明）

### 🟢 低优先级（内部清理）

7. `complete_task` 的 `.catch(console.error)` 系列 — 保持现状，内部状态清理无需用户感知

---

## 5. 推荐的前端错误处理模式

```typescript
// 模式 A：Toast 通知（短生命周期）
try {
  await invoke("save_settings", { settings });
  setToast(t("settings.saved"));
} catch (e) {
  setToast(t("settings.saveFailed")); // ← 新增
  console.error("Failed to save settings:", e);
}

// 模式 B：Error Banner（长生命周期，需用户 dismiss）
const [error, setError] = useState("");
try {
  const data = await invoke("list_tasks");
  setTasks(data);
} catch (e) {
  setError(t("tasks.loadFailed")); // ← 新增
  console.error("Failed to list tasks:", e);
}
// JSX: {error && <div className="error-banner">{error}</div>}

// 模式 C：状态更新（操作相关）
try {
  await invoke("download_model", { repoId, filename });
} catch (e) {
  setDownloadStatus("error"); // ← 新增
  console.error("Download failed:", e);
}
```

---

## 6. i18n 键建议（需补充到翻译文件）

```json
{
  "settings.saveFailed": "保存失败，请重试",
  "settings.loadFailed": "无法加载设置",
  "tasks.loadFailed": "无法加载任务列表",
  "tasks.cancelFailed": "取消任务失败",
  "sessions.loadFailed": "无法加载会话列表",
  "sessions.saveFailed": "会话保存失败",
  "sessions.deleteFailed": "会话删除失败",
  "fileBrowser.loadFailed": "无法加载文件树"
}
```

---

*本审计由代码健康维护会话生成，修复实施可分批进行，每次 PR 附带对应组件的测试更新。*
