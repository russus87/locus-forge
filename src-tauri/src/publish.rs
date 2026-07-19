use std::time::Duration;

use crate::model::CasoIn;
use crate::state::{AppState, PublishStatus};

const CHUNK: usize = 100;

/// Pubblica tutti i casi non ancora inviati al backend Spring, a blocchi,
/// aggiornando lo stato via callback. Idempotente: marca i casi come pubblicati.
pub async fn publish_all(
    state: &AppState,
    backend_url: &str,
    mut on_progress: impl FnMut(&PublishStatus),
) {
    let pending = match state.db.casi_pending_publish() {
        Ok(p) => p,
        Err(e) => {
            let status = PublishStatus {
                running: false,
                phase: "Errore".to_string(),
                last_error: Some(e.to_string()),
                ..Default::default()
            };
            on_progress(&status);
            return;
        }
    };

    let total = pending.len();
    let mut status = PublishStatus {
        running: true,
        phase: "Preparazione".to_string(),
        percent: 0,
        sent: 0,
        total,
        last_error: None,
    };
    on_progress(&status);

    if total == 0 {
        status.running = false;
        status.phase = "Nessun caso da pubblicare".to_string();
        status.percent = 100;
        on_progress(&status);
        return;
    }

    let url = format!("{}/api/ingest/casi/batch", backend_url.trim_end_matches('/'));
    // Chiave opzionale per il backend di produzione (header X-Ingest-Key).
    let ingest_key = std::env::var("LOCUS_INGEST_KEY").ok().filter(|k| !k.is_empty());

    for chunk in pending.chunks(CHUNK) {
        let payload: Vec<CasoIn> = chunk.iter().map(|(_, c)| c.clone()).collect();

        let mut attempt: u32 = 0;
        let sent_ok = loop {
            let mut req = state.http.post(&url).json(&payload);
            if let Some(k) = &ingest_key {
                req = req.header("X-Ingest-Key", k);
            }
            match req.send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        break true;
                    }
                    if attempt < 3 {
                        backoff(attempt).await;
                        attempt += 1;
                        continue;
                    }
                    status.last_error = Some(format!("HTTP {} dal backend", resp.status()));
                    break false;
                }
                Err(e) => {
                    if attempt < 3 {
                        backoff(attempt).await;
                        attempt += 1;
                        continue;
                    }
                    status.last_error = Some(e.to_string());
                    break false;
                }
            }
        };

        if !sent_ok {
            status.running = false;
            status.phase = "Errore".to_string();
            on_progress(&status);
            return;
        }

        let ids: Vec<i64> = chunk.iter().map(|(id, _)| *id).collect();
        if let Err(e) = state.db.mark_published(&ids) {
            status.last_error = Some(e.to_string());
        }
        status.sent += chunk.len();
        status.phase = "Invio al backend".to_string();
        status.percent = ((status.sent as f64 / total as f64) * 100.0) as u8;
        on_progress(&status);
    }

    status.running = false;
    status.phase = "Completato".to_string();
    status.percent = 100;
    on_progress(&status);
}

async fn backoff(attempt: u32) {
    let secs = 2f64.powi(attempt.min(6) as i32);
    tokio::time::sleep(Duration::from_secs_f64(secs.min(30.0))).await;
}
