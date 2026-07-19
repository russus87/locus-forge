use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

use crate::model::{
    CasoDettaglio, CasoEdit, CasoIn, CasoRow, CategoriaCount, DbStats, FonteIn, LuogoIn, MediaIn,
    MediaRow, NormalizedCaso, PersonaIn, PersonaRow,
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
    contenuto_html TEXT,
    lingua        TEXT DEFAULT 'it',
    paese         TEXT DEFAULT 'IT',
    UNIQUE(source, source_id)
);

CREATE TABLE IF NOT EXISTS persona (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    caso_id       INTEGER NOT NULL REFERENCES caso(id) ON DELETE CASCADE,
    nome          TEXT NOT NULL,
    ruolo         TEXT NOT NULL,
    descrizione   TEXT,
    biografia     TEXT,
    immagine_url  TEXT,
    data_nascita  TEXT,
    data_morte    TEXT,
    luogo_nascita TEXT,
    occupazione   TEXT,
    nazionalita   TEXT,
    wikidata_qid  TEXT,
    wikipedia_url TEXT
);
CREATE INDEX IF NOT EXISTS idx_persona_caso ON persona(caso_id);

CREATE TABLE IF NOT EXISTS media (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    caso_id    INTEGER NOT NULL REFERENCES caso(id) ON DELETE CASCADE,
    tipo       TEXT NOT NULL,
    url        TEXT NOT NULL,
    titolo     TEXT,
    didascalia TEXT,
    ordine     INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_media_caso ON media(caso_id);

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
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=15000; PRAGMA foreign_keys=ON;")?;
        conn.execute_batch(SCHEMA_SQL)?;
        // Migrazione idempotente per DB creati prima dell'editor: aggiunge le
        // colonne mancanti su `caso` (la tabella `media` la crea SCHEMA_SQL).
        Self::ensure_column(&conn, "caso", "contenuto_html", "TEXT")?;
        Self::ensure_column(&conn, "caso", "lingua", "TEXT DEFAULT 'it'")?;
        Self::ensure_column(&conn, "caso", "paese", "TEXT DEFAULT 'IT'")?;
        for col in [
            "descrizione", "biografia", "immagine_url", "data_nascita", "data_morte",
            "luogo_nascita", "occupazione", "nazionalita",
        ] {
            Self::ensure_column(&conn, "persona", col, "TEXT")?;
        }
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Aggiunge una colonna a una tabella se non esiste già (migrazione soft).
    fn ensure_column(conn: &Connection, table: &str, col: &str, decl: &str) -> anyhow::Result<()> {
        let present: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM pragma_table_info(?1) WHERE name = ?2",
                params![table, col],
                |r| r.get(0),
            )
            .optional()?;
        if present.is_none() {
            conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {col} {decl};"))?;
        }
        Ok(())
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
                "INSERT INTO persona (caso_id, nome, ruolo, descrizione, biografia, \
                 immagine_url, data_nascita, data_morte, luogo_nascita, occupazione, \
                 nazionalita, wikidata_qid, wikipedia_url) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
                params![
                    caso_id, p.nome, p.ruolo, p.descrizione, p.biografia, p.immagine_url,
                    p.data_nascita, p.data_morte, p.luogo_nascita, p.occupazione,
                    p.nazionalita, p.wikidata_qid, p.wikipedia_url
                ],
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

    /// Scheda completa di un caso per l'editor (campi + media + persone).
    pub fn get_caso(&self, id: i64) -> anyhow::Result<Option<CasoDettaglio>> {
        let conn = self.conn.lock().unwrap();
        let base: Option<CasoDettaglio> = conn
            .query_row(
                "SELECT id, titolo, sommario, descrizione, contenuto_html, categoria, anno, \
                 wikipedia_url, immagine_url, luogo_nome, lat, lon, published_at \
                 FROM caso WHERE id = ?1",
                params![id],
                |r| {
                    let published_at: Option<String> = r.get(12)?;
                    Ok(CasoDettaglio {
                        id: r.get(0)?,
                        titolo: r.get(1)?,
                        sommario: r.get(2)?,
                        descrizione: r.get(3)?,
                        contenuto_html: r.get(4)?,
                        categoria: r.get(5)?,
                        anno: r.get(6)?,
                        wikipedia_url: r.get(7)?,
                        immagine_url: r.get(8)?,
                        luogo_nome: r.get(9)?,
                        lat: r.get(10)?,
                        lon: r.get(11)?,
                        published: published_at.is_some(),
                        media: Vec::new(),
                        persone: Vec::new(),
                    })
                },
            )
            .optional()?;

        let mut caso = match base {
            Some(c) => c,
            None => return Ok(None),
        };

        let mut mstmt = conn.prepare(
            "SELECT tipo, url, titolo, didascalia, ordine FROM media \
             WHERE caso_id = ?1 ORDER BY ordine ASC, id ASC",
        )?;
        caso.media = mstmt
            .query_map(params![id], |r| {
                Ok(MediaRow {
                    tipo: r.get(0)?,
                    url: r.get(1)?,
                    titolo: r.get(2)?,
                    didascalia: r.get(3)?,
                    ordine: r.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut pstmt = conn.prepare(
            "SELECT nome, ruolo, descrizione, biografia, immagine_url, data_nascita, \
             data_morte, luogo_nascita, occupazione, nazionalita, wikidata_qid, wikipedia_url \
             FROM persona WHERE caso_id = ?1 ORDER BY id ASC",
        )?;
        caso.persone = pstmt
            .query_map(params![id], |r| {
                Ok(PersonaRow {
                    nome: r.get(0)?,
                    ruolo: r.get(1)?,
                    descrizione: r.get(2)?,
                    biografia: r.get(3)?,
                    immagine_url: r.get(4)?,
                    data_nascita: r.get(5)?,
                    data_morte: r.get(6)?,
                    luogo_nascita: r.get(7)?,
                    occupazione: r.get(8)?,
                    nazionalita: r.get(9)?,
                    wikidata_qid: r.get(10)?,
                    wikipedia_url: r.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Some(caso))
    }

    /// Salva le modifiche dell'editor su un caso. Rimette il caso come "da
    /// pubblicare" (published_at = NULL) e sostituisce i media in blocco.
    pub fn update_caso(&self, id: i64, edit: &CasoEdit) -> anyhow::Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let contenuto = edit
            .contenuto_html
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        tx.execute(
            "UPDATE caso SET titolo=?1, sommario=?2, categoria=?3, anno=?4, \
             contenuto_html=?5, published_at=NULL WHERE id=?6",
            params![
                edit.titolo.trim(),
                edit.sommario.as_deref().map(str::trim),
                edit.categoria.trim(),
                edit.anno,
                contenuto,
                id
            ],
        )?;
        tx.execute("DELETE FROM media WHERE caso_id = ?1", params![id])?;
        for (i, m) in edit.media.iter().enumerate() {
            if m.url.trim().is_empty() {
                continue;
            }
            tx.execute(
                "INSERT INTO media (caso_id, tipo, url, titolo, didascalia, ordine) \
                 VALUES (?1,?2,?3,?4,?5,?6)",
                params![
                    id,
                    m.tipo.trim(),
                    m.url.trim(),
                    m.titolo.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                    m.didascalia.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                    i as i64
                ],
            )?;
        }
        // Sostituzione delle persone curate.
        tx.execute("DELETE FROM persona WHERE caso_id = ?1", params![id])?;
        for p in &edit.persone {
            if p.nome.trim().is_empty() {
                continue;
            }
            let clean = |o: &Option<String>| {
                o.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
            };
            tx.execute(
                "INSERT INTO persona (caso_id, nome, ruolo, descrizione, biografia, \
                 immagine_url, data_nascita, data_morte, luogo_nascita, occupazione, \
                 nazionalita, wikidata_qid, wikipedia_url) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
                params![
                    id,
                    p.nome.trim(),
                    p.ruolo.trim(),
                    clean(&p.descrizione),
                    clean(&p.biografia),
                    clean(&p.immagine_url),
                    clean(&p.data_nascita),
                    clean(&p.data_morte),
                    clean(&p.luogo_nascita),
                    clean(&p.occupazione),
                    clean(&p.nazionalita),
                    clean(&p.wikidata_qid),
                    clean(&p.wikipedia_url)
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Torna al testo originale: azzera il contenuto curato (l'app mostrerà la
    /// descrizione del crawler). Rimette il caso come "da pubblicare".
    pub fn revert_original(&self, id: i64) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE caso SET contenuto_html=NULL, published_at=NULL WHERE id=?1",
            params![id],
        )?;
        Ok(())
    }

    /// Casi non ancora pubblicati, come coppie (id, DTO pronto per il backend).
    pub fn casi_pending_publish(&self) -> anyhow::Result<Vec<(i64, CasoIn)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, titolo, categoria, sommario, descrizione, anno, data_evento, \
             wikidata_qid, wikipedia_url, immagine_url, lat, lon, luogo_nome, \
             contenuto_html, lingua, paese \
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
                    contenuto_html: r.get(13)?,
                    lingua: r.get(14)?,
                    paese: r.get(15)?,
                    anno: r.get(5)?,
                    data_evento: r.get(6)?,
                    wikidata_qid: r.get(7)?,
                    wikipedia_url,
                    immagine_url: r.get(9)?,
                    immagine_licenza: None,
                    luogo,
                    persone: Vec::new(),
                    fonti,
                    media: Vec::new(),
                };
                Ok((id, caso))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Aggancia le persone (vittime/colpevoli) a ciascun caso.
        let mut pstmt = conn.prepare(
            "SELECT nome, ruolo, descrizione, biografia, immagine_url, data_nascita, \
             data_morte, luogo_nascita, occupazione, nazionalita, wikidata_qid, wikipedia_url \
             FROM persona WHERE caso_id = ?1 ORDER BY id ASC",
        )?;
        for (id, caso) in rows.iter_mut() {
            let persone = pstmt
                .query_map(params![*id], |r| {
                    Ok(PersonaIn {
                        nome: r.get(0)?,
                        ruolo: r.get(1)?,
                        descrizione: r.get(2)?,
                        biografia: r.get(3)?,
                        immagine_url: r.get(4)?,
                        data_nascita: r.get(5)?,
                        data_morte: r.get(6)?,
                        luogo_nascita: r.get(7)?,
                        occupazione: r.get(8)?,
                        nazionalita: r.get(9)?,
                        wikidata_qid: r.get(10)?,
                        wikipedia_url: r.get(11)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            caso.persone = persone;
        }

        // Aggancia i media (embed) curati a ciascun caso.
        let mut mstmt = conn.prepare(
            "SELECT tipo, url, titolo, didascalia, ordine FROM media \
             WHERE caso_id = ?1 ORDER BY ordine ASC, id ASC",
        )?;
        for (id, caso) in rows.iter_mut() {
            let media = mstmt
                .query_map(params![*id], |r| {
                    Ok(MediaIn {
                        tipo: r.get(0)?,
                        url: r.get(1)?,
                        titolo: r.get(2)?,
                        didascalia: r.get(3)?,
                        ordine: r.get(4)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            caso.media = media;
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
