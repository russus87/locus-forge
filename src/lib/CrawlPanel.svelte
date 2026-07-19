<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { api } from "./api";
  import type { CrawlStatus, SourceInfo } from "./types";

  let sources = $state<SourceInfo[]>([]);
  let selected = $state("WIKIDATA");
  let limit = $state(50);
  let starting = $state(false);
  let startError = $state<string | null>(null);
  let status = $state<CrawlStatus>({
    running: false, source: null, processed: 0, total: 0,
    inserted: 0, updated: 0, skipped: 0, errors: 0, last_error: null, cancelled: false,
  });
  let unlisten: (() => void) | null = null;

  const pct = $derived(status.total > 0 ? Math.round((status.processed / status.total) * 100) : 0);
  const descr = $derived(sources.find((s) => s.code === selected)?.description ?? "");

  onMount(async () => {
    sources = await api.listSources();
    if (sources.length) selected = sources[0].code;
    status = await api.crawlStatus();
    unlisten = await api.onCrawlProgress((s) => (status = s));
  });
  onDestroy(() => unlisten?.());

  async function start() {
    starting = true;
    startError = null;
    try {
      await api.startCrawl(selected, limit);
    } catch (e) {
      startError = String(e);
    } finally {
      starting = false;
    }
  }

  async function stop() {
    try {
      await api.stopTask();
    } catch (e) {
      startError = String(e);
    }
  }
</script>

<div class="card">
  <h2>Crawl</h2>
  <p class="hint">Pesca i casi da una sorgente aperta e li salva nel database locale del Forge.</p>

  <div class="row">
    <div class="field">
      <span class="label">Sorgente</span>
      <select bind:value={selected} disabled={status.running}>
        {#each sources as s}
          <option value={s.code}>{s.label}</option>
        {/each}
      </select>
    </div>
    <div class="field limit">
      <span class="label">Limite</span>
      <input type="number" min="1" max="500" bind:value={limit} disabled={status.running} />
    </div>
    <div class="actions">
      {#if status.running}
        <button class="ghost" onclick={stop}>Interrompi</button>
      {:else}
        <button class="primary" onclick={start} disabled={starting}>Avvia crawl</button>
      {/if}
    </div>
  </div>

  {#if descr}
    <p class="descr">{descr}</p>
  {/if}

  {#if status.running || status.processed > 0}
    <div class="progress">
      <div class="bar"><div class="fill" style="width:{pct}%"></div></div>
      <span class="pct">{status.processed}/{status.total} · {pct}%</span>
    </div>
    <div class="counts">
      <div><b>{status.inserted}</b><span>nuovi</span></div>
      <div><b>{status.updated}</b><span>aggiornati</span></div>
      <div><b>{status.skipped}</b><span>saltati</span></div>
      <div class:err={status.errors > 0}><b>{status.errors}</b><span>errori</span></div>
    </div>
  {/if}

  {#if status.cancelled}
    <p class="warn">Crawl interrotto.</p>
  {/if}
  {#if status.last_error}
    <p class="warn">Ultimo errore: {status.last_error}</p>
  {/if}
  {#if startError}
    <p class="warn">{startError}</p>
  {/if}
</div>

<style>
  h2 { margin: 0 0 4px; }
  .hint { color: var(--ink-muted); margin: 0 0 18px; font-size: 14px; }
  .row { display: flex; gap: 14px; align-items: flex-end; flex-wrap: wrap; }
  .field { display: flex; flex-direction: column; gap: 6px; }
  .field select { min-width: 260px; }
  .limit input { width: 90px; }
  .actions { margin-left: auto; }
  .descr { color: var(--ink-faint); font-size: 13px; margin: 14px 0 0; }
  .progress { display: flex; align-items: center; gap: 12px; margin-top: 20px; }
  .bar { flex: 1; height: 8px; background: var(--surface-hi); border-radius: 4px; overflow: hidden; }
  .fill { height: 100%; background: var(--accent); transition: width 0.2s ease; }
  .pct { color: var(--ink-muted); font-size: 13px; font-variant-numeric: tabular-nums; }
  .counts { display: flex; gap: 24px; margin-top: 16px; }
  .counts div { display: flex; flex-direction: column; }
  .counts b { font-size: 22px; }
  .counts span { color: var(--ink-faint); font-size: 12px; text-transform: uppercase; letter-spacing: 1px; }
  .counts .err b { color: var(--accent); }
  .warn { color: #e8907f; font-size: 13px; margin: 12px 0 0; }
</style>
