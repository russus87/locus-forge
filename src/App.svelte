<script lang="ts">
  import CrawlPanel from "./lib/CrawlPanel.svelte";
  import BrowsePanel from "./lib/BrowsePanel.svelte";
  import PublishPanel from "./lib/PublishPanel.svelte";
  import SettingsPanel from "./lib/SettingsPanel.svelte";

  type Tab = "crawl" | "browse" | "publish" | "config";
  let tab = $state<Tab>("crawl");

  const tabs: { id: Tab; label: string }[] = [
    { id: "crawl", label: "Crawl" },
    { id: "browse", label: "Sfoglia" },
    { id: "publish", label: "Pubblica" },
    { id: "config", label: "Config" },
  ];
</script>

<header>
  <div class="brand">
    <span class="mark">◆</span>
    <div>
      <h1>Locus Forge</h1>
      <span class="sub">Crawler &amp; curation · Locus Criminis</span>
    </div>
  </div>
  <nav>
    {#each tabs as t}
      <button class="tab" class:active={tab === t.id} onclick={() => (tab = t.id)}>{t.label}</button>
    {/each}
  </nav>
</header>

<main>
  {#if tab === "crawl"}
    <CrawlPanel />
  {:else if tab === "browse"}
    <BrowsePanel />
  {:else if tab === "publish"}
    <PublishPanel />
  {:else}
    <SettingsPanel />
  {/if}
</main>

<style>
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 18px 28px;
    border-bottom: 1px solid var(--line);
    background: var(--surface);
  }
  .brand { display: flex; align-items: center; gap: 14px; }
  .mark { color: var(--accent); font-size: 26px; }
  h1 { margin: 0; font-size: 20px; }
  .sub { color: var(--ink-faint); font-size: 12px; letter-spacing: 0.4px; }
  nav { display: flex; gap: 6px; }
  .tab { background: transparent; color: var(--ink-muted); padding: 8px 16px; border-radius: 9px; }
  .tab.active { background: var(--surface-hi); color: var(--ink); }
  main { max-width: 900px; margin: 0 auto; padding: 28px; }
</style>
