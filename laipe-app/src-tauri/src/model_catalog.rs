//! models.dev model catalog: Tauri commands + app-level wiring.
//!
//! 1:1 mirror of PlotCraft `src-tauri/src/model_catalog.rs` — slim + resolve
//! logic lives in `laipe_streaming::model_catalog` (Tauri-free library), this
//! file is the Tauri integration layer:
//!
//! - **Embedded snapshot**: gzipped JSON, `include_bytes!` from
//!   `../assets/model_catalog.json.gz` (you'll need to drop the snapshot here
//!   at build time — see `assets/README.md`).
//! - **On-disk cache**: `%APPDATA%/Laipe/model_catalog.json` (via Tauri
//!   `app_config_dir()`).
//! - **Remote refresh**: `https://models.dev/api.json` (override via
//!   `LAIPE_MODELS_URL` env var).
//! - **Background refresh**: app 启动 5s 后 spawn tokio task, cache 超 24h
//!   才真拉；失败不致命，fallback freshest local。
//! - **Tauri commands**: `get_model_catalog` / `refresh_model_catalog` (镜像 PlotCraft).
//!
//! v0.1 简化: 不接 Locus "Import from Locus" 跨 app 工具 (laipe-app 是 Vue+Vite
//! 浏览器 demo, 没 Tauri FS access; 走 localStorage). 走 localStorage 用同样的
//! `useModelCatalog` composable (lib/llm.ts) 调 Tauri command, Tauri 端
//! 镜像本文件。

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use chrono::{DateTime, Utc};
use laipe_streaming::model_catalog as lib;
use tauri::{AppHandle, Manager};
use tokio::sync::{Mutex, RwLock};

// === Snapshot + config ===

const EMBEDDED_SNAPSHOT_GZ: &[u8] = include_bytes!("../assets/model_catalog.json.gz");
const BG_REFRESH_DELAY: Duration = Duration::from_secs(5);
const REFRESH_TTL: Duration = lib::REFRESH_TTL;

// Re-export the slim / resolve types so the frontend sees the same shape
// (TS `CatalogModel` / `CatalogProvider` / `ModelCatalog` mirror these).
pub use lib::{ModelCatalog, ResolvedCatalog};

// === In-memory state (OnceLock + RwLock + Mutex) ===

struct CatalogState {
    catalog: std::sync::Arc<ModelCatalog>,
    #[allow(dead_code)]
    source: &'static str,
}

fn catalog_cell() -> &'static RwLock<Option<std::sync::Arc<CatalogState>>> {
    static CELL: OnceLock<RwLock<Option<std::sync::Arc<CatalogState>>>> = OnceLock::new();
    CELL.get_or_init(|| RwLock::new(None))
}

fn refresh_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

// === Cache path ===

fn cache_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("cache_path: app_config_dir failed: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("cache_path: mkdir failed: {e}"))?;
    Ok(lib::cache_path_in(&dir, lib::CACHE_FILE_NAME))
}

async fn current_state(app: &AppHandle) -> Result<std::sync::Arc<CatalogState>, String> {
    if let Some(state) = catalog_cell().read().await.as_ref() {
        return Ok(state.clone());
    }
    let mut guard = catalog_cell().write().await;
    if let Some(state) = guard.as_ref() {
        return Ok(state.clone());
    }
    let path = cache_path(app)?;
    let catalog = std::sync::Arc::new(lib::load_freshest(&path, EMBEDDED_SNAPSHOT_GZ));
    let state = std::sync::Arc::new(CatalogState {
        catalog,
        source: "init",
    });
    *guard = Some(state.clone());
    Ok(state)
}

async fn replace_state(catalog: ModelCatalog) -> Result<(), String> {
    let mut guard = catalog_cell().write().await;
    *guard = Some(std::sync::Arc::new(CatalogState {
        catalog: std::sync::Arc::new(catalog),
        source: "remote",
    }));
    Ok(())
}

// === Remote refresh ===

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(lib::FETCH_TIMEOUT)
        .timeout(lib::FETCH_TIMEOUT)
        .build()
        .map_err(|e| format!("refresh: client build: {e}"))
}

async fn refresh_catalog_inner(app: &AppHandle) -> Result<ModelCatalog, String> {
    let _guard = refresh_lock().lock().await;

    let client = build_client()?;
    let slim = lib::fetch_and_slim(&client).await?;

    // 写盘 (失败不致命 — 内存里的新 catalog 还能用)
    if let Ok(path) = cache_path(app) {
        if let Err(e) = lib::save_cached_catalog(&path, &slim) {
            eprintln!(
                "[model_catalog] refresh: cache write failed (continuing with in-memory): {e}"
            );
        } else {
            let total_models: usize = slim.providers.values().map(|p| p.models.len()).sum();
            eprintln!(
                "[model_catalog] refresh: cached {} providers / {} models to disk",
                slim.providers.len(),
                total_models
            );
        }
    }

    replace_state(slim.clone()).await?;
    Ok(slim)
}

/// Background refresh: app 启动后 spawn 一次
/// - 第一次启动或 cache 缺失 → 等 5s 就拉
/// - cache 已有且新于 24h → 不动
/// - cache 已有但旧于 24h → 等 5s 就拉
/// - 失败 → 静默 (fallback freshest local data)
pub fn spawn_background_refresh(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(BG_REFRESH_DELAY).await;

        let needs_refresh = match current_state(&app).await {
            Ok(state) => {
                if let Ok(parsed) = DateTime::parse_from_rfc3339(&state.catalog.fetched_at) {
                    let age = Utc::now().signed_duration_since(parsed.with_timezone(&Utc));
                    age > chrono::Duration::from_std(REFRESH_TTL).unwrap_or_default()
                } else {
                    true
                }
            }
            Err(_) => true,
        };

        if !needs_refresh {
            eprintln!("[model_catalog] bg refresh: cache fresh, skipping");
            return;
        }

        if let Err(e) = refresh_catalog_inner(&app).await {
            eprintln!("[model_catalog] bg refresh: failed (using local fallback): {e}");
        } else {
            eprintln!("[model_catalog] bg refresh: ok");
        }
    });
}

// === Tauri commands ===

#[tauri::command]
pub async fn get_model_catalog(app: AppHandle) -> Result<ResolvedCatalog, String> {
    let state = current_state(&app)
        .await
        .map_err(|e| format!("get_model_catalog: {e}"))?;
    Ok(lib::resolve_catalog(&state.catalog))
}

#[tauri::command]
pub async fn refresh_model_catalog(app: AppHandle) -> Result<ResolvedCatalog, String> {
    let slim = refresh_catalog_inner(&app).await?;
    Ok(lib::resolve_catalog(&slim))
}
