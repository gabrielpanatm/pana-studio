<script lang="ts">
  import {
    IconExternalLink,
    IconFile,
    IconPhoto,
    IconPhotoCheck,
    IconSearch,
  } from "@tabler/icons-svelte";
  import type { AppState } from "$lib/state/app.svelte";
  import type { SourceGraphAsset } from "$lib/types";

  let { app }: { app: AppState } = $props();

  type UsageFilter = "all" | "used" | "unused";
  type AssetFilter = "all" | "image" | "font" | "other";

  let query = $state("");
  let usageFilter = $state<UsageFilter>("all");
  let typeFilter = $state<AssetFilter>("all");
  let selectedAssetId = $state("");
  let applying = $state(false);

  const assets = $derived(app.sourceGraph?.assets ?? []);
  const normalizedQuery = $derived(query.trim().toLocaleLowerCase("ro"));
  const filteredAssets = $derived(
    assets.filter((asset) => {
      const usages = usageCount(asset);
      return (usageFilter === "all" || (usageFilter === "used" ? usages > 0 : usages === 0))
        && (typeFilter === "all" || assetKind(asset) === typeFilter)
        && (!normalizedQuery || `${asset.logicalPath} ${asset.file} ${asset.themeName ?? ""}`
          .toLocaleLowerCase("ro")
          .includes(normalizedQuery));
    }),
  );
  const selectedAsset = $derived(
    assets.find((asset) => asset.id === selectedAssetId) ?? filteredAssets[0] ?? null,
  );
  const unusedCount = $derived(assets.filter((asset) => usageCount(asset) === 0).length);
  const selectedImageTarget = $derived(app.selectedElement?.tag === "img" ? app.selectedElement : null);

  function usageCount(asset: SourceGraphAsset) {
    return (app.sourceGraph?.relations ?? []).filter((relation) => relation.to === asset.nodeId).length;
  }

  function extension(asset: SourceGraphAsset) {
    return asset.logicalPath.split(".").at(-1)?.toLocaleLowerCase("ro") ?? "";
  }

  function assetKind(asset: SourceGraphAsset): Exclude<AssetFilter, "all"> {
    const ext = extension(asset);
    if (["avif", "gif", "jpeg", "jpg", "png", "svg", "webp"].includes(ext)) return "image";
    if (["otf", "ttf", "woff", "woff2"].includes(ext)) return "font";
    return "other";
  }

  function assetUrl(asset: SourceGraphAsset) {
    const base = app.scannedProject?.previewBaseUrl;
    if (!base) return "";
    try {
      return new URL(`/${asset.logicalPath.replace(/^\/+/, "")}`, base).href;
    } catch {
      return "";
    }
  }

  function sourceValue(asset: SourceGraphAsset) {
    return `/${asset.logicalPath.replace(/^\/+/, "")}`;
  }

  async function applyToSelectedImage(asset: SourceGraphAsset) {
    if (!selectedImageTarget || assetKind(asset) !== "image" || applying) return;
    applying = true;
    try {
      const outcome = await app.applyImageSourceToHtml(sourceValue(asset));
      if (outcome.status === "committed" || outcome.status === "noop") {
        await app.setWorkbenchActivity("editor");
      }
    } finally {
      applying = false;
    }
  }
</script>

<section class="assets-workspace" aria-labelledby="assets-title">
  <header class="workspace-header">
    <div>
      <span class="eyebrow"><IconPhoto size={15} stroke={1.9} /> Resource workspace</span>
      <h1 id="assets-title">Resurse</h1>
      <p>Inventarul și utilizările vin din harta surselor; aplicarea pe un element selectat trece prin mutația HTML Rust.</p>
    </div>
    <dl>
      <div><dt>Total</dt><dd>{assets.length}</dd></div>
      <div><dt>Utilizate</dt><dd>{assets.length - unusedCount}</dd></div>
      <div class:warning={unusedCount > 0}><dt>Nefolosite</dt><dd>{unusedCount}</dd></div>
    </dl>
  </header>

  <div class="workspace-toolbar">
    <label class="search-field">
      <span class="sr-only">Caută asset-uri</span>
      <IconSearch size={14} stroke={1.9} />
      <input bind:value={query} type="search" placeholder="Caută după nume sau cale" />
    </label>
    <label>
      <span class="sr-only">Tip asset</span>
      <select bind:value={typeFilter} aria-label="Tip asset">
        <option value="all">Toate tipurile</option>
        <option value="image">Imagini</option>
        <option value="font">Fonturi</option>
        <option value="other">Altele</option>
      </select>
    </label>
    <label>
      <span class="sr-only">Filtru utilizare</span>
      <select bind:value={usageFilter} aria-label="Filtru utilizare">
        <option value="all">Toate utilizările</option>
        <option value="used">Utilizate</option>
        <option value="unused">Nefolosite</option>
      </select>
    </label>
  </div>

  <div class="workspace-body">
    <div class="asset-grid" aria-label="Bibliotecă asset-uri">
      {#each filteredAssets as asset (asset.id)}
        <button
          type="button"
          class="asset-card"
          class:selected={selectedAsset?.id === asset.id}
          onclick={() => { selectedAssetId = asset.id; }}
        >
          <span class="asset-preview">
            {#if assetKind(asset) === "image" && assetUrl(asset)}
              <img src={assetUrl(asset)} alt="" />
            {:else}
              <IconFile size={25} stroke={1.5} />
            {/if}
          </span>
          <span class="asset-copy">
            <strong>{asset.logicalPath.split("/").at(-1)}</strong>
            <small>{asset.logicalPath}</small>
          </span>
          <span class:unused={usageCount(asset) === 0} class="usage-badge">
            {usageCount(asset) === 0 ? "Nefolosit" : `${usageCount(asset)} utilizări`}
          </span>
        </button>
      {:else}
        <div class="workspace-state">
          <IconPhoto size={28} stroke={1.5} />
          <strong>{assets.length === 0 ? "Proiectul nu conține asset-uri statice" : "Niciun rezultat"}</strong>
          <span>Harta surselor indexează automat resursele locale și cele provenite din tema activă.</span>
        </div>
      {/each}
    </div>

    <aside class="asset-detail" aria-label="Detalii asset">
      {#if selectedAsset}
        <span class="detail-kicker">{assetKind(selectedAsset)} · {selectedAsset.origin}</span>
        <h2>{selectedAsset.logicalPath.split("/").at(-1)}</h2>
        {#if assetKind(selectedAsset) === "image" && assetUrl(selectedAsset)}
          <div class="detail-preview"><img src={assetUrl(selectedAsset)} alt={`Previzualizare ${selectedAsset.logicalPath}`} /></div>
        {/if}
        <dl class="asset-metadata">
          <div><dt>Cale publică</dt><dd>{sourceValue(selectedAsset)}</dd></div>
          <div><dt>Sursă</dt><dd>{selectedAsset.file}</dd></div>
          <div><dt>Format</dt><dd>{extension(selectedAsset).toUpperCase() || "—"}</dd></div>
          <div><dt>Utilizări</dt><dd>{usageCount(selectedAsset)}</dd></div>
        </dl>
        <div class="target-card">
          <strong>{selectedImageTarget ? "Imagine selectată în Editor" : "Nicio imagine selectată"}</strong>
          <span>{selectedImageTarget?.sourceLocation?.file ?? "Selectează un <img> pentru replacement controlat."}</span>
        </div>
        <button
          class="primary-action"
          type="button"
          disabled={!selectedImageTarget || assetKind(selectedAsset) !== "image" || applying}
          onclick={() => { void applyToSelectedImage(selectedAsset); }}
        >
          <IconPhotoCheck size={15} stroke={1.9} />
          {applying ? "Se aplică prin Rust…" : "Aplică pe imaginea selectată"}
        </button>
        <button class="secondary-action" type="button" onclick={() => { void app.openCurrentProjectInBrowser(sourceValue(selectedAsset)); }}>
          Deschide asset-ul <IconExternalLink size={13} stroke={1.9} />
        </button>
      {:else}
        <div class="workspace-state">Selectează un asset pentru detalii și utilizări.</div>
      {/if}
    </aside>
  </div>
</section>

<style>
  .assets-workspace { display: grid; grid-template-rows: auto 42px minmax(0, 1fr); min-width: 0; min-height: 0; height: 100%; overflow: hidden; border: 1px solid var(--wb-border-subtle); border-radius: 10px; color: var(--wb-text-primary); background: var(--wb-surface-document); box-shadow: var(--shadow); }
  .workspace-header { display: flex; align-items: center; justify-content: space-between; gap: 24px; padding: 17px 20px; border-bottom: 1px solid var(--wb-border-subtle); background: radial-gradient(circle at 18% 0%, var(--wb-accent-soft), transparent 36%), var(--wb-surface-chrome); }
  .eyebrow { display: inline-flex; align-items: center; gap: 6px; color: var(--wb-accent-strong); font-size: 12px; font-weight: 850; letter-spacing: .06em; text-transform: uppercase; }
  h1 { margin: 6px 0 0; color: var(--text-strong); font-size: 24px; letter-spacing: -.025em; }
  .workspace-header p { margin: 5px 0 0; color: var(--wb-text-muted); font-size: 12px; }
  .workspace-header > dl { display: flex; gap: 7px; margin: 0; }
  .workspace-header > dl div { min-width: 78px; padding: 7px 9px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .workspace-header > dl div.warning { border-color: color-mix(in srgb, var(--wb-warning) 45%, var(--wb-border-subtle)); }
  dt { color: var(--wb-text-muted); font-size: 12px; font-weight: 800; text-transform: uppercase; }
  dd { margin: 3px 0 0; color: var(--text-strong); font-size: 15px; font-weight: 800; }
  .workspace-toolbar, .search-field, .primary-action, .secondary-action { display: flex; align-items: center; }
  .workspace-toolbar { justify-content: flex-end; gap: 7px; padding: 6px 9px; border-bottom: 1px solid var(--wb-border-subtle); background: var(--wb-surface-chrome); }
  .search-field { position: relative; flex: 1; max-width: 360px; margin-right: auto; }
  .search-field :global(svg) { position: absolute; left: 8px; color: var(--wb-text-muted); }
  .search-field input, .workspace-toolbar select { height: 28px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; }
  .search-field input { width: 100%; padding: 0 8px 0 28px; }
  .workspace-toolbar select { min-width: 126px; padding: 0 7px; }
  .workspace-body { display: grid; grid-template-columns: minmax(360px, 1fr) minmax(280px, .52fr); min-width: 0; min-height: 0; }
  .asset-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); align-content: start; gap: 8px; min-width: 0; min-height: 0; padding: 9px; overflow: auto; border-right: 1px solid var(--wb-border-subtle); }
  .asset-card { display: grid; grid-template-rows: 98px auto auto; min-width: 0; padding: 0; overflow: hidden; border: 1px solid var(--wb-border-subtle); border-radius: 8px; color: var(--wb-text-primary); background: var(--wb-surface-chrome); text-align: left; }
  .asset-card:hover, .asset-card.selected { border-color: color-mix(in srgb, var(--wb-accent) 55%, var(--wb-border-subtle)); }
  .asset-card.selected { box-shadow: inset 0 0 0 1px var(--wb-accent); }
  .asset-preview { display: grid; min-width: 0; overflow: hidden; place-items: center; color: var(--wb-text-muted); background: var(--surface-7); }
  .asset-preview img { width: 100%; height: 100%; object-fit: contain; }
  .asset-copy { display: grid; gap: 3px; min-width: 0; padding: 8px 8px 4px; }
  .asset-copy strong { overflow: hidden; color: var(--text-strong); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .asset-copy small { overflow: hidden; color: var(--wb-text-muted); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .usage-badge { justify-self: start; margin: 2px 8px 8px; padding: 2px 5px; border-radius: 999px; color: var(--success); background: color-mix(in srgb, var(--success) 10%, var(--wb-surface-document)); font-size: 12px; font-weight: 800; }
  .usage-badge.unused { color: var(--wb-warning); background: color-mix(in srgb, var(--wb-warning) 10%, var(--wb-surface-document)); }
  .asset-detail { min-width: 0; min-height: 0; padding: 17px; overflow: auto; background: var(--wb-surface-chrome); }
  .detail-kicker { color: var(--wb-accent-strong); font-size: 12px; font-weight: 850; text-transform: uppercase; }
  h2 { margin: 7px 0 0; overflow-wrap: anywhere; color: var(--text-strong); font-size: 19px; }
  .detail-preview { display: grid; height: 180px; margin-top: 12px; overflow: hidden; place-items: center; border: 1px solid var(--wb-border-subtle); border-radius: 8px; background: var(--surface-7); }
  .detail-preview img { width: 100%; height: 100%; object-fit: contain; }
  .asset-metadata { display: grid; gap: 6px; margin: 11px 0 0; }
  .asset-metadata div { display: grid; gap: 3px; padding: 8px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  .asset-metadata dd { overflow-wrap: anywhere; font-size: 12px; }
  .target-card { display: grid; gap: 3px; margin-top: 9px; padding: 9px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  .target-card strong { color: var(--text-strong); font-size: 12px; }
  .target-card span { overflow: hidden; color: var(--wb-text-muted); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .primary-action, .secondary-action { justify-content: center; gap: 6px; width: 100%; min-height: 32px; margin-top: 8px; border: 1px solid var(--wb-accent); border-radius: 6px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); font-size: 12px; font-weight: 800; }
  .secondary-action { border-color: var(--wb-border-subtle); color: var(--wb-text-primary); background: var(--wb-surface-document); }
  button:disabled { opacity: .5; }
  button:not(:disabled) { cursor: pointer; }
  button:focus-visible, input:focus-visible, select:focus-visible { outline: 2px solid var(--wb-focus-ring); outline-offset: 1px; }
  .workspace-state { display: grid; grid-column: 1 / -1; min-height: 220px; place-items: center; align-content: center; gap: 6px; color: var(--wb-text-muted); font-size: 12px; text-align: center; }
  .workspace-state strong { color: var(--text-strong); font-size: 12px; }
  .workspace-state span { max-width: 360px; }
  .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; }
  @media (max-width: 900px) { .workspace-body { grid-template-columns: 1fr; } .asset-detail { display: none; } .asset-grid { border-right: 0; } }
</style>
