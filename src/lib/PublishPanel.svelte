<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { api, categoriaMeta } from "./api";
  import type { DbStats, PublishStatus } from "./types";

  let stats = $state<DbStats | null>(null);
  let backend = $state("");
  let publishing = $state(false);
  let refreshing = $state(false);
  let lastRefresh = $state("");
  let error = $state<string | null>(null);
  let status = $state<PublishStatus>({ running: false, phase: "", percent: 0, sent: 0, total: 0, last_error: null });
  let unlisten: (() => void) | null = null;

  async function refresh() {
    refreshing = true;
    error = null;
    try {
      stats = await api.dbStats();
      lastRefresh = new Date().toLocaleTimeString();
    } catch (e) {
      error = String(e);
    } finally {
      refreshing = false;
    }
  }

  onMount(async () => {
    backend = await api.backendTarget();
    await refresh();
    status = await api.publishStatus();
    unlisten = await api.onPublishProgress(async (s) => {
      status = s;
      if (!s.running) await refresh();
    });
  });
  onDestroy(() => unlisten?.());

  async function publish() {
    publishing = true;
    error = null;
    try {
      await api.publishBatch();
    } catch (e) {
      error = String(e);
    } finally {
      publishing = false;
    }
  }
</script>

<div class="card">
  <h2>Pubblica sul backend</h2>
  <p class="hint">Invia i casi non ancora pubblicati a <code>{backend}</code> via <code>/api/ingest/casi/batch</code>.</p>

  {#if stats}
    <div class="stats">
      <div><b>{stats.totale}</b><span>totali</span></div>
      <div><b>{stats.con_coordinate}</b><span>con coordinate</span></div>
      <div><b>{stats.pubblicati}</b><span>pubblicati</span></div>
      <div class="hi"><b>{stats.da_pubblicare}</b><span>da pubblicare</span></div>
    </div>

    {#if stats.per_categoria.length}
      <div class="cats">
        {#each stats.per_categoria as pc}
          {@const m = categoriaMeta(pc.categoria)}
          <span class="cat" style="border-color:{m.color}">{m.emoji} {m.label} · {pc.count}</span>
        {/each}
      </div>
    {/if}
  {/if}

  <div class="actions">
    <button class="primary" onclick={publish} disabled={publishing || status.running || (stats?.da_pubblicare ?? 0) === 0}>
      {status.running ? "Pubblicazione in corso…" : "Pubblica ora"}
    </button>
    <button class="ghost" onclick={refresh} disabled={status.running || refreshing}>
      {refreshing ? "Aggiorno…" : "Aggiorna"}
    </button>
    {#if lastRefresh}
      <span class="refreshed">Aggiornato alle {lastRefresh}</span>
    {/if}
  </div>

  {#if status.running || status.percent > 0}
    <div class="progress">
      <div class="bar"><div class="fill" style="width:{status.percent}%"></div></div>
      <span class="pct">{status.phase} · {status.sent}/{status.total}</span>
    </div>
  {/if}

  {#if status.last_error}<p class="warn">Errore: {status.last_error}</p>{/if}
  {#if error}<p class="warn">{error}</p>{/if}
</div>

<style>
  h2 { margin: 0 0 4px; }
  .hint { color: var(--ink-muted); font-size: 14px; margin: 0 0 18px; }
  code { background: var(--surface-hi); padding: 1px 6px; border-radius: 5px; font-size: 12px; }
  .stats { display: flex; gap: 28px; margin-bottom: 16px; }
  .stats div { display: flex; flex-direction: column; }
  .stats b { font-size: 24px; }
  .stats span { color: var(--ink-faint); font-size: 12px; text-transform: uppercase; letter-spacing: 1px; }
  .stats .hi b { color: var(--accent); }
  .cats { display: flex; flex-wrap: wrap; gap: 8px; margin-bottom: 18px; }
  .cat { font-size: 12px; padding: 3px 9px; border: 1px solid; border-radius: 7px; }
  .actions { display: flex; gap: 10px; align-items: center; }
  .refreshed { color: var(--ok); font-size: 12px; font-weight: 600; }
  .progress { display: flex; align-items: center; gap: 12px; margin-top: 18px; }
  .bar { flex: 1; height: 8px; background: var(--surface-hi); border-radius: 4px; overflow: hidden; }
  .fill { height: 100%; background: var(--accent); transition: width 0.2s ease; }
  .pct { color: var(--ink-muted); font-size: 13px; }
  .warn { color: #e8907f; font-size: 13px; margin: 12px 0 0; }
</style>
