<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "./api";
  import type { Settings } from "./types";

  let backendUrl = $state("");
  let ingestKey = $state("");
  let showKey = $state(false);
  let loading = $state(true);
  let saving = $state(false);
  let savedAt = $state("");
  let error = $state<string | null>(null);

  onMount(async () => {
    try {
      const s = await api.getSettings();
      backendUrl = s.backend_url;
      ingestKey = s.ingest_key;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  });

  const isRemote = $derived(/^https:\/\//i.test(backendUrl.trim()));

  async function save() {
    saving = true;
    error = null;
    try {
      const settings: Settings = { backend_url: backendUrl.trim(), ingest_key: ingestKey.trim() };
      await api.saveSettings(settings);
      // Rileggo il valore effettivo risolto (setting → env → default).
      const s = await api.getSettings();
      backendUrl = s.backend_url;
      ingestKey = s.ingest_key;
      savedAt = new Date().toLocaleTimeString();
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }

  function usaProduzione() {
    backendUrl = "https://locuscriminis.russus.it";
  }
  function usaLocale() {
    backendUrl = "http://localhost:8790";
  }
</script>

<div class="card">
  <h2>Configurazione</h2>
  <p class="hint">
    Destinazione e chiave usate da <b>Pubblica</b>. Salvate localmente: cambiarle
    <b>non</b> richiede un rebuild. Precedenza: questo valore → variabile d'ambiente → default.
  </p>

  {#if loading}
    <p class="muted">Carico…</p>
  {:else}
    <label class="field">
      <span class="lbl">Backend URL</span>
      <input type="text" bind:value={backendUrl} placeholder="https://locuscriminis.russus.it" spellcheck="false" autocapitalize="off" />
    </label>

    <div class="presets">
      <button class="chip" onclick={usaProduzione}>Produzione</button>
      <button class="chip" onclick={usaLocale}>Locale</button>
      <span class="target" class:remote={isRemote}>
        {isRemote ? "● remoto (produzione)" : "● locale"}
      </span>
    </div>

    <label class="field">
      <span class="lbl">Chiave di ingest <span class="opt">(X-Ingest-Key — obbligatoria in produzione)</span></span>
      <div class="keyrow">
        {#if showKey}
          <input type="text" bind:value={ingestKey} placeholder="obbligatoria solo per il backend di produzione" spellcheck="false" autocapitalize="off" />
        {:else}
          <input type="password" bind:value={ingestKey} placeholder="obbligatoria solo per il backend di produzione" spellcheck="false" autocapitalize="off" />
        {/if}
        <button class="ghost" onclick={() => (showKey = !showKey)}>{showKey ? "Nascondi" : "Mostra"}</button>
      </div>
    </label>

    {#if isRemote && !ingestKey}
      <p class="warn">Backend di produzione senza chiave: la pubblicazione verrà rifiutata (401).</p>
    {/if}

    <div class="actions">
      <button class="primary" onclick={save} disabled={saving}>{saving ? "Salvo…" : "Salva"}</button>
      {#if savedAt}<span class="ok">Salvato alle {savedAt}</span>{/if}
    </div>

    {#if error}<p class="warn">{error}</p>{/if}
  {/if}
</div>

<style>
  h2 { margin: 0 0 4px; }
  .hint { color: var(--ink-muted); font-size: 14px; margin: 0 0 22px; }
  .muted { color: var(--ink-faint); }
  .field { display: block; margin-bottom: 18px; }
  .lbl { display: block; font-size: 13px; color: var(--ink-muted); margin-bottom: 6px; }
  .opt { color: var(--ink-faint); font-weight: 400; }
  input {
    width: 100%; box-sizing: border-box; padding: 10px 12px;
    background: var(--surface-hi); color: var(--ink);
    border: 1px solid var(--line); border-radius: 9px; font-size: 14px;
  }
  input:focus { outline: none; border-color: var(--accent); }
  .keyrow { display: flex; gap: 8px; }
  .keyrow input { flex: 1; }
  .presets { display: flex; align-items: center; gap: 8px; margin: -8px 0 20px; }
  .chip { font-size: 12px; padding: 5px 12px; border-radius: 7px; background: var(--surface-hi); color: var(--ink-muted); border: 1px solid var(--line); }
  .chip:hover { color: var(--ink); }
  .target { font-size: 12px; color: var(--ink-faint); margin-left: auto; }
  .target.remote { color: var(--accent); }
  .actions { display: flex; gap: 12px; align-items: center; margin-top: 6px; }
  .ok { color: var(--ok); font-size: 12px; font-weight: 600; }
  .warn { color: #e8907f; font-size: 13px; margin: 4px 0 0; }
</style>
