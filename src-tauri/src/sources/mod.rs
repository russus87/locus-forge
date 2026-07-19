pub mod wikidata;

use crate::model::SourceInfo;

/// Item grezzo restituito da una sorgente: identità + payload JSON non tipizzato.
#[derive(Debug, Clone)]
pub struct RawItem {
    pub source: String,
    pub source_id: String,
    pub source_url: Option<String>,
    pub raw_json: serde_json::Value,
}

/// Sorgenti disponibili per il crawl (mostrate nel menu della UI).
pub fn available_sources() -> Vec<SourceInfo> {
    vec![SourceInfo {
        code: "WIKIDATA".to_string(),
        label: "Wikidata + Wikipedia (Italia)".to_string(),
        description:
            "Casi di cronaca nera italiani da Wikidata (SPARQL), arricchiti con l'estratto e \
             l'immagine dalla Wikipedia in italiano."
                .to_string(),
    }]
}
