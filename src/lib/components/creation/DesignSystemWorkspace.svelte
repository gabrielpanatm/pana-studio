<script lang="ts">
  import { IconPalette, IconPlus, IconSearch } from "@tabler/icons-svelte";
  import type { DesignClassInventorySnapshot } from "$lib/css/design-system-contract";
  import type { ScssVariable } from "$lib/css/contracts";
  import { FontManagerState } from "$lib/fonts/manager-state.svelte";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import { l10n, t } from "$lib/i18n/runtime.svelte";
  import type { FileExplorerSnapshot } from "$lib/project/file-explorer-contract";
  import type { ProjectWorkspaceIdentity } from "$lib/project/workspace-contract";
  import type { ProjectWorkspaceMutationService } from "$lib/session/workspace-mutation-service";
  import type { SourceGraph } from "$lib/source-graph/graph-contract";
  import type { GlobalStatusState } from "$lib/status/state.svelte";
  import ThemeStylesWorkspace from "./ThemeStylesWorkspace.svelte";
  import { DesignTokenCatalogState, ThemeStyleCatalogState } from "./design-system/catalog-state.svelte";
  import type { DesignSystemCommands, DesignView } from "./design-system/contracts";
  import DesignClassesWorkspace from "./design-system/DesignClassesWorkspace.svelte";
  import DesignTokensWorkspace from "./design-system/DesignTokensWorkspace.svelte";
  import FontManagerWorkspace from "./design-system/FontManagerWorkspace.svelte";
  import StylesheetsWorkspace from "./design-system/StylesheetsWorkspace.svelte";

  let {
    sourceGraph,
    designClassInventory,
    designClassInventoryLoading,
    designClassInventoryError,
    scssVariables,
    fileExplorerSnapshot,
    commands,
    globalStatus,
    workspaceMutations,
    openWorkspaceSource,
  }: {
    sourceGraph: SourceGraph | null;
    designClassInventory: DesignClassInventorySnapshot | null;
    designClassInventoryLoading: boolean;
    designClassInventoryError: string;
    scssVariables: ScssVariable[];
    fileExplorerSnapshot: FileExplorerSnapshot | null;
    commands: DesignSystemCommands;
    globalStatus: GlobalStatusState;
    workspaceMutations: ProjectWorkspaceMutationService;
    openWorkspaceSource: (path: string) => void | Promise<void>;
  } = $props();

  let activeView = $state<DesignView>("global-styles");
  let query = $state("");
  let tokenCategory = $state("all");
  let styleCategory = $state("all");
  let createRequest = $state(0);
  let activeViewBusy = $state(false);

  const catalogAuthority = () => ({
    projectRoot: workspaceMutations.identity?.expectedProjectRoot ?? "",
    runtimeSessionId: workspaceMutations.identity?.expectedSessionId ?? "",
    workspaceRevision: workspaceMutations.snapshot?.revision ?? null,
  });
  const workspaceIdentity = (): ProjectWorkspaceIdentity | null => {
    const snapshot = workspaceMutations.snapshot;
    if (!snapshot) return null;
    return {
      expectedProjectRoot: snapshot.projectRoot,
      expectedSessionId: snapshot.runtimeSessionId,
      expectedRevision: snapshot.revision,
    };
  };
  const tokenCatalog = new DesignTokenCatalogState(catalogAuthority);
  const themeStyleCatalog = new ThemeStyleCatalogState(catalogAuthority);
  const fontManager = new FontManagerState(workspaceIdentity);

  const designViews = $derived([
    { id: "global-styles" as const, label: t("design-view-styles") },
    { id: "tokens" as const, label: t("design-view-tokens") },
    { id: "classes" as const, label: t("design-view-classes") },
    { id: "styles" as const, label: t("design-view-stylesheets") },
    { id: "fonts" as const, label: t("design-view-fonts") },
  ]);

  $effect(() => {
    const view = activeView;
    const authority = catalogAuthority();
    if (!authority.projectRoot || !authority.runtimeSessionId || authority.workspaceRevision === null) {
      tokenCatalog.reset();
      themeStyleCatalog.reset();
      fontManager.reset();
      return;
    }
    if (view === "global-styles") void themeStyleCatalog.refresh();
    else if (view === "tokens") void tokenCatalog.refresh();
    else if (view === "classes") void commands.refreshClassInventory();
    else if (view === "fonts") void fontManager.refresh();
  });

  $effect(() => {
    const catalog = tokenCatalog.snapshot;
    if (tokenCategory !== "all" && !catalog?.categories.some((entry) => entry.id === tokenCategory)) tokenCategory = "all";
  });

  $effect(() => {
    const catalog = themeStyleCatalog.snapshot;
    if (styleCategory !== "all" && !catalog?.categories.some((entry) => entry.id === styleCategory)) styleCategory = "all";
  });

  function selectView(view: DesignView) {
    activeView = view;
    activeViewBusy = false;
  }

  function handleViewKeydown(event: KeyboardEvent, index: number) {
    let nextIndex: number | null = null;
    if (event.key === "ArrowLeft") nextIndex = (index - 1 + designViews.length) % designViews.length;
    if (event.key === "ArrowRight") nextIndex = (index + 1) % designViews.length;
    if (event.key === "Home") nextIndex = 0;
    if (event.key === "End") nextIndex = designViews.length - 1;
    if (nextIndex === null) return;
    event.preventDefault();
    const next = designViews[nextIndex];
    if (!next) return;
    selectView(next.id);
    requestAnimationFrame(() => document.getElementById(`design-tab-${next.id}`)?.focus());
  }
</script>

<section class="activity-workspace design-workspace" aria-labelledby="design-title">
  <header class="workspace-header">
    <div><span class="eyebrow"><IconPalette size={15} stroke={1.9} /> {t("design-eyebrow")}</span><h1 id="design-title">{t("design-title")}</h1><p>{t("design-description")}</p></div>
    <dl>
      <div><dt>{t("design-view-styles")}</dt><dd>{l10n.formatNumber(themeStyleCatalog.snapshot?.targets.length ?? 0)}</dd></div>
      <div><dt>{t("design-view-tokens")}</dt><dd>{l10n.formatNumber(tokenCatalog.snapshot?.tokens.length ?? 0)}</dd></div>
      <div><dt>{t("design-view-classes")}</dt><dd>{l10n.formatNumber(designClassInventory?.classes.length ?? 0)}</dd></div>
      <div><dt>{t("design-view-stylesheets")}</dt><dd>{l10n.formatNumber(sourceGraph?.styles.length ?? 0)}</dd></div>
      <div><dt>{t("design-view-fonts")}</dt><dd>{l10n.formatNumber(fontManager.snapshot?.graph.families.length ?? 0)}</dd></div>
    </dl>
  </header>

  <div class="workspace-toolbar">
    <div class="ui-tabs view-tabs" role="tablist" aria-label={t("design-areas-label") }>
      {#each designViews as view, index (view.id)}<button id={`design-tab-${view.id}`} type="button" role="tab" aria-selected={activeView === view.id ? "true" : "false"} aria-controls={`design-panel-${view.id}`} tabindex={activeView === view.id ? 0 : -1} class="ui-tab" class:active={activeView === view.id} onclick={() => selectView(view.id)} onkeydown={(event) => handleViewKeydown(event, index)}>{view.label}</button>{/each}
    </div>
    <div class="toolbar-query-group" class:with-filter={activeView === "global-styles" || activeView === "tokens"}>
      {#if activeView === "global-styles"}<div class="toolbar-filter"><SelectControl size="toolbar" value={styleCategory} options={[{ value: "all", label: t("design-all-categories") }, ...(themeStyleCatalog.snapshot?.categories ?? []).map((entry) => ({ value: entry.id, label: `${entry.label} (${entry.targetCount})` }))]} ariaLabel={t("design-style-category")} onchange={(value) => { styleCategory = value; }} /></div>
      {:else if activeView === "tokens"}<div class="toolbar-filter"><SelectControl size="toolbar" value={tokenCategory} options={[{ value: "all", label: t("design-all-categories") }, ...(tokenCatalog.snapshot?.categories ?? []).map((entry) => ({ value: entry.id, label: `${entry.label} (${entry.tokenCount})` }))]} ariaLabel={t("design-token-category")} onchange={(value) => { tokenCategory = value; }} /></div>{/if}
      <label class="search-field"><span class="sr-only">{t("design-search-label")}</span><IconSearch size={14} stroke={1.9} /><input class="ui-field toolbar" bind:value={query} type="search" placeholder={activeView === "global-styles" ? t("design-search-styles") : t("design-search-resources")} /></label>
    </div>
    {#if activeView !== "global-styles"}<button class="ui-button primary toolbar toolbar-action" type="button" disabled={activeViewBusy} onclick={() => { createRequest += 1; }}><IconPlus size={14} stroke={2} /> {t("design-add")}</button>{/if}
  </div>

  {#if activeView === "global-styles"}
    <ThemeStylesWorkspace {globalStatus} {workspaceMutations} {scssVariables} injectRawCss={(id, css) => commands.injectRawCss(id, css)} projectCommittedCssMutation={(authority, liveEpoch) => commands.projectCommittedCssMutation(authority, liveEpoch)} catalog={themeStyleCatalog.snapshot} loading={themeStyleCatalog.loading} error={themeStyleCatalog.error} {query} category={styleCategory} reload={async () => { await themeStyleCatalog.refresh(true); }} {openWorkspaceSource} />
  {:else if activeView === "tokens"}
    <DesignTokensWorkspace catalogState={tokenCatalog} {sourceGraph} {query} category={tokenCategory} {createRequest} bind:busy={activeViewBusy} createVariable={commands.createVariable} updateVariable={commands.updateVariable} {openWorkspaceSource} />
  {:else if activeView === "classes"}
    <DesignClassesWorkspace inventory={designClassInventory} loading={designClassInventoryLoading} error={designClassInventoryError} {sourceGraph} {query} {createRequest} bind:busy={activeViewBusy} createClass={commands.createClass} renameClass={commands.renameClass} {openWorkspaceSource} />
  {:else if activeView === "styles"}
    <StylesheetsWorkspace {sourceGraph} {fileExplorerSnapshot} {query} {createRequest} bind:busy={activeViewBusy} commands={{ refreshFileExplorer: commands.refreshFileExplorer, planFileExplorer: commands.planFileExplorer, commitFileExplorer: commands.commitFileExplorer }} {globalStatus} {workspaceMutations} {openWorkspaceSource} />
  {:else}
    <FontManagerWorkspace state={fontManager} {query} {createRequest} bind:busy={activeViewBusy} {globalStatus} {workspaceMutations} />
  {/if}
</section>

<style>
  dt { color: var(--wb-text-muted); font-size: 12px; font-weight: 650; text-transform: uppercase; }
  dd { margin: 3px 0 0; color: var(--text-strong); font-size: 15px; font-weight: 650; }
</style>
