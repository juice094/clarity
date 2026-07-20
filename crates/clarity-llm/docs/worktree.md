# clarity-llm 模块 Worktree

> 本文件由脚本生成，展示 `crates/clarity-llm/src` 的模块层级、规模与内部依赖关系。

## 模块规模

| 层级 | 模块 | 行数 |
|------|------|------|
| **api** | | **15** |
| | `api` | 15 |
| **auth** | | **939** |
| | `· auth::kimi_code` | 516 |
| | `· auth::mod` | 15 |
| | `· auth::service` | 116 |
| | `· auth::token_store` | 292 |
| **catalog** | | **818** |
| | `· catalog::cache` | 55 |
| | `· catalog::entry` | 38 |
| | `· catalog::fetcher` | 21 |
| | `· · catalog::fetchers::mod` | 7 |
| | `· · catalog::fetchers::ollama` | 146 |
| | `· · catalog::fetchers::openai_compatible` | 138 |
| | `· catalog::mod` | 185 |
| | `· catalog::service` | 228 |
| **deepseek** | | **150** |
| | `deepseek` | 150 |
| **deepseek_device** | | **1300** |
| | `deepseek_device` | 1300 |
| **deepseek_pow** | | **538** |
| | `deepseek_pow` | 538 |
| **factory** | | **301** |
| | `factory` | 301 |
| **kalosm** | | **104** |
| | `kalosm` | 104 |
| **lib** | | **345** |
| | `lib` | 345 |
| **llama_server** | | **133** |
| | `llama_server` | 133 |
| **local_gguf** | | **1096** |
| | `local_gguf` | 1096 |
| **mesh** | | **579** |
| | `· mesh::circuit` | 131 |
| | `· mesh::mod` | 365 |
| | `· mesh::registry` | 52 |
| | `· mesh::router` | 31 |
| **model_listing** | | **220** |
| | `model_listing` | 220 |
| **model_registry** | | **882** |
| | `model_registry` | 882 |
| **ollama** | | **665** |
| | `ollama` | 665 |
| **policy** | | **133** |
| | `policy` | 133 |
| **providers** | | **895** |
| | `· providers::anthropic` | 305 |
| | `· providers::kimi` | 76 |
| | `· providers::mod` | 15 |
| | `· providers::oauth` | 129 |
| | `· providers::openai_compatible` | 370 |
| **rate_limit** | | **78** |
| | `rate_limit` | 78 |
| **registry_table** | | **201** |
| | `registry_table` | 201 |
| **request** | | **272** |
| | `request` | 272 |
| **runtime** | | **271** |
| | `runtime` | 271 |
| **runtime_router** | | **354** |
| | `runtime_router` | 354 |
| **sse** | | **202** |
| | `sse` | 202 |
| **tool_payload** | | **327** |
| | `tool_payload` | 327 |

## 模块依赖图

```mermaid
flowchart LR
    subgraph api [api (15 loc)]
        api["api<br/>15"]
    end
    subgraph auth [auth (939 loc)]
        auth_kimi_code["auth::kimi_code<br/>516"]
        auth_mod["auth::mod<br/>15"]
        auth_service["auth::service<br/>116"]
        auth_token_store["auth::token_store<br/>292"]
    end
    subgraph catalog [catalog (818 loc)]
        catalog_cache["catalog::cache<br/>55"]
        catalog_entry["catalog::entry<br/>38"]
        catalog_fetcher["catalog::fetcher<br/>21"]
        catalog_fetchers_mod["catalog::fetchers::mod<br/>7"]
        catalog_fetchers_ollama["catalog::fetchers::ollama<br/>146"]
        catalog_fetchers_openai_compatible["catalog::fetchers::openai_compatible<br/>138"]
        catalog_mod["catalog::mod<br/>185"]
        catalog_service["catalog::service<br/>228"]
    end
    subgraph deepseek [deepseek (150 loc)]
        deepseek["deepseek<br/>150"]
    end
    subgraph deepseek_device [deepseek_device (1300 loc)]
        deepseek_device["deepseek_device<br/>1300"]
    end
    subgraph deepseek_pow [deepseek_pow (538 loc)]
        deepseek_pow["deepseek_pow<br/>538"]
    end
    subgraph factory [factory (301 loc)]
        factory["factory<br/>301"]
    end
    subgraph kalosm [kalosm (104 loc)]
        kalosm["kalosm<br/>104"]
    end
    subgraph lib [lib (345 loc)]
        lib["lib<br/>345"]
    end
    subgraph llama_server [llama_server (133 loc)]
        llama_server["llama_server<br/>133"]
    end
    subgraph local_gguf [local_gguf (1096 loc)]
        local_gguf["local_gguf<br/>1096"]
    end
    subgraph mesh [mesh (579 loc)]
        mesh_circuit["mesh::circuit<br/>131"]
        mesh_mod["mesh::mod<br/>365"]
        mesh_registry["mesh::registry<br/>52"]
        mesh_router["mesh::router<br/>31"]
    end
    subgraph model_listing [model_listing (220 loc)]
        model_listing["model_listing<br/>220"]
    end
    subgraph model_registry [model_registry (882 loc)]
        model_registry["model_registry<br/>882"]
    end
    subgraph ollama [ollama (665 loc)]
        ollama["ollama<br/>665"]
    end
    subgraph policy [policy (133 loc)]
        policy["policy<br/>133"]
    end
    subgraph providers [providers (895 loc)]
        providers_anthropic["providers::anthropic<br/>305"]
        providers_kimi["providers::kimi<br/>76"]
        providers_mod["providers::mod<br/>15"]
        providers_oauth["providers::oauth<br/>129"]
        providers_openai_compatible["providers::openai_compatible<br/>370"]
    end
    subgraph rate_limit [rate_limit (78 loc)]
        rate_limit["rate_limit<br/>78"]
    end
    subgraph registry_table [registry_table (201 loc)]
        registry_table["registry_table<br/>201"]
    end
    subgraph request [request (272 loc)]
        request["request<br/>272"]
    end
    subgraph runtime [runtime (271 loc)]
        runtime["runtime<br/>271"]
    end
    subgraph runtime_router [runtime_router (354 loc)]
        runtime_router["runtime_router<br/>354"]
    end
    subgraph sse [sse (202 loc)]
        sse["sse<br/>202"]
    end
    subgraph tool_payload [tool_payload (327 loc)]
        tool_payload["tool_payload<br/>327"]
    end

    catalog --> model_registry
    catalog --> registry_table
    catalog --> runtime
    deepseek --> api
    deepseek_device --> deepseek_pow
    deepseek_device --> tool_payload
    factory --> api
    factory --> auth
    factory --> deepseek
    factory --> deepseek_device
    factory --> local_gguf
    factory --> model_registry
    factory --> providers
    kalosm --> api
    llama_server --> api
    local_gguf --> api
    model_listing --> catalog
    model_listing --> model_registry
    model_listing --> registry_table
    model_registry --> api
    model_registry --> auth
    ollama --> api
    providers --> api
    providers --> auth
    providers --> rate_limit
    providers --> request
    providers --> sse
    providers --> tool_payload
    registry_table --> model_registry
    request --> api
    runtime --> api
    runtime --> catalog
    runtime --> model_listing
    runtime_router --> api
    runtime_router --> model_registry
    sse --> api
```

## 观察与优化建议

1. **大文件拆分候选**：`deepseek_device` (1300 行)、`local_gguf` (1096 行)、`model_registry` (882 行) 超过仓库舒适阈值，后续可按职责拆分为 `device/config.rs`、`device/auth.rs`、`device/session.rs` 等子模块。
2. **`reliable.rs` 已合并**：原 1 行的纯 re-export 文件已合并到 `lib.rs`，避免空壳模块。
3. **测试代理稳定性**：`ollama` mock 测试新增 `with_client` 构造函数，允许注入 `no_proxy` 客户端，避免系统 HTTP 代理拦截 127.0.0.1 请求导致 502。
4. **依赖方向清晰**：`api` 处于最底层，被所有 provider 依赖；`factory` 作为顶层聚合器依赖多数子模块；未发现循环依赖。
