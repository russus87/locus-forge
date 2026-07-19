use std::collections::HashMap;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::Mutex;

use super::RawItem;

const SPARQL_ENDPOINT: &str = "https://query.wikidata.org/sparql";
const ITWIKI_API: &str = "https://it.wikipedia.org/w/api.php";

/// Riga risultante dalla query SPARQL, messa in cache tra `list_ids` e `fetch`.
#[derive(Debug, Clone, Default)]
struct SparqlRow {
    label: String,
    lat: Option<f64>,
    lon: Option<f64>,
    date: Option<String>,
    type_label: Option<String>,
    article_url: Option<String>,
}

/// Sorgente Wikidata (SPARQL) con arricchimento dalla Wikipedia in italiano.
pub struct WikidataSource {
    http: reqwest::Client,
    cache: Mutex<HashMap<String, SparqlRow>>,
}

impl WikidataSource {
    pub fn new(http: reqwest::Client) -> Self {
        Self {
            http,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Conservativo: Wikidata SPARQL e MediaWiki vanno trattati con gentilezza.
    pub fn rate_limit(&self) -> f64 {
        1.0
    }

    fn sparql_query(limit: usize) -> String {
        // Casi in Italia (P17 = Q38) che sono istanze di una vasta gamma di classi
        // di reato/evento violento (omicidi, stragi, massacri, attentati, sequestri,
        // sparatorie, rapine, crimini irrisolti, ecc.), con un articolo sulla
        // Wikipedia in italiano; coordinate e data opzionali. Lista di classi
        // esplicita (query veloce) ricavata dall'albero delle sottoclassi di
        // "crimine" (Q83267) e "omicidio/uccisione" (Q149086).
        format!(
            r#"SELECT ?item ?itemLabel ?coord ?date ?typeLabel ?article WHERE {{
  VALUES ?type {{
    wd:Q3199915 wd:Q750215 wd:Q2223653 wd:Q3882219 wd:Q149086 wd:Q132821
    wd:Q4676786 wd:Q1139665 wd:Q81672 wd:Q51159758 wd:Q318296 wd:Q891854
    wd:Q28934204 wd:Q217327 wd:Q21480300 wd:Q216169 wd:Q11519624 wd:Q53706
    wd:Q806824 wd:Q327541 wd:Q132781 wd:Q177716 wd:Q135010 wd:Q19841484
    wd:Q83267 wd:Q1174599 wd:Q1920219
  }}
  ?item wdt:P31 ?type .
  ?item wdt:P17 wd:Q38 .
  ?article schema:about ?item ; schema:isPartOf <https://it.wikipedia.org/> .
  OPTIONAL {{ ?item wdt:P625 ?coord. }}
  OPTIONAL {{ ?item wdt:P585 ?date. }}
  SERVICE wikibase:label {{ bd:serviceParam wikibase:language "it". }}
}}
ORDER BY DESC(?date)
LIMIT {limit}"#
        )
    }

    /// Esegue la SPARQL, popola la cache e ritorna i QID (fino a `limit`).
    pub async fn list_ids(&self, limit: usize) -> anyhow::Result<Vec<String>> {
        let query = Self::sparql_query(limit.max(1));
        let json = self
            .get_json_retry(SPARQL_ENDPOINT, &[("query", query.as_str()), ("format", "json")], true)
            .await?;

        let bindings = json
            .get("results")
            .and_then(|r| r.get("bindings"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let mut cache = self.cache.lock().await;
        let mut ids = Vec::new();
        for b in bindings {
            let item_uri = b
                .get("item")
                .and_then(|v| v.get("value"))
                .and_then(Value::as_str);
            let Some(item_uri) = item_uri else { continue };
            let qid = item_uri.rsplit('/').next().unwrap_or("").to_string();
            if qid.is_empty() || cache.contains_key(&qid) {
                if !qid.is_empty() && !ids.contains(&qid) {
                    ids.push(qid);
                }
                continue;
            }

            let label = b
                .get("itemLabel")
                .and_then(|v| v.get("value"))
                .and_then(Value::as_str)
                .unwrap_or(&qid)
                .to_string();
            let (lat, lon) = b
                .get("coord")
                .and_then(|v| v.get("value"))
                .and_then(Value::as_str)
                .and_then(parse_point)
                .map(|(la, lo)| (Some(la), Some(lo)))
                .unwrap_or((None, None));
            let date = b
                .get("date")
                .and_then(|v| v.get("value"))
                .and_then(Value::as_str)
                .map(|s| s.to_string());
            let type_label = b
                .get("typeLabel")
                .and_then(|v| v.get("value"))
                .and_then(Value::as_str)
                .map(|s| s.to_string());
            let article_url = b
                .get("article")
                .and_then(|v| v.get("value"))
                .and_then(Value::as_str)
                .map(|s| s.to_string());

            cache.insert(
                qid.clone(),
                SparqlRow { label, lat, lon, date, type_label, article_url },
            );
            ids.push(qid);
        }
        Ok(ids)
    }

    /// Costruisce il RawItem combinando la riga SPARQL con l'estratto Wikipedia.
    pub async fn fetch(&self, qid: &str) -> anyhow::Result<RawItem> {
        let row = {
            let cache = self.cache.lock().await;
            cache
                .get(qid)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("QID non in cache: {qid}"))?
        };

        let mut extract: Option<String> = None;
        let mut image: Option<String> = None;
        let mut lat = row.lat;
        let mut lon = row.lon;

        if let Some(title) = row.article_url.as_deref().and_then(article_title) {
            let params = [
                ("action", "query"),
                ("format", "json"),
                ("formatversion", "2"),
                ("redirects", "1"),
                ("prop", "extracts|pageimages|coordinates"),
                ("exintro", "1"),
                ("explaintext", "1"),
                ("piprop", "thumbnail"),
                ("pithumbsize", "800"),
                ("titles", title.as_str()),
            ];
            if let Ok(json) = self.get_json_retry(ITWIKI_API, &params, false).await {
                if let Some(page) = json
                    .get("query")
                    .and_then(|q| q.get("pages"))
                    .and_then(Value::as_array)
                    .and_then(|a| a.first())
                {
                    extract = page.get("extract").and_then(Value::as_str).map(|s| s.to_string());
                    image = page
                        .get("thumbnail")
                        .and_then(|t| t.get("source"))
                        .and_then(Value::as_str)
                        .map(|s| s.to_string());
                    if lat.is_none() {
                        if let Some(c) = page
                            .get("coordinates")
                            .and_then(Value::as_array)
                            .and_then(|a| a.first())
                        {
                            lat = c.get("lat").and_then(Value::as_f64);
                            lon = c.get("lon").and_then(Value::as_f64);
                        }
                    }
                }
            }
        }

        let (persone_raw, luogo) = self.fetch_enrichment(qid).await;
        let persone: Vec<Value> = persone_raw
            .into_iter()
            .map(|(nome, pqid, ruolo)| {
                serde_json::json!({ "nome": nome, "ruolo": ruolo, "qid": pqid })
            })
            .collect();

        let raw_json = serde_json::json!({
            "qid": qid,
            "label": row.label,
            "typeLabel": row.type_label,
            "date": row.date,
            "lat": lat,
            "lon": lon,
            "article": row.article_url,
            "extract": extract,
            "image": image,
            "luogo": luogo,
            "persone": persone,
        });

        Ok(RawItem {
            source: "WIKIDATA".to_string(),
            source_id: qid.to_string(),
            source_url: Some(format!("https://www.wikidata.org/wiki/{qid}")),
            raw_json,
        })
    }

    /// Arricchimento per caso: vittime (P8032, P533) e colpevoli (P8031) come
    /// persone, più il nome del luogo (P131 comune, o P276 località). Un'unica query.
    /// Ritorna (persone: [(nome, qid, ruolo)] deduplicate, luogo: Option<String>).
    async fn fetch_enrichment(&self, qid: &str) -> (Vec<(String, String, String)>, Option<String>) {
        let query = format!(
            r#"SELECT ?kind ?e ?eLabel ?role WHERE {{
  {{
    VALUES (?prop ?role) {{ (wdt:P8032 "VITTIMA") (wdt:P533 "VITTIMA") (wdt:P8031 "CONDANNATO") }}
    wd:{qid} ?prop ?e . ?e wdt:P31 wd:Q5 . BIND("PERSONA" AS ?kind)
  }} UNION {{
    VALUES ?prop2 {{ wdt:P131 wdt:P276 }}
    wd:{qid} ?prop2 ?e . BIND("LUOGO" AS ?kind) BIND("" AS ?role)
  }}
  SERVICE wikibase:label {{ bd:serviceParam wikibase:language "it,en". ?e rdfs:label ?eLabel. }}
}}"#
        );
        let json = match self
            .get_json_retry(SPARQL_ENDPOINT, &[("query", query.as_str()), ("format", "json")], true)
            .await
        {
            Ok(j) => j,
            Err(_) => return (Vec::new(), None),
        };

        let bindings = json
            .get("results")
            .and_then(|r| r.get("bindings"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let field = |b: &Value, k: &str| {
            b.get(k)
                .and_then(|v| v.get("value"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };

        let mut seen = std::collections::HashSet::new();
        let mut persone = Vec::new();
        let mut luogo: Option<String> = None;
        for b in &bindings {
            let kind = field(b, "kind");
            let nome = field(b, "eLabel");
            if nome.is_empty() {
                continue;
            }
            if kind == "LUOGO" {
                if luogo.is_none() {
                    luogo = Some(nome);
                }
                continue;
            }
            let pqid = field(b, "e").rsplit('/').next().unwrap_or("").to_string();
            let ruolo = {
                let r = field(b, "role");
                if r.is_empty() { "ALTRO".to_string() } else { r }
            };
            if pqid.is_empty() {
                continue;
            }
            if seen.insert((pqid.clone(), ruolo.clone())) {
                persone.push((nome, pqid, ruolo));
            }
        }
        (persone, luogo)
    }

    /// GET con retry/backoff su 429 e 5xx (Wikidata SPARQL throttla facilmente).
    async fn get_json_retry(
        &self,
        url: &str,
        params: &[(&str, &str)],
        sparql: bool,
    ) -> anyhow::Result<Value> {
        let mut attempt: u32 = 0;
        loop {
            let mut req = self.http.get(url).query(params);
            if sparql {
                req = req.header("Accept", "application/sparql-results+json");
            }
            match req.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        return Ok(resp.json::<Value>().await?);
                    }
                    if (status.as_u16() == 429 || status.is_server_error()) && attempt < 4 {
                        backoff(attempt).await;
                        attempt += 1;
                        continue;
                    }
                    anyhow::bail!("HTTP {} da {}", status, url);
                }
                Err(e) => {
                    if attempt < 4 {
                        backoff(attempt).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(e.into());
                }
            }
        }
    }
}

async fn backoff(attempt: u32) {
    let secs = 2f64.powi(attempt.min(6) as i32);
    tokio::time::sleep(Duration::from_secs_f64(secs.min(60.0))).await;
}

/// "Point(lon lat)" → (lat, lon).
fn parse_point(s: &str) -> Option<(f64, f64)> {
    let inner = s.strip_prefix("Point(")?.strip_suffix(')')?;
    let mut parts = inner.split_whitespace();
    let lon: f64 = parts.next()?.parse().ok()?;
    let lat: f64 = parts.next()?.parse().ok()?;
    Some((lat, lon))
}

/// Estrae il titolo articolo da un URL "https://it.wikipedia.org/wiki/Titolo".
fn article_title(url: &str) -> Option<String> {
    let seg = url.rsplit("/wiki/").next()?;
    if seg.is_empty() {
        return None;
    }
    Some(decode_title(seg))
}

/// Decodifica minimale (%XX) e sostituisce gli underscore con spazi.
fn decode_title(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(h) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(h);
                i += 3;
                continue;
            }
        }
        out.push(if bytes[i] == b'_' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
