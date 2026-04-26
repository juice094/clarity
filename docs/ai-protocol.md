# AI 关键决策记录 · Clarity

> 本文件记录跨 AI 会话的关键架构决策、状态锚点和 Hard Veto 边界。
> 用途：在上下文压缩后，新会话可快速恢复项目认知。
> 协议版本：V3.1-EP-O

---

## 一、当前会话锚点

**最后更新**：2026-04-26
**当前分支**：`main` @ `b9e45b6`
**架构模式**：CLI（单轮/短轮次）→ 如需长程自主迭代，建议切换至 Claw

---

## 二、Hard Veto（不可逾越边界）

| 约束 | 状态 | 说明 |
|------|------|------|
| 本地 LLM 优先 | ✅ 生效 | `LocalGgufProvider` 已验证；`ensure_llm` 自动 fallback |
| 禁止数据外泄 | ✅ 生效 | API key 仅存储本地；云端 Provider 由用户显式选择 |
| 禁止 Docker | ✅ 生效 | 无容器化依赖 |
| 禁止 RAG(Qdrant) | ✅ 生效 | `clarity-memory` 使用 SQLite + BM25 + CosineIndex |
| 禁止 Electron | ✅ 生效 | Tauri 2 已替代 |
| 项目广度 ≤ 5 核心工具 | ⚠️ 接近边界 | 当前 6 crates + Tauri GUI + Gateway + TUI，已达上限，新增功能需裁减 |

---

## 三、近期关键决策（2026-04-26）

### 3.1 UI/UX 重构（plan: guy-gardner-jade-polaris）

**决策**：按 Phase 1→2→3→4 顺序执行，一次性提交。

| Phase | 内容 | 状态 |
|-------|------|------|
| 1 | Header 精简（More 菜单 + 状态圆点） | ✅ `b95d26b` |
| 2 | Chat Input 居中卡片式 | ✅ `b95d26b` |
| 3 | Welcome 页（大标题 + 快捷操作 pill） | ✅ `b95d26b` |
| 4 | Sidebar Tools 分组 | ✅ `b95d26b` |

**验收结果**：5/5 标准通过。发现 4 个新 bug（见 5.1）。

### 3.2 Tauri 自动更新（Updater Plugin）

**决策**：采用 `tauri-plugin-updater` 官方方案，非自研轮询。

- 公钥硬编码于 `tauri.conf.json`
- 私钥存放于 GitHub Secrets（`TAURI_SIGNING_PRIVATE_KEY`）
- Release workflow 生成 `.sig` + `latest.json`
- signtool 仅签名 `.exe`，避免破坏 `.msi/.nsis` 的 minisign 签名

**Commit**：`3f270d8`

### 3.3 cargo audit 策略

**决策**：由 `--deny warnings` 调整为 `--deny unsound --deny yanked`。

- 上游 20 个 `unmaintained` 警告不再阻断 CI
- `.cargo/audit.toml` 显式忽略 2 个已评估的 `unsound`（rand 0.7.3 / glib VariantStrIter）
- 保留对新 soundness issue 和 yanked crate 的阻断能力

**Commit**：`b9e45b6`

---

## 四、依赖关系图

```
clarity-core ← clarity-gateway
      ↑              ↑
      └────── clarity-tauri ──────┘
      ↑
clarity-memory
      ↑
clarity-wire
```

- `clarity-tui` / `clarity-headless` / `clarity-claw` 均依赖 `clarity-core`
- `clarity-integration-tests` 依赖 gateway + core

---

## 五、活跃问题与冻结项

### 5.1 待修复（验收发现）

| 优先级 | 问题 | 文件 | 根因 |
|--------|------|------|------|
| P1 | OnboardingModal 不关闭 | `OnboardingModal.tsx:75` | 按钮缺 `onDismiss()` 调用 |
| P1 | Settings 下拉框空白 | `SettingsPanel.tsx` | `fetchMeta` catch 静默失败 |
| P2 | 多面板拥挤 | `App.css` | 无互斥 + 缺 `min-width` |
| P2 | LSP `invoke undefined` | `LspPanel.tsx` | Dev 模式 IPC 偶发未注入 |

### 5.2 长期冻结（约束解除前不投入）

- T_APPROVAL V2（AI 分类器）
- 快捷键系统
- Mobile 适配
- Plugin SDK / Sandbox
- Vim 集成

### 5.3 外部阻塞

- **T_KALOSM_REAL**：agri-paper 7B 模型数据未到达 → 不等待，云端 Provider 作为首次体验默认路径

---

## 六、关键配置速查

| 配置项 | 位置 | 状态 |
|--------|------|------|
| Tauri updater 公钥 | `tauri.conf.json` | ✅ 已配置 |
| Tauri updater 私钥 | GitHub Secrets | ✅ 已配置（用户操作） |
| Git 用户名/邮箱 | `.gitconfig` | `juice094` / `160722440+juice094@users.noreply.github.com` |
| cargo audit 忽略 | `.cargo/audit.toml` | 2 个已评估 unsound |
| CI workflow | `.github/workflows/` | check/test/clippy/fmt/audit/coverage |

---

## 七、下一步方向（待决策）

| 方向 | 工作量 | 前提条件 |
|------|--------|----------|
| 修复 4 个验收 bug | ½ 天 | 无 |
| 参考 OpenClaw WebUI 改进 GUI | 1-2 天 | 需用户收集素材 |
| Release workflow 端到端验证 | ½ 天 | push test tag |
| 前端测试覆盖（RTL + Tauri mock） | 1-2 天 | 无 |

---

*本文件由 AI 会话维护，人类开发者可直接编辑。重大架构变更需同步更新。*
