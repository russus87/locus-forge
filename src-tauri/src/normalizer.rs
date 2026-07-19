use serde_json::Value;

use crate::model::{NormPersona, NormalizedCaso};
use crate::sources::RawItem;

/// Converte un RawItem Wikidata+Wikipedia nel modello canonico.
pub fn normalize(item: &RawItem) -> anyhow::Result<NormalizedCaso> {
    let j = &item.raw_json;
    let get_str = |k: &str| j.get(k).and_then(Value::as_str).map(|s| s.to_string());

    let titolo = get_str("label").unwrap_or_else(|| item.source_id.clone());
    let extract = get_str("extract");
    let date = get_str("date");
    let type_label = get_str("typeLabel").unwrap_or_default();

    let lat = j.get("lat").and_then(Value::as_f64);
    let lon = j.get("lon").and_then(Value::as_f64);

    let anno = date.as_deref().and_then(parse_year);
    let data_evento = date.as_deref().and_then(parse_iso_date);

    let categoria = classifica(&titolo, &type_label, extract.as_deref());
    let sommario = extract.as_deref().map(prima_frase);

    let persone = j
        .get("persone")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|p| {
                    let nome = p.get("nome").and_then(Value::as_str)?.to_string();
                    if nome.is_empty() {
                        return None;
                    }
                    let ps = |k: &str| p.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string);
                    let ruolo = p.get("ruolo").and_then(Value::as_str).unwrap_or("ALTRO").to_string();
                    let biografia = ps("biografia");
                    let descrizione = ps("descrizione").or_else(|| biografia.as_deref().map(prima_frase));
                    Some(NormPersona {
                        nome,
                        ruolo,
                        descrizione,
                        biografia,
                        immagine_url: ps("image"),
                        data_nascita: ps("dataNascita").as_deref().and_then(parse_iso_date),
                        data_morte: ps("dataMorte").as_deref().and_then(parse_iso_date),
                        luogo_nascita: ps("luogoNascita"),
                        occupazione: ps("occupazione"),
                        nazionalita: ps("nazionalita"),
                        wikidata_qid: ps("qid"),
                        wikipedia_url: ps("article"),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(NormalizedCaso {
        source: item.source.clone(),
        source_id: item.source_id.clone(),
        source_url: item.source_url.clone(),
        wikidata_qid: Some(item.source_id.clone()),
        wikipedia_url: get_str("article"),
        titolo,
        sommario,
        descrizione: extract,
        categoria,
        anno,
        data_evento,
        lat,
        lon,
        luogo_nome: get_str("luogo"),
        immagine_url: get_str("image"),
        persone,
    })
}

/// Euristica di categoria a partire da titolo, tipo Wikidata e testo.
fn classifica(titolo: &str, type_label: &str, extract: Option<&str>) -> String {
    let mut hay = format!("{} {}", titolo.to_lowercase(), type_label.to_lowercase());
    if let Some(e) = extract {
        hay.push(' ');
        hay.push_str(&e.to_lowercase());
    }
    let has = |k: &str| hay.contains(k);

    if has("mafia") || has("cosa nostra") || has("'ndrangheta") || has("ndrangheta") || has("camorra") {
        "MAFIA"
    } else if has("terror") || has("attentato") || type_label.to_lowercase().contains("attentato") {
        "TERRORISMO"
    } else if has("serial") || has("serie di omicidi") || has("mostro") {
        "SERIAL_KILLER"
    } else if has("rapina") {
        "RAPINA"
    } else if has("irrisolt") || has("cold case") || has("mistero") {
        "COLD_CASE"
    } else {
        "OMICIDIO"
    }
    .to_string()
}

/// Prima frase (o max ~300 caratteri) dell'estratto. UTF-8 safe.
fn prima_frase(testo: &str) -> String {
    let t = testo.trim();
    // Il primo ". " chiude la prima frase; l'indice cade su un '.' ASCII (boundary sicuro).
    if let Some(idx) = t.find(". ") {
        return t[..=idx].trim().to_string();
    }
    let troncato: String = t.chars().take(300).collect();
    if troncato.chars().count() < t.chars().count() {
        format!("{troncato}…")
    } else {
        troncato
    }
}

/// Estrae l'anno (prime 4 cifre) da una data ISO tipo "1992-05-23T00:00:00Z".
fn parse_year(date: &str) -> Option<i64> {
    let d = date.strip_prefix('+').unwrap_or(date);
    d.get(0..4).and_then(|y| y.parse::<i64>().ok())
}

/// Estrae la data "YYYY-MM-DD" da una data ISO, se valida.
fn parse_iso_date(date: &str) -> Option<String> {
    let d = date.strip_prefix('+').unwrap_or(date);
    let ymd = d.get(0..10)?;
    let ok = ymd.len() == 10
        && ymd.as_bytes()[4] == b'-'
        && ymd.as_bytes()[7] == b'-'
        && ymd[0..4].chars().all(|c| c.is_ascii_digit());
    if ok && !ymd.ends_with("-00-00") {
        Some(ymd.to_string())
    } else {
        None
    }
}
