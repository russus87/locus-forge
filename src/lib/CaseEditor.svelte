<script lang="ts">
  import { onMount } from "svelte";
  import { api, CATEGORIE } from "./api";
  import type { CasoDettaglio, MediaEdit } from "./types";

  let { id, onClose }: { id: number; onClose: (changed: boolean) => void } = $props();

  let caso = $state<CasoDettaglio | null>(null);
  let loading = $state(true);
  let saving = $state(false);
  let error = $state<string | null>(null);
  let savedMsg = $state("");

  // Campi editabili
  let titolo = $state("");
  let sommario = $state("");
  let categoria = $state("OMICIDIO");
  let anno = $state<number | null>(null);
  let media = $state<MediaEdit[]>([]);

  // Editor HTML
  let editorEl: HTMLDivElement | undefined = $state();
  let savedRange: Range | null = null;
  let linkOpen = $state(false);
  let linkUrl = $state("");

  const TIPI = [
    { v: "YOUTUBE", l: "YouTube" },
    { v: "VIDEO", l: "Video (mp4)" },
    { v: "IMMAGINE", l: "Immagine" },
    { v: "EMBED", l: "Embed" },
  ];
  const categorie = Object.keys(CATEGORIE);

  onMount(async () => {
    try {
      caso = await api.getCaso(id);
      if (caso) {
        titolo = caso.titolo;
        sommario = caso.sommario ?? "";
        categoria = caso.categoria;
        anno = caso.anno;
        media = caso.media.map((m) => ({ tipo: m.tipo, url: m.url, titolo: m.titolo, didascalia: m.didascalia }));
        if (editorEl) editorEl.innerHTML = caso.contenutoHtml ?? "";
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  });

  // ---- editor rich-text (self-contained, contenteditable) ----
  function exec(cmd: string, val?: string) {
    editorEl?.focus();
    document.execCommand(cmd, false, val);
  }
  function block(tag: string) {
    exec("formatBlock", tag);
  }
  function saveSelection() {
    const sel = window.getSelection();
    if (sel && sel.rangeCount > 0) savedRange = sel.getRangeAt(0).cloneRange();
  }
  function openLink() {
    saveSelection();
    linkUrl = "";
    linkOpen = true;
  }
  function applyLink() {
    const url = linkUrl.trim();
    if (url && savedRange) {
      const sel = window.getSelection();
      sel?.removeAllRanges();
      sel?.addRange(savedRange);
      document.execCommand("createLink", false, url);
    }
    linkOpen = false;
  }
  function caricaOriginale() {
    if (!caso?.descrizione || !editorEl) return;
    const paragrafi = caso.descrizione
      .split(/\n{2,}/)
      .map((p) => `<p>${escapeHtml(p.trim())}</p>`)
      .join("");
    editorEl.innerHTML = paragrafi;
  }
  function escapeHtml(s: string): string {
    return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  }

  // ---- media ----
  function addMedia() {
    media = [...media, { tipo: "YOUTUBE", url: "", titolo: null, didascalia: null }];
  }
  function removeMedia(i: number) {
    media = media.filter((_, j) => j !== i);
  }
  function moveMedia(i: number, dir: -1 | 1) {
    const j = i + dir;
    if (j < 0 || j >= media.length) return;
    const copy = [...media];
    [copy[i], copy[j]] = [copy[j], copy[i]];
    media = copy;
  }

  async function save() {
    saving = true;
    error = null;
    savedMsg = "";
    try {
      const contenutoHtml = editorEl ? editorEl.innerHTML.trim() : "";
      await api.updateCaso(id, {
        titolo: titolo.trim(),
        sommario: sommario.trim() || null,
        categoria,
        anno,
        contenutoHtml: contenutoHtml || null,
        media: media.filter((m) => m.url.trim()),
      });
      savedMsg = "Salvato. Il caso è ora «da pubblicare».";
      caso = await api.getCaso(id);
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }

  async function ripristina() {
    if (!confirmReset) {
      confirmReset = true;
      return;
    }
    saving = true;
    try {
      await api.revertOriginal(id);
      if (editorEl) editorEl.innerHTML = "";
      confirmReset = false;
      savedMsg = "Contenuto curato azzerato: l'app mostrerà il testo originale.";
      caso = await api.getCaso(id);
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }
  let confirmReset = $state(false);
</script>

<div class="editor">
  <div class="topbar">
    <button class="ghost" onclick={() => onClose(false)}>← Torna all'elenco</button>
    <div class="right">
      {#if savedMsg}<span class="ok">{savedMsg}</span>{/if}
      <button class="primary" onclick={save} disabled={saving || loading}>{saving ? "Salvo…" : "Salva"}</button>
    </div>
  </div>

  {#if loading}
    <p class="muted">Carico…</p>
  {:else if !caso}
    <p class="muted">Caso non trovato.</p>
  {:else}
    <div class="grid">
      <div class="col main">
        <label class="field"><span class="lbl">Titolo</span>
          <input bind:value={titolo} /></label>

        <label class="field"><span class="lbl">Sommario</span>
          <textarea rows="2" bind:value={sommario} placeholder="Frase di sintesi mostrata nelle card"></textarea></label>

        <div class="lbl">Contenuto (HTML)</div>
        <div class="rte">
          <div class="toolbar">
            <button onmousedown={(e) => e.preventDefault()} onclick={() => exec("bold")} title="Grassetto"><b>B</b></button>
            <button onmousedown={(e) => e.preventDefault()} onclick={() => exec("italic")} title="Corsivo"><i>I</i></button>
            <span class="sep"></span>
            <button onmousedown={(e) => e.preventDefault()} onclick={() => block("h2")}>H2</button>
            <button onmousedown={(e) => e.preventDefault()} onclick={() => block("h3")}>H3</button>
            <button onmousedown={(e) => e.preventDefault()} onclick={() => block("p")}>P</button>
            <span class="sep"></span>
            <button onmousedown={(e) => e.preventDefault()} onclick={() => exec("insertUnorderedList")} title="Elenco">• Lista</button>
            <button onmousedown={(e) => e.preventDefault()} onclick={() => block("blockquote")} title="Citazione">❝</button>
            <button onmousedown={(e) => e.preventDefault()} onclick={openLink} title="Link">🔗</button>
            <span class="sep"></span>
            <button onmousedown={(e) => e.preventDefault()} onclick={() => exec("removeFormat")} title="Pulisci">✕</button>
          </div>
          {#if linkOpen}
            <div class="linkbar">
              <input placeholder="https://…" bind:value={linkUrl} onkeydown={(e) => e.key === "Enter" && applyLink()} />
              <button class="ghost" onclick={applyLink}>Applica</button>
              <button class="ghost" onclick={() => (linkOpen = false)}>Annulla</button>
            </div>
          {/if}
          <div class="area" bind:this={editorEl} contenteditable="true"></div>
        </div>
        <button class="ghost small" onclick={caricaOriginale}>↧ Carica il testo originale nell'editor</button>

        <div class="media">
          <div class="media-head">
            <div class="lbl">Media / embed</div>
            <button class="ghost small" onclick={addMedia}>+ Aggiungi media</button>
          </div>
          {#if media.length === 0}
            <p class="muted small">Nessun media. Aggiungi video YouTube, video diretti o immagini.</p>
          {/if}
          {#each media as m, i}
            <div class="media-row">
              <select bind:value={m.tipo}>
                {#each TIPI as t}<option value={t.v}>{t.l}</option>{/each}
              </select>
              <input class="url" placeholder="URL (es. https://youtu.be/…)" bind:value={m.url} />
              <input class="cap" placeholder="Didascalia" bind:value={m.didascalia} />
              <div class="mbtns">
                <button class="ghost tiny" onclick={() => moveMedia(i, -1)} disabled={i === 0}>↑</button>
                <button class="ghost tiny" onclick={() => moveMedia(i, 1)} disabled={i === media.length - 1}>↓</button>
                <button class="ghost tiny danger" onclick={() => removeMedia(i)}>✕</button>
              </div>
            </div>
          {/each}
        </div>
      </div>

      <div class="col side">
        <label class="field"><span class="lbl">Categoria</span>
          <select bind:value={categoria}>
            {#each categorie as c}<option value={c}>{CATEGORIE[c].emoji} {CATEGORIE[c].label}</option>{/each}
          </select></label>
        <label class="field"><span class="lbl">Anno</span>
          <input type="number" bind:value={anno} /></label>

        <div class="status">
          {#if caso.published}<span class="pub">● pubblicato</span>{:else}<span class="pend">● da pubblicare</span>{/if}
        </div>

        {#if caso.persone.length}
          <div class="side-block">
            <div class="lbl">Persone</div>
            <ul>{#each caso.persone as p}<li>{p.nome} <span class="ruolo">· {p.ruolo}</span></li>{/each}</ul>
          </div>
        {/if}

        {#if caso.descrizione}
          <div class="side-block">
            <div class="lbl">Testo originale (crawler)</div>
            <p class="orig">{caso.descrizione}</p>
          </div>
        {/if}

        <div class="side-block">
          <button class="ghost small danger" onclick={ripristina} disabled={saving}>
            {confirmReset ? "Conferma: azzera contenuto curato" : "Ripristina originale"}
          </button>
        </div>
      </div>
    </div>

    {#if error}<p class="warn">{error}</p>{/if}
  {/if}
</div>

<style>
  .topbar { display: flex; justify-content: space-between; align-items: center; margin-bottom: 18px; }
  .right { display: flex; align-items: center; gap: 12px; }
  .ok { color: var(--ok); font-size: 12px; font-weight: 600; }
  .grid { display: grid; grid-template-columns: 1fr 260px; gap: 24px; }
  .col.main { min-width: 0; }
  .field { display: block; margin-bottom: 14px; }
  .lbl { display: block; font-size: 12px; color: var(--ink-muted); margin-bottom: 6px; text-transform: uppercase; letter-spacing: 0.5px; }
  input, textarea, select {
    width: 100%; box-sizing: border-box; padding: 9px 11px; background: var(--surface-hi);
    color: var(--ink); border: 1px solid var(--line); border-radius: 8px; font-size: 14px; font-family: inherit;
  }
  input:focus, textarea:focus, select:focus { outline: none; border-color: var(--accent); }
  .rte { border: 1px solid var(--line); border-radius: 10px; overflow: hidden; background: var(--surface-hi); }
  .toolbar { display: flex; align-items: center; gap: 2px; padding: 6px 8px; border-bottom: 1px solid var(--line); flex-wrap: wrap; }
  .toolbar button { background: transparent; color: var(--ink-muted); border: none; padding: 5px 9px; border-radius: 6px; font-size: 13px; cursor: pointer; }
  .toolbar button:hover { background: var(--surface); color: var(--ink); }
  .sep { width: 1px; height: 18px; background: var(--line); margin: 0 4px; }
  .linkbar { display: flex; gap: 6px; padding: 6px 8px; border-bottom: 1px solid var(--line); }
  .linkbar input { flex: 1; }
  .area { min-height: 260px; padding: 14px 16px; font-size: 15px; line-height: 1.6; outline: none; }
  .area :global(h2) { font-size: 20px; margin: 14px 0 6px; }
  .area :global(h3) { font-size: 16px; margin: 12px 0 4px; }
  .area :global(blockquote) { border-left: 3px solid var(--accent); margin: 10px 0; padding: 2px 0 2px 14px; color: var(--ink-muted); }
  .area :global(a) { color: var(--accent); }
  .small { font-size: 12px; }
  .ghost.small { margin-top: 8px; }
  .media { margin-top: 22px; }
  .media-head { display: flex; justify-content: space-between; align-items: center; }
  .media-row { display: flex; gap: 8px; align-items: center; margin: 8px 0; }
  .media-row select { width: 130px; flex: none; }
  .media-row .url { flex: 2; }
  .media-row .cap { flex: 1; }
  .mbtns { display: flex; gap: 3px; flex: none; }
  .tiny { padding: 6px 8px; font-size: 12px; }
  .danger { color: #e8907f; }
  .status { margin: 4px 0 14px; }
  .pub { color: var(--ok); font-size: 12px; font-weight: 700; }
  .pend { color: var(--ink-faint); font-size: 12px; font-weight: 700; }
  .side-block { margin-top: 18px; }
  .side-block ul { list-style: none; margin: 0; padding: 0; font-size: 13px; }
  .side-block li { padding: 3px 0; }
  .ruolo { color: var(--ink-faint); }
  .orig { font-size: 12px; color: var(--ink-muted); line-height: 1.5; max-height: 220px; overflow: auto; white-space: pre-wrap; }
  .muted { color: var(--ink-muted); }
  .warn { color: #e8907f; font-size: 13px; margin-top: 12px; }
  @media (max-width: 720px) { .grid { grid-template-columns: 1fr; } }
</style>
