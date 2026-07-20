# clarity-mcp 模块 Worktree

> 本文件由脚本生成，展示 `crates/clarity-mcp/src` 的模块层级、规模与内部依赖关系。

## 模块规模

| 顶层 | 子模块 | 行数 |
|------|--------|------|
| **config** | | **209** |
| | `config` | 209 |
| **devkit** | | **171** |
| | `devkit` | 171 |
| **enhanced** | | **2167** |
| | `· enhanced::builder` | 199 |
| | `· enhanced::client` | 71 |
| | `· enhanced::error` | 43 |
| | `· enhanced::http` | 118 |
| | `· enhanced::instance` | 116 |
| | `· enhanced::mod` | 46 |
| | `· enhanced::result_types` | 228 |
| | `· enhanced::rpc` | 39 |
| | `· enhanced::sse` | 314 |
| | `· enhanced::stdio` | 215 |
| | `· enhanced::tests` | 287 |
| | `· enhanced::types` | 194 |
| | `· enhanced::validate` | 81 |
| | `· enhanced::websocket` | 216 |
| **lib** | | **505** |
| | `lib` | 505 |
| **llm_provider** | | **108** |
| | `llm_provider` | 108 |
| **server** | | **396** |
| | `server` | 396 |

## 模块依赖图

```mermaid
flowchart LR
    subgraph config [config (209 loc)]
        config["config<br/>209"]
    end
    subgraph devkit [devkit (171 loc)]
        devkit["devkit<br/>171"]
    end
    subgraph enhanced [enhanced (2167 loc)]
        enhanced_builder["enhanced::builder<br/>199"]
        enhanced_client["enhanced::client<br/>71"]
        enhanced_error["enhanced::error<br/>43"]
        enhanced_http["enhanced::http<br/>118"]
        enhanced_instance["enhanced::instance<br/>116"]
        enhanced_mod["enhanced::mod<br/>46"]
        enhanced_result_types["enhanced::result_types<br/>228"]
        enhanced_rpc["enhanced::rpc<br/>39"]
        enhanced_sse["enhanced::sse<br/>314"]
        enhanced_stdio["enhanced::stdio<br/>215"]
        enhanced_tests["enhanced::tests<br/>287"]
        enhanced_types["enhanced::types<br/>194"]
        enhanced_validate["enhanced::validate<br/>81"]
        enhanced_websocket["enhanced::websocket<br/>216"]
    end
    subgraph lib [lib (505 loc)]
        lib["lib<br/>505"]
    end
    subgraph llm_provider [llm_provider (108 loc)]
        llm_provider["llm_provider<br/>108"]
    end
    subgraph server [server (396 loc)]
        server["server<br/>396"]
    end

    enhanced --> config
```

## 优化完成记录

1. **`enhanced.rs` 已拆分**：原 2069 行的单体文件已拆分为 12 个子模块，职责边界如下：
   - `types`：传输配置（`McpTransport`、`McpServerConfig`、`OAuthConfig`）
   - `rpc`：内部 JSON-RPC 请求/响应/错误类型
   - `client`：`McpClient` trait
   - `validate`：stdio 命令安全校验
   - `stdio` / `http` / `sse` / `websocket`：四种 transport 实现
   - `builder`：`McpClientBuilder` 与各 transport builder
   - `result_types`：tool/resource/prompt 结果类型
   - `error`：`McpError`
   - `instance`：`McpClientInstance`、`McpRegistry`
   - `tests`：原 `enhanced.rs` 的测试集合
2. **mock 测试代理稳定性**：`SseMcpClient` 新增 `with_client` 构造函数，mock 测试注入 `no_proxy` reqwest 客户端。
3. **依赖方向**：`api`（本 crate 为 `types` + `client`）处于底层；各 transport 依赖 `client`；`builder` 与 `instance` 作为聚合层位于顶层。
