use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

use crate::model::{
    CasoIn, CasoRow, CategoriaCount, DbStats, FonteIn, LuogoIn, NormalizedCaso, PersonaIn,
};

const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS caso (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    source        TEXT NOT NULL,
    source_id     TEXT NOT NULL,
    source_url    TEXT,
    wikidata_qid  TEXT,
    wikipedia_url TEXT,
    titolo        TEXT NOT NULL,
    sommario      TEXT,
    descrizione   TEXT,
    categoria     TEXT NOT NULL,
    anno          INTEGER,
    data_evento   TEXT,
    lat           REAL,
    lon           REAL,
    luogo_nome    TEXT,
    immagine_url  TEXT,
    raw_payload   TEXT,
    payload_hash  TEXT,
    fetched_at    TEXT,
    published_at  TEXT,
    UNIQUE(source, source_id)
);

CREATE TABLE IF NOT EXISTS persona (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    caso_id       INTEGER NOT NULL REFERENCES caso(id) ON DELETE CASCADE,
    nome          TEXT NOT NULL,
    ruolo         TEXT NOT NULL,
    wikidata_qid  TEXT,
    wikipedia_url TEXT
);
CREATE INDEX IF NOT EXISTS idx_persona_caso ON persona(caso_id);

CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

pub fn sha256_hex(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    format!("{:x}", h.finalize())
}

/// Wrapper attorno a una singola connessione SQLite (sincrona, dietro Mutex).
pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=15000;")?;
        conn.execute_batch(SCHEMA_SQL)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Valore di configurazione persistente (tabella `settings`), se presente.
    pub fn get_setting(&self, key: &str) -> anyhow::Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let v: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()?;
        Ok(v)
    }

    /// Salva (o aggiorna) un valore di configurazione persistente.
    pub fn set_setting(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Hash del payload già salvato per (source, source_id), se presente.
    pub fn existing_payload_hash(&self, source: &str, source_id: &str) -> anyhow::Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let v: Option<String> = conn
            .query_row(
                "SELECT payload_hash FROM caso WHERE source = ?1 AND source_id = ?2",
                params![source, source_id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(v)
    }

    /// Upsert idempotente. Ritorna true se è stato creato (insert), false se aggiornato.
    pub fn upsert_caso(&self, n: &NormalizedCaso, raw_payload: &str, hash: &str) -> anyhow::Result<bool> {
        let conn = self.conn.lock().unwrap();
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM caso WHERE source = ?1 AND source_id = ?2",
                params![n.source, n.source_id],
                |r| r.get(0),
            )
            .optional()?;

        let (caso_id, created) = if let Some(id) = existing {
            conn.execute(
                "UPDATE caso SET wikidata_qid=?1, wikipedia_url=?2, titolo=?3, sommario=?4, \
                 descrizione=?5, categoria=?6, anno=?7, data_evento=?8, lat=?9, lon=?10, \
                 luogo_nome=?11, immagine_url=?12, raw_payload=?13, payload_hash=?14, \
                 fetched_at=strftime('%Y-%m-%dT%H:%M:%SZ','now') \
                 WHERE id=?15",
                params![
                    n.wikidata_qid, n.wikipedia_url, n.titolo, n.sommario, n.descrizione,
                    n.categoria, n.anno, n.data_evento, n.lat, n.lon, n.luogo_nome,
                    n.immagine_url, raw_payload, hash, id
                ],
            )?;
            (id, false)
        } else {
            conn.execute(
                "INSERT INTO caso (source, source_id, source_url, wikidata_qid, wikipedia_url, \
                 titolo, sommario, descrizione, categoria, anno, data_evento, lat, lon, \
                 luogo_nome, immagine_url, raw_payload, payload_hash, fetched_at) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17, \
                 strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
                params![
                    n.source, n.source_id, n.source_url, n.wikidata_qid, n.wikipedia_url,
                    n.titolo, n.sommario, n.descrizione, n.categoria, n.anno, n.data_evento,
                    n.lat, n.lon, n.luogo_nome, n.immagine_url, raw_payload, hash
                ],
            )?;
            (conn.last_insert_rowid(), true)
        };

        // Sostituisce le persone del caso (derivate dal payload di questa fonte).
        conn.execute("DELETE FROM persona WHERE caso_id = ?1", params![caso_id])?;
        for p in &n.persone {
            conn.execute(
                "INSERT INTO persona (caso_id, nome, ruolo, wikidata_qid, wikipedia_url) \
                 VALUES (?1,?2,?3,?4,?5)",
                params![caso_id, p.nome, p.ruolo, p.wikidata_qid, p.wikipedia_url],
            )?;
        }

        Ok(created)
    }

    pub fn list_casi(&self, query: Option<&str>) -> anyhow::Result<Vec<CasoRow>> {
        let conn = self.conn.lock().unwrap();
        let like = query
            .filter(|q| !q.trim().is_empty())
            .map(|q| format!("%{}%", q.trim().to_lowercase()));
        let mut stmt = conn.prepare(
            "SELECT id, titolo, categoria, anno, wikidata_qid, wikipedia_url, sommario, \
             lat, lon, immagine_url, published_at \
             FROM caso \
             WHERE (?1 IS NULL OR lower(titolo) LIKE ?1) \
             ORDER BY anno DESC NULLS LAST, titolo ASC",
        )?;
        let rows = stmt
            .query_map(params![like], |r| {
                let published_at: Option<String> = r.get(10)?;
                Ok(CasoRow {
                    id: r.get(0)?,
                    titolo: r.get(1)?,
                    categoria: r.get(2)?,
                    anno: r.get(3)?,
                    wikidata_qid: r.get(4)?,
                    wikipedia_url: r.get(5)?,
                    sommario: r.get(6)?,
                    lat: r.get(7)?,
                    lon: r.get(8)?,
                    immagine_url: r.get(9)?,
                    published: published_at.is_some(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn stats(&self) -> anyhow::Result<DbStats> {
        let conn = self.conn.lock().unwrap();
        let totale: i64 = conn.query_row("SELECT COUNT(*) FROM caso", [], |r| r.get(0))?;
        let con_coordinate: i64 =
            conn.query_row("SELECT COUNT(*) FROM caso WHERE lat IS NOT NULL", [], |r| r.get(0))?;
        let pubblicati: i64 = conn.query_row(
            "SELECT COUNT(*) FROM caso WHERE published_at IS NOT NULL",
            [],
            |r| r.get(0),
        )?;
        let mut stmt = conn.prepare(
            "SELECT categoria, COUNT(*) FROM caso GROUP BY categoria ORDER BY COUNT(*) DESC",
        )?;
        let per_categoria = stmt
            .query_map([], |r| {
                Ok(CategoriaCount {
                    categoria: r.get(0)?,
                    count: r.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(DbStats {
            totale,
            con_coordinate,
            pubblicati,
            da_pubblicare: totale - pubblicati,
            per_categoria,
        })
    }

    /// Casi non ancora pubblicati, come coppie (id, DTO pronto per il backend).
    pub fn casi_pending_publish(&self) -> anyhow::Result<Vec<(i64, CasoIn)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, titolo, categoria, sommario, descrizione, anno, data_evento, \
             wikidata_qid, wikipedia_url, immagine_url, lat, lon, luogo_nome \
             FROM caso WHERE published_at IS NULL ORDER BY id ASC",
        )?;
        let mut rows = stmt
            .query_map([], |r| {
                let id: i64 = r.get(0)?;
                let wikipedia_url: Option<String> = r.get(8)?;
                let lat: Option<f64> = r.get(10)?;
                let lon: Option<f64> = r.get(11)?;
                let luogo_nome: Option<String> = r.get(12)?;
                let luogo = match (lat, lon) {
                    (Some(la), Some(lo)) => Some(LuogoIn {
                        nome: luogo_nome.unwrap_or_else(|| "Luogo".to_string()),
                        lat: la,
                        lon: lo,
                        wikidata_qid: None,
                    }),
                    _ => None,
                };
                let mut fonti = Vec::new();
                if let Some(url) = &wikipedia_url {
                    fonti.push(FonteIn {
                        titolo: "Wikipedia".to_string(),
                        url: Some(url.clone()),
                        tipo: "WIKIPEDIA".to_string(),
                        licenza: Some("CC BY-SA 4.0".to_string()),
                    });
                }
                let caso = CasoIn {
                    titolo: r.get(1)?,
                    categoria: r.get(2)?,
                    sommario: r.get(3)?,
                    descrizione: r.get(4)?,
                    anno: r.get(5)?,
                    data_evento: r.get(6)?,
                    wikidata_qid: r.get(7)?,
                    wikipedia_url,
                    immagine_url: r.get(9)?,
                    immagine_licenza: None,
                    luogo,
                    persone: Vec::new(),
                    fonti,
                };
                Ok((id, caso))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Aggancia le persone (vittime/colpevoli) a ciascun caso.
        let mut pstmt = conn.prepare(
            "SELECT nome, ruolo, wikidata_qid, wikipedia_url FROM persona WHERE caso_id = ?1",
        )?;
        for (id, caso) in rows.iter_mut() {
            let persone = pstmt
                .query_map(params![*id], |r| {
                    Ok(PersonaIn {
                        nome: r.get(0)?,
                        ruolo: r.get(1)?,
                        wikidata_qid: r.get(2)?,
                        wikipedia_url: r.get(3)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            caso.persone = persone;
        }
        Ok(rows)
    }

    pub fn mark_published(&self, ids: &[i64]) -> anyhow::Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        for id in ids {
            tx.execute(
                "UPDATE caso SET published_at = strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE id = ?1",
                params![id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Azzera il flag di pubblicazione di TUTTI i casi: li rende di nuovo "da
    /// pubblicare". Serve per ri-inviare l'intero DB a un backend diverso.
    /// Ritorna il numero di casi resettati.
    pub fn reset_all_published(&self) -> anyhow::Result<usize> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute("UPDATE caso SET published_at = NULL", [])?;
        Ok(n)
    }
}
