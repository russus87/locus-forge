# Locus Forge

Tool desktop di **crawling e curation** per il database di **Locus Criminis**.
Ispirato a *Musea Forge*: **Tauri v2 + Rust + Svelte 5**. Pesca i casi di cronaca
nera da **Wikidata (SPARQL)** e li arricchisce con estratto e immagine dalla
**Wikipedia in italiano (MediaWiki API)**, normalizza, deduplica, salva in un
SQLite locale e li **pubblica nel backend** via `POST /api/ingest/casi/batch`.

## Filosofia

Solo **fonti aperte e tracciabili** (Wikidata/Wikipedia, licenza CC BY-SA) —
nessuno scraping di siti terzi, nessun contenuto generato da IA. È il pilastro
di differenziazione di Locus Criminis: *fonti verificate*.

## Requisiti

- Rust (stable), Node 18+, e le librerie di sistema di Tauri (`libwebkit2gtk-4.1`, `libgtk-3`).
- Il backend di Locus Criminis in ascolto (default `http://localhost:8790`).

## Sviluppo

```bash
npm install
npm run tauri dev
```

Override dell'URL del backend:

```bash
LOCUS_BACKEND_URL=http://localhost:8790 npm run tauri dev
```

## Come funziona

1. **Crawl** — scegli la sorgente (`Wikidata + Wikipedia`), imposta un limite e avvia.
   Il Forge esegue una query SPARQL per i casi in Italia (istanze di omicidio,
   assassinio, attentato, strage…) con articolo su it.wikipedia, poi per ciascuno
   scarica l'estratto e l'immagine. Progresso live (nuovi/aggiornati/saltati/errori).
2. **Sfoglia** — controlla i casi nel database locale prima di pubblicarli.
3. **Pubblica** — invia al backend i casi non ancora pubblicati, a blocchi. È
   **idempotente**: i casi già inviati vengono marcati e non re-inviati; lato
   backend l'upsert avviene per `wikidataQid`.

## Architettura (Rust, `src-tauri/src/`)

| File | Ruolo |
|---|---|
| `lib.rs` | Comandi Tauri + orchestrazione del crawl (`crawl_core`), eventi di progresso |
| `state.rs` | `AppState`, `CrawlStatus`/`PublishStatus`, `data_dir()`, cancellazione cooperativa |
| `error.rs` | `AppError` tipato (thiserror) serializzato all'IPC |
| `ratelimit.rs` | Token-bucket per sorgente + backoff esponenziale |
| `db.rs` | SQLite (`rusqlite` bundled): schema, upsert idempotente, query, selezione publish |
| `model.rs` | Modello normalizzato interno + DTO `CasoIn` (camelCase) per il backend |
| `normalizer.rs` | `RawItem` → `NormalizedCaso` + euristica di categoria |
| `sources/wikidata.rs` | Query SPARQL + arricchimento MediaWiki, con retry/backoff su 429/5xx |
| `publish.rs` | POST batch al backend con retry, marcatura "pubblicato" |

Eventi verso il frontend: `crawl-progress` (`CrawlStatus`) e `publish-progress` (`PublishStatus`).

## Nota

La query SPARQL è volutamente conservativa (poche classi Wikidata, solo Italia,
solo item con articolo su it.wikipedia). L'euristica di categoria è basata su
parole chiave: i casi importati vanno **rivisti** prima o dopo la pubblicazione
(la categoria e i dettagli possono essere corretti lato backend). È un motore di
*ingest assistito*, non un sostituto della curatela editoriale.
