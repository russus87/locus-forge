mod db;
mod error;
mod model;
mod normalizer;
mod publish;
mod ratelimit;
mod sources;
mod state;

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, State};

use error::{AppError, AppResult};
use model::{CasoRow, DbStats, SourceInfo};
use sources::wikidata::WikidataSource;
use state::{AppState, CrawlStatus, PublishStatus};

const DEFAULT_BACKEND: &str = "http://localhost:8790";
const KEY_BACKEND: &str = "backend_url";
const KEY_INGEST: &str = "ingest_key";

/// Configurazione modificabile dalla UI (pagina Config). Persistita nel DB.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    pub backend_url: String,
    pub ingest_key: String,
}

/// Backend attivo: setting persistente → env `LOCUS_BACKEND_URL` → default localhost.
fn resolve_backend_url(state: &AppState) -> String {
    if let Ok(Some(v)) = state.db.get_setting(KEY_BACKEND) {
        if !v.trim().is_empty() {
            return v;
        }
    }
    std::env::var("LOCUS_BACKEND_URL").unwrap_or_else(|_| DEFAULT_BACKEND.to_string())
}

/// Chiave di ingest: setting persistente → env `LOCUS_INGEST_KEY` → nessuna.
fn resolve_ingest_key(state: &AppState) -> Option<String> {
    if let Ok(Some(v)) = state.db.get_setting(KEY_INGEST) {
        if !v.trim().is_empty() {
            return Some(v);
        }
    }
    std::env::var("LOCUS_INGEST_KEY").ok().filter(|k| !k.is_empty())
}

fn emit_crawl(app: &tauri::AppHandle) {
    let snap = app.state::<AppState>().snapshot_crawl();
    let _ = app.emit("crawl-progress", snap);
}

fn emit_publish(app: &tauri::AppHandle) {
    let snap = app.state::<AppState>().snapshot_publish();
    let _ = app.emit("publish-progress", snap);
}

// ---- comandi ----

#[tauri::command]
fn list_sources() -> Vec<SourceInfo> {
    sources::available_sources()
}

#[tauri::command]
fn backend_target(state: State<AppState>) -> String {
    resolve_backend_url(&state)
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> AppResult<Settings> {
    Ok(Settings {
        backend_url: resolve_backend_url(&state),
        ingest_key: resolve_ingest_key(&state).unwrap_or_default(),
    })
}

#[tauri::command]
fn save_settings(state: State<AppState>, settings: Settings) -> AppResult<()> {
    state.db.set_setting(KEY_BACKEND, settings.backend_url.trim())?;
    state.db.set_setting(KEY_INGEST, settings.ingest_key.trim())?;
    Ok(())
}

#[tauri::command]
fn crawl_status(app: tauri::AppHandle) -> CrawlStatus {
    app.state::<AppState>().snapshot_crawl()
}

#[tauri::command]
fn publish_status(app: tauri::AppHandle) -> PublishStatus {
    app.state::<AppState>().snapshot_publish()
}

#[tauri::command]
fn stop_task(app: tauri::AppHandle) -> AppResult<()> {
    app.state::<AppState>().request_cancel();
    Ok(())
}

#[tauri::command]
fn list_casi(state: State<AppState>, query: Option<String>) -> AppResult<Vec<CasoRow>> {
    Ok(state.db.list_casi(query.as_deref())?)
}

#[tauri::command]
fn db_stats(state: State<AppState>) -> AppResult<DbStats> {
    Ok(state.db.stats()?)
}

#[tauri::command]
async fn start_crawl(app: tauri::AppHandle, source: String, limit: usize) -> AppResult<()> {
    {
        let state = app.state::<AppState>();
        if state.crawl_running() {
            return Err(AppError::Busy);
        }
        if source != "WIKIDATA" {
            return Err(AppError::Other(format!("sorgente sconosciuta: {source}")));
        }
        state.reset_cancel();
        state.set_crawl(CrawlStatus {
            running: true,
            source: Some(source.clone()),
            ..Default::default()
        });
    }
    emit_crawl(&app);

    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        run_crawl(app2, limit).await;
    });
    Ok(())
}

async fn run_crawl(app: tauri::AppHandle, limit: usize) {
    let state = app.state::<AppState>();
    let src = WikidataSource::new(state.http.clone());
    let mut status = state.snapshot_crawl();

    let ids = match src.list_ids(limit).await {
        Ok(v) => v,
        Err(e) => {
            status.running = false;
            status.errors += 1;
            status.last_error = Some(e.to_string());
            state.set_crawl(status);
            emit_crawl(&app);
            return;
        }
    };

    status.total = ids.len();
    state.set_crawl(status.clone());
    emit_crawl(&app);

    for id in ids {
        if state.is_cancelled() {
            status.cancelled = true;
            break;
        }
        state.limiter.acquire("WIKIDATA", src.rate_limit()).await;
        match fetch_normalize_persist(state.inner(), &src, &id).await {
            Ok(Some(true)) => status.inserted += 1,
            Ok(Some(false)) => status.updated += 1,
            Ok(None) => status.skipped += 1,
            Err(e) => {
                status.errors += 1;
                status.last_error = Some(e.to_string());
            }
        }
        status.processed += 1;
        state.set_crawl(status.clone());
        emit_crawl(&app);
    }

    status.running = false;
    state.set_crawl(status);
    emit_crawl(&app);
}

/// fetch → skip incrementale → normalizza → upsert. Ok(Some(created)) o Ok(None) se saltato.
async fn fetch_normalize_persist(
    state: &AppState,
    src: &WikidataSource,
    id: &str,
) -> AppResult<Option<bool>> {
    let item = src.fetch(id).await?;
    let raw = item.raw_json.to_string();
    let hash = db::sha256_hex(&raw);
    if let Some(existing) = state.db.existing_payload_hash(&item.source, &item.source_id)? {
        if existing == hash {
            return Ok(None);
        }
    }
    let norm = normalizer::normalize(&item)?;
    let created = state.db.upsert_caso(&norm, &raw, &hash)?;
    Ok(Some(created))
}

#[tauri::command]
fn reset_published(state: State<AppState>) -> AppResult<usize> {
    Ok(state.db.reset_all_published()?)
}

#[tauri::command]
async fn publish_batch(app: tauri::AppHandle) -> AppResult<()> {
    {
        let state = app.state::<AppState>();
        if state.publish_running() {
            return Err(AppError::Busy);
        }
        state.set_publish(PublishStatus {
            running: true,
            phase: "Avvio".to_string(),
            ..Default::default()
        });
    }
    emit_publish(&app);

    // Risolvo destinazione e chiave PRIMA dello spawn (i valori vengono spostati nel task).
    let backend = resolve_backend_url(app.state::<AppState>().inner());
    let ingest_key = resolve_ingest_key(app.state::<AppState>().inner());

    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app2.state::<AppState>();
        let cb_app = app2.clone();
        publish::publish_all(state.inner(), &backend, ingest_key, move |s| {
            cb_app.state::<AppState>().set_publish(s.clone());
            let _ = cb_app.emit("publish-progress", s.clone());
        })
        .await;
    });
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }));
    }

    builder
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = state::data_dir()?;
            let db_path = dir.join("locus-forge.sqlite");
            let app_state = AppState::new(&db_path)?;
            app.manage(app_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_sources,
            backend_target,
            get_settings,
            save_settings,
            crawl_status,
            publish_status,
            stop_task,
            list_casi,
            db_stats,
            start_crawl,
            publish_batch,
            reset_published,
        ])
        .run(tauri::generate_context!())
        .expect("errore nell'avvio dell'app Tauri");
}
