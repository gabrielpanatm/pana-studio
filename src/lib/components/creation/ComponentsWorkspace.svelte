<script lang="ts">
  import {
    IconBraces,
    IconBrandJavascript,
    IconCode,
    IconExternalLink,
    IconFileCode,
    IconPlus,
    IconSearch,
  } from "@tabler/icons-svelte";
  import LoopBuilderPanel from "$lib/components/creation/LoopBuilderPanel.svelte";
  import { pageComponentPaletteGroupsFromRegistry } from "$lib/page-components/registry";
  import { readPageComponentRegistry } from "$lib/project/io";
  import type { AppState } from "$lib/state/app.svelte";
  import type { PageComponentRegistrySnapshot, SourceGraphTemplate } from "$lib/types";

  let {
    app,
    openWorkspaceSource,
  }: {
    app: AppState;
    openWorkspaceSource: (path: string) => void | Promise<void>;
  } = $props();

  type ComponentView = "library" | "partials" | "macros" | "loops";
  const componentViews: { id: ComponentView; label: string }[] = [
    { id: "library", label: "Bibliotecă" },
    { id: "partials", label: "Partials" },
    { id: "macros", label: "Macrocomenzi" },
    { id: "loops", label: "Liste dinamice" },
  ];

  let activeView = $state<ComponentView>("library");
  let registry = $state<PageComponentRegistrySnapshot | null>(null);
  let loading = $state(false);
  let loadError = $state("");
  let query = $state("");
  let selectedId = $state<string | null>(null);
  let insertingId = $state<string | null>(null);

  const paletteComponents = $derived(
    pageComponentPaletteGroupsFromRegistry(registry).flatMap((group) => group.elements),
  );
  const partials = $derived(
    (app.sourceGraph?.templates ?? []).filter((template) => template.isPartial),
  );
  const macros = $derived(
    (app.sourceGraph?.templates ?? []).flatMap((template) => (
      template.macros.map((macro) => ({
        id: `${template.id}:${macro}`,
        name: macro,
        template,
      }))
    )),
  );
  const normalizedQuery = $derived(query.trim().toLocaleLowerCase("ro"));
  const filteredComponents = $derived(
    paletteComponents.filter((component) => (
      !normalizedQuery
      || `${component.label} ${component.description} ${component.componentKind ?? ""}`
        .toLocaleLowerCase("ro")
        .includes(normalizedQuery)
    )),
  );
  const filteredPartials = $derived(
    partials.filter((template) => (
      !normalizedQuery
      || `${template.name} ${template.file}`.toLocaleLowerCase("ro").includes(normalizedQuery)
    )),
  );
  const filteredMacros = $derived(
    macros.filter((macro) => (
      !normalizedQuery
      || `${macro.name} ${macro.template.file}`.toLocaleLowerCase("ro").includes(normalizedQuery)
    )),
  );
  const selectedComponent = $derived(
    paletteComponents.find((component) => component.id === selectedId) ?? paletteComponents[0] ?? null,
  );
  const selectedTarget = $derived(app.selectedElement);

  $effect(() => {
    const sessionId = app.kernelProjectSessionId;
    if (!sessionId || registry || loading) return;
    loading = true;
    loadError = "";
    void readPageComponentRegistry()
      .then((snapshot) => {
        if (app.kernelProjectSessionId !== sessionId) return;
        registry = snapshot;
        selectedId = snapshot.components[0]?.id ?? null;
      })
      .catch((error) => {
        if (app.kernelProjectSessionId === sessionId) {
          loadError = error instanceof Error ? error.message : String(error);
        }
      })
      .finally(() => {
        if (app.kernelProjectSessionId === sessionId) loading = false;
      });
  });

  function usageCount(template: SourceGraphTemplate) {
    return (app.sourceGraph?.relations ?? []).filter((relation) => (
      relation.to === template.nodeId
      && (relation.kind === "includes" || relation.kind === "imports")
    )).length;
  }

  async function addAfterSelection(componentId: string) {
    const target = app.selectedElement;
    const element = paletteComponents.find((candidate) => candidate.id === componentId);
    if (!target?.sourceLocation || !element || insertingId) return;
    insertingId = componentId;
    try {
      await app.insertPaletteElementAtTarget({
        targetSelector: target.domPath || target.cssSelector || target.selector,
        targetSessionId: target.sessionId,
        targetSourceId: target.sourceId,
        targetTemplateSourceId: target.templateSourceId,
        targetSourceLocation: target.sourceLocation,
        targetTag: target.tag,
        position: "after",
        element,
      });
      await app.setWorkbenchActivity("editor");
    } finally {
      insertingId = null;
    }
  }

  function selectView(view: ComponentView) {
    activeView = view;
  }

  function handleViewKeydown(event: KeyboardEvent, index: number) {
    let nextIndex: number | null = null;
    if (event.key === "ArrowLeft") nextIndex = (index - 1 + componentViews.length) % componentViews.length;
    if (event.key === "ArrowRight") nextIndex = (index + 1) % componentViews.length;
    if (event.key === "Home") nextIndex = 0;
    if (event.key === "End") nextIndex = componentViews.length - 1;
    if (nextIndex === null) return;
    event.preventDefault();
    const next = componentViews[nextIndex];
    if (!next) return;
    selectView(next.id);
    requestAnimationFrame(() => document.getElementById(`components-tab-${next.id}`)?.focus());
  }
</script>

<section class="creation-workspace" aria-labelledby="components-title">
  <header class="workspace-header">
    <div>
      <span class="eyebrow"><IconBraces size={15} stroke={1.9} /> Atelier de creare</span>
      <h1 id="components-title">Componente</h1>
      <p>Biblioteca interactivă, parțialele și macro-urile sunt proiectate din registrul și harta surselor Rust.</p>
    </div>
    <dl>
      <div><dt>Bibliotecă</dt><dd>{paletteComponents.length}</dd></div>
      <div><dt>Partials</dt><dd>{partials.length}</dd></div>
      <div><dt>Macros</dt><dd>{macros.length}</dd></div>
      <div><dt>Liste</dt><dd>{app.loopDefinitions.length}</dd></div>
    </dl>
  </header>

  <div class="workspace-toolbar">
    <div class="ui-tabs view-tabs" role="tablist" aria-label="Categorii componente">
      {#each componentViews as view, index (view.id)}
        <button
          id={`components-tab-${view.id}`}
          type="button"
          role="tab"
          aria-selected={activeView === view.id ? "true" : "false"}
          aria-controls={`components-panel-${view.id}`}
          tabindex={activeView === view.id ? 0 : -1}
          class="ui-tab"
          class:active={activeView === view.id}
          onclick={() => selectView(view.id)}
          onkeydown={(event) => handleViewKeydown(event, index)}
        >{view.label}</button>
      {/each}
    </div>
    {#if activeView !== "loops"}
      <label class="search-field">
        <span class="sr-only">Caută componente</span>
        <IconSearch size={14} stroke={1.9} />
        <input class="ui-field" bind:value={query} type="search" placeholder="Caută în componente" />
      </label>
    {/if}
  </div>

  <div class:loops-view={activeView === "loops"} class="workspace-body">
    <div class="resource-list" id={`components-panel-${activeView}`} role="tabpanel" aria-labelledby={`components-tab-${activeView}`}>
      {#if loadError}
        <div class="ui-message error workspace-state" role="alert">{loadError}</div>
      {:else if loading && !registry}
        <div class="workspace-state">Se citește registrul Rust…</div>
      {:else if activeView === "library"}
        {#each filteredComponents as component (component.id)}
          <button
            type="button"
            class="resource-card"
            class:selected={selectedComponent?.id === component.id}
            onclick={() => { selectedId = component.id; }}
          >
            <span class="resource-icon"><IconBrandJavascript size={17} stroke={1.8} /></span>
            <span><strong>{component.label}</strong><small>{component.description}</small></span>
            <code>{component.componentKind?.toUpperCase() ?? "JS"}</code>
          </button>
        {/each}
      {:else if activeView === "partials"}
        {#each filteredPartials as partial (partial.id)}
          <article class="source-card">
            <span class="resource-icon"><IconFileCode size={17} stroke={1.8} /></span>
            <div><strong>{partial.name}</strong><small>{partial.file}</small></div>
            <span>{usageCount(partial)} utilizări</span>
            <button type="button" onclick={() => { void openWorkspaceSource(partial.file); }}>Deschide <IconExternalLink size={13} stroke={1.9} /></button>
          </article>
        {:else}
          <div class="workspace-state">Nu există partials pentru filtrul curent.</div>
        {/each}
      {:else if activeView === "macros"}
        {#each filteredMacros as macro (macro.id)}
          <article class="source-card">
            <span class="resource-icon"><IconCode size={17} stroke={1.8} /></span>
            <div><strong>{macro.name}</strong><small>{macro.template.file}</small></div>
            <span>{usageCount(macro.template)} importuri</span>
            <button type="button" onclick={() => { void openWorkspaceSource(macro.template.file); }}>Deschide <IconExternalLink size={13} stroke={1.9} /></button>
          </article>
        {:else}
          <div class="workspace-state">Nu există macros pentru filtrul curent.</div>
        {/each}
      {:else}
        <LoopBuilderPanel
          definitions={app.loopDefinitions}
          register={(definition) => app.registerLoopDefinition(definition)}
          remove={(id) => app.removeLoopDefinition(id)}
        />
      {/if}
    </div>

    {#if activeView !== "loops"}<aside class="resource-detail" aria-label="Detalii componentă">
      {#if activeView === "library" && selectedComponent}
        <span class="detail-kicker">Componentă {selectedComponent.componentKind?.toUpperCase() ?? "JS"}</span>
        <h2>{selectedComponent.label}</h2>
        <p>{selectedComponent.description}</p>
        <dl class="component-contract">
          <div><dt>Element rădăcină</dt><dd>&lt;{selectedComponent.tag}&gt;</dd></div>
          <div><dt>Clasă contract</dt><dd>.{selectedComponent.className}</dd></div>
          <div><dt>Execuție</dt><dd>JavaScript local al paginii</dd></div>
        </dl>
        <pre aria-label={`HTML pentru ${selectedComponent.label}`}><code>{selectedComponent.html}</code></pre>
        <div class="target-card">
          <strong>{selectedTarget ? `Țintă: <${selectedTarget.tag}>` : "Selectează întâi un element în Editor"}</strong>
          <span>{selectedTarget?.sourceLocation?.file ?? "Inserarea controlată are nevoie de o locație sursă stabilă."}</span>
        </div>
        <button
          class="primary-action"
          type="button"
          disabled={!selectedTarget?.sourceLocation || Boolean(insertingId)}
          onclick={() => { void addAfterSelection(selectedComponent.id); }}
        >
          <IconPlus size={15} stroke={2} />
          {insertingId === selectedComponent.id ? "Se adaugă prin Rust…" : "Adaugă după selecție"}
        </button>
      {:else}
        <span class="detail-kicker">Harta surselor</span>
        <h2>{activeView === "partials" ? "Partials Tera" : "Macros Tera"}</h2>
        <p>Deschide o sursă pentru editare. Numărul de utilizări vine din relațiile indexate de Rust.</p>
      {/if}
    </aside>{/if}
  </div>
</section>

<style>
  .creation-workspace { display: grid; grid-template-rows: auto 42px minmax(0, 1fr); min-width: 0; min-height: 0; height: 100%; overflow: hidden; border: 1px solid var(--wb-border-subtle); border-radius: 10px; color: var(--wb-text-primary); background: var(--wb-surface-document); box-shadow: var(--shadow); }
  .workspace-header { display: flex; align-items: center; justify-content: space-between; gap: 24px; padding: 17px 20px; border-bottom: 1px solid var(--wb-border-subtle); background: radial-gradient(circle at 18% 0%, var(--wb-accent-soft), transparent 36%), var(--wb-surface-chrome); }
  .eyebrow { display: inline-flex; align-items: center; gap: 6px; color: var(--wb-accent-strong); font-size: 12px; font-weight: 850; letter-spacing: .06em; text-transform: uppercase; }
  h1 { margin: 6px 0 0; color: var(--text-strong); font-size: 24px; letter-spacing: -.025em; }
  .workspace-header p { margin: 5px 0 0; color: var(--wb-text-muted); font-size: 12px; }
  .workspace-header > dl { display: flex; gap: 7px; margin: 0; }
  .workspace-header > dl div { min-width: 76px; padding: 7px 9px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  dt { color: var(--wb-text-muted); font-size: 12px; font-weight: 800; text-transform: uppercase; }
  dd { margin: 3px 0 0; color: var(--text-strong); font-size: 15px; font-weight: 800; }
  .workspace-toolbar, .view-tabs, .search-field, .resource-card, .source-card, .source-card > button, .primary-action { display: flex; align-items: center; }
  .workspace-toolbar { justify-content: space-between; gap: 10px; padding: 5px 9px; border-bottom: 1px solid var(--wb-border-subtle); background: var(--wb-surface-chrome); }
  .view-tabs { align-self: stretch; gap: 2px; }
  .view-tabs button { height: 100%; padding: 0 10px; border: 0; border-bottom: 2px solid transparent; color: var(--wb-text-muted); background: transparent; font-size: 12px; font-weight: 800; }
  .view-tabs button.active { border-bottom-color: var(--wb-accent); color: var(--wb-text-primary); }
  .search-field { position: relative; width: min(280px, 35vw); }
  .search-field :global(svg) { position: absolute; left: 8px; color: var(--wb-text-muted); }
  .search-field input { width: 100%; height: 28px; padding: 0 8px 0 28px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; }
  .workspace-body { display: grid; grid-template-columns: minmax(300px, 1fr) minmax(280px, .58fr); min-width: 0; min-height: 0; }
  .workspace-body.loops-view { grid-template-columns: minmax(0, 1fr); }
  .workspace-body.loops-view .resource-list { padding: 0; border-right: 0; }
  .resource-list { min-width: 0; min-height: 0; overflow: auto; padding: 9px; border-right: 1px solid var(--wb-border-subtle); }
  .resource-card { width: 100%; gap: 9px; min-height: 54px; padding: 7px 9px; border: 1px solid transparent; border-radius: 7px; color: var(--wb-text-primary); background: transparent; text-align: left; }
  .resource-card:hover, .resource-card.selected { border-color: var(--wb-border-subtle); background: var(--wb-control-hover); }
  .resource-card.selected { box-shadow: inset 3px 0 0 var(--wb-accent); }
  .resource-icon { display: grid; flex: 0 0 auto; width: 30px; height: 30px; place-items: center; border-radius: 7px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .resource-card > span:nth-child(2), .source-card > div { display: grid; flex: 1; gap: 3px; min-width: 0; }
  .resource-card strong, .source-card strong { color: var(--text-strong); font-size: 12px; }
  .resource-card small, .source-card small { overflow: hidden; color: var(--wb-text-muted); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .resource-card code { color: var(--wb-text-muted); font-size: 12px; }
  .source-card { display: grid; grid-template-columns: 34px minmax(0, 1fr) auto auto; gap: 8px; min-height: 52px; padding: 7px 9px; border-bottom: 1px solid var(--wb-border-subtle); }
  .source-card > span:nth-last-child(2) { align-self: center; color: var(--wb-text-muted); font-size: 12px; }
  .source-card > button { align-self: center; gap: 4px; min-height: 26px; padding: 0 8px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-primary); background: var(--wb-surface-chrome); font-size: 12px; }
  .resource-detail { min-width: 0; min-height: 0; overflow: auto; padding: 17px; background: var(--wb-surface-chrome); }
  .detail-kicker { color: var(--wb-accent-strong); font-size: 12px; font-weight: 850; text-transform: uppercase; }
  h2 { margin: 7px 0 0; color: var(--text-strong); font-size: 19px; }
  .resource-detail > p { margin: 6px 0 0; color: var(--wb-text-muted); font-size: 12px; line-height: 1.5; }
  .component-contract { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 6px; margin: 14px 0 0; }
  .component-contract div { min-width: 0; padding: 7px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  .component-contract dd { overflow: hidden; font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  pre { max-height: 220px; margin: 10px 0 0; padding: 10px; overflow: auto; border: 1px solid var(--wb-border-subtle); border-radius: 7px; color: var(--wb-text-muted); background: var(--surface-7); font-size: 12px; line-height: 1.45; white-space: pre-wrap; }
  .target-card { display: grid; gap: 3px; margin-top: 10px; padding: 9px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .target-card strong { color: var(--text-strong); font-size: 12px; }
  .target-card span { overflow: hidden; color: var(--wb-text-muted); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .primary-action { justify-content: center; gap: 6px; width: 100%; min-height: 32px; margin-top: 8px; border: 1px solid var(--wb-accent); border-radius: 6px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); font-size: 12px; font-weight: 800; }
  button:disabled { opacity: .5; }
  button:not(:disabled) { cursor: pointer; }
  button:focus-visible, input:focus-visible { outline: 2px solid var(--wb-focus-ring); outline-offset: 1px; }
  .workspace-state { display: grid; min-height: 180px; place-items: center; color: var(--wb-text-muted); font-size: 12px; text-align: center; }
  .workspace-state.error { color: var(--danger); }
  .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; }
  @media (max-width: 900px) { .workspace-body { grid-template-columns: 1fr; } .resource-detail { display: none; } .resource-list { border-right: 0; } }
</style>
