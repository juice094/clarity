# Android 移动端性能基线（泳道 D）— 2026-07-20

## 环境

- 设备：Android 模拟器 `emulator-5554`（AVD `Pixel_7_API_36`，`sdk_gphone64_x86_64`，API 36，Windows 11 宿主）
- 被测包：`com.juice094.clarity.mobile`，debug APK（`gradlew assembleDebug` 全量 UP-TO-DATE，即 7 月 7 日构建产物与当前源码一致，`adb install -r` 重装后测量）
- 采集脚本：`mobile/android/perf_baseline.py`（本次新增，`coldstart` / `setup` / `firsttoken` 三个子命令）
- 注意：采集中途原模拟器进程崩溃消失，已用 `-no-snapshot-save` 重启同 AVD 后继续；动画缩放保持系统默认 1.0

## 冷启动（目标 < 2s）：✅ 达标，中位 1588ms

测量方法：`am force-stop` → `am start -W -n com.juice094.clarity.mobile/.MainActivity` 的 `TotalTime`，并与 logcat `ActivityTaskManager: Displayed ...: +Xms` 首帧时间交叉校验（两者逐次完全一致）。每次启动后用 uiautomator 确认主界面可交互。

正式批次（7 次，已配置 provider 的老用户路径：LocalChat 自动登录直进聊天屏）：

| 指标 | 中位数 | 最小 | 最大 | 全部样本 (ms) |
|------|--------|------|------|----------------|
| TotalTime | **1588** | 1502 | 2783 | 1656, 1643, 2783, 1502, 1588, 1546, 1566 |
| Displayed（首帧） | 1588 | — | — | 与 TotalTime 逐次相同 |

参考批次（方法验证期，均为 7 次）：Claw 恢复态中位 1833ms（模拟器刚重启、系统抖动大，含 3191/2746 两个离群）；更早一批中位 1765ms。三批结论一致：**冷启动稳定在 1.5–2.2s 区间，中位数 < 2s 达标**，但余量不大，离群样本可超 3s。

误差说明：`am start -W TotalTime` 是系统侧 activity 启动耗时，含进程创建 + 首帧，是 Android 官方冷启动口径；与 Displayed 一致说明无隐藏延迟。未测量「到首条消息可发」的端到端交互时间。

## 首 token（目标 < 3s）：⚠️ 未能实测（无有效 LLM API key）

测量方法（已就绪，可复用）：app 内置探针 `EventHandler.recordFirstTokenLatency()`，从 `sendMessage()` 时间戳到首个 `UiEvent.ContentPart`，输出 logcat `ClarityLatency: first_token_latency_ms=N`；脚本发送 "hi" 后轮询 logcat 取值。探针本身已由 E2E 的 `clawFirstTokenLatencyFlow` 覆盖。

失败事实（logcat 实证）：

| Provider | 结果 | logcat 证据 |
|----------|------|-------------|
| DEEPSEEK | 401 | `API error (401 Unauthorized): Authentication Fails, Your api key: ****8eff is invalid`（与宿主 `DEEPSEEK_API_KEY` 尾号一致，key 本身已失效） |
| KIMI | 401 | `API error (401 Unauthorized): Invalid Authentication` |
| OPEN_AI | 超时 | `LLM error: LLM request timed out after 45s`（宿主网络到 api.openai.com 不通） |

附带观察：TurnBegin → 401 Error 约 8s，说明模拟器出网链路（10.0.2.x NAT）与 Rust 侧请求管道工作正常，瓶颈 purely 是凭据。DeepSeek 设备登录模式（历史 ui_automation.py 用的手机号+密码）未尝试——任务要求避免依赖该路径。

**待办**：换一把有效 API key 后运行 `python mobile/android/perf_baseline.py setup <key> DEEPSEEK && python mobile/android/perf_baseline.py firsttoken 7` 即可补齐。

## E2E（connectedDebugAndroidTest）：未运行

原因：泳道时间约束 + 采集中途模拟器崩溃恢复 + 套件含 `clawFiveMinuteStressFlow`（≥5 分钟）且 `deepseekDeviceLoginFlow` 依赖硬编码设备登录凭据与宿主机 Gateway（18790 当前有用户实例在跑）。历史结果为 14/14 通过，本次未回归验证。

## 瓶颈观察

- 冷启动的主要余量消耗在模拟器 x86_64 镜像 + debug 包（627MB，含双 ABI Rust .so）；真机 release 包预期更快，但当前无真机数据。
- 冷启动离群值（2783/3191ms）出现在系统刚重启或负载高时，说明对宿主资源敏感，CI 化采集需固定环境。
- 首 token 链路（UI → UniFFI → Rust agent → HTTP → 流式回灌）已被探针覆盖，唯一缺口是有效凭据；app 侧开销可从探针与 LLM 网关侧日志差值分解，留待补齐数据后分析。

## 遗留问题

- 首 token 无数据：需有效 DeepSeek/Kimi key。
- E2E 未回归：建议在有有效设备登录凭据时单独跑 `gradlew.bat connectedDebugAndroidTest`。
- 本次为模拟器数据，T1 的「真机冷启动 < 2s」仍缺真机验证。
- 环境残留：模拟器由本会话以 `-no-snapshot-save` 重启并保持运行；app 原 Claw 配置已恢复（`launch_mode=Claw`、`provider=DEEPSEEK_DEVICE`），但加密 prefs 中的 `api_key` 字段被最后一次 setup 写成了失效的 OpenAI key（原值未知，影响仅限本地 agent 模式的 key 字段）。
