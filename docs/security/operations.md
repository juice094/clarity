---
title: 安全与运维
category: Security
date: 2026-06-13
tags: [security, operations, audit]
---

# 安全与运维

> 安全设计原则、漏洞报告流程与支持版本见根目录 [`SECURITY.md`](../../SECURITY.md)。威胁模型见 [`THREAT_MODEL.md`](./THREAT_MODEL.md)。

---

## 工具与执行安全

- **路径消毒**：`ToolError::sanitize_paths()` 将绝对路径脱敏为 `~` 前缀后再返回用户/协议层。
- **路径遍历**：`resolve_path()` 校验路径边界；Gateway 文件接口使用 `sanitize_path()` + `canonicalize()`。
- **MCP 命令校验**：`validate_mcp_command()` 拒绝 shell 元字符、`..`、相对路径、不存在的绝对路径；裸命令（`npx`、`uvx`）经 PATH 解析。
- **审批模式**：`interactive` / `smart` / `plan` / `yolo` 四层；`Plan` 模式一次性审批后批量执行。
- **熔断**：同一工具单轮内 recoverable 失败 3 次后升级为 fatal，停止 Agent 循环。

---

## 密钥与隐私

- `clarity-secrets` 使用 ChaCha20-Poly1305 加密 `enc2:` 密钥。
- 禁止硬编码真实 token / api_key / 密码。
- 禁止将 env 注入的密钥回写到未加密持久化文件。
- `clarity-memory` 以审批记录 JSON 持久化到 SQLite，失败不阻塞审批流。

---

## 网络安全

- HTTP 客户端统一使用 `rustls-tls`，已移除 `openssl`。
- Gateway 公共端口 `18790` 绑定 `0.0.0.0`，管理/Web UI 端口 `18800` 仅绑定 `127.0.0.1`。
- Discord/Telegram 通道因上游 `rustls-webpki` advisory 默认禁用。

---

## 审计

- `cargo audit --deny unsound --deny yanked` 是 CI 强制步骤。
- `.cargo/audit.toml` 仅忽略已评估的 `RUSTSEC-2024-0429`。

---

*最后更新：2026-06-13*
