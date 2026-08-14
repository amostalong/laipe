//! models.dev model catalog: embed + on-disk cache + remote refresh + slim.
//!
//! 1:1 mirror of PlotCraft `src-tauri/src/model_catalog.rs` — but extracted
//! into the library so Tauri-free consumers (browser / Node) can also use
//! the catalog. The Tauri command layer (`get_model_catalog` /
//! `refresh_model_catalog`) lives in `laipe-app/src-tauri/src/model_catalog.rs`
//! and wraps these helpers.
//!
//! ## Design
//!
//! - **Embedded snapshot**: gzipped JSON, build-time `include_bytes!`
//! - **On-disk cache**: passed in by the host (Tauri AppHandle resolves
//!   `app_config_dir`; browsers pass a localStorage key, etc.)
//! - **Remote refresh**: caller invokes [`refresh_catalog`]; pass a reqwest
//!   `Client` (or implement your own fetcher).
//! - **Background refresh**: host-side concern (Tauri command does the spawn).
//! - **Sanity check**: refresh data < 50 providers / < 1000 models → drop,
//!   keep embedded (defends against broken mirrors / truncated responses).
//!
//! ## Schema
//!
//! Slim [`ModelCatalog`] is the persistent on-disk format (gzip-compressed
//! embedded, plain JSON cache). [`ResolvedCatalog`] is the frontend-facing
//! view: filters deprecated / no-tool-call models, applies endpoint fallbacks
//! from [`OFFICIAL_API_FALLBACKS`], and serializes to camelCase via
//! `#[serde(rename_all = "camelCase")]` so it matches the TS type without
//! conversion.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use chrono::Utc;
use flate2::read::GzDecoder;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::RwLock;

// === Embedded snapshot + config constants ===

/// Filename used by [`cache_path_in`] when the host wants a default name.
pub const CACHE_FILE_NAME: &str = "model_catalog.json";
const CACHE_TMP_SUFFIX: &str = "tmp";
const DEFAULT_SOURCE_URL: &str = "https://models.dev/api.json";
pub const REFRESH_TTL: Duration = Duration::from_secs(24 * 60 * 60);
pub const FETCH_TIMEOUT: Duration = Duration::from_secs(30);
/// sanity check: refresh 拉回来的数据 < 这两个阈值就不写盘
pub const MIN_SANE_PROVIDERS: usize = 50;
pub const MIN_SANE_MODELS: usize = 1000;
/// cache health: local cache < 30 providers 视为损坏
pub const MIN_HEALTHY_PROVIDERS: usize = 30;

// === Schema：embedded snapshot + cached file (slim) ===

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CatalogLimit {
    #[serde(default)]
    pub context: u64,
    #[serde(default)]
    pub output: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogModel {
    pub name: String,
    #[serde(default)]
    pub limit: CatalogLimit,
    #[serde(default)]
    pub reasoning: bool,
    #[serde(default)]
    pub tool_call: bool,
    #[serde(default)]
    pub attachment: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogProvider {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub npm: Option<String>,
    #[serde(default)]
    pub models: IndexMap<String, CatalogModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCatalog {
    #[serde(default)]
    pub fetched_at: String,
    pub providers: IndexMap<String, CatalogProvider>,
}

// === Frontend-facing resolved schema（filter + endpoint fallback 后） ===

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedModel {
    pub id: String,
    pub name: String,
    pub context_window: u64,
    pub output_limit: u64,
    pub reasoning: bool,
    pub tool_call: bool,
    pub vision: bool,
    pub release_date: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedProvider {
    pub id: String,
    pub name: String,
    pub endpoint: String,
    pub npm: Option<String>,
    pub suggested_api_format: String,
    pub models: Vec<ResolvedModel>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedCatalog {
    pub fetched_at: String,
    pub providers: Vec<ResolvedProvider>,
}

// === OFFICIAL_API_FALLBACKS (mirror Locus / PlotCraft) ===

pub const OFFICIAL_API_FALLBACKS: &[(&str, &str)] = &[
    ("anthropic", "https://api.anthropic.com"),
    ("openai", "https://api.openai.com/v1"),
    (
        "google",
        "https://generativelanguage.googleapis.com/v1beta/openai",
    ),
    ("xai", "https://api.x.ai/v1"),
    ("mistral", "https://api.mistral.ai/v1"),
    ("groq", "https://api.groq.com/openai/v1"),
    ("cohere", "https://api.cohere.ai/compatibility/v1"),
    ("perplexity", "https://api.perplexity.ai"),
    ("togetherai", "https://api.together.xyz/v1"),
    ("deepinfra", "https://api.deepinfra.com/v1/openai"),
    ("cerebras", "https://api.cerebras.ai/v1"),
    ("v0", "https://api.v0.dev/v1"),
    ("vercel", "https://ai-gateway.vercel.sh/v1"),
];

// === Slim 函数 ===

fn slim_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn slim_model(id: &str, raw: &Value) -> CatalogModel {
    let limit = raw.get("limit");
    CatalogModel {
        name: slim_string(raw, "name").unwrap_or_else(|| id.to_string()),
        limit: CatalogLimit {
            context: limit
                .and_then(|l| l.get("context"))
                .and_then(Value::as_u64)
                .unwrap_or(0),
            output: limit
                .and_then(|l| l.get("output"))
                .and_then(Value::as_u64)
                .unwrap_or(0),
        },
        reasoning: raw
            .get("reasoning")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        tool_call: raw
            .get("tool_call")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        attachment: raw
            .get("attachment")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        release_date: slim_string(raw, "release_date"),
        status: slim_string(raw, "status"),
    }
}

fn slim_provider(_id: &str, raw: &Value) -> Option<CatalogProvider> {
    // 必须有 name + models 字段才算合法 provider
    let name = slim_string(raw, "name")?;
    let api = slim_string(raw, "api");
    let npm = slim_string(raw, "npm");
    let models_obj = raw.get("models").and_then(Value::as_object);
    let mut models = IndexMap::new();
    if let Some(obj) = models_obj {
        for (mid, mval) in obj {
            if mval.is_object() {
                models.insert(mid.clone(), slim_model(mid, mval));
            }
        }
    }
    Some(CatalogProvider {
        name,
        api,
        npm,
        models,
    })
}

/// 把 models.dev 整个 raw JSON 摘成 ModelCatalog
/// - raw 顶层 = `{ provider_id: provider_obj, ... }`
/// - 失败的 provider（缺 name / 无 models object）跳过
pub fn slim_catalog(raw: Value) -> ModelCatalog {
    let mut providers = IndexMap::new();
    if let Some(obj) = raw.as_object() {
        for (id, pval) in obj {
            if let Some(p) = slim_provider(id, pval) {
                providers.insert(id.clone(), p);
            }
        }
    }
    ModelCatalog {
        fetched_at: Utc::now().to_rfc3339(),
        providers,
    }
}

// === Cache I/O (path 由 host 传入) ===

/// Read slim catalog from `cache_path`. Returns `None` on any error
/// (missing file, parse failure, etc.) — the host treats this as "use
/// embedded snapshot".
pub fn load_cached_catalog(cache_path: &Path) -> Option<ModelCatalog> {
    let json = std::fs::read_to_string(cache_path).ok()?;
    serde_json::from_str(&json).ok()
}

/// Atomic write: write to `<path>.tmp`, then rename. On any failure the
/// existing file (if any) is left untouched.
pub fn save_cached_catalog(cache_path: &Path, catalog: &ModelCatalog) -> Result<(), String> {
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("save_cached: mkdir parent: {e}"))?;
    }
    let mut tmp = cache_path.to_path_buf();
    tmp.set_extension(CACHE_TMP_SUFFIX);
    let json = serde_json::to_string_pretty(catalog)
        .map_err(|e| format!("save_cached: serialize: {e}"))?;
    std::fs::write(&tmp, json).map_err(|e| format!("save_cached: write tmp: {e}"))?;
    std::fs::rename(&tmp, cache_path).map_err(|e| format!("save_cached: rename: {e}"))?;
    Ok(())
}

/// Try to remove an unhealthy cache file. Best-effort.
pub fn drop_unhealthy_cache(cache_path: &Path) {
    let _ = std::fs::remove_file(cache_path);
}

/// Convenience: full path for a cache file under a given directory.
pub fn cache_path_in(dir: &Path, file_name: &str) -> PathBuf {
    dir.join(file_name)
}

// === Embedded snapshot parsing ===
//
// The library does NOT bundle a snapshot; the host embeds its own
// gzipped JSON via `include_bytes!` and calls `parse_embedded_snapshot`
// with the bytes. This keeps the library asset-free and lets each
// consumer pin its own snapshot version (Tauri app, browser bundle,
// tests, etc).

/// Decompress and parse a gzipped models.dev snapshot.
/// `bytes` typically comes from `include_bytes!("path/to/snapshot.json.gz")`
/// in the host crate.
pub fn parse_embedded_snapshot(bytes: &[u8]) -> Result<ModelCatalog, String> {
    let mut decoder = GzDecoder::new(bytes);
    let mut json = String::new();
    decoder
        .read_to_string(&mut json)
        .map_err(|e| format!("decompress embedded: {e}"))?;
    serde_json::from_str(&json).map_err(|e| format!("parse embedded: {e}"))
}

// === In-memory state (host-driven) ===

/// Load the freshest data: try cache first (if healthy + newer than
/// embedded), fall back to embedded snapshot. If embedded parse fails,
/// fall back to an empty catalog (host decides how to recover).
pub fn load_freshest(cache_path: &Path, embedded_bytes: &[u8]) -> ModelCatalog {
    let snapshot = parse_embedded_snapshot(embedded_bytes).unwrap_or_else(|_| ModelCatalog {
        fetched_at: String::new(),
        providers: IndexMap::new(),
    });
    match load_cached_catalog(cache_path) {
        Some(cached) if is_cache_healthy(&cached) && freshest(&cached, &snapshot) => cached,
        _ => {
            // unhealthy cache → 删了, 下次 start 用 embedded
            drop_unhealthy_cache(cache_path);
            snapshot
        }
    }
}

/// ISO 8601 strings 比较 (lexical = chronological for ISO 8601 UTC)
fn freshest(a: &ModelCatalog, b: &ModelCatalog) -> bool {
    a.fetched_at.as_str() > b.fetched_at.as_str()
}

/// v0.1.4+ cache health check: embedded 通常 167+ providers
///  - < 30 providers → cache 拉坏了 / 某个旧 dev run 留下的脏数据 → 删掉
///  - 空 fetched_at → 解析损坏 → 删掉
pub fn is_cache_healthy(catalog: &ModelCatalog) -> bool {
    if catalog.providers.len() < MIN_HEALTHY_PROVIDERS {
        return false;
    }
    if catalog.fetched_at.is_empty() {
        return false;
    }
    true
}

// === Resolve + api_format ===

fn resolve_endpoint(provider_id: &str, p: &CatalogProvider) -> Option<String> {
    if let Some(api) = &p.api {
        if !api.is_empty() && !api.contains("${") {
            return Some(api.clone());
        }
    }
    OFFICIAL_API_FALLBACKS
        .iter()
        .find(|(k, _)| *k == provider_id)
        .map(|(_, v)| v.to_string())
        .filter(|s| !s.is_empty())
}

/// Map `npm` SDK id from models.dev to the wire format we should use.
/// Mirrors PlotCraft / Locus behavior.
pub fn suggested_api_format(npm: Option<&str>) -> &'static str {
    match npm {
        Some("@ai-sdk/anthropic") => "anthropic_messages",
        _ => "openai_chat",
    }
}

fn resolve_model(id: &str, m: &CatalogModel) -> ResolvedModel {
    ResolvedModel {
        id: id.to_string(),
        name: m.name.clone(),
        context_window: m.limit.context,
        output_limit: m.limit.output,
        reasoning: m.reasoning,
        tool_call: m.tool_call,
        vision: m.attachment,
        release_date: m.release_date.clone(),
        status: m.status.clone(),
    }
}

fn is_listable_model(m: &CatalogModel) -> bool {
    m.status.as_deref() != Some("deprecated") && m.tool_call
}

/// Resolve a [`ModelCatalog`] into the frontend-facing shape
/// (filter deprecated / no-tool-call models, apply endpoint fallbacks).
pub fn resolve_catalog(catalog: &ModelCatalog) -> ResolvedCatalog {
    let mut providers = Vec::new();
    for (id, p) in &catalog.providers {
        let Some(endpoint) = resolve_endpoint(id, p) else {
            continue;
        };
        let models: Vec<ResolvedModel> = p
            .models
            .iter()
            .filter(|(_, m)| is_listable_model(m))
            .map(|(mid, m)| resolve_model(mid, m))
            .collect();
        if models.is_empty() {
            continue;
        }
        providers.push(ResolvedProvider {
            id: id.clone(),
            name: p.name.clone(),
            endpoint,
            npm: p.npm.clone(),
            suggested_api_format: suggested_api_format(p.npm.as_deref()).to_string(),
            models,
        });
    }
    ResolvedCatalog {
        fetched_at: catalog.fetched_at.clone(),
        providers,
    }
}

// === Remote refresh ===

/// URL to fetch the raw models.dev JSON from. Override via
/// `LAIPE_MODELS_URL` env var (Tauri-side `PLOTCRAFT_MODELS_URL` analog).
pub fn source_url() -> String {
    match std::env::var("LAIPE_MODELS_URL") {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => DEFAULT_SOURCE_URL.to_string(),
    }
}

/// Fetch raw JSON + slim + sanity check. Does NOT save to disk or update
/// in-memory state — the host does that. The split lets hosts run this on
/// a worker thread, do the disk write on a `spawn_blocking`, and update
/// the cell from the appropriate runtime.
pub async fn fetch_and_slim(client: &reqwest::Client) -> Result<ModelCatalog, String> {
    let url = source_url();
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("refresh: GET {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("refresh: GET {url}: HTTP {}", resp.status()));
    }
    let raw: Value = resp
        .json()
        .await
        .map_err(|e| format!("refresh: parse JSON: {e}"))?;
    let slim = slim_catalog(raw);

    // sanity check
    let total_models: usize = slim.providers.values().map(|p| p.models.len()).sum();
    if slim.providers.len() < MIN_SANE_PROVIDERS {
        return Err(format!(
            "refresh: sanity check failed — got {} providers, expected >= {}",
            slim.providers.len(),
            MIN_SANE_PROVIDERS
        ));
    }
    if total_models < MIN_SANE_MODELS {
        return Err(format!(
            "refresh: sanity check failed — got {} models, expected >= {}",
            total_models, MIN_SANE_MODELS
        ));
    }
    Ok(slim)
}

// === Host-side singleton (optional) ===
//
// The library ships an opt-in [`CatalogCell`] so apps that don't need
// custom storage can drop in one line. Hosts that need app_config_dir
// resolution / Tauri state can ignore this and roll their own.

type Cell = RwLock<Option<std::sync::Arc<ModelCatalog>>>;

fn global_cell() -> &'static Cell {
    static CELL: OnceLock<RwLock<Option<std::sync::Arc<ModelCatalog>>>> = OnceLock::new();
    CELL.get_or_init(|| RwLock::new(None))
}

/// Initialize the in-memory catalog from the freshest local source (cache
/// or embedded). Idempotent — repeat calls return the cached value.
pub async fn ensure_loaded(
    cache_path: &Path,
    embedded_bytes: &[u8],
) -> std::sync::Arc<ModelCatalog> {
    if let Some(c) = global_cell().read().await.as_ref() {
        return c.clone();
    }
    let mut guard = global_cell().write().await;
    if let Some(c) = guard.as_ref() {
        return c.clone();
    }
    let cat = std::sync::Arc::new(load_freshest(cache_path, embedded_bytes));
    *guard = Some(cat.clone());
    cat
}

/// Replace the in-memory catalog (e.g. after a successful refresh).
/// Does NOT touch disk — the host decides where the new state comes from.
pub async fn replace_state(catalog: ModelCatalog) -> std::sync::Arc<ModelCatalog> {
    let cat = std::sync::Arc::new(catalog);
    let mut guard = global_cell().write().await;
    *guard = Some(cat.clone());
    cat
}

/// Current in-memory catalog, or `None` if [`ensure_loaded`] hasn't run.
pub async fn current() -> Option<std::sync::Arc<ModelCatalog>> {
    global_cell().read().await.clone()
}

// === Tests ===

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn slim_string_extracts_field() {
        let v = json!({"name": "Anthropic", "api": "https://api.anthropic.com"});
        assert_eq!(slim_string(&v, "name").as_deref(), Some("Anthropic"));
        assert_eq!(
            slim_string(&v, "api").as_deref(),
            Some("https://api.anthropic.com")
        );
        assert_eq!(slim_string(&v, "missing"), None);
    }

    #[test]
    fn slim_model_defaults_are_safe() {
        let raw = json!({});
        let m = slim_model("claude-test", &raw);
        assert_eq!(m.name, "claude-test"); // falls back to id
        assert_eq!(m.limit.context, 0);
        assert!(!m.reasoning);
        assert!(!m.tool_call);
    }

    #[test]
    fn slim_provider_drops_when_name_missing() {
        let raw = json!({"api": "https://x", "models": {}});
        assert!(slim_provider("x", &raw).is_none());
    }

    #[test]
    fn slim_catalog_drops_broken_providers() {
        let raw = json!({
            "anthropic": {"name": "Anthropic", "models": {}},
            "broken": {"models": {}} // missing name
        });
        let cat = slim_catalog(raw);
        assert_eq!(cat.providers.len(), 1);
        assert!(cat.providers.contains_key("anthropic"));
    }

    #[test]
    fn resolve_endpoint_uses_official_fallback() {
        let p = CatalogProvider {
            name: "Anthropic".into(),
            api: None,
            npm: None,
            models: IndexMap::new(),
        };
        assert_eq!(
            resolve_endpoint("anthropic", &p).as_deref(),
            Some("https://api.anthropic.com")
        );
    }

    #[test]
    fn resolve_endpoint_skips_template_api() {
        let p = CatalogProvider {
            name: "Custom".into(),
            api: Some("https://${API_KEY}.example.com".into()),
            npm: None,
            models: IndexMap::new(),
        };
        assert_eq!(
            resolve_endpoint("custom", &p).as_deref(),
            None, // no fallback for unknown id
        );
    }

    #[test]
    fn suggested_api_format_maps_anthropic_npm() {
        assert_eq!(
            suggested_api_format(Some("@ai-sdk/anthropic")),
            "anthropic_messages"
        );
        assert_eq!(suggested_api_format(Some("@ai-sdk/openai")), "openai_chat");
        assert_eq!(suggested_api_format(None), "openai_chat");
    }

    #[test]
    fn is_listable_model_filters_deprecated_and_no_tool_call() {
        let good = CatalogModel {
            name: "gpt-4o".into(),
            limit: CatalogLimit::default(),
            reasoning: false,
            tool_call: true,
            attachment: false,
            release_date: None,
            status: None,
        };
        assert!(is_listable_model(&good));

        let deprecated = CatalogModel {
            status: Some("deprecated".into()),
            ..good.clone()
        };
        assert!(!is_listable_model(&deprecated));

        let no_tool_call = CatalogModel {
            tool_call: false,
            ..good
        };
        assert!(!is_listable_model(&no_tool_call));
    }

    #[test]
    fn resolve_catalog_filters_unreachable_and_empty() {
        let mut providers = IndexMap::new();
        providers.insert(
            "anthropic".to_string(),
            CatalogProvider {
                name: "Anthropic".into(),
                api: None, // fallback ok
                npm: None,
                models: {
                    let mut m = IndexMap::new();
                    m.insert(
                        "claude-sonnet-4-5".to_string(),
                        CatalogModel {
                            name: "Claude Sonnet 4.5".into(),
                            limit: CatalogLimit {
                                context: 200_000,
                                output: 8_000,
                            },
                            reasoning: true,
                            tool_call: true,
                            attachment: false,
                            release_date: Some("2025-09-29".into()),
                            status: None,
                        },
                    );
                    m
                },
            },
        );
        providers.insert(
            "unknown-provider".to_string(),
            CatalogProvider {
                name: "Unknown".into(),
                api: Some("https://${API_KEY}.x".into()), // template
                npm: None,
                models: IndexMap::new(), // empty
            },
        );
        let cat = ModelCatalog {
            fetched_at: "2026-01-01T00:00:00Z".into(),
            providers,
        };
        let r = resolve_catalog(&cat);
        assert_eq!(r.providers.len(), 1);
        assert_eq!(r.providers[0].id, "anthropic");
        assert_eq!(r.providers[0].endpoint, "https://api.anthropic.com");
        assert_eq!(r.providers[0].models[0].id, "claude-sonnet-4-5");
        assert_eq!(r.providers[0].models[0].context_window, 200_000);
    }

    #[test]
    fn is_cache_healthy_rejects_empty_or_too_small() {
        let empty = ModelCatalog {
            fetched_at: String::new(),
            providers: IndexMap::new(),
        };
        assert!(!is_cache_healthy(&empty));
        let mut cat = ModelCatalog {
            fetched_at: String::new(),
            providers: IndexMap::new(),
        };
        cat.providers.insert(
            "a".into(),
            CatalogProvider {
                name: "A".into(),
                api: None,
                npm: None,
                models: IndexMap::new(),
            },
        );
        // fetched_at still empty → unhealthy
        assert!(!is_cache_healthy(&cat));
        // Bump to a few providers but no fetched_at
        let mut providers = IndexMap::new();
        for i in 0..MIN_HEALTHY_PROVIDERS {
            providers.insert(
                format!("p{i}"),
                CatalogProvider {
                    name: format!("P{i}"),
                    api: None,
                    npm: None,
                    models: IndexMap::new(),
                },
            );
        }
        // 30+ providers but empty fetched_at → still unhealthy
        let cat2 = ModelCatalog {
            fetched_at: String::new(),
            providers,
        };
        assert!(!is_cache_healthy(&cat2));
    }

    #[test]
    fn freshest_picks_later_iso_string() {
        let older = ModelCatalog {
            fetched_at: "2025-01-01T00:00:00Z".into(),
            providers: IndexMap::new(),
        };
        let newer = ModelCatalog {
            fetched_at: "2026-01-01T00:00:00Z".into(),
            providers: IndexMap::new(),
        };
        assert!(freshest(&newer, &older));
        assert!(!freshest(&older, &newer));
    }
}
