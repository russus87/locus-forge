use serde::{Deserialize, Serialize};

/// Persona coinvolta in un caso (modello interno).
#[derive(Debug, Clone, Default)]
pub struct NormPersona {
    pub nome: String,
    pub ruolo: String,
    pub wikidata_qid: Option<String>,
    pub wikipedia_url: Option<String>,
}

/// Modello canonico interno (uso solo Rust: adapter → SQLite). Indipendente dalla fonte.
#[derive(Debug, Clone, Default)]
pub struct NormalizedCaso {
    pub source: String,
    pub source_id: String,
    pub source_url: Option<String>,
    pub wikidata_qid: Option<String>,
    pub wikipedia_url: Option<String>,
    pub titolo: String,
    pub sommario: Option<String>,
    pub descrizione: Option<String>,
    pub categoria: String,
    pub anno: Option<i64>,
    pub data_evento: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub luogo_nome: Option<String>,
    pub immagine_url: Option<String>,
    pub persone: Vec<NormPersona>,
}

/// Riga di caso per la UI (browse/stat) — serializzata verso l'IPC.
#[derive(Debug, Clone, Serialize)]
pub struct CasoRow {
    pub id: i64,
    pub titolo: String,
    pub categoria: String,
    pub anno: Option<i64>,
    pub wikidata_qid: Option<String>,
    pub wikipedia_url: Option<String>,
    pub sommario: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub immagine_url: Option<String>,
    pub published: bool,
}

/// Media curato di un caso, verso la UI dell'editor.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaRow {
    pub tipo: String,
    pub url: String,
    pub titolo: Option<String>,
    pub didascalia: Option<String>,
    pub ordine: i64,
}

/// Persona di un caso, verso la UI (sola lettura nell'editor v1).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaRow {
    pub nome: String,
    pub ruolo: String,
}

/// Scheda completa di un caso per l'editor del Forge.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CasoDettaglio {
    pub id: i64,
    pub titolo: String,
    pub sommario: Option<String>,
    /// Testo ORIGINALE dal crawler (immutabile lato editor).
    pub descrizione: Option<String>,
    /// Contenuto CURATO (HTML) — ciò che si edita e pubblica.
    pub contenuto_html: Option<String>,
    pub categoria: String,
    pub anno: Option<i64>,
    pub wikipedia_url: Option<String>,
    pub immagine_url: Option<String>,
    pub luogo_nome: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub published: bool,
    pub media: Vec<MediaRow>,
    pub persone: Vec<PersonaRow>,
}

/// Un media in ingresso dall'editor (senza id/ordine: l'ordine è quello di lista).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaEdit {
    pub tipo: String,
    pub url: String,
    pub titolo: Option<String>,
    pub didascalia: Option<String>,
}

/// Modifiche salvate dall'editor su un caso.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CasoEdit {
    pub titolo: String,
    pub sommario: Option<String>,
    pub categoria: String,
    pub anno: Option<i64>,
    pub contenuto_html: Option<String>,
    pub media: Vec<MediaEdit>,
}

/// Conteggio per categoria (per il pannello statistiche).
#[derive(Debug, Clone, Serialize)]
pub struct CategoriaCount {
    pub categoria: String,
    pub count: i64,
}

/// Statistiche del database locale.
#[derive(Debug, Clone, Default, Serialize)]
pub struct DbStats {
    pub totale: i64,
    pub con_coordinate: i64,
    pub pubblicati: i64,
    pub da_pubblicare: i64,
    pub per_categoria: Vec<CategoriaCount>,
}

/// Descrittore di una sorgente di crawl (per il menu a tendina in UI).
#[derive(Debug, Clone, Serialize)]
pub struct SourceInfo {
    pub code: String,
    pub label: String,
    pub description: String,
}

// ---- DTO verso il backend Spring (/api/ingest/casi/batch). camelCase. ----

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LuogoIn {
    pub nome: String,
    pub lat: f64,
    pub lon: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wikidata_qid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaIn {
    pub nome: String,
    pub ruolo: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wikidata_qid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wikipedia_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FonteIn {
    pub titolo: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub tipo: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub licenza: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaIn {
    pub tipo: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub titolo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub didascalia: Option<String>,
    pub ordine: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CasoIn {
    pub titolo: String,
    pub categoria: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sommario: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descrizione: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contenuto_html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lingua: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paese: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anno: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_evento: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wikidata_qid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wikipedia_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub immagine_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub immagine_licenza: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub luogo: Option<LuogoIn>,
    pub persone: Vec<PersonaIn>,
    pub fonti: Vec<FonteIn>,
    pub media: Vec<MediaIn>,
}
