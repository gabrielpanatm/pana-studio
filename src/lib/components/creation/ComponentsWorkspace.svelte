<script lang="ts">
  import {
    IconAlertTriangle,
    IconBraces,
    IconCopy,
    IconDeviceFloppy,
    IconEdit,
    IconExternalLink,
    IconFileCode,
    IconGitBranch,
    IconPlus,
    IconSearch,
    IconTrash,
    IconX,
  } from "@tabler/icons-svelte";
  import {
    applyComponentMutation,
    readFileBufferText,
  } from "$lib/project/io";
  import type { AppState } from "$lib/state/app.svelte";
  import type {
    ComponentCompanionDraft,
    ComponentDefinition,
    ComponentDefinitionKind,
    ComponentDraftKind,
    ComponentMutationInput,
    FileBufferRequestIdentity,
  } from "$lib/types";
  import { errorMessage } from "$lib/util";

  let {
    app,
    openWorkspaceSource,
  }: {
    app: AppState;
    openWorkspaceSource: (path: string) => void | Promise<void>;
  } = $props();

  type ComponentView = "all" | "partials" | "macros" | "shortcodes" | "repeats";
  type DetailMode = "info" | "create" | "edit";

  const componentViews: Array<{ id: ComponentView; label: string }> = [
    { id: "all", label: "Toate" },
    { id: "partials", label: "Parțiale" },
    { id: "macros", label: "Macro-uri" },
    { id: "shortcodes", label: "Shortcode-uri" },
    { id: "repeats", label: "Liste Tera" },
  ];

  let activeView = $state<ComponentView>("all");
  let detailMode = $state<DetailMode>("info");
  let selectedDefinitionId = $state("");
  let query = $state("");
  let formError = $state("");
  let mutating = $state(false);
  let loadingSource = $state(false);
  let deleteConfirmationOpen = $state(false);
  let graphRefreshKey = "";

  let formKind = $state<ComponentDraftKind>("partial");
  let formName = $state("");
  let formSource = $state("");
  let formStylePath = $state("");
  let formStyleSource = $state("");
  let formScriptPath = $state("");
  let formScriptSource = $state("");
  let formDataPath = $state("");
  let formDataSource = $state("");

  const componentGraph = $derived(app.sourceGraph?.componentGraph ?? null);
  const definitions = $derived(
    (componentGraph?.definitions ?? []).filter((definition) => (
      definition.kind !== "templateFile" && definition.kind !== "templateBlock"
    )),
  );
  const normalizedQuery = $derived(query.trim().toLocaleLowerCase("ro"));
  const filteredDefinitions = $derived(
    definitions.filter((definition) => (
      definitionMatchesView(definition, activeView)
      && (
        !normalizedQuery
        || [
          definition.displayName,
          definition.name,
          definition.file ?? "",
          definition.templateName ?? "",
          definition.symbol ?? "",
          definition.origin,
        ].join(" ").toLocaleLowerCase("ro").includes(normalizedQuery)
      )
    )),
  );
  const selectedDefinition = $derived(
    definitions.find((definition) => definition.id === selectedDefinitionId)
      ?? filteredDefinitions[0]
      ?? null,
  );
  const selectedInvocations = $derived(
    selectedDefinition
      ? (componentGraph?.invocations ?? []).filter((invocation) => (
        invocation.resolvedDefinitionIds.includes(selectedDefinition.id)
      ))
      : [],
  );
  const projectDefinitionCount = $derived(
    definitions.filter((definition) => definition.origin === "project").length,
  );
  const themeDefinitionCount = $derived(
    definitions.filter((definition) => definition.origin === "theme").length,
  );

  $effect(() => {
    const projectRoot = app.sessionProjectRoot;
    const sessionId = app.kernelProjectSessionId;
    const revision = app.projectWorkspaceSnapshot?.revision;
    if (!projectRoot || !sessionId || revision === undefined) return;
    const key = `${projectRoot}\u0000${sessionId}\u0000${revision}`;
    if (key === graphRefreshKey) return;
    graphRefreshKey = key;
    void app.refreshSourceGraph({ strict: true }).catch((cause) => {
      if (graphRefreshKey === key) formError = errorMessage(cause);
    });
  });

  function identity(): FileBufferRequestIdentity {
    return {
      expectedProjectRoot: app.sessionProjectRoot,
      expectedSessionId: app.kernelProjectSessionId,
    };
  }

  function definitionMatchesView(definition: ComponentDefinition, view: ComponentView) {
    if (view === "all") return true;
    if (view === "partials") return definition.kind === "partial";
    if (view === "macros") return definition.kind === "macroLibrary" || definition.kind === "macro";
    if (view === "shortcodes") return definition.kind === "shortcode";
    if (view === "repeats") return definition.kind === "inlineRepeat";
    return false;
  }

  function isMutableFileDefinition(definition: ComponentDefinition | null) {
    return Boolean(
      definition?.file
      && ["partial", "macroLibrary", "shortcode"].includes(definition.kind),
    );
  }

  function kindLabel(kind: ComponentDefinitionKind) {
    const labels: Record<ComponentDefinitionKind, string> = {
      templateFile: "Șablon",
      partial: "Parțială",
      macroLibrary: "Bibliotecă macro",
      macro: "Macro",
      shortcode: "Shortcode",
      templateBlock: "Bloc",
      inlineRepeat: "Listă Tera",
    };
    return labels[kind];
  }

  function originLabel(origin: ComponentDefinition["origin"]) {
    if (origin === "project") return "Proiect";
    return "Temă";
  }

  function iconForDefinition(definition: ComponentDefinition) {
    if (definition.kind === "inlineRepeat") return IconGitBranch;
    if (definition.kind === "shortcode") return IconBraces;
    return IconFileCode;
  }

  function logicalName(definition: ComponentDefinition) {
    const name = definition.templateName ?? definition.name;
    return name
      .replace(/^(partials|macros|shortcodes)\//, "")
      .replace(/\.(html|md)$/i, "");
  }

  function resetPanel() {
    detailMode = "info";
    formError = "";
    deleteConfirmationOpen = false;
    loadingSource = false;
  }

  function selectView(view: ComponentView) {
    activeView = view;
    selectedDefinitionId = "";
    query = "";
    resetPanel();
  }

  function selectDefinition(id: string) {
    selectedDefinitionId = id;
    resetPanel();
  }

  function kindForView(): ComponentDraftKind {
    if (activeView === "macros") return "macro_library";
    if (activeView === "shortcodes") return "shortcode_html";
    return "partial";
  }

  function defaultSource(kind: ComponentDraftKind, name: string) {
    const safeName = name.trim().split("/").at(-1)?.replace(/\.(?:html|md)$/i, "") || "componenta";
    if (kind === "macro_library") {
      return `{% macro ${safeName}(text) %}\n  <span>{{ text }}</span>\n{% endmacro ${safeName} %}\n`;
    }
    if (kind === "shortcode_markdown") return "**Shortcode nou**\n";
    if (kind === "shortcode_html") {
      return `<span class="shortcode-${safeName}">Shortcode nou</span>\n`;
    }
    return `<section class="${safeName}">\n  Componentă nouă\n</section>\n`;
  }

  function beginCreate() {
    formError = "";
    deleteConfirmationOpen = false;
    formKind = kindForView();
    formName = formKind === "macro_library"
      ? "macros-noi"
      : formKind.startsWith("shortcode")
        ? "shortcode-nou"
        : "componenta-noua";
    formSource = defaultSource(formKind, formName);
    formStylePath = "";
    formStyleSource = "";
    formScriptPath = "";
    formScriptSource = "";
    formDataPath = "";
    formDataSource = "";
    detailMode = "create";
  }

  function updateCreateKind(value: string) {
    formKind = value as ComponentDraftKind;
    formSource = defaultSource(formKind, formName);
  }

  async function beginEdit() {
    if (!selectedDefinition?.file || selectedDefinition.origin !== "project") return;
    if (!isMutableFileDefinition(selectedDefinition)) {
      await openWorkspaceSource(selectedDefinition.file);
      return;
    }
    formError = "";
    deleteConfirmationOpen = false;
    loadingSource = true;
    detailMode = "edit";
    formName = logicalName(selectedDefinition);
    try {
      const snapshot = await readFileBufferText(selectedDefinition.file, identity());
      if (selectedDefinitionId && selectedDefinition.id !== selectedDefinitionId) return;
      formSource = snapshot.text;
    } catch (cause) {
      formError = errorMessage(cause);
    } finally {
      loadingSource = false;
    }
  }

  function createCompanions(): ComponentCompanionDraft[] {
    const companions: ComponentCompanionDraft[] = [];
    if (formStylePath.trim()) {
      companions.push({
        kind: "style",
        relativePath: formStylePath.trim(),
        contents: formStyleSource,
        createOnly: true,
      });
    }
    if (formScriptPath.trim()) {
      companions.push({
        kind: "script",
        relativePath: formScriptPath.trim(),
        contents: formScriptSource,
        createOnly: true,
      });
    }
    if (formDataPath.trim()) {
      companions.push({
        kind: "data",
        relativePath: formDataPath.trim(),
        contents: formDataSource,
        createOnly: true,
      });
    }
    return companions;
  }

  async function applyMutation(input: ComponentMutationInput, successMessage: string) {
    const receipt = await applyComponentMutation(input, identity());
    await app.rescanCurrentProject(receipt.workspace.relativePath, { strict: true });
    const destination = receipt.plan.destinationRelativePath;
    if (destination) {
      selectedDefinitionId = app.sourceGraph?.componentGraph.definitions.find((definition) => (
        definition.file === destination && definition.active
      ))?.id ?? "";
    } else {
      selectedDefinitionId = "";
    }
    app.setGlobalStatus(`${successMessage} — Ctrl+S persistă pe disc`, "unsaved");
    resetPanel();
  }

  async function submitComponent() {
    if (mutating) return;
    formError = "";
    mutating = true;
    try {
      if (detailMode === "create") {
        await applyMutation({
          operation: "create",
          definitionId: null,
          kind: formKind,
          name: formName,
          destinationName: null,
          contents: formSource,
          sourceFile: null,
          sourceRange: null,
          companions: createCompanions(),
        }, `Componentă creată: ${formName}`);
      } else if (detailMode === "edit" && selectedDefinition) {
        await applyMutation({
          operation: "update",
          definitionId: selectedDefinition.id,
          kind: null,
          name: null,
          destinationName: formName,
          contents: formSource,
          sourceFile: null,
          sourceRange: null,
          companions: [],
        }, `Componentă actualizată: ${formName}`);
      }
    } catch (cause) {
      formError = errorMessage(cause);
    } finally {
      mutating = false;
    }
  }

  async function duplicateSelected() {
    if (!selectedDefinition || mutating) return;
    formError = "";
    mutating = true;
    try {
      await applyMutation({
        operation: "duplicate",
        definitionId: selectedDefinition.id,
        kind: null,
        name: null,
        destinationName: `${logicalName(selectedDefinition)}-copie`,
        contents: null,
        sourceFile: null,
        sourceRange: null,
        companions: [],
      }, `Componentă duplicată: ${selectedDefinition.displayName}`);
    } catch (cause) {
      formError = errorMessage(cause);
    } finally {
      mutating = false;
    }
  }

  async function overrideSelected() {
    if (!selectedDefinition || mutating) return;
    formError = "";
    mutating = true;
    try {
      await applyMutation({
        operation: "override_theme",
        definitionId: selectedDefinition.id,
        kind: null,
        name: null,
        destinationName: null,
        contents: null,
        sourceFile: null,
        sourceRange: null,
        companions: [],
      }, `Override local creat: ${selectedDefinition.displayName}`);
    } catch (cause) {
      formError = errorMessage(cause);
    } finally {
      mutating = false;
    }
  }

  async function deleteSelected() {
    if (!selectedDefinition || mutating) return;
    formError = "";
    mutating = true;
    try {
      await applyMutation({
        operation: "delete",
        definitionId: selectedDefinition.id,
        kind: null,
        name: null,
        destinationName: null,
        contents: null,
        sourceFile: null,
        sourceRange: null,
        companions: [],
      }, `Componentă eliminată: ${selectedDefinition.displayName}`);
    } catch (cause) {
      formError = errorMessage(cause);
    } finally {
      mutating = false;
    }
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

<section class="components-workspace" aria-labelledby="components-title">
  <header class="workspace-header">
    <div>
      <span class="eyebrow"><IconBraces size={15} stroke={1.9} /> Graph semantic Rust</span>
      <h1 id="components-title">Componente</h1>
      <p>Definițiile, invocările, datele și dependențele sunt proiectate direct din sursele Zola și Tera.</p>
    </div>
    <dl>
      <div><dt>Definiții</dt><dd>{definitions.length}</dd></div>
      <div><dt>Proiect</dt><dd>{projectDefinitionCount}</dd></div>
      <div><dt>Temă</dt><dd>{themeDefinitionCount}</dd></div>
      <div><dt>Invocări</dt><dd>{componentGraph?.invocations.length ?? 0}</dd></div>
    </dl>
  </header>

  <div class="workspace-toolbar">
    <div class="ui-tabs view-tabs" role="tablist" aria-label="Tipuri de componente">
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
    <label class="search-field">
      <span class="sr-only">Caută componente</span>
      <IconSearch size={14} stroke={1.9} />
      <input class="ui-field compact" bind:value={query} type="search" placeholder="Caută definiții, fișiere sau simboluri" />
    </label>
    <button
      class="ui-button primary compact toolbar-action"
      type="button"
      disabled={mutating || activeView === "repeats"}
      title={activeView === "repeats" ? "Listele Tera sunt derivate din blocurile for reale din Editor." : ""}
      onclick={beginCreate}
    >
      <IconPlus size={14} stroke={2} /> Adaugă
    </button>
  </div>

  <div class="workspace-body">
    <div
      class="resource-list"
      id={`components-panel-${activeView}`}
      role="tabpanel"
      aria-labelledby={`components-tab-${activeView}`}
    >
      {#if !componentGraph}
        <div class="workspace-state">Se construiește ComponentGraph…</div>
      {:else}
        {#each filteredDefinitions as definition (definition.id)}
          {@const DefinitionIcon = iconForDefinition(definition)}
          <button
            type="button"
            class="resource-card"
            class:selected={selectedDefinition?.id === definition.id}
            class:shadowed={!definition.active}
            onclick={() => selectDefinition(definition.id)}
          >
            <span class="resource-icon"><DefinitionIcon size={17} stroke={1.8} /></span>
            <span>
              <strong>{definition.displayName}</strong>
              <small>{definition.file ?? definition.name}</small>
            </span>
            <span class="resource-badges">
              <code>{kindLabel(definition.kind)}</code>
              <code>{originLabel(definition.origin)}</code>
            </span>
          </button>
        {:else}
          <div class="workspace-state">Nu există definiții pentru filtrul curent.</div>
        {/each}
      {/if}
    </div>

    <aside class="resource-detail" aria-label="Informații și editare componentă">
      {#if detailMode === "create" || detailMode === "edit"}
        <form class="component-form" onsubmit={(event) => { event.preventDefault(); void submitComponent(); }}>
          <header class="detail-heading">
            <div>
              <span class="detail-kicker">{detailMode === "create" ? "Definiție nouă" : "Editare atomică"}</span>
              <h2>{detailMode === "create" ? "Creează componentă" : selectedDefinition?.displayName}</h2>
              <p>Rust validează candidatul complet înainte să creeze o singură intrare Undo/Redo.</p>
            </div>
            <button type="button" aria-label="Renunță" disabled={mutating} onclick={resetPanel}><IconX size={14} /></button>
          </header>

          {#if detailMode === "create"}
            <label>
              <span>Tip</span>
              <select value={formKind} disabled={mutating} onchange={(event) => updateCreateKind(event.currentTarget.value)}>
                <option value="partial">Parțială Tera</option>
                <option value="macro_library">Bibliotecă macro</option>
                <option value="shortcode_html">Shortcode HTML</option>
                <option value="shortcode_markdown">Shortcode Markdown</option>
              </select>
            </label>
          {/if}
          <label>
            <span>Nume logic</span>
            <input bind:value={formName} disabled={mutating || loadingSource} placeholder="catalog/card" />
            <small>Folderul semantic și extensia sunt stabilite de tip; subdirectoarele sunt permise.</small>
          </label>
          <label>
            <span>Sursă {formKind === "shortcode_markdown" ? "Markdown + Tera" : "HTML + Tera"}</span>
            <textarea bind:value={formSource} disabled={mutating || loadingSource} spellcheck="false"></textarea>
          </label>

          {#if detailMode === "create"}
            <details>
              <summary>Resurse companion în aceeași tranzacție</summary>
              <div class="companion-fields">
                <label>
                  <span>Stil SCSS/CSS</span>
                  <input bind:value={formStylePath} disabled={mutating} placeholder="sass/componente/_card.scss" />
                  <textarea bind:value={formStyleSource} disabled={mutating} spellcheck="false" placeholder={".card { }"}></textarea>
                </label>
                <label>
                  <span>Script</span>
                  <input bind:value={formScriptPath} disabled={mutating} placeholder="static/js/card.js" />
                  <textarea bind:value={formScriptSource} disabled={mutating} spellcheck="false" placeholder="// JavaScript opțional"></textarea>
                </label>
                <label>
                  <span>Date TOML canonice</span>
                  <input bind:value={formDataPath} disabled={mutating} placeholder="date/card.toml" />
                  <textarea bind:value={formDataSource} disabled={mutating} spellcheck="false" placeholder="[[items]]"></textarea>
                </label>
              </div>
            </details>
          {/if}

          {#if formError}<p class="ui-message error form-error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>{/if}
          <div class="form-actions">
            <button type="button" disabled={mutating} onclick={resetPanel}>Renunță</button>
            <button class="primary" type="submit" disabled={mutating || loadingSource || !formName.trim()}>
              <IconDeviceFloppy size={14} />
              {mutating ? "Se validează…" : detailMode === "create" ? "Creează în sesiune" : "Salvează modificările"}
            </button>
          </div>
        </form>
      {:else if selectedDefinition}
        <div class="detail-kicker-row">
          <span class="detail-kicker">{kindLabel(selectedDefinition.kind)}</span>
          <span class:inactive={!selectedDefinition.active}>
            {selectedDefinition.active ? originLabel(selectedDefinition.origin) : "Înlocuită"}
          </span>
        </div>
        <h2>{selectedDefinition.displayName}</h2>
        <p>{selectedDefinition.file ?? selectedDefinition.name}</p>

        <dl class="component-contract">
          <div><dt>Utilizări</dt><dd>{selectedInvocations.length}</dd></div>
          <div><dt>Parametri</dt><dd>{selectedDefinition.parameters.length}</dd></div>
          <div><dt>Dependențe</dt><dd>{selectedDefinition.dependencies.length}</dd></div>
          <div><dt>Binding-uri</dt><dd>{selectedDefinition.dataBindings.length}</dd></div>
        </dl>

        {#if selectedDefinition.parameters.length}
          <section class="detail-section">
            <h3>Parametri</h3>
            {#each selectedDefinition.parameters as parameter (parameter.name)}
              <div class="semantic-row">
                <code>{parameter.name}</code>
                <span>{parameter.required ? "obligatoriu" : "opțional"}</span>
              </div>
            {/each}
          </section>
        {/if}

        {#if selectedDefinition.dataBindings.length || selectedDefinition.contextDependencies.length}
          <section class="detail-section">
            <h3>Date și context</h3>
            {#each selectedDefinition.dataBindings as binding (`${binding.name}:${binding.path}`)}
              <div class="semantic-row"><code>{binding.name}</code><span>{binding.path} · {binding.producer}</span></div>
            {/each}
            {#each selectedDefinition.contextDependencies as dependency (dependency)}
              <div class="semantic-row"><code>context</code><span>{dependency}</span></div>
            {/each}
          </section>
        {/if}

        {#if selectedDefinition.dependencies.length}
          <section class="detail-section">
            <h3>Dependențe</h3>
            {#each selectedDefinition.dependencies as dependency (`${dependency.kind}:${dependency.reference}`)}
              <div class="semantic-row">
                <code>{dependency.kind}</code>
                <span class:unresolved={!dependency.resolved}>{dependency.reference}</span>
              </div>
            {/each}
          </section>
        {/if}

        {#if selectedDefinition.diagnostics.length}
          <section class="detail-section diagnostics">
            <h3>Diagnostice</h3>
            {#each selectedDefinition.diagnostics as diagnostic (`${diagnostic.code}:${diagnostic.message}`)}
              <p><IconAlertTriangle size={13} /> {diagnostic.message}</p>
            {/each}
          </section>
        {/if}

        {#if formError}<p class="ui-message error form-error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>{/if}
        <div class="detail-actions">
          {#if selectedDefinition.origin === "theme" && isMutableFileDefinition(selectedDefinition)}
            <button class="primary-action" type="button" disabled={mutating} onclick={() => { void overrideSelected(); }}>
              <IconCopy size={14} /> Creează override local
            </button>
          {:else if selectedDefinition.file && selectedDefinition.capabilities.canEdit}
            <button class="primary-action" type="button" disabled={mutating} onclick={() => { void beginEdit(); }}>
              <IconEdit size={14} /> Editează
            </button>
          {/if}
          {#if selectedDefinition.file}
            <button type="button" disabled={mutating} onclick={() => { void openWorkspaceSource(selectedDefinition.file!); }}>
              <IconExternalLink size={14} /> Deschide sursa
            </button>
          {/if}
          {#if selectedDefinition.capabilities.canDuplicate && isMutableFileDefinition(selectedDefinition)}
            <button type="button" disabled={mutating} onclick={() => { void duplicateSelected(); }}>
              <IconCopy size={14} /> Duplică
            </button>
          {/if}
          {#if selectedDefinition.capabilities.canDelete && selectedDefinition.origin === "project" && isMutableFileDefinition(selectedDefinition)}
            <button class="danger" type="button" disabled={mutating} onclick={() => { deleteConfirmationOpen = true; }}>
              <IconTrash size={14} /> Șterge
            </button>
          {/if}
        </div>

        {#if deleteConfirmationOpen}
          <div class="delete-confirmation" role="alert">
            <strong>Elimini „{selectedDefinition.displayName}”?</strong>
            <span>Plannerul refuză automat ștergerea dacă există invocări active.</span>
            <div>
              <button type="button" disabled={mutating} onclick={() => { deleteConfirmationOpen = false; }}>Renunță</button>
              <button class="danger" type="button" disabled={mutating} onclick={() => { void deleteSelected(); }}>
                {mutating ? "Se verifică…" : "Elimină din sesiune"}
              </button>
            </div>
          </div>
        {/if}
      {:else}
        <div class="workspace-state">Selectează o definiție.</div>
      {/if}
    </aside>
  </div>
</section>

<style>
  .components-workspace { display: grid; grid-template-rows: auto 42px minmax(0, 1fr); min-width: 0; min-height: 0; height: 100%; overflow: hidden; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-panel); color: var(--wb-text-primary); background: var(--wb-surface-document); }
  .workspace-header { display: flex; align-items: center; justify-content: space-between; gap: 24px; padding: 17px 20px; border-bottom: 1px solid var(--wb-border-subtle); background: var(--wb-surface-chrome); }
  .workspace-header > div { min-width: 0; }
  .eyebrow { display: inline-flex; align-items: center; gap: 6px; color: var(--wb-accent-strong); font-size: 11px; font-weight: 800; letter-spacing: .035em; text-transform: uppercase; }
  h1 { margin: 6px 0 0; color: var(--text-strong); font-size: 20px; }
  .workspace-header p { margin: 4px 0 0; color: var(--wb-text-muted); font-size: 12px; line-height: 1.4; }
  .workspace-header dl { display: grid; grid-template-columns: repeat(4, minmax(68px, auto)); gap: 7px; margin: 0; }
  .workspace-header dl div { min-width: 68px; padding: 7px 9px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  dt { color: var(--wb-text-muted); font-size: var(--font-meta); font-weight: 800; text-transform: uppercase; }
  dd { margin: 3px 0 0; color: var(--text-strong); font-size: 16px; font-weight: 750; }
  .workspace-toolbar { display: flex; align-items: center; gap: 8px; min-width: 0; padding: 0 9px; border-bottom: 1px solid var(--wb-border-subtle); }
  .view-tabs { display: flex; align-self: stretch; min-width: 0; overflow-x: auto; scrollbar-width: none; }
  .view-tabs::-webkit-scrollbar { display: none; }
  .view-tabs button { flex: 0 0 auto; height: 100%; padding: 0 10px; border: 0; border-bottom: 2px solid transparent; color: var(--wb-text-muted); background: transparent; font-size: 12px; font-weight: 650; }
  .view-tabs button.active { border-bottom-color: var(--wb-accent); color: var(--wb-accent-strong); }
  .search-field { position: relative; display: flex; flex: 1; min-width: 150px; margin-left: auto; }
  .search-field :global(svg) { position: absolute; left: 8px; top: 7px; color: var(--wb-text-muted); pointer-events: none; }
  .search-field input { width: 100%; height: 28px; padding: 0 8px 0 28px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; }
  .toolbar-action { display: inline-flex; align-items: center; justify-content: center; gap: 5px; min-height: 28px; padding: 0 10px; border: 1px solid var(--wb-accent); border-radius: var(--radius-control); color: #fff; background: var(--wb-accent); font-size: 12px; font-weight: 700; }
  .workspace-body { display: grid; grid-template-columns: minmax(330px, 1fr) minmax(330px, .62fr); min-width: 0; min-height: 0; }
  .resource-list { min-width: 0; min-height: 0; overflow: auto; padding: 9px; border-right: 1px solid var(--wb-border-subtle); }
  .resource-card { display: flex; align-items: center; width: 100%; gap: 9px; min-height: 54px; padding: 7px 9px; border: 1px solid transparent; border-radius: 7px; color: var(--wb-text-primary); background: transparent; text-align: left; }
  .resource-card:hover, .resource-card.selected { border-color: var(--wb-border-subtle); background: var(--wb-control-hover); }
  .resource-card.selected { box-shadow: inset 3px 0 0 var(--wb-accent); }
  .resource-card.shadowed { opacity: .6; }
  .resource-icon { display: grid; flex: 0 0 auto; width: 30px; height: 30px; place-items: center; border-radius: 7px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .resource-card > span:nth-child(2) { display: grid; flex: 1; gap: 3px; min-width: 0; }
  .resource-card strong { color: var(--text-strong); font-size: 12px; }
  .resource-card small { overflow: hidden; color: var(--wb-text-muted); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .resource-badges { display: grid; justify-items: end; gap: 3px; }
  .resource-badges code { padding: 2px 4px; border-radius: 4px; color: var(--wb-text-muted); background: var(--wb-surface-chrome); font-size: var(--font-meta); }
  .resource-detail { min-width: 0; min-height: 0; overflow: auto; padding: 17px; background: var(--wb-surface-chrome); }
  .detail-kicker-row, .detail-heading, .form-actions, .detail-actions { display: flex; align-items: center; }
  .detail-kicker-row { justify-content: space-between; gap: 8px; }
  .detail-kicker-row > span:last-child { padding: 3px 6px; border-radius: 999px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); font-size: var(--font-meta); font-weight: 750; }
  .detail-kicker-row > span.inactive { color: var(--wb-text-muted); background: var(--wb-control-hover); }
  .detail-kicker { color: var(--wb-accent-strong); font-size: 11px; font-weight: 850; text-transform: uppercase; }
  h2 { margin: 7px 0 0; color: var(--text-strong); font-size: 19px; }
  .resource-detail > p, .detail-heading p { margin: 6px 0 0; color: var(--wb-text-muted); font-size: 12px; line-height: 1.5; overflow-wrap: anywhere; }
  .detail-heading { align-items: flex-start; justify-content: space-between; gap: 12px; }
  .detail-heading > button { display: grid; flex: 0 0 auto; width: 28px; height: 28px; padding: 0; place-items: center; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-muted); background: var(--wb-surface-document); }
  .component-contract { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 6px; margin: 14px 0 0; }
  .component-contract div { min-width: 0; padding: 7px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  .component-contract dd { font-size: 13px; }
  .detail-section { margin-top: 14px; }
  .detail-section h3 { margin: 0 0 6px; color: var(--text-strong); font-size: 12px; }
  .semantic-row { display: grid; grid-template-columns: minmax(80px, .34fr) minmax(0, 1fr); gap: 8px; padding: 6px 0; border-top: 1px solid var(--wb-border-subtle); font-size: 11px; }
  .semantic-row code { color: var(--wb-accent-strong); }
  .semantic-row span { overflow-wrap: anywhere; color: var(--wb-text-muted); }
  .semantic-row span.unresolved { color: var(--danger); }
  .diagnostics p { display: flex; align-items: flex-start; gap: 5px; margin: 5px 0; color: var(--danger); font-size: 11px; }
  .component-form { display: grid; gap: 11px; }
  .component-form label, .companion-fields label { display: grid; gap: 5px; min-width: 0; }
  .component-form label > span, .companion-fields label > span { color: var(--wb-text-muted); font-size: 11px; font-weight: 750; }
  .component-form label > small { color: var(--wb-text-muted); font-size: var(--font-meta); line-height: 1.4; }
  input, select, textarea { width: 100%; min-width: 0; border: 1px solid var(--wb-border-subtle); border-radius: 6px; color: var(--text-strong); background: var(--wb-surface-document); font: inherit; font-size: 12px; }
  input, select { height: 32px; padding: 0 9px; }
  textarea { min-height: 180px; padding: 9px; resize: vertical; font-family: var(--font-mono); line-height: 1.45; tab-size: 2; }
  details { overflow: hidden; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  summary { min-height: 34px; padding: 9px 10px; color: var(--text-strong); font-size: 11px; font-weight: 700; cursor: pointer; }
  .companion-fields { display: grid; gap: 12px; padding: 10px; border-top: 1px solid var(--wb-border-subtle); }
  .companion-fields textarea { min-height: 74px; }
  .form-error { display: flex; align-items: flex-start; gap: 6px; margin: 9px 0 0; padding: 8px; border: 1px solid color-mix(in srgb, var(--danger) 36%, var(--wb-border-subtle)); border-radius: 6px; color: var(--danger); background: color-mix(in srgb, var(--danger) 7%, var(--wb-surface-document)); font-size: 11px; line-height: 1.4; }
  .form-actions { justify-content: flex-end; gap: 7px; margin-top: 4px; }
  .form-actions button, .detail-actions button, .delete-confirmation button { display: inline-flex; align-items: center; justify-content: center; gap: 5px; min-height: 31px; padding: 0 10px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 11px; font-weight: 650; }
  .form-actions button.primary, .detail-actions .primary-action { border-color: var(--wb-accent); color: #fff; background: var(--wb-accent); }
  .detail-actions { flex-wrap: wrap; align-items: stretch; gap: 7px; margin-top: 14px; }
  .detail-actions button { flex: 1 1 130px; }
  .detail-actions button.danger, .delete-confirmation button.danger { border-color: var(--danger); color: var(--danger); }
  .delete-confirmation { display: grid; gap: 6px; margin-top: 9px; padding: 10px; border: 1px solid color-mix(in srgb, var(--danger) 34%, var(--wb-border-subtle)); border-radius: 7px; background: var(--wb-surface-document); }
  .delete-confirmation strong { color: var(--text-strong); font-size: 12px; }
  .delete-confirmation > span { color: var(--wb-text-muted); font-size: 11px; line-height: 1.4; }
  .delete-confirmation > div { display: flex; justify-content: flex-end; gap: 7px; }
  .workspace-state { display: grid; min-height: 180px; place-items: center; color: var(--wb-text-muted); font-size: 12px; text-align: center; }
  button:disabled { opacity: .5; }
  button:not(:disabled) { cursor: pointer; }
  button:focus-visible, input:focus-visible, select:focus-visible, textarea:focus-visible, summary:focus-visible { outline: 2px solid var(--wb-focus-ring); outline-offset: 1px; }
  .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; }
  @media (max-width: 1050px) { .workspace-header dl { grid-template-columns: repeat(2, 70px); } }
  @media (max-width: 900px) { .workspace-body { grid-template-columns: 1fr; } .resource-detail { display: none; } .resource-list { border-right: 0; } }
</style>
