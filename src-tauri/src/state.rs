use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde::Serialize;

use crate::db::Db;
use crate::ratelimit::RateLimiter;

/// Directory dati dell'app (`~/.local/share/locus-forge` su Linux, `%APPDATA%` su Windows).
/// Usa `dirs` invece di `app_data_dir()` di Tauri così lo stesso codice gira anche headless.
pub fn data_dir() -> anyhow::Result<PathBuf> {
    let base = dirs::data_dir().ok_or_else(|| anyhow::anyhow!("data dir non disponibile"))?;
    let dir = base.join("locus-forge");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Stato di un crawl in corso — snapshot inviato via evento `crawl-progress`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CrawlStatus {
    pub running: bool,
    pub source: Option<String>,
    pub processed: usize,
    pub total: usize,
    pub inserted: usize,
    pub updated: usize,
    pub skipped: usize,
    pub errors: usize,
    pub last_error: Option<String>,
    pub cancelled: bool,
}

/// Stato di una pubblicazione — snapshot inviato via evento `publish-progress`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PublishStatus {
    pub running: bool,
    pub phase: String,
    pub percent: u8,
    pub sent: usize,
    pub total: usize,
    pub last_error: Option<String>,
}

/// Stato condiviso dell'applicazione, gestito da Tauri con `app.manage(...)`.
pub struct AppState {
    pub db: Db,
    pub http: reqwest::Client,
    pub limiter: RateLimiter,
    crawl: Mutex<CrawlStatus>,
    publish: Mutex<PublishStatus>,
    cancel: AtomicBool,
}

impl AppState {
    pub fn new(db_path: &std::path::Path) -> anyhow::Result<Self> {
        let db = Db::open(db_path)?;
        // Wikimedia/Wikidata richiedono uno User-Agent descrittivo, altrimenti 403.
        let http = reqwest::Client::builder()
            .user_agent("LocusForge/0.1 (https://anvil.russus.it; Locus Criminis crawler)")
            .build()?;
        Ok(Self {
            db,
            http,
            limiter: RateLimiter::new(),
            crawl: Mutex::new(CrawlStatus::default()),
            publish: Mutex::new(PublishStatus::default()),
            cancel: AtomicBool::new(false),
        })
    }

    pub fn snapshot_crawl(&self) -> CrawlStatus {
        self.crawl.lock().unwrap().clone()
    }

    pub fn set_crawl(&self, status: CrawlStatus) {
        *self.crawl.lock().unwrap() = status;
    }

    pub fn crawl_running(&self) -> bool {
        self.crawl.lock().unwrap().running
    }

    pub fn snapshot_publish(&self) -> PublishStatus {
        self.publish.lock().unwrap().clone()
    }

    pub fn set_publish(&self, status: PublishStatus) {
        *self.publish.lock().unwrap() = status;
    }

    pub fn publish_running(&self) -> bool {
        self.publish.lock().unwrap().running
    }

    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    pub fn reset_cancel(&self) {
        self.cancel.store(false, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
}
