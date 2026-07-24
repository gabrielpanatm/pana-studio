<script lang="ts">
  import {
    IconAlertTriangle,
    IconEdit,
    IconExternalLink,
    IconFile,
    IconFolderOpen,
    IconPhoto,
    IconPhotoCheck,
    IconPlus,
    IconSearch,
    IconUpload,
    IconX,
  } from "@tabler/icons-svelte";
  import {
    chooseAssetFile,
    importProjectAsset,
  } from "$lib/project/io";
  import type { AppState } from "$lib/state/app.svelte";
  import type { SourceGraphAsset } from "$lib/types";
  import { errorMessage } from "$lib/util";

  let { app }: { app: AppState } = $props();

  type UsageFilter = "all" | "used" | "unused";
  type AssetView = "all" | "images" | "fonts" | "other";
  type AssetKind = Exclude<AssetView, "all">;
  type DetailMode = "info" | "create" | "edit";

  const assetViews: { id: AssetView; label: string }[] = [
    { id: "all", label: "Toate" },
    { id: "images", label: "Imagini" },
    { id: "fonts", label: "Fonturi" },
    { id: "other", label: "Altele" },
  ];

  let activeView = $state<AssetView>("all");
  let detailMode = $state<DetailMode>("info");
  let query = $state("");
  let usageFilter = $state<UsageFilter>("all");
  let selectedAssetId = $state("");
  let applying = $state(false);
  let importing = $state(false);
  let formError = $state("");
  let sourcePath = $state("");
  let fileName = $state("");
  let destinationDirectory = $state("static/images");

  const graphAssets = $derived(app.sourceGraph?.assets ?? []);
  const assets = $derived.by(() => {
    const existing = new Set(graphAssets.map((asset) => asset.file));
    const staged = (app.projectWorkspaceSnapshot?.stagedBinaryResources ?? [])
      .filter((path) => path.startsWith("static/") && !existing.has(path))
      .map((path): SourceGraphAsset => ({
        id: `staged:${path}`,
        file: path,
        origin: "local",
        themeName: null,
        logicalPath: path.replace(/^static\/?/, ""),
        nodeId: `staged-asset:${path}`,
      }));
    return [...graphAssets, ...staged];
  });
  const normalizedQuery = $derived(query.trim().toLocaleLowerCase("ro"));
  const filteredAssets = $derived(
    assets.filter((asset) => {
      const usages = usageCount(asset);
      return (usageFilter === "all" || (usageFilter === "used" ? usages > 0 : usages === 0))
        && (activeView === "all" || assetKind(asset) === activeView)
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

  function fileExtension(value: string) {
    return value.split(".").at(-1)?.toLocaleLowerCase("ro") ?? "";
  }

  function extension(asset: SourceGraphAsset) {
    return fileExtension(asset.logicalPath);
  }

  function kindFromExtension(ext: string): AssetKind {
    if (["avif", "gif", "jpeg", "jpg", "png", "svg", "webp"].includes(ext)) return "images";
    if (["otf", "ttf", "woff", "woff2"].includes(ext)) return "fonts";
    return "other";
  }

  function assetKind(asset: SourceGraphAsset): AssetKind {
    return kindFromExtension(extension(asset));
  }

  function kindLabel(kind: AssetKind) {
    if (kind === "images") return "Imagine";
    if (kind === "fonts") return "Font";
    return "Fișier";
  }

  function defaultDestination(view: AssetView) {
    if (view === "fonts") return "static/fonts";
    if (view === "other") return "static/files";
    return "static/images";
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

  function resetPanel() {
    detailMode = "info";
    formError = "";
  }

  function selectView(view: AssetView) {
    activeView = view;
    resetPanel();
  }

  function selectAsset(id: string) {
    selectedAssetId = id;
    resetPanel();
  }

  function beginCreate() {
    sourcePath = "";
    fileName = "";
    destinationDirectory = defaultDestination(activeView);
    formError = "";
    detailMode = "create";
  }

  function beginEdit() {
    if (!selectedAsset || assetKind(selectedAsset) !== "images") return;
    formError = "";
    detailMode = "edit";
  }

  async function selectImportFile() {
    formError = "";
    try {
      const selected = await chooseAssetFile();
      if (!selected) return;
      sourcePath = selected;
      fileName = selected.replaceAll("\\", "/").split("/").at(-1) ?? "";
      if (activeView === "all") {
        destinationDirectory = defaultDestination(kindFromExtension(fileExtension(fileName)));
      }
    } catch (error) {
      formError = errorMessage(error);
    }
  }

  async function importAsset() {
    if (importing) return;
    if (!sourcePath.trim()) {
      formError = "Alege mai întâi un fișier pentru import.";
      return;
    }
    importing = true;
    formError = "";
    try {
      const receipt = await importProjectAsset(
        sourcePath,
        destinationDirectory,
        fileName,
        {
          expectedProjectRoot: app.sessionProjectRoot,
          expectedSessionId: app.kernelProjectSessionId,
        },
      );
      app.projectWorkspaceSnapshot = receipt.workspace;
      if (receipt.relativePath) selectedAssetId = `staged:${receipt.relativePath}`;
      resetPanel();
      app.setGlobalStatus(
        `Resursa ${receipt.relativePath ?? fileName} este pregătită în ProjectWorkspace — Ctrl+S persistă pe disc.`,
        "unsaved",
      );
    } catch (error) {
      formError = errorMessage(error);
    } finally {
      importing = false;
    }
  }

  async function applyToSelectedImage(asset: SourceGraphAsset) {
    if (!selectedImageTarget || assetKind(asset) !== "images" || applying) return;
    applying = true;
    formError = "";
    try {
      const outcome = await app.applyImageSourceToHtml(sourceValue(asset));
      if (outcome.status === "committed" || outcome.status === "noop") {
        resetPanel();
        await app.setWorkbenchActivity("editor");
      }
    } catch (error) {
      formError = errorMessage(error);
    } finally {
      applying = false;
    }
  }

  function handleViewKeydown(event: KeyboardEvent, index: number) {
    let nextIndex: number | null = null;
    if (event.key === "ArrowLeft") nextIndex = (index - 1 + assetViews.length) % assetViews.length;
    if (event.key === "ArrowRight") nextIndex = (index + 1) % assetViews.length;
    if (event.key === "Home") nextIndex = 0;
    if (event.key === "End") nextIndex = assetViews.length - 1;
    if (nextIndex === null) return;
    event.preventDefault();
    const next = assetViews[nextIndex];
    if (!next) return;
    selectView(next.id);
    requestAnimationFrame(() => document.getElementById(`assets-tab-${next.id}`)?.focus());
  }
</script>

<section class="assets-workspace" aria-labelledby="assets-title">
  <header class="workspace-header">
    <div>
      <span class="eyebrow"><IconPhoto size={15} stroke={1.9} /> Bibliotecă media</span>
      <h1 id="assets-title">Resurse</h1>
      <p>Inventarul vine din harta surselor, iar importul și utilizarea resurselor sunt controlate de Rust.</p>
    </div>
    <dl>
      <div><dt>Total</dt><dd>{assets.length}</dd></div>
      <div><dt>Utilizate</dt><dd>{assets.length - unusedCount}</dd></div>
      <div class:warning={unusedCount > 0}><dt>Nefolosite</dt><dd>{unusedCount}</dd></div>
    </dl>
  </header>

  <div class="workspace-toolbar">
    <div class="view-tabs" role="tablist" aria-label="Tipuri de resurse">
      {#each assetViews as view, index (view.id)}
        <button
          id={`assets-tab-${view.id}`}
          type="button"
          role="tab"
          aria-selected={activeView === view.id ? "true" : "false"}
          aria-controls={`assets-panel-${view.id}`}
          tabindex={activeView === view.id ? 0 : -1}
          class:active={activeView === view.id}
          onclick={() => selectView(view.id)}
          onkeydown={(event) => handleViewKeydown(event, index)}
        >{view.label}</button>
      {/each}
    </div>
    <label>
      <span class="sr-only">Filtru utilizare</span>
      <select bind:value={usageFilter} aria-label="Filtru utilizare">
        <option value="all">Toate utilizările</option>
        <option value="used">Utilizate</option>
        <option value="unused">Nefolosite</option>
      </select>
    </label>
    <label class="search-field">
      <span class="sr-only">Caută resurse</span>
      <IconSearch size={14} stroke={1.9} />
      <input bind:value={query} type="search" placeholder="Caută după nume sau cale" />
    </label>
    <button class="toolbar-action" type="button" disabled={importing} onclick={beginCreate}>
      <IconPlus size={14} stroke={2} /> Adaugă
    </button>
  </div>

  <div class="workspace-body">
    <div
      class="asset-grid"
      id={`assets-panel-${activeView}`}
      role="tabpanel"
      aria-labelledby={`assets-tab-${activeView}`}
      aria-label="Bibliotecă resurse"
    >
      {#each filteredAssets as asset (asset.id)}
        <button
          type="button"
          class="asset-card"
          class:selected={selectedAsset?.id === asset.id}
          onclick={() => selectAsset(asset.id)}
        >
          <span class="asset-preview">
            {#if assetKind(asset) === "images" && assetUrl(asset)}
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
            {asset.id.startsWith("staged:")
              ? "În sesiune"
              : usageCount(asset) === 0 ? "Nefolosit" : `${usageCount(asset)} utilizări`}
          </span>
        </button>
      {:else}
        <div class="workspace-state">
          <IconPhoto size={28} stroke={1.5} />
          <strong>{assets.length === 0 ? "Proiectul nu conține resurse statice" : "Niciun rezultat"}</strong>
          <span>Importă o resursă sau schimbă tabul, utilizarea ori termenul de căutare.</span>
        </div>
      {/each}
    </div>

    <aside class="asset-detail" aria-label="Panou contextual resurse">
      {#if detailMode === "create"}
        <form class="import-form" onsubmit={(event) => { event.preventDefault(); void importAsset(); }}>
          <header class="detail-heading">
            <div>
              <span class="detail-kicker">Resursă nouă</span>
              <h2>Importă în proiect</h2>
              <p>Fișierul este citit de Rust și etapizat create-only în ProjectWorkspace.</p>
            </div>
            <button type="button" aria-label="Renunță la import" disabled={importing} onclick={resetPanel}><IconX size={14} /></button>
          </header>
          <button class="file-picker" type="button" disabled={importing} onclick={() => { void selectImportFile(); }}>
            <IconFolderOpen size={16} />
            <span><strong>{fileName || "Alege un fișier"}</strong><small>{sourcePath || "Fișierul original nu este modificat."}</small></span>
          </button>
          <label><span>Nume în proiect</span><input bind:value={fileName} disabled={importing} placeholder="imagine.webp" /></label>
          <label><span>Director destinație</span><input bind:value={destinationDirectory} disabled={importing} placeholder="static/images" /></label>
          {#if formError}<p class="form-error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>{/if}
          <div class="form-actions">
            <button type="button" disabled={importing} onclick={resetPanel}>Renunță</button>
            <button class="primary" type="submit" disabled={importing || !sourcePath || !fileName.trim()}>
              <IconUpload size={14} /> {importing ? "Se importă prin Rust…" : "Importă în sesiune"}
            </button>
          </div>
        </form>
      {:else if detailMode === "edit" && selectedAsset}
        <header class="detail-heading">
          <div>
            <span class="detail-kicker">Editare utilizare</span>
            <h2>{selectedAsset.logicalPath.split("/").at(-1)}</h2>
            <p>Aplicarea schimbă sursa elementului &lt;img&gt; selectat prin mutația HTML Rust.</p>
          </div>
          <button type="button" aria-label="Încheie editarea" disabled={applying} onclick={resetPanel}><IconX size={14} /></button>
        </header>
        <div class="target-card">
          <strong>{selectedImageTarget ? "Imagine selectată în Editor" : "Nicio imagine selectată"}</strong>
          <span>{selectedImageTarget?.sourceLocation?.file ?? "Selectează un <img> pentru înlocuire controlată."}</span>
        </div>
        {#if formError}<p class="form-error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>{/if}
        <div class="form-actions">
          <button type="button" disabled={applying} onclick={resetPanel}>Renunță</button>
          <button
            class="primary"
            type="button"
            disabled={!selectedImageTarget || assetKind(selectedAsset) !== "images" || applying}
            onclick={() => { void applyToSelectedImage(selectedAsset); }}
          >
            <IconPhotoCheck size={14} />
            {applying ? "Se aplică prin Rust…" : "Aplică pe imagine"}
          </button>
        </div>
      {:else if selectedAsset}
        <span class="detail-kicker">{kindLabel(assetKind(selectedAsset))} · {selectedAsset.origin}</span>
        <h2>{selectedAsset.logicalPath.split("/").at(-1)}</h2>
        {#if assetKind(selectedAsset) === "images" && assetUrl(selectedAsset)}
          <div class="detail-preview"><img src={assetUrl(selectedAsset)} alt={`Previzualizare ${selectedAsset.logicalPath}`} /></div>
        {/if}
        <dl class="asset-metadata">
          <div><dt>Cale publică</dt><dd>{sourceValue(selectedAsset)}</dd></div>
          <div><dt>Sursă</dt><dd>{selectedAsset.file}</dd></div>
          <div><dt>Format</dt><dd>{extension(selectedAsset).toUpperCase() || "—"}</dd></div>
          <div><dt>Utilizări</dt><dd>{usageCount(selectedAsset)}</dd></div>
        </dl>
        {#if selectedAsset.id.startsWith("staged:")}
          <p class="pending-note">Resursa există momentan numai în sesiune. Ctrl+S o persistă pe disc.</p>
        {/if}
        <div class="detail-actions">
          {#if assetKind(selectedAsset) === "images"}
            <button class="primary-action" type="button" onclick={beginEdit}>
              <IconEdit size={14} /> Editează utilizarea
            </button>
          {/if}
          <button class="secondary-action" type="button" onclick={() => { void app.openCurrentProjectInBrowser(sourceValue(selectedAsset)); }}>
            Deschide resursa <IconExternalLink size={13} stroke={1.9} />
          </button>
        </div>
      {:else}
        <div class="workspace-state">Selectează o resursă pentru informații.</div>
      {/if}
    </aside>
  </div>
</section>

<style>
  .assets-workspace { display: grid; grid-template-rows: auto 42px minmax(0, 1fr); min-width: 0; min-height: 0; height: 100%; overflow: hidden; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-panel); color: var(--wb-text-primary); background: var(--wb-surface-document); }
  .workspace-header { display: flex; align-items: center; justify-content: space-between; gap: 24px; padding: 17px 20px; border-bottom: 1px solid var(--wb-border-subtle); background: var(--wb-surface-chrome); }
  .eyebrow { display: inline-flex; align-items: center; gap: 6px; color: var(--wb-accent-strong); font-size: 12px; font-weight: 650; letter-spacing: .04em; text-transform: uppercase; }
  h1 { margin: 6px 0 0; color: var(--text-strong); font-size: 20px; font-weight: 650; letter-spacing: -.015em; }
  .workspace-header p { margin: 5px 0 0; color: var(--wb-text-muted); font-size: 12px; }
  .workspace-header > dl { display: flex; gap: 7px; margin: 0; }
  .workspace-header > dl div { min-width: 78px; padding: 7px 9px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .workspace-header > dl div.warning { border-color: color-mix(in srgb, var(--wb-warning) 45%, var(--wb-border-subtle)); }
  dt { color: var(--wb-text-muted); font-size: 12px; font-weight: 650; text-transform: uppercase; }
  dd { margin: 3px 0 0; color: var(--text-strong); font-size: 15px; font-weight: 650; }
  .workspace-toolbar, .view-tabs, .search-field, .toolbar-action, .detail-heading, .file-picker, .form-error, .form-actions, .detail-actions, .primary-action, .secondary-action { display: flex; align-items: center; }
  .workspace-toolbar { justify-content: flex-end; gap: 8px; padding: 5px 9px; border-bottom: 1px solid var(--wb-border-subtle); background: var(--wb-surface-chrome); }
  .view-tabs { align-self: stretch; gap: 2px; margin-right: auto; }
  .view-tabs button { height: 100%; padding: 0 10px; border: 0; border-bottom: 2px solid transparent; color: var(--wb-text-muted); background: transparent; font-size: 12px; font-weight: 600; }
  .view-tabs button.active { border-bottom-color: var(--wb-accent); color: var(--wb-accent-strong); }
  .search-field { position: relative; width: min(300px, 30vw); }
  .search-field :global(svg) { position: absolute; left: 8px; color: var(--wb-text-muted); }
  .search-field input, .workspace-toolbar select { height: 28px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; }
  .search-field input { width: 100%; padding: 0 8px 0 28px; }
  .workspace-toolbar select { min-width: 132px; padding: 0 7px; }
  .toolbar-action { flex: 0 0 auto; justify-content: center; gap: 5px; min-height: 28px; padding: 0 10px; border: 1px solid var(--wb-accent); border-radius: var(--radius-control); color: #fff; background: var(--wb-accent); font-size: 12px; font-weight: 650; }
  .workspace-body { display: grid; grid-template-columns: minmax(360px, 1fr) minmax(300px, .52fr); min-width: 0; min-height: 0; }
  .asset-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); align-content: start; gap: 8px; min-width: 0; min-height: 0; padding: 9px; overflow: auto; border-right: 1px solid var(--wb-border-subtle); }
  .asset-card { display: grid; grid-template-rows: 98px auto auto; min-width: 0; padding: 0; overflow: hidden; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-chrome); text-align: left; }
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
  .detail-heading { align-items: flex-start; justify-content: space-between; gap: 12px; }
  .detail-heading h2 { margin-top: 5px; }
  .detail-heading p { margin: 5px 0 0; color: var(--wb-text-muted); font-size: 12px; line-height: 1.45; }
  .detail-heading > button { display: grid; flex: 0 0 auto; width: 28px; height: 28px; padding: 0; place-items: center; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-muted); background: var(--wb-surface-document); }
  .detail-preview { display: grid; height: 180px; margin-top: 12px; overflow: hidden; place-items: center; border: 1px solid var(--wb-border-subtle); border-radius: 8px; background: var(--surface-7); }
  .detail-preview img { width: 100%; height: 100%; object-fit: contain; }
  .asset-metadata { display: grid; gap: 6px; margin: 11px 0 0; }
  .asset-metadata div { display: grid; gap: 3px; padding: 8px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  .asset-metadata dd { margin: 0; overflow-wrap: anywhere; font-size: 12px; font-weight: 500; }
  .pending-note { margin: 9px 0 0; padding: 8px; border: 1px dashed var(--wb-border-subtle); border-radius: 6px; color: var(--wb-text-muted); background: var(--wb-surface-document); font-size: 12px; line-height: 1.4; }
  .import-form { display: grid; gap: 11px; }
  .import-form > label { display: grid; gap: 5px; color: var(--wb-text-muted); font-size: 12px; font-weight: 700; }
  .import-form > label input { width: 100%; height: 34px; padding: 0 9px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; color: var(--text-strong); background: var(--wb-surface-document); font-size: 12px; }
  .file-picker { width: 100%; gap: 9px; min-height: 52px; padding: 8px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; color: var(--wb-accent-strong); background: var(--wb-surface-document); text-align: left; }
  .file-picker > span { display: grid; gap: 3px; min-width: 0; }
  .file-picker strong, .file-picker small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .file-picker strong { color: var(--text-strong); font-size: 12px; }
  .file-picker small { color: var(--wb-text-muted); font-size: 12px; }
  .target-card { display: grid; gap: 3px; margin-top: 12px; padding: 9px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  .target-card strong { color: var(--text-strong); font-size: 12px; }
  .target-card span { overflow: hidden; color: var(--wb-text-muted); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .form-error { align-items: flex-start; gap: 6px; margin: 0; padding: 8px; border: 1px solid color-mix(in srgb, var(--danger) 36%, var(--wb-border-subtle)); border-radius: 6px; color: var(--danger); background: color-mix(in srgb, var(--danger) 7%, var(--wb-surface-document)); font-size: 12px; line-height: 1.4; }
  .form-actions { justify-content: flex-end; gap: 7px; margin-top: 4px; }
  .form-actions button, .primary-action, .secondary-action { justify-content: center; gap: 6px; min-height: 32px; padding: 0 10px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; font-weight: 600; }
  .form-actions button.primary, .primary-action { border-color: var(--wb-accent); color: #fff; background: var(--wb-accent); }
  .detail-actions { align-items: stretch; gap: 7px; margin-top: 10px; }
  .detail-actions .primary-action, .detail-actions .secondary-action { flex: 1; }
  button:disabled { opacity: .5; }
  button:not(:disabled) { cursor: pointer; }
  button:focus-visible, input:focus-visible, select:focus-visible { outline: 2px solid var(--wb-focus-ring); outline-offset: 1px; }
  .workspace-state { display: grid; grid-column: 1 / -1; min-height: 220px; place-items: center; align-content: center; gap: 6px; color: var(--wb-text-muted); font-size: 12px; text-align: center; }
  .workspace-state strong { color: var(--text-strong); font-size: 12px; }
  .workspace-state span { max-width: 360px; }
  .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; }
  @media (max-width: 900px) { .workspace-body { grid-template-columns: 1fr; } .asset-detail { display: none; } .asset-grid { border-right: 0; } }
</style>
