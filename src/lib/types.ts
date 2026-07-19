export interface CrawlStatus {
  running: boolean;
  source: string | null;
  processed: number;
  total: number;
  inserted: number;
  updated: number;
  skipped: number;
  errors: number;
  last_error: string | null;
  cancelled: boolean;
}

export interface PublishStatus {
  running: boolean;
  phase: string;
  percent: number;
  sent: number;
  total: number;
  last_error: string | null;
}

export interface CasoRow {
  id: number;
  titolo: string;
  categoria: string;
  anno: number | null;
  wikidata_qid: string | null;
  wikipedia_url: string | null;
  sommario: string | null;
  lat: number | null;
  lon: number | null;
  immagine_url: string | null;
  published: boolean;
}

export interface CategoriaCount {
  categoria: string;
  count: number;
}

export interface DbStats {
  totale: number;
  con_coordinate: number;
  pubblicati: number;
  da_pubblicare: number;
  per_categoria: CategoriaCount[];
}

export interface SourceInfo {
  code: string;
  label: string;
  description: string;
}

export interface Settings {
  backend_url: string;
  ingest_key: string;
}
