<script lang="ts">
  import {
    IconAlertTriangle,
    IconBox,
    IconCode,
    IconPlus,
    IconSearch,
    IconX,
  } from "@tabler/icons-svelte";
  import { nativeBlockPaletteGroupsFromRegistry } from "$lib/blocks/registry";
  import { l10n, t } from "$lib/i18n/runtime.svelte";
  import { readNativeBlockRegistry, readUiBlockGraph } from "$lib/project/io";
  import type { HtmlPaletteElement } from "$lib/project/html-palette";
  import type { AppState } from "$lib/state/app.svelte";
  import type {
    BlockDefinition,
    FileBufferRequestIdentity,
    NativeBlockRegistrySnapshot,
    UiBlockGraphSnapshot,
  } from "$lib/types";
  import { errorMessage } from "$lib/util";

  let { app }: { app: AppState } = $props();

  type BlockView = "all" | "element" | "section" | "composition";
  type DetailMode = "info" | "insert";

  const blockViews = $derived([
    { id: "all" as const, label: t("blocks-view-all") },
    { id: "element" as const, label: t("blocks-view-elements") },
    { id: "section" as const, label: t("blocks-view-sections") },
    { id: "composition" as const, label: t("blocks-view-compositions") },
  ]);

  let activeView = $state<BlockView>("all");
  let detailMode = $state<DetailMode>("info");
  let selectedDefinitionId = $state("");
  let query = $state("");
  let registry = $state<NativeBlockRegistrySnapshot | null>(null);
  let uiBlockGraph = $state<UiBlockGraphSnapshot | null>(null);
  let loadError = $state("");
  let inserting = $state(false);
  let refreshKey = "";

  const blockGraph = $derived(uiBlockGraph);
  const definitions = $derived(blockGraph?.definitions ?? []);
  const normalizedQuery = $derived(query.trim().toLocaleLowerCase(l10n.locale));
  const filteredDefinitions = $derived(definitions.filter((definition) => {
    const inView = activeView === "all" || definition.scale === activeView;
    if (!inView) return false;
    if (!normalizedQuery) return true;
    return [
      definition.displayName,
      definition.description,
      definition.providerId,
      definition.familyId,
      definition.variantId,
      definition.origin,
    ].join(" ").toLocaleLowerCase(l10n.locale).includes(normalizedQuery);
  }));
  const selectedDefinition = $derived(
    definitions.find((definition) => definition.id === selectedDefinitionId)
      ?? filteredDefinitions[0]
      ?? null,
  );
  const paletteElements = $derived(
    nativeBlockPaletteGroupsFromRegistry(registry).flatMap((group) => group.elements),
  );
  const selectedPaletteElement = $derived(
    selectedDefinition
      ? paletteElements.find((element) => element.blockId === selectedDefinition.providerId) ?? null
      : null,
  );
  const selectedSourceInstances = $derived(
    selectedDefinition
      ? (blockGraph?.sourceInstances ?? []).filter(
        (instance) => instance.definitionId === selectedDefinition.id,
      )
      : [],
  );
  const selectedRenderedInstances = $derived(
    selectedDefinition
      ? (uiBlockGraph?.renderedInstances ?? []).filter(
        (instance) => instance.definitionId === selectedDefinition.id,
      )
      : [],
  );
  const insertTarget = $derived(app.coordinatedElementSelection);

  $effect(() => {
    const projectRoot = app.sessionProjectRoot;
    const sessionId = app.kernelProjectSessionId;
    const revision = app.projectWorkspaceSnapshot?.revision;
    const previewRevision = app.activeCanvasIdentity?.previewRevision ?? "";
    if (!projectRoot || !sessionId || revision === undefined) return;
    const key = `${projectRoot}\u0000${sessionId}\u0000${revision}\u0000${previewRevision}`;
    if (key === refreshKey) return;
    refreshKey = key;
    loadError = "";
    void Promise.all([
      readNativeBlockRegistry(),
      readUiBlockGraph(identity()),
    ])
      .then(([nextRegistry, nextGraph]) => {
        if (refreshKey !== key) return;
        registry = nextRegistry;
        uiBlockGraph = nextGraph;
      })
      .catch((cause) => {
        if (refreshKey === key) loadError = errorMessage(cause);
      });
  });

  function identity(): FileBufferRequestIdentity {
    return {
      expectedProjectRoot: app.sessionProjectRoot,
      expectedSessionId: app.kernelProjectSessionId,
    };
  }

  function selectView(view: BlockView) {
    activeView = view;
    selectedDefinitionId = "";
    detailMode = "info";
  }

  function selectDefinition(definition: BlockDefinition) {
    selectedDefinitionId = definition.id;
    detailMode = "info";
    loadError = "";
  }

  function beginInsert() {
    if (!selectedDefinition?.capabilities.canInsert || !selectedPaletteElement) return;
    loadError = "";
    detailMode = "insert";
  }

  async function insertSelectedBlock() {
    const target = insertTarget;
    const element = selectedPaletteElement as HtmlPaletteElement | null;
    if (!element || !target?.sourceLocation || inserting) return;
    const observation = target.observation;
    inserting = true;
    loadError = "";
    try {
      const outcome = await app.insertPaletteElementAtTarget({
        targetRenderInstanceId: target.renderInstanceId,
        targetSelector: observation.domPath || observation.cssSelector || observation.selector,
        targetSessionId: target.snapshot.runtimeSessionId,
        targetSourceId: target.sourceNodeId,
        targetTemplateSourceId: null,
        targetSourceLocation: target.sourceLocation,
        targetTag: observation.tag,
        position: "after",
        element,
      });
      if (outcome.status !== "committed") {
        loadError = outcome.reason ?? t("blocks-insert-failed");
        return;
      }
      detailMode = "info";
      await app.setWorkbenchActivity("editor");
    } catch (cause) {
      loadError = errorMessage(cause);
    } finally {
      inserting = false;
    }
  }

  function handleViewKeydown(event: KeyboardEvent, index: number) {
    let nextIndex: number | null = null;
    if (event.key === "ArrowLeft") nextIndex = (index - 1 + blockViews.length) % blockViews.length;
    if (event.key === "ArrowRight") nextIndex = (index + 1) % blockViews.length;
    if (event.key === "Home") nextIndex = 0;
    if (event.key === "End") nextIndex = blockViews.length - 1;
    if (nextIndex === null) return;
    event.preventDefault();
    const next = blockViews[nextIndex];
    if (!next) return;
    selectView(next.id);
    requestAnimationFrame(() => document.getElementById(`blocks-tab-${next.id}`)?.focus());
  }
</script>

<section class="activity-workspace blocks-workspace" aria-labelledby="blocks-title">
  <header class="workspace-header">
    <div>
      <span class="eyebrow"><IconBox size={15} stroke={1.9} /> {t("blocks-eyebrow")}</span>
      <h1 id="blocks-title">{t("blocks-title")}</h1>
      <p>{t("blocks-description")}</p>
    </div>
    <dl>
      <div><dt>{t("blocks-stat-definitions")}</dt><dd>{l10n.formatNumber(definitions.length)}</dd></div>
      <div><dt>{t("blocks-stat-source")}</dt><dd>{l10n.formatNumber(blockGraph?.sourceInstances.length ?? 0)}</dd></div>
      <div><dt>{t("blocks-stat-rendered")}</dt><dd>{l10n.formatNumber(uiBlockGraph?.renderedInstances.length ?? 0)}</dd></div>
      <div><dt>{t("blocks-stat-diagnostics")}</dt><dd>{l10n.formatNumber(blockGraph?.diagnostics.length ?? 0)}</dd></div>
    </dl>
  </header>

  <div class="workspace-toolbar">
    <div class="ui-tabs view-tabs" role="tablist" aria-label={t("blocks-scale-label")}>
      {#each blockViews as view, index (view.id)}
        <button
          id={`blocks-tab-${view.id}`}
          type="button"
          role="tab"
          aria-selected={activeView === view.id ? "true" : "false"}
          tabindex={activeView === view.id ? 0 : -1}
          class="ui-tab"
          class:active={activeView === view.id}
          onclick={() => selectView(view.id)}
          onkeydown={(event) => handleViewKeydown(event, index)}
        >{view.label}</button>
      {/each}
    </div>
    <label class="search-field">
      <span class="sr-only">{t("blocks-search-label")}</span>
      <IconSearch size={14} stroke={1.9} />
      <input class="ui-field toolbar" bind:value={query} type="search" placeholder={t("blocks-search-placeholder")} />
    </label>
    <button
      class="ui-button primary toolbar toolbar-action"
      type="button"
      disabled={!selectedDefinition?.capabilities.canInsert || !selectedPaletteElement}
      onclick={beginInsert}
    >
      <IconPlus size={14} stroke={2} /> {t("blocks-add")}
    </button>
  </div>

  <div class="workspace-body">
    <div class="resource-list" role="tabpanel" aria-labelledby={`blocks-tab-${activeView}`}>
      {#if !blockGraph}
        <div class="workspace-state">{t("blocks-loading")}</div>
      {:else}
        {#each filteredDefinitions as definition (definition.id)}
          <button
            type="button"
            class="resource-card ui-entity-selectable"
            data-ui-selected={selectedDefinition?.id === definition.id ? "true" : undefined}
            aria-pressed={selectedDefinition?.id === definition.id}
            onclick={() => selectDefinition(definition)}
          >
            <span class="resource-icon"><IconBox size={17} stroke={1.8} /></span>
            <span>
              <strong>{definition.displayName}</strong>
              <small>{definition.description}</small>
            </span>
            <span class="resource-badges">
              <code>{definition.scale}</code>
              <code>{definition.origin}</code>
            </span>
          </button>
        {:else}
          <div class="workspace-state">{t("blocks-empty-filter")}</div>
        {/each}
      {/if}
    </div>

    <aside class="resource-detail" aria-label={t("blocks-detail-label")}>
      {#if detailMode === "insert" && selectedDefinition && selectedPaletteElement}
        <header class="detail-heading">
          <div>
            <span class="detail-kicker">{t("blocks-prepare-insert")}</span>
            <h2>{selectedDefinition.displayName}</h2>
            <p>{t("blocks-insert-description")}</p>
          </div>
          <button class="ui-icon-button ui-close-button" type="button" aria-label={t("blocks-cancel")} onclick={() => { detailMode = "info"; }}>
            <IconX size={14} />
          </button>
        </header>
        <div class="target-card">
          <strong>{insertTarget
            ? t("blocks-after-target", { tag: insertTarget.observation.tag })
            : t("blocks-no-target")}</strong>
          <span>{insertTarget?.sourceLocation?.file ?? t("blocks-select-target-help")}</span>
        </div>
        <pre><code>{selectedPaletteElement.html}</code></pre>
        {#if loadError}<p class="form-error" role="alert"><IconAlertTriangle size={14} /> {loadError}</p>{/if}
        <div class="detail-actions">
          <button type="button" onclick={() => { detailMode = "info"; }}>{t("blocks-cancel")}</button>
          <button
            class="ui-button primary primary-action"
            type="button"
            disabled={!insertTarget?.sourceLocation || inserting}
            onclick={() => { void insertSelectedBlock(); }}
          >
            <IconPlus size={14} />
            {inserting ? t("blocks-inserting") : t("blocks-add-after-selection")}
          </button>
        </div>
      {:else if selectedDefinition}
        <div class="detail-kicker-row">
          <span class="detail-kicker">{selectedDefinition.scale}</span>
          <span>{selectedDefinition.origin}</span>
        </div>
        <h2>{selectedDefinition.displayName}</h2>
        <p>{selectedDefinition.description}</p>
        <dl class="block-contract">
          <div><dt>{t("blocks-provider")}</dt><dd>{selectedDefinition.providerId}</dd></div>
          <div><dt>{t("blocks-family")}</dt><dd>{selectedDefinition.familyId}</dd></div>
          <div><dt>{t("blocks-variant")}</dt><dd>{selectedDefinition.variantId}</dd></div>
          <div><dt>{t("blocks-version")}</dt><dd>{selectedDefinition.schemaVersion}</dd></div>
        </dl>
        <section class="detail-section">
          <h3>{t("blocks-instances")}</h3>
          <div class="semantic-row"><code>{t("blocks-source")}</code><span>{l10n.formatNumber(selectedSourceInstances.length)}</span></div>
          <div class="semantic-row"><code>{t("blocks-canvas")}</code><span>{l10n.formatNumber(selectedRenderedInstances.length)}</span></div>
        </section>
        {#if selectedSourceInstances.some((instance) => Boolean(instance.diagnostic))}
          <section class="detail-section diagnostics">
            <h3>{t("blocks-diagnostics")}</h3>
            {#each selectedSourceInstances.filter((instance) => instance.diagnostic) as instance}
              <p><IconAlertTriangle size={13} /> {errorMessage(instance.diagnostic)}</p>
            {/each}
          </section>
        {/if}
        {#if loadError}<p class="form-error" role="alert"><IconAlertTriangle size={14} /> {loadError}</p>{/if}
        <div class="detail-actions">
          <button
            class="ui-button primary primary-action"
            type="button"
            disabled={!selectedDefinition.capabilities.canInsert || !selectedPaletteElement}
            onclick={beginInsert}
          >
            <IconPlus size={14} /> {t("blocks-prepare-add")}
          </button>
          <button type="button" disabled title={t("blocks-readonly-title")}>
            <IconCode size={14} /> {t("blocks-readonly")}
          </button>
        </div>
      {:else}
        <div class="workspace-state">{t("blocks-select-help")}</div>
      {/if}
    </aside>
  </div>
</section>

<style>
  .resource-detail > p, .detail-heading p { margin: 4px 0 0; color: var(--wb-text-muted); font-size: 12px; line-height: 1.45; }
  dt { color: var(--wb-text-muted); font-size: var(--font-meta); font-weight: 800; text-transform: uppercase; }
  dd { margin: 3px 0 0; color: var(--text-strong); font-size: 16px; font-weight: 750; }
  .workspace-body { display: grid; grid-template-columns: minmax(330px, 1fr) minmax(330px, .62fr); min-width: 0; min-height: 0; }
  .resource-icon { display: grid; flex: 0 0 auto; width: 30px; height: 30px; place-items: center; border-radius: 7px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .resource-card > span:nth-child(2) { display: grid; flex: 1; gap: 3px; min-width: 0; }
  .resource-card strong { color: var(--text-strong); font-size: 12px; }
  .resource-card small { overflow: hidden; color: var(--wb-text-muted); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .resource-badges { display: grid; justify-items: end; gap: 3px; }
  .resource-badges code { padding: 2px 4px; border-radius: 4px; color: var(--wb-text-muted); background: var(--wb-surface-chrome); font-size: var(--font-meta); }
  .detail-kicker-row, .detail-heading, .detail-actions { display: flex; align-items: center; }
  .detail-kicker-row, .detail-heading { justify-content: space-between; gap: 12px; }
  .detail-heading { align-items: flex-start; }
  .detail-heading > button { display: grid; flex: 0 0 auto; width: 28px; height: 28px; padding: 0; place-items: center; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-muted); background: var(--wb-surface-document); }
  .detail-kicker { color: var(--wb-accent-strong); font-size: 11px; font-weight: 850; text-transform: uppercase; }
  .detail-kicker-row > span:last-child { padding: 3px 6px; border-radius: 999px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); font-size: var(--font-meta); font-weight: 750; }
  h2 { margin: 7px 0 0; color: var(--text-strong); font-size: 19px; }
  .block-contract { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 6px; margin: 14px 0 0; }
  .block-contract div { min-width: 0; padding: 7px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  .block-contract dd { overflow: hidden; font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .detail-section { margin-top: 14px; }
  .detail-section h3 { margin: 0 0 6px; color: var(--text-strong); font-size: 12px; }
  .semantic-row { display: grid; grid-template-columns: minmax(80px, .34fr) minmax(0, 1fr); gap: 8px; padding: 6px 0; border-top: 1px solid var(--wb-border-subtle); font-size: 11px; }
  .semantic-row code { color: var(--wb-accent-strong); }
  .semantic-row span { color: var(--wb-text-muted); }
  .diagnostics p, .form-error { display: flex; align-items: flex-start; gap: 6px; color: var(--danger); font-size: 11px; line-height: 1.4; }
  .form-error { margin: 9px 0 0; padding: 8px; border: 1px solid color-mix(in srgb, var(--danger) 36%, var(--wb-border-subtle)); border-radius: 6px; background: color-mix(in srgb, var(--danger) 7%, var(--wb-surface-document)); }
  .target-card { display: grid; gap: 3px; margin-top: 12px; padding: 9px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .target-card strong { color: var(--text-strong); font-size: 12px; }
  .target-card span { color: var(--wb-text-muted); font-size: 11px; overflow-wrap: anywhere; }
  pre { max-height: 260px; margin: 12px 0 0; padding: 10px; overflow: auto; border: 1px solid var(--wb-border-subtle); border-radius: 7px; color: var(--wb-text-muted); background: var(--surface-7); font-size: 11px; line-height: 1.45; white-space: pre-wrap; }
  .detail-actions { flex-wrap: wrap; align-items: stretch; gap: 7px; margin-top: 14px; }
  button:disabled { opacity: .5; cursor: not-allowed; }
  .workspace-state { display: grid; min-height: 120px; place-items: center; padding: 20px; color: var(--wb-text-muted); font-size: 12px; text-align: center; }
  .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap; }
</style>
