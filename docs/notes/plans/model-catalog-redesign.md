---
type: plan
id: model-catalog-redesign
title: Model Catalog Redesign for clarity-llm
description: Replace hard-coded model lists with a dynamic, cacheable, provider-aware model catalog system.
tags: [plan, clarity-llm, architecture, model-catalog]
related_concepts: [model-registry, model-listing, registry-table, runtime-router]
timestamp: 2026-06-26T13:00:00Z
---

# Model Catalog Redesign for `clarity-llm`

## 1. Problem Statement

The current `clarity-llm` model listing relies on a hard-coded fallback catalog
(stored in `registry_table::FamilyDefaults::known_models`). This creates
several operational problems:

- **Stale catalogs**: model names change frequently (`gpt-4o` → `gpt-4.1`,
  dated Claude snapshots, new DeepSeek-V variants).
- **Deprecation blindness**: a hard-coded model may have been retired by the
  provider, but the UI still offers it.
- **Account/region variance**: two users with the same provider may see
  different available models.
- **Local process dynamics**: Ollama and llama-server model lists are defined
  by whatever the local daemon currently has loaded or downloaded.
- **Maintenance burden**: every new model release requires a code change and
  release.

Hard-coded lists are acceptable as an **offline bootstrap**, but they must not
be the primary source of truth.

## 2. Scope of Change

### In Scope

- `crates/clarity-llm/src/model_listing.rs`
- `crates/clarity-llm/src/registry_table.rs`
- `crates/clarity-llm/src/model_registry.rs`
- New `crates/clarity-llm/src/catalog/` module
- Settings UI consumption path (`clarity-egui` model picker)
- Local cache format under `~/.clarity/catalogs/`

### Out of Scope (for this redesign)

- Changing the `LlmProvider` trait or provider construction logic.
- Replacing `models.toml` as the user override layer.
- Adding model download/pull capabilities (Ollama pull, HuggingFace download).

### Boundary

The redesign affects **model discovery for UI and routing hints** only. Runtime
inference still uses whatever `model_id` the user or alias selects.

## 3. Commercial / Mature Implementation References

| Product / Project | Discovery Mechanism | Fallback Strategy | Notes |
|-------------------|---------------------|-------------------|-------|
| **Ollama** | Local: `GET /api/tags`; Cloud: `https://ollama.com/api/tags` | Hard-coded seed list | Returns `name`, `model`, `size`, `digest`, `details.family`, `quantization_level` |
| **OpenAI API** | `GET /v1/models` | Documentation-only seed list | Returns `id`, `object`, `created`, `owned_by`; requires valid API key |
| **Azure OpenAI** | `GET /openai/v1/models?api-version=...` | Portal/model docs | Account-specific; often filtered by deployment |
| **LM Studio** | Native: `GET /api/v1/models`; OpenAI-compat: `GET /v1/models` | In-app downloaded catalog | Daemon returns currently loaded + available models |
| **OpenClaw** | Fetches `ollama.com/api/tags` (capped at 500); custom endpoint `/models` | Previous hard-coded suggestions | Falls back to seed list when remote is unreachable |
| **OpenWebUI / LibreChat** | Calls provider `/models` endpoints | User-defined model list | Uses OpenAI-compatible `/v1/models` as de facto standard |

Common patterns across mature tools:

1. **Dynamic fetch first**: ask the provider daemon or API what is available.
2. **Local cache**: persist the last successful fetch for offline use.
3. **Hard-coded seed**: only used when remote and cache are unavailable.
4. **User override**: manual entries always win.
5. **Manual refresh**: UI exposes a refresh action, not automatic polling on
   every launch.

## 4. Design Principles

1. **Single source of truth at runtime**: the catalog returned to consumers is
   always the merged result of `user override → cached remote → bootstrap`.
2. **Provider-aware, not provider-agnostic**: each provider family decides how
   (and whether) it can discover models.
3. **Offline-capable**: a cache file makes the UI usable without network.
4. **Minimal bootstrap**: hard-coded lists contain only the most stable 1–2
   models per family, used solely for offline bootstrapping.
5. **No breaking changes to `models.toml`**: user-configured aliases remain the
   highest-priority source.

## 5. Proposed Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│  Consumer (egui model picker, runtime router, headless CLI) │
└───────────────────────┬─────────────────────────────────────┘
                        │ Vec<(provider_id, model_id, metadata)>
                        ▼
┌─────────────────────────────────────────────────────────────┐
│  ModelCatalogService                                        │
│  - merge(user_overrides, cached_remote, bootstrap)          │
│  - refresh(provider_id) → fetch → validate → cache          │
└───────────────────────┬─────────────────────────────────────┘
                        │
        ┌───────────────┼───────────────┐
        ▼               ▼               ▼
┌─────────────┐ ┌─────────────┐ ┌─────────────┐
│ UserCatalog │ │ RemoteCache │ │ Bootstrap   │
│ models.toml │ │ ~/.clarity/ │ │ registry_   │
│             │ │ catalogs/   │ │ table seed  │
└─────────────┘ └─────────────┘ └─────────────┘
                        ▲
                        │ async fetch
        ┌───────────────┼───────────────┐
        ▼               ▼               ▼
┌─────────────┐ ┌─────────────┐ ┌─────────────┐
│ OllamaFetcher│ │ OpenAiFetcher│ │ NullFetcher │
│ /api/tags   │ │ /v1/models  │ │ (always     │
│             │ │             │ │  empty)     │
└─────────────┘ └─────────────┘ └─────────────┘
```

### 5.1 Core Trait

```rust
#[async_trait]
pub trait ModelCatalogFetcher: Send + Sync {
    /// Provider family this fetcher serves, e.g. "ollama", "openai".
    fn provider_id(&self) -> &str;

    /// Return the list of models currently available from this provider.
    ///
    /// Implementations should return an error only when the provider is
    /// reachable but misbehaving. An unreachable provider should return an
    /// empty list so the caller can fall back to cache/bootstrap.
    async fn fetch(&self, config: &FetchConfig) -> Result<Vec<ModelCatalogEntry>, CatalogError>;

    /// Whether this provider supports remote discovery at all.
    fn supports_fetch(&self) -> bool;
}

#[derive(Debug, Clone)]
pub struct ModelCatalogEntry {
    pub provider_id: String,
    pub model_id: String,
    pub display_name: Option<String>,
    pub capabilities: Vec<String>,
    pub quantization: Option<String>,
    pub context_length: Option<usize>,
}
```

### 5.2 Fetcher Implementations

| Provider | Endpoint | Notes |
|----------|----------|-------|
| `ollama` | `GET /api/tags` (local) or `https://ollama.com/api/tags` (cloud) | Parse `name`, `details.family`, `details.quantization_level` |
| `openai`, `kimi`, `deepseek`, `moonshot`, `kimi-code` | `GET /v1/models` | OpenAI-compatible; requires API key |
| `anthropic` | **None** | No public model list endpoint; use bootstrap + user config |
| `llama-server` | `GET /v1/models` if supported, else **None** | Best-effort; many builds do not implement it |
| `local` (Candle GGUF) | filesystem scan | Existing `scan_local_models()` |

### 5.3 Cache Format

Location: `~/.clarity/catalogs/{provider_id}.json`

```json
{
  "fetched_at": "2026-06-26T13:00:00Z",
  "ttl_seconds": 86400,
  "source": "remote",
  "models": [
    {
      "provider_id": "ollama",
      "model_id": "qwen2.5-coder:14b",
      "display_name": "qwen2.5-coder:14b",
      "capabilities": ["chat"],
      "quantization": "Q4_K_M",
      "context_length": null
    }
  ]
}
```

The cache is a local runtime artifact, **not** committed to git or stored as OKF.

### 5.4 Bootstrap List

`registry_table::FamilyDefaults::known_models` is reduced to the smallest viable
offline seed (typically 1 model per family). Example:

- `openai`: `["gpt-4o"]`
- `anthropic`: `["claude-sonnet"]`
- `kimi`: `["kimi-k2.6"]`
- `deepseek`: `["deepseek-chat"]`
- `ollama`: `[]` (local daemon required)
- `local`: `[]` (filesystem scan required)

### 5.5 Merge Priority

When `get_available_models()` is called:

1. Start with `models.toml` configured models (user override).
2. For each provider not already covered, try the remote cache.
3. For each provider still missing, use the bootstrap seed.
4. For `local`/`ollama`, use filesystem scan or daemon fetch respectively.

## 6. Data Model Additions

### `crates/clarity-llm/src/catalog/mod.rs`

```rust
pub struct CatalogCache {
    pub fetched_at: DateTime<Utc>,
    pub ttl_seconds: u64,
    pub entries: Vec<ModelCatalogEntry>,
}

pub struct ModelCatalogService {
    registry: Arc<ModelRegistry>,
    fetchers: HashMap<String, Arc<dyn ModelCatalogFetcher>>,
    cache_dir: PathBuf,
}

impl ModelCatalogService {
    pub fn new(registry: Arc<ModelRegistry>, cache_dir: PathBuf) -> Self;

    /// Return the merged catalog for all providers.
    pub async fn list(&self) -> Result<Vec<ModelCatalogEntry>, CatalogError>;

    /// Return the merged catalog for one provider.
    pub async fn list_for_provider(&self, provider_id: &str) -> Vec<ModelCatalogEntry>;

    /// Fetch fresh models from the provider and update the cache.
    pub async fn refresh(&self, provider_id: &str) -> Result<Vec<ModelCatalogEntry>, CatalogError>;
}
```

## 7. Integration Points

### `model_listing::get_available_models()`

Replace the current implementation with:

```rust
pub async fn get_available_models() -> Vec<(String, String, Vec<String>)> {
    let registry = ModelRegistry::load_async().await.unwrap_or_default();
    let service = ModelCatalogService::new(
        Arc::new(registry),
        clarity_dirs::catalog_dir(),
    );
    service.to_provider_groups().await
}
```

For backwards compatibility, keep a synchronous wrapper that reads only cache
and bootstrap (no remote fetch).

### Settings UI

Add a **Refresh** button next to each provider in `clarity-egui` Settings →
Provider. Trigger `ModelCatalogService::refresh(provider_id)`.

### Runtime Router

The router can use catalog metadata (capabilities, pricing tags) to improve
`RouterHint` resolution without hard-coding model names.

## 8. Implementation Phases

### Phase 1: Foundation (1–2 days)

- Define `ModelCatalogFetcher`, `ModelCatalogEntry`, `CatalogCache`.
- Implement filesystem cache read/write.
- Reduce `registry_table::known_models` to minimal bootstrap.

### Phase 2: Fetchers (2–3 days)

- Implement `OllamaFetcher` (`/api/tags`).
- Implement `OpenAiCompatibleFetcher` (`/v1/models`).
- Implement `NullFetcher` for providers without discovery.

### Phase 3: Service & Migration (2 days)

- Implement `ModelCatalogService` with merge logic.
- Migrate `model_listing::get_available_models()`.
- Add async API and sync fallback wrapper.

### Phase 4: UI (2–3 days)

- Add refresh buttons to `clarity-egui` provider settings.
- Show last-fetched timestamp and cache status.

### Phase 5: Tests

- Mock-server tests for each fetcher.
- Cache read/write tests.
- Merge priority tests.
- Offline fallback tests.

## 9. Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Provider `/models` endpoint returns models the provider cannot actually serve | Cache them anyway; runtime errors are handled by `ReliableProvider` |
| Frequent refreshes hit rate limits / cost | Manual refresh only; default TTL 24h; no background polling |
| Cache corruption | Versioned cache schema; invalid cache falls back to bootstrap |
| Added startup latency | Sync API reads cache only; async refresh is user-triggered |
| `models.toml` drift | User overrides always win; refresh never overwrites them |

## 10. Recommendation Summary

Adopt the layered catalog architecture:

1. **User override** (`models.toml`) — highest priority, manual curation.
2. **Cached remote catalog** — refreshed on demand, offline-capable.
3. **Minimal bootstrap seed** — hard-coded, only for first offline launch.

Start implementation with **Phase 1 + Ollama fetcher**, because Ollama has a
stable, well-documented local discovery endpoint and is the most common local
runtime for Clarity users. Then extend to OpenAI-compatible providers.

Do not continue expanding `known_models` with new model names; treat it as a
static bootstrap and invest in dynamic discovery instead.
