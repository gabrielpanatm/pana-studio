<script lang="ts">
  import {
    IconEdit,
    IconExternalLink,
    IconFile,
    IconFolderOpen,
    IconPhoto,
    IconPhotoCheck,
    IconPlus,
    IconSearch,
    IconTrash,
    IconUpload,
    IconX,
  } from "@tabler/icons-svelte";
  import EmptyState from "$lib/components/ui/EmptyState.svelte";
  import InlineMessage from "$lib/components/ui/InlineMessage.svelte";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import TextFieldControl from "$lib/components/ui/TextFieldControl.svelte";
  import {
  chooseAssetFile,
  importProjectAsset,
} from "$lib/page-assets/io";
  import { projectPreviewResourceUrl } from "$lib/project/assets";
  import { l10n, t } from "$lib/i18n/runtime.svelte";
  import type { EditorActionOutcome } from "$lib/editor-runtime/action-outcome";
  import type { GlobalStatusState } from "$lib/status/state.svelte";
  import type { ProjectWorkspaceMutationService } from "$lib/session/workspace-mutation-service";
  import type { CoordinatedElementSelection } from "$lib/canvas/contracts";
  import type {
    FileExplorerCommitReceipt,
    FileExplorerOperationPlan,
    FileExplorerOperationRequest,
    FileExplorerSnapshot,
  } from "$lib/project/file-explorer-contract";
  import type { SourceGraph } from "$lib/source-graph/graph-contract";
  import type { SourceGraphAsset } from "$lib/source-graph/contracts";
  import { errorMessage } from "$lib/util";

  let {
    sourceGraph,
    previewRevision,
    coordinatedElementSelection,
    previewBaseUrl,
    fileExplorerSnapshot,
    commands,
    globalStatus,
    workspaceMutations,
  }: {
    sourceGraph: SourceGraph | null;
    previewRevision: string | null;
    coordinatedElementSelection: CoordinatedElementSelection | null;
    previewBaseUrl: string | null;
    fileExplorerSnapshot: FileExplorerSnapshot | null;
    commands: {
      refreshFileExplorer: () => Promise<FileExplorerSnapshot | null>;
      planFileExplorer: (request: FileExplorerOperationRequest) => Promise<FileExplorerOperationPlan>;
      commitFileExplorer: (plan: FileExplorerOperationPlan) => Promise<FileExplorerCommitReceipt>;
      applyImageSource: (source: string) => Promise<EditorActionOutcome>;
      openEditor: () => Promise<unknown>;
      openInBrowser: (route: string) => Promise<unknown>;
    };
    globalStatus: GlobalStatusState;
    workspaceMutations: ProjectWorkspaceMutationService;
  } = $props();

  type UsageFilter = "all" | "used" | "unused";
  type AssetView = "all" | "images" | "fonts" | "other";
  type AssetKind = Exclude<AssetView, "all">;
  type DetailMode = "info" | "create" | "edit";

  const assetViews = $derived([
    { id: "all" as const, label: t("assets-view-all") },
    { id: "images" as const, label: t("assets-view-images") },
    { id: "fonts" as const, label: t("assets-view-fonts") },
    { id: "other" as const, label: t("assets-view-other") },
  ]);

  let activeView = $state<AssetView>("all");
  let detailMode = $state<DetailMode>("info");
  let query = $state("");
  let usageFilter = $state<UsageFilter>("all");
  let selectedAssetId = $state("");
  let applying = $state(false);
  let importing = $state(false);
  let deleting = $state(false);
  let pendingDeleteAsset = $state<SourceGraphAsset | null>(null);
  let deleteError = $state("");
  let formError = $state("");
  let sourcePath = $state("");
  let fileName = $state("");
  let destinationDirectory = $state("static/images");

  const graphAssets = $derived(sourceGraph?.assets ?? []);
  const assets = $derived.by(() => {
    const deleted = new Set([
      ...(workspaceMutations.snapshot?.deletedDocuments ?? []),
      ...(workspaceMutations.snapshot?.deletedBinaryResources ?? []),
    ]);
    const visibleGraphAssets = graphAssets.filter((asset) => !deleted.has(asset.file));
    const existing = new Set(visibleGraphAssets.map((asset) => asset.file));
    const staged = (workspaceMutations.snapshot?.stagedBinaryResources ?? [])
      .filter((path) => path.startsWith("static/") && !deleted.has(path) && !existing.has(path))
      .map((path): SourceGraphAsset => ({
        id: `staged:${path}`,
        file: path,
        origin: "local",
        themeName: null,
        logicalPath: path.replace(/^static\/?/, ""),
        nodeId: `staged-asset:${path}`,
      }));
    return [...visibleGraphAssets, ...staged];
  });
  const normalizedQuery = $derived(query.trim().toLocaleLowerCase(l10n.locale));
  const filteredAssets = $derived(
    assets.filter((asset) => {
      const usages = usageCount(asset);
      return (usageFilter === "all" || (usageFilter === "used" ? usages > 0 : usages === 0))
        && (activeView === "all" || assetKind(asset) === activeView)
        && (!normalizedQuery || `${asset.logicalPath} ${asset.file} ${asset.themeName ?? ""}`
          .toLocaleLowerCase(l10n.locale)
          .includes(normalizedQuery));
    }),
  );
  const selectedAsset = $derived(
    assets.find((asset) => asset.id === selectedAssetId) ?? filteredAssets[0] ?? null,
  );
  const unusedCount = $derived(assets.filter((asset) => usageCount(asset) === 0).length);
  const assetPreviewRevision = $derived(previewRevision);
  const selectedImageTarget = $derived(
    coordinatedElementSelection?.observation.tag === "img"
      ? coordinatedElementSelection
      : null,
  );
  const pendingDeleteUsages = $derived.by(() => {
    if (!pendingDeleteAsset) return [];
    const nodes = new Map((sourceGraph?.nodes ?? []).map((node) => [node.id, node]));
    return (sourceGraph?.relations ?? [])
      .filter((relation) => relation.to === pendingDeleteAsset?.nodeId)
      .map((relation) => ({
        id: relation.id,
        label: `${nodes.get(relation.from)?.file ?? relation.from} — ${relation.label}`,
      }));
  });

  function usageCount(asset: SourceGraphAsset) {
    return (sourceGraph?.relations ?? []).filter((relation) => relation.to === asset.nodeId).length;
  }

  function fileExtension(value: string) {
    return value.split(".").at(-1)?.toLocaleLowerCase(l10n.locale) ?? "";
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
    if (kind === "images") return t("assets-kind-image");
    if (kind === "fonts") return t("assets-kind-font");
    return t("assets-kind-file");
  }

  function defaultDestination(view: AssetView) {
    if (view === "fonts") return "static/fonts";
    if (view === "other") return "static/files";
    return "static/images";
  }

  function assetUrl(asset: SourceGraphAsset) {
    return projectPreviewResourceUrl(
      previewBaseUrl,
      asset.logicalPath,
      assetPreviewRevision,
    );
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

  function requestDelete(asset: SourceGraphAsset) {
    deleteError = "";
    pendingDeleteAsset = asset;
  }

  function cancelDelete() {
    if (deleting) return;
    pendingDeleteAsset = null;
    deleteError = "";
  }

  async function resolveAssetExplorerEntry(asset: SourceGraphAsset) {
    const workspace = workspaceMutations.snapshot;
    let explorer = fileExplorerSnapshot;
    if (
      !workspace
      || !explorer
      || explorer.projectRoot !== workspace.projectRoot
      || explorer.runtimeSessionId !== workspace.runtimeSessionId
      || explorer.workspaceRevision !== workspace.revision
      || !explorer.entries.some((entry) => entry.relativePath === asset.file)
    ) {
      explorer = await commands.refreshFileExplorer();
    }
    return explorer?.entries.find((entry) => entry.relativePath === asset.file) ?? null;
  }

  async function confirmDelete() {
    const asset = pendingDeleteAsset;
    if (!asset || deleting) return;
    deleting = true;
    deleteError = "";
    const stagedOnly = asset.id.startsWith("staged:");
    try {
      const entry = await resolveAssetExplorerEntry(asset);
      if (!entry) throw new Error(t("assets-delete-entry-unavailable", { path: asset.file }));
      const plan = await commands.planFileExplorer({ kind: "delete", entryId: entry.id });
      if (!plan.allowed) {
        throw new Error(plan.diagnostic ?? t("assets-delete-plan-blocked"));
      }
      await commands.commitFileExplorer(plan);
      selectedAssetId = "";
      pendingDeleteAsset = null;
      globalStatus.set(
        stagedOnly
          ? t("assets-delete-staged-success", { path: asset.file })
          : t("assets-delete-success", { path: asset.file }),
        workspaceMutations.snapshot?.dirty ? "unsaved" : "idle",
      );
    } catch (error) {
      deleteError = errorMessage(error);
    } finally {
      deleting = false;
    }
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && pendingDeleteAsset && !deleting) cancelDelete();
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
      formError = t("assets-choose-first");
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
          expectedProjectRoot: workspaceMutations.identity?.expectedProjectRoot ?? "",
          expectedSessionId: workspaceMutations.identity?.expectedSessionId ?? "",
        },
      );
      const settlement = await workspaceMutations.settle(receipt, {
        preferredRelativePath: receipt.relativePath,
        warningLabel: t("assets-import-operation"),
      });
      if (receipt.relativePath) selectedAssetId = `staged:${receipt.relativePath}`;
      resetPanel();
      globalStatus.set(
        settlement.warnings.length > 0
          ? t("assets-import-warning", { path: receipt.relativePath ?? fileName })
          : t("assets-import-success", { path: receipt.relativePath ?? fileName }),
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
      const outcome = await commands.applyImageSource(sourceValue(asset));
      if (outcome.status === "committed" || outcome.status === "noop") {
        resetPanel();
        await commands.openEditor();
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

<svelte:window onkeydown={handleWindowKeydown} />

<section class="activity-workspace assets-workspace" aria-labelledby="assets-title">
  <header class="workspace-header">
    <div>
      <span class="eyebrow"><IconPhoto size={15} stroke={1.9} /> {t("assets-eyebrow")}</span>
      <h1 id="assets-title">{t("assets-title")}</h1>
      <p>{t("assets-description")}</p>
    </div>
    <dl>
      <div><dt>{t("assets-stat-total")}</dt><dd>{l10n.formatNumber(assets.length)}</dd></div>
      <div><dt>{t("assets-stat-used")}</dt><dd>{l10n.formatNumber(assets.length - unusedCount)}</dd></div>
      <div class:warning={unusedCount > 0}><dt>{t("assets-stat-unused")}</dt><dd>{l10n.formatNumber(unusedCount)}</dd></div>
    </dl>
  </header>

  <div class="workspace-toolbar">
    <div class="ui-tabs view-tabs" role="tablist" aria-label={t("assets-types-label")}>
      {#each assetViews as view, index (view.id)}
        <button
          id={`assets-tab-${view.id}`}
          type="button"
          role="tab"
          aria-selected={activeView === view.id ? "true" : "false"}
          aria-controls={`assets-panel-${view.id}`}
          tabindex={activeView === view.id ? 0 : -1}
          class="ui-tab"
          class:active={activeView === view.id}
          onclick={() => selectView(view.id)}
          onkeydown={(event) => handleViewKeydown(event, index)}
        >{view.label}</button>
      {/each}
    </div>
    <div class="toolbar-query-group with-filter">
      <div class="toolbar-filter">
        <span class="sr-only">{t("assets-usage-filter")}</span>
        <SelectControl size="toolbar" value={usageFilter} options={[
          { value: "all", label: t("assets-usage-all") },
          { value: "used", label: t("assets-usage-used") },
          { value: "unused", label: t("assets-usage-unused") },
        ]} ariaLabel={t("assets-usage-filter")} onchange={(value) => { usageFilter = value as UsageFilter; }} />
      </div>
      <label class="search-field">
        <span class="sr-only">{t("assets-search-label")}</span>
        <IconSearch size={14} stroke={1.9} />
        <input class="ui-field toolbar" bind:value={query} type="search" placeholder={t("assets-search-placeholder")} />
      </label>
    </div>
    <button class="ui-button primary toolbar toolbar-action" type="button" disabled={importing} onclick={beginCreate}>
      <IconPlus size={14} stroke={2} /> {t("assets-add")}
    </button>
  </div>

  <div class="workspace-body">
    <div
      class="asset-grid"
      id={`assets-panel-${activeView}`}
      role="tabpanel"
      aria-labelledby={`assets-tab-${activeView}`}
      aria-label={t("assets-library-label")}
    >
      {#each filteredAssets as asset (asset.id)}
        <button
          type="button"
          class="asset-card ui-entity-selectable"
          data-ui-selected={selectedAsset?.id === asset.id ? "true" : undefined}
          aria-pressed={selectedAsset?.id === asset.id}
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
              ? t("assets-in-session")
              : usageCount(asset) === 0
                ? t("assets-unused")
                : t("assets-usage-count", { count: usageCount(asset) })}
          </span>
        </button>
      {:else}
        <EmptyState title={assets.length === 0 ? t("assets-empty-project") : t("assets-no-results")} description={t("assets-empty-description")} />
      {/each}
    </div>

    <aside class="asset-detail" aria-label={t("assets-detail-label")}>
      {#if detailMode === "create"}
        <form class="import-form" onsubmit={(event) => { event.preventDefault(); void importAsset(); }}>
          <header class="detail-heading">
            <div>
              <span class="detail-kicker">{t("assets-new")}</span>
              <h2>{t("assets-import-title")}</h2>
              <p>{t("assets-import-description")}</p>
            </div>
            <button class="ui-icon-button ui-close-button" type="button" aria-label={t("assets-cancel-import")} disabled={importing} onclick={resetPanel}><IconX size={14} /></button>
          </header>
          <button class="file-picker" type="button" disabled={importing} onclick={() => { void selectImportFile(); }}>
            <IconFolderOpen size={16} />
            <span><strong>{fileName || t("assets-choose-file")}</strong><small>{sourcePath || t("assets-source-preserved")}</small></span>
          </button>
          <TextFieldControl label={t("assets-project-name")} bind:value={fileName} disabled={importing} placeholder="image.webp" />
          <TextFieldControl label={t("assets-destination-directory")} bind:value={destinationDirectory} disabled={importing} placeholder="static/images" />
          {#if formError}<InlineMessage message={formError} tone="error" />{/if}
          <div class="form-actions">
            <button class="ui-button compact" type="button" disabled={importing} onclick={resetPanel}>{t("assets-cancel")}</button>
            <button class="ui-button primary" type="submit" disabled={importing || !sourcePath || !fileName.trim()}>
              <IconUpload size={14} /> {importing ? t("assets-importing") : t("assets-import-session")}
            </button>
          </div>
        </form>
      {:else if detailMode === "edit" && selectedAsset}
        <header class="detail-heading">
          <div>
            <span class="detail-kicker">{t("assets-edit-usage")}</span>
            <h2>{selectedAsset.logicalPath.split("/").at(-1)}</h2>
            <p>{t("assets-edit-description")}</p>
          </div>
          <button class="ui-icon-button ui-close-button" type="button" aria-label={t("assets-finish-editing")} disabled={applying} onclick={resetPanel}><IconX size={14} /></button>
        </header>
        <div class="target-card">
          <strong>{selectedImageTarget ? t("assets-selected-image") : t("assets-no-selected-image")}</strong>
          <span>{selectedImageTarget?.sourceLocation?.file ?? t("assets-select-img-help")}</span>
        </div>
        {#if formError}<InlineMessage message={formError} tone="error" />{/if}
        <div class="form-actions">
          <button class="ui-button compact" type="button" disabled={applying} onclick={resetPanel}>{t("assets-cancel")}</button>
          <button
            class="ui-button primary"
            type="button"
            disabled={!selectedImageTarget || assetKind(selectedAsset) !== "images" || applying}
            onclick={() => { void applyToSelectedImage(selectedAsset); }}
          >
            <IconPhotoCheck size={14} />
            {applying ? t("assets-applying") : t("assets-apply-image")}
          </button>
        </div>
      {:else if selectedAsset}
        <span class="detail-kicker">{kindLabel(assetKind(selectedAsset))} · {selectedAsset.origin}</span>
        <h2>{selectedAsset.logicalPath.split("/").at(-1)}</h2>
        {#if assetKind(selectedAsset) === "images" && assetUrl(selectedAsset)}
          <div class="detail-preview"><img src={assetUrl(selectedAsset)} alt={t("assets-preview-label", { path: selectedAsset.logicalPath })} /></div>
        {/if}
        <dl class="asset-metadata">
          <div><dt>{t("assets-public-path")}</dt><dd>{sourceValue(selectedAsset)}</dd></div>
          <div><dt>{t("assets-source")}</dt><dd>{selectedAsset.file}</dd></div>
          <div><dt>{t("assets-format")}</dt><dd>{extension(selectedAsset).toUpperCase() || "—"}</dd></div>
          <div><dt>{t("assets-usages")}</dt><dd>{l10n.formatNumber(usageCount(selectedAsset))}</dd></div>
        </dl>
        {#if selectedAsset.id.startsWith("staged:")}
          <p class="pending-note">{t("assets-pending-note")}</p>
        {/if}
        <div class="detail-actions">
          {#if assetKind(selectedAsset) === "images"}
            <button class="ui-button primary primary-action" type="button" onclick={beginEdit}>
              <IconEdit size={14} /> {t("assets-edit-usage-action")}
            </button>
          {/if}
          <button class="ui-button secondary-action" type="button" onclick={() => { void commands.openInBrowser(sourceValue(selectedAsset)); }}>
            {t("assets-open")} <IconExternalLink size={13} stroke={1.9} />
          </button>
          <button class="ui-button danger delete-action" type="button" onclick={() => requestDelete(selectedAsset)}>
            <IconTrash size={14} /> {t("assets-delete")}
          </button>
        </div>
      {:else}
        <EmptyState title={t("assets-select-help")} />
      {/if}
    </aside>
  </div>
</section>

{#if pendingDeleteAsset}
  <div class="delete-modal-backdrop" role="presentation">
    <div class="delete-modal" role="dialog" aria-modal="true" aria-labelledby="asset-delete-title">
      <div class="delete-modal-icon"><IconTrash size={18} stroke={2} /></div>
      <div class="delete-modal-body">
        <h3 id="asset-delete-title">{t("assets-delete-title", { name: pendingDeleteAsset.logicalPath.split("/").at(-1) ?? pendingDeleteAsset.logicalPath })}</h3>
        <p>{t("assets-delete-description", { path: pendingDeleteAsset.file })}</p>
        {#if pendingDeleteUsages.length > 0}
          <p class="delete-modal-warning">{t("assets-delete-used-warning", { count: pendingDeleteUsages.length })}</p>
          <ul class="delete-usage-list">
            {#each pendingDeleteUsages as usage (usage.id)}<li>{usage.label}</li>{/each}
          </ul>
          <p class="delete-modal-note">{t("assets-delete-references-preserved")}</p>
        {:else}
          <p class="delete-modal-note neutral">{t("assets-delete-unused-note")}</p>
        {/if}
        {#if assetKind(pendingDeleteAsset) === "other"}
          <p class="delete-modal-note neutral">{t("assets-delete-auxiliary-note")}</p>
        {/if}
        {#if deleteError}<div class="delete-error"><InlineMessage message={deleteError} tone="error" /></div>{/if}
      </div>
      <div class="delete-modal-actions">
        <button type="button" class="ui-button delete-cancel-button" disabled={deleting} onclick={cancelDelete}>{t("assets-cancel")}</button>
        <button type="button" class="ui-button danger delete-confirm-button" disabled={deleting} onclick={() => { void confirmDelete(); }}>
          {deleting
            ? t("assets-deleting")
            : pendingDeleteUsages.length > 0
              ? t("assets-delete-anyway")
              : t("assets-delete-confirm")}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .workspace-header > dl div.warning { border-color: color-mix(in srgb, var(--wb-warning) 45%, var(--wb-border-subtle)); }
  dt { color: var(--wb-text-muted); font-size: 12px; font-weight: 650; text-transform: uppercase; }
  dd { margin: 3px 0 0; color: var(--text-strong); font-size: 15px; font-weight: 650; }
  .detail-heading, .file-picker, .form-actions, .detail-actions, .primary-action, .secondary-action, .delete-action { display: flex; align-items: center; }
  .workspace-body { display: grid; grid-template-columns: minmax(360px, 1fr) minmax(300px, .52fr); min-width: 0; min-height: 0; }
  .asset-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); align-content: start; gap: 8px; min-width: 0; min-height: 0; padding: 9px; overflow: auto; border-right: 1px solid var(--wb-border-subtle); }
  .asset-grid :global(.ui-empty-state) { grid-column: 1 / -1; }
  .asset-card { --ui-entity-background: var(--wb-surface-chrome); --ui-entity-border-color: var(--wb-border-subtle); display: grid; grid-template-rows: 98px auto auto; min-width: 0; padding: 0; overflow: hidden; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-chrome); text-align: left; }
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
  .detail-preview { display: grid; height: 180px; margin-top: 12px; overflow: hidden; place-items: center; border: 1px solid var(--wb-border-subtle); border-radius: 8px; background: var(--surface-7); }
  .detail-preview img { width: 100%; height: 100%; object-fit: contain; }
  .asset-metadata { display: grid; gap: 6px; margin: 11px 0 0; }
  .asset-metadata div { display: grid; gap: 3px; padding: 8px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  .asset-metadata dd { margin: 0; overflow-wrap: anywhere; font-size: 12px; font-weight: 500; }
  .pending-note { margin: 9px 0 0; padding: 8px; border: 1px dashed var(--wb-border-subtle); border-radius: 6px; color: var(--wb-text-muted); background: var(--wb-surface-document); font-size: 12px; line-height: 1.4; }
  .import-form { display: grid; gap: 11px; }
  .file-picker { width: 100%; gap: 9px; min-height: 52px; padding: 8px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; color: var(--wb-accent-strong); background: var(--wb-surface-document); text-align: left; }
  .file-picker > span { display: grid; gap: 3px; min-width: 0; }
  .file-picker strong, .file-picker small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .file-picker strong { color: var(--text-strong); font-size: 12px; }
  .file-picker small { color: var(--wb-text-muted); font-size: 12px; }
  .target-card { display: grid; gap: 3px; margin-top: 12px; padding: 9px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  .target-card strong { color: var(--text-strong); font-size: 12px; }
  .target-card span { overflow: hidden; color: var(--wb-text-muted); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .form-actions { justify-content: flex-end; gap: 7px; margin-top: 4px; }
  .primary-action, .secondary-action, .delete-action { justify-content: center; }
  .detail-actions { flex-wrap: wrap; align-items: stretch; gap: 7px; margin-top: 10px; }
  .detail-actions .primary-action, .detail-actions .secondary-action { flex: 1; }
  .detail-actions .delete-action { flex: 1 0 100%; }
  .delete-modal-backdrop { position: fixed; z-index: 12000; inset: 0; display: flex; align-items: center; justify-content: center; padding: 18px; background: rgb(0 0 0 / 48%); }
  .delete-modal { display: grid; grid-template-columns: 36px minmax(0, 1fr); gap: 12px; width: min(460px, 100%); max-height: min(620px, calc(100vh - 36px)); padding: 14px; overflow: auto; border: 1px solid color-mix(in srgb, var(--danger) 32%, var(--wb-border-subtle)); border-radius: 8px; background: var(--wb-surface-document); box-shadow: 0 24px 70px rgb(0 0 0 / 36%); }
  .delete-modal-icon { display: grid; width: 34px; height: 34px; place-items: center; border-radius: 8px; color: var(--danger); background: color-mix(in srgb, var(--danger) 12%, var(--wb-surface-chrome)); }
  .delete-modal-body h3 { margin: 0 0 7px; color: var(--text-strong); font-size: 14px; font-weight: 900; }
  .delete-modal-body p { margin: 0; color: var(--wb-text-primary); font-size: 12px; line-height: 1.45; }
  .delete-modal-body .delete-modal-warning { margin-top: 9px; color: var(--danger); font-weight: 800; }
  .delete-modal-body .delete-modal-note { margin-top: 8px; color: var(--danger); font-weight: 700; }
  .delete-modal-body .delete-modal-note.neutral { color: var(--wb-text-muted); font-weight: 500; }
  .delete-usage-list { display: grid; gap: 4px; max-height: 130px; margin: 7px 0 0; padding: 7px 7px 7px 25px; overflow: auto; border: 1px solid var(--wb-border-subtle); border-radius: 6px; color: var(--wb-text-primary); background: var(--wb-surface-chrome); font-size: 12px; }
  .delete-error { margin-top: 9px; }
  .delete-modal-actions { grid-column: 1 / -1; display: flex; justify-content: flex-end; gap: 8px; padding-top: 2px; }
  .delete-cancel-button, .delete-confirm-button { min-width: 92px; }
  .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; }
  @media (max-width: 900px) { .workspace-body { grid-template-columns: 1fr; } .asset-detail { display: none; } .asset-grid { border-right: 0; } }
</style>
