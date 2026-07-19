import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { CasoDettaglio, CasoEdit, CasoRow, CrawlStatus, DbStats, PublishStatus, Settings, SourceInfo } from "./types";

/// Wrapper sottile 1:1 sui comandi Tauri (invoke) e sugli eventi (listen).
export const api = {
  listSources: () => invoke<SourceInfo[]>("list_sources"),
  backendTarget: () => invoke<string>("backend_target"),

  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),

  startCrawl: (source: string, limit: number) =>
    invoke<void>("start_crawl", { source, limit }),
  crawlStatus: () => invoke<CrawlStatus>("crawl_status"),
  stopTask: () => invoke<void>("stop_task"),
  onCrawlProgress: (cb: (s: CrawlStatus) => void) =>
    listen<CrawlStatus>("crawl-progress", (e) => cb(e.payload)),

  listCasi: (query: string) => invoke<CasoRow[]>("list_casi", { query }),
  dbStats: () => invoke<DbStats>("db_stats"),

  getCaso: (id: number) => invoke<CasoDettaglio | null>("get_caso", { id }),
  updateCaso: (id: number, edit: CasoEdit) => invoke<void>("update_caso", { id, edit }),
  revertOriginal: (id: number) => invoke<void>("revert_original", { id }),

  publishBatch: () => invoke<void>("publish_batch"),
  resetPublished: () => invoke<number>("reset_published"),
  publishStatus: () => invoke<PublishStatus>("publish_status"),
  onPublishProgress: (cb: (s: PublishStatus) => void) =>
    listen<PublishStatus>("publish-progress", (e) => cb(e.payload)),
};

/// Metadati locali delle categorie (etichetta + colore) per la UI del Forge.
export const CATEGORIE: Record<string, { label: string; color: string; emoji: string }> = {
  OMICIDIO: { label: "Omicidio", color: "#E53935", emoji: "🔴" },
  COLD_CASE: { label: "Cold case", color: "#8E24AA", emoji: "🟣" },
  SERIAL_KILLER: { label: "Serial killer", color: "#BDBDBD", emoji: "⚫" },
  MAFIA: { label: "Mafia", color: "#1E88E5", emoji: "🔵" },
  TERRORISMO: { label: "Terrorismo", color: "#FB8C00", emoji: "🟠" },
  RAPINA: { label: "Rapina", color: "#43A047", emoji: "🟢" },
};

export function categoriaMeta(nome: string) {
  return CATEGORIE[nome] ?? { label: nome, color: "#9aa1b0", emoji: "•" };
}
