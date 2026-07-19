<script lang="ts">
  import { onMount } from "svelte";
  import { api, categoriaMeta } from "./api";
  import type { CasoRow } from "./types";
  import CaseEditor from "./CaseEditor.svelte";

  let query = $state("");
  let casi = $state<CasoRow[]>([]);
  let loading = $state(false);
  let editId = $state<number | null>(null);

  async function load() {
    loading = true;
    try {
      casi = await api.listCasi(query);
    } finally {
      loading = false;
    }
  }

  function onEditorClose() {
    editId = null;
    load(); // rinfresca stato pubblicato/da-pubblicare
  }

  onMount(load);
</script>

{#if editId !== null}
  <CaseEditor id={editId} onClose={onEditorClose} />
{:else}

<div class="card">
  <div class="head">
    <h2>Casi nel database locale</h2>
    <div class="search">
      <input placeholder="Filtra per titolo…" bind:value={query} onkeydown={(e) => e.key === "Enter" && load()} />
      <button class="ghost" onclick={load}>Cerca</button>
    </div>
  </div>

  {#if loading}
    <p class="muted">Caricamento…</p>
  {:else if casi.length === 0}
    <p class="muted">Nessun caso. Avvia un crawl per popolare il database.</p>
  {:else}
    <ul class="list">
      {#each casi as c}
        {@const m = categoriaMeta(c.categoria)}
        <li>
          <button class="rowbtn" onclick={() => (editId = c.id)}>
          <span class="dot" style="background:{m.color}"></span>
          <div class="info">
            <div class="title-row">
              <span class="title">{c.titolo}</span>
              {#if c.anno}<span class="year">{c.anno}</span>{/if}
            </div>
            {#if c.sommario}<span class="summary">{c.sommario}</span>{/if}
          </div>
          <div class="meta">
            <span class="cat" style="border-color:{m.color}">{m.emoji} {m.label}</span>
            {#if c.published}
              <span class="pub">pubblicato</span>
            {:else}
              <span class="pending">locale</span>
            {/if}
          </div>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>
{/if}

<style>
  .head { display: flex; justify-content: space-between; align-items: center; gap: 16px; margin-bottom: 16px; flex-wrap: wrap; }
  h2 { margin: 0; }
  .search { display: flex; gap: 8px; }
  .search input { width: 240px; }
  .muted { color: var(--ink-muted); }
  .list { list-style: none; margin: 0; padding: 0; }
  li { border-top: 1px solid var(--line); }
  .rowbtn { display: flex; align-items: center; gap: 12px; width: 100%; text-align: left; padding: 12px 10px; margin: 0 -10px; background: transparent; border: none; cursor: pointer; border-radius: 8px; color: inherit; font: inherit; }
  .rowbtn:hover { background: var(--surface-hi); }
  .dot { width: 10px; height: 10px; border-radius: 50%; flex: none; }
  .info { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .title-row { display: flex; align-items: baseline; gap: 10px; }
  .title { font-weight: 600; }
  .year { color: var(--ink-faint); font-size: 13px; }
  .summary { color: var(--ink-muted); font-size: 13px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .meta { display: flex; align-items: center; gap: 10px; flex: none; }
  .cat { font-size: 11px; font-weight: 700; padding: 3px 8px; border: 1px solid; border-radius: 6px; text-transform: uppercase; letter-spacing: 0.5px; }
  .pub { font-size: 11px; color: var(--ok); font-weight: 700; }
  .pending { font-size: 11px; color: var(--ink-faint); font-weight: 700; }
</style>
