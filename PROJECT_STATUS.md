# Clarity Project Status

> Last updated: 2026-04-26
> Branch: `main` @ `1a6cbc0`
> Test baseline: **524 passed, 0 failed, 4 ignored**
> Clippy: **0 warnings** (`-D warnings`)

---

## Release Chain Status (Shape Up Cycle)

| Sprint | Goal | Status | Key Deliverable |
|--------|------|--------|-----------------|
| 1 | Documentation止血 + 版本对齐 | ✅ Complete | CHANGELOG顺序修正、版本同步、README表述修正 |
| 2 | 单二进制打包验证 | ✅ Complete | MSI (`Clarity_0.2.1_x64_en-US.msi`, 8.5MB) + NSIS (`Clarity_0.2.1_x64-setup.exe`, 5.9MB) |
| 3 | CI闭环 | ✅ Complete | `.github/workflows/release.yml` `working-directory` 修复 |
| 4 | FTUE闭环 | ✅ Complete | `SettingsPanel` 保存后自动触发 `reload_llm` |
| 5 | 冷却验证 | ✅ Complete | 测试524 passed, Clippy零警告 |
| 6 | 可用性急救 | ✅ Complete | GUI API key 输入框 + `LlmFactory::create_with_key` — Clarity 真正可用 |

---

## Verified (Tested / Built Successfully)

| Item | Evidence | Date |
|------|----------|------|
| Workspace lib tests | 524 passed, 0 failed | 2026-04-26 |
| Clippy zero warnings | `-D warnings` clean | 2026-04-26 |
| Tauri dev build | `cargo tauri dev` starts | 2026-04-26 |
| Tauri release build | `.msi` + `.exe` produced | 2026-04-26 |
| EXE runtime dependency scan | Pure system DLLs + UCRT only | 2026-04-26 |
| EXE launch test | `clarity-tauri.exe` starts (GUI blocking) | 2026-04-26 |
| Frontend npm build | `npm run build` succeeds (75 modules) | 2026-04-26 |
| CI workflow syntax | YAML valid, `working-directory` set | 2026-04-26 |

---

## Unverified / Untested (Requires Action)

| # | Item | Risk Level | Blocker | Proposed Verification Method |
|---|------|------------|---------|------------------------------|
| U1 | **纯净Windows环境安装** — 在无Rust/Node/WebView2的VM上安装MSI并运行 | 🔴 High | 无本地VM | Windows Sandbox 或 GitHub Actions `windows-latest` runner E2E 测试 |
| U2 | **CI端到端验证** — push tag后GitHub Actions完整构建→签名→Release | 🔴 High | 需push测试tag | Push `v0.2.1-test.1` tag 触发 workflow，验证 artifact 产出 |
| U3 | **代码签名效果** — 自签名证书在Defender/SmartScreen下的实际表现 | 🟡 Medium | 需U2完成 | 下载CI产出的.exe，检查属性→数字签名页 |
| U4 | **自动更新检查** — Tauri updater检测新版本并提示下载 | 🟡 Medium | 需U2完成 | 发布测试tag后，运行旧版本看是否提示更新 |
| U5 | **FTUE实际GUI流程** — OnboardingModal在打包应用中的显示、关闭、设置跳转 | 🟡 Medium | 需U1完成 | 人工在VM中完成首次安装→启动→配置→对话 |
| U6 | **模型下载引导** — 用户从Onboarding到下载.gguf到完成首次对话 | 🟡 Medium | T_KALOSM_REAL阻塞 | 云端Provider作为默认路径，本地模型作为进阶选项 |
| U7 | **WebView2缺失环境** — Win10未预装WebView2时的自动下载行为 | 🟡 Medium | 无Win10 VM | 文档说明；依赖Tauri内置的WebView2引导 |
| U8 | **NSIS便携版运行** — `.exe` 直接运行（非安装） | 🟢 Low | 无 | 双击验证即可 |

---

## External Blockers

| ID | Item | Status | Impact | Mitigation |
|----|------|--------|--------|------------|
| T_KALOSM_REAL | agri-paper 7B模型数据未到达 | 🔴 持续阻塞 | 本地模型首次体验路径不完整 | 云端Provider（OpenAI/Anthropic）作为默认首次体验路径 |

---

## Known Limitations (Documented, Not Blockers)

1. **WebView2 Dependency** — Windows 11预装；Windows 10可能需自动下载（Tauri处理）
2. **CUDA Optional** — `cuda` feature flag控制，非必需
3. **Self-Signed Certificate** — 无商业证书，SmartScreen可能拦截，文档已说明
4. **Discord/Telegram Channels** — 因CVE禁用，Slack可用
5. **cargo audit warnings** — 11个unmaintained（上游依赖），已标记为允许

---

## Decision: Continue Sprint 2-4 Validation vs Switch to Plan B

### Current Assessment

方案A的核心约束是：**"任意Windows用户可在3分钟内从GitHub Release下载并运行Clarity"**。

当前状态：
- ✅ 构建产物已生成（MSI/NSIS）
- ✅ 代码已修复并推送
- ❌ 未在真实/纯净环境中验证
- ❌ CI未实际触发验证
- ❌ 首次用户体验未端到端验证

**结论：继续完成方案A的验证工作，不切入方案B。**

理由：
1. 构建产物已产出，验证成本远低于重新开发
2. 方案B（功能驱动）的ROI在当前阶段为负——没有发布链，功能越多债务越重
3. 剩余未验证项（U1-U5）均可通过GitHub Actions + 少量人工验证完成

### Recommended Next Steps

| Priority | Action | Est. Time | Owner |
|----------|--------|-----------|-------|
| P0 | Push test tag `v0.2.1-test.1` 触发CI，验证 workflow 完整跑通 | 10 min + CI time | Human |
| P0 | 人工验证CI产出的MSI/NSIS在本地安装运行 | 15 min | Human |
| P1 | 使用Windows Sandbox验证纯净环境安装 | 30 min | Human |
| P1 | 验证FTUE完整路径：安装→启动→Onboarding→设置OpenAI key→首次对话 | 20 min | Human |
| P2 | 评估是否购买商业代码签名证书（如SmartScreen拦截严重） | — | Deferred |
| P2 | 发布v0.3.0-alpha并开始社区推广 | — | After P0-P1 |

### Abort Criteria (Switch to Plan B)

若以下任一条件触发，重新评估方案：
1. CI workflow 在3次尝试后仍无法产出可用artifact
2. 包体积在UPX压缩后仍 >100MB（当前8.5MB MSI，远低于阈值）
3. WebView2缺失环境导致>50% Win10用户无法安装
4. 30天风险对冲窗口到期（GitHub Star < 50 且 Issue+PR < 3）

---

## Quality Gates (Every Commit)

```bash
cargo test --workspace --lib              # 524 passed
cargo clippy --workspace --lib --bins --tests -- -D warnings  # 零警告
cargo fmt --all -- --check               # 格式检查
```

## Release Gates (Every Release)

```bash
cargo audit                              # 无高危漏洞
cargo tauri build                        # 本地打包成功（Windows）
# 以上 + U1-U5 验证通过
```
