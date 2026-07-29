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
  import { l10n, t } from "$lib/i18n/runtime.svelte";
  import { settleProjectWorkspaceMutation } from "$lib/session/workspace-mutation-coordinator";
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

  const componentViews = $derived([
    { id: "all" as const, label: t("components-view-all") },
    { id: "partials" as const, label: t("components-view-partials") },
    { id: "macros" as const, label: t("components-view-macros") },
    { id: "shortcodes" as const, label: t("components-view-shortcodes") },
    { id: "repeats" as const, label: t("components-view-repeats") },
  ]);

  let activeView = $state<ComponentView>("all");
  let detailMode = $state<DetailMode>("info");
  let selectedDefinitionId = $state("");
  let query = $state("");
  let formError = $state("");
  let mutating = $state(false);
  let loadingSource = $state(false);
  let deleteConfirmationOpen = $state(false);

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
  const normalizedQuery = $derived(query.trim().toLocaleLowerCase(l10n.locale));
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
        ].join(" ").toLocaleLowerCase(l10n.locale).includes(normalizedQuery)
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
    const labels: Record<ComponentDefinitionKind, ReturnType<typeof t>> = {
      templateFile: t("components-kind-template"),
      partial: t("components-kind-partial"),
      macroLibrary: t("components-kind-macro-library"),
      macro: t("components-kind-macro"),
      shortcode: t("components-kind-shortcode"),
      templateBlock: t("components-kind-block"),
      inlineRepeat: t("components-kind-repeat"),
    };
    return labels[kind];
  }

  function originLabel(origin: ComponentDefinition["origin"]) {
    if (origin === "project") return t("components-origin-project");
    return t("components-origin-theme");
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
    if (kind === "shortcode_markdown") return `**${t("components-new-shortcode")}**\n`;
    if (kind === "shortcode_html") {
      return `<span class="shortcode-${safeName}">${t("components-new-shortcode")}</span>\n`;
    }
    return `<section class="${safeName}">\n  ${t("components-new-placeholder")}\n</section>\n`;
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
    const settlement = await settleProjectWorkspaceMutation(app, receipt.workspace, {
      preferredRelativePath: receipt.workspace.relativePath,
      warningLabel: t("components-mutation-operation"),
    });
    const destination = receipt.plan.destinationRelativePath;
    if (destination) {
      selectedDefinitionId = app.sourceGraph?.componentGraph.definitions.find((definition) => (
        definition.file === destination && definition.active
      ))?.id ?? "";
    } else {
      selectedDefinitionId = "";
    }
    app.setGlobalStatus(
      settlement.warnings.length > 0
        ? t("components-mutation-warning", { message: successMessage })
        : t("components-mutation-success", { message: successMessage }),
      "unsaved",
    );
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
        }, t("components-created-status", { name: formName }));
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
        }, t("components-updated-status", { name: formName }));
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
        destinationName: `${logicalName(selectedDefinition)}-${t("components-copy-suffix")}`,
        contents: null,
        sourceFile: null,
        sourceRange: null,
        companions: [],
      }, t("components-duplicated-status", { name: selectedDefinition.displayName }));
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
      }, t("components-override-status", { name: selectedDefinition.displayName }));
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
      }, t("components-removed-status", { name: selectedDefinition.displayName }));
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

<section class="activity-workspace components-workspace" aria-labelledby="components-title">
  <header class="workspace-header">
    <div>
      <span class="eyebrow"><IconBraces size={15} stroke={1.9} /> {t("components-eyebrow")}</span>
      <h1 id="components-title">{t("components-title")}</h1>
      <p>{t("components-description")}</p>
    </div>
    <dl>
      <div><dt>{t("components-stat-definitions")}</dt><dd>{l10n.formatNumber(definitions.length)}</dd></div>
      <div><dt>{t("components-stat-project")}</dt><dd>{l10n.formatNumber(projectDefinitionCount)}</dd></div>
      <div><dt>{t("components-stat-theme")}</dt><dd>{l10n.formatNumber(themeDefinitionCount)}</dd></div>
      <div><dt>{t("components-stat-invocations")}</dt><dd>{l10n.formatNumber(componentGraph?.invocations.length ?? 0)}</dd></div>
    </dl>
  </header>

  <div class="workspace-toolbar">
    <div class="ui-tabs view-tabs" role="tablist" aria-label={t("components-types-label")}>
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
      <span class="sr-only">{t("components-search-label")}</span>
      <IconSearch size={14} stroke={1.9} />
      <input class="ui-field toolbar" bind:value={query} type="search" placeholder={t("components-search-placeholder")} />
    </label>
    <button
      class="ui-button primary toolbar toolbar-action"
      type="button"
      disabled={mutating || activeView === "repeats"}
      title={activeView === "repeats" ? t("components-repeat-derived-title") : ""}
      onclick={beginCreate}
    >
      <IconPlus size={14} stroke={2} /> {t("components-add")}
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
        <div class="workspace-state">{t("components-loading")}</div>
      {:else}
        {#each filteredDefinitions as definition (definition.id)}
          {@const DefinitionIcon = iconForDefinition(definition)}
          <button
            type="button"
            class="resource-card ui-entity-selectable"
            data-ui-selected={selectedDefinition?.id === definition.id ? "true" : undefined}
            aria-pressed={selectedDefinition?.id === definition.id}
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
          <div class="workspace-state">{t("components-empty-filter")}</div>
        {/each}
      {/if}
    </div>

    <aside class="resource-detail" aria-label={t("components-detail-label")}>
      {#if detailMode === "create" || detailMode === "edit"}
        <form class="component-form" onsubmit={(event) => { event.preventDefault(); void submitComponent(); }}>
          <header class="detail-heading">
            <div>
              <span class="detail-kicker">{detailMode === "create"
                ? t("components-new-definition")
                : t("components-atomic-edit")}</span>
              <h2>{detailMode === "create" ? t("components-create-title") : selectedDefinition?.displayName}</h2>
              <p>{t("components-form-description")}</p>
            </div>
            <button class="ui-icon-button ui-close-button" type="button" aria-label={t("components-cancel")} disabled={mutating} onclick={resetPanel}><IconX size={14} /></button>
          </header>

          {#if detailMode === "create"}
            <label>
              <span>{t("components-type")}</span>
              <select value={formKind} disabled={mutating} onchange={(event) => updateCreateKind(event.currentTarget.value)}>
                <option value="partial">{t("components-type-tera-partial")}</option>
                <option value="macro_library">{t("components-type-macro-library")}</option>
                <option value="shortcode_html">{t("components-type-html-shortcode")}</option>
                <option value="shortcode_markdown">{t("components-type-markdown-shortcode")}</option>
              </select>
            </label>
          {/if}
          <label>
            <span>{t("components-logical-name")}</span>
            <input bind:value={formName} disabled={mutating || loadingSource} placeholder="catalog/card" />
            <small>{t("components-logical-name-help")}</small>
          </label>
          <label>
            <span>{t("components-source", {
              format: formKind === "shortcode_markdown" ? "Markdown + Tera" : "HTML + Tera",
            })}</span>
            <textarea bind:value={formSource} disabled={mutating || loadingSource} spellcheck="false"></textarea>
          </label>

          {#if detailMode === "create"}
            <details>
              <summary>{t("components-companions")}</summary>
              <div class="companion-fields">
                <label>
                  <span>{t("components-style")}</span>
                  <input bind:value={formStylePath} disabled={mutating} placeholder={t("components-style-path-placeholder")} />
                  <textarea bind:value={formStyleSource} disabled={mutating} spellcheck="false" placeholder={".card { }"}></textarea>
                </label>
                <label>
                  <span>{t("components-script")}</span>
                  <input bind:value={formScriptPath} disabled={mutating} placeholder="static/js/card.js" />
                  <textarea bind:value={formScriptSource} disabled={mutating} spellcheck="false" placeholder={`// ${t("components-script-placeholder")}`}></textarea>
                </label>
                <label>
                  <span>{t("components-canonical-data")}</span>
                  <input bind:value={formDataPath} disabled={mutating} placeholder={t("components-data-path-placeholder")} />
                  <textarea bind:value={formDataSource} disabled={mutating} spellcheck="false" placeholder="[[items]]"></textarea>
                </label>
              </div>
            </details>
          {/if}

          {#if formError}<p class="ui-message error form-error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>{/if}
          <div class="form-actions">
            <button type="button" disabled={mutating} onclick={resetPanel}>{t("components-cancel")}</button>
            <button class="ui-button primary" type="submit" disabled={mutating || loadingSource || !formName.trim()}>
              <IconDeviceFloppy size={14} />
              {mutating
                ? t("components-validating")
                : detailMode === "create"
                  ? t("components-create-session")
                  : t("components-save-changes")}
            </button>
          </div>
        </form>
      {:else if selectedDefinition}
        <div class="detail-kicker-row">
          <span class="detail-kicker">{kindLabel(selectedDefinition.kind)}</span>
          <span class:inactive={!selectedDefinition.active}>
            {selectedDefinition.active ? originLabel(selectedDefinition.origin) : t("components-shadowed")}
          </span>
        </div>
        <h2>{selectedDefinition.displayName}</h2>
        <p>{selectedDefinition.file ?? selectedDefinition.name}</p>

        <dl class="component-contract">
          <div><dt>{t("components-uses")}</dt><dd>{l10n.formatNumber(selectedInvocations.length)}</dd></div>
          <div><dt>{t("components-parameters")}</dt><dd>{l10n.formatNumber(selectedDefinition.parameters.length)}</dd></div>
          <div><dt>{t("components-dependencies")}</dt><dd>{l10n.formatNumber(selectedDefinition.dependencies.length)}</dd></div>
          <div><dt>{t("components-bindings")}</dt><dd>{l10n.formatNumber(selectedDefinition.dataBindings.length)}</dd></div>
        </dl>

        {#if selectedDefinition.parameters.length}
          <section class="detail-section">
            <h3>{t("components-parameters")}</h3>
            {#each selectedDefinition.parameters as parameter (parameter.name)}
              <div class="semantic-row">
                <code>{parameter.name}</code>
                <span>{parameter.required ? t("components-required") : t("components-optional")}</span>
              </div>
            {/each}
          </section>
        {/if}

        {#if selectedDefinition.dataBindings.length || selectedDefinition.contextDependencies.length}
          <section class="detail-section">
            <h3>{t("components-data-context")}</h3>
            {#each selectedDefinition.dataBindings as binding (`${binding.name}:${binding.path}`)}
              <div class="semantic-row"><code>{binding.name}</code><span>{binding.path} · {binding.producer}</span></div>
            {/each}
            {#each selectedDefinition.contextDependencies as dependency (dependency)}
              <div class="semantic-row"><code>{t("components-context")}</code><span>{dependency}</span></div>
            {/each}
          </section>
        {/if}

        {#if selectedDefinition.dependencies.length}
          <section class="detail-section">
            <h3>{t("components-dependencies")}</h3>
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
            <h3>{t("components-diagnostics")}</h3>
            {#each selectedDefinition.diagnostics as diagnostic (diagnostic.code)}
              <p><IconAlertTriangle size={13} /> {errorMessage(diagnostic.diagnostic)}</p>
            {/each}
          </section>
        {/if}

        {#if formError}<p class="ui-message error form-error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>{/if}
        <div class="detail-actions">
          {#if selectedDefinition.origin === "theme" && isMutableFileDefinition(selectedDefinition)}
            <button class="ui-button primary primary-action" type="button" disabled={mutating} onclick={() => { void overrideSelected(); }}>
              <IconCopy size={14} /> {t("components-create-override")}
            </button>
          {:else if selectedDefinition.file && selectedDefinition.capabilities.canEdit}
            <button class="ui-button primary primary-action" type="button" disabled={mutating} onclick={() => { void beginEdit(); }}>
              <IconEdit size={14} /> {t("components-edit")}
            </button>
          {/if}
          {#if selectedDefinition.file}
            <button type="button" disabled={mutating} onclick={() => { void openWorkspaceSource(selectedDefinition.file!); }}>
              <IconExternalLink size={14} /> {t("components-open-source")}
            </button>
          {/if}
          {#if selectedDefinition.capabilities.canDuplicate && isMutableFileDefinition(selectedDefinition)}
            <button type="button" disabled={mutating} onclick={() => { void duplicateSelected(); }}>
              <IconCopy size={14} /> {t("components-duplicate")}
            </button>
          {/if}
          {#if selectedDefinition.capabilities.canDelete && selectedDefinition.origin === "project" && isMutableFileDefinition(selectedDefinition)}
            <button class="ui-button danger" type="button" disabled={mutating} onclick={() => { deleteConfirmationOpen = true; }}>
              <IconTrash size={14} /> {t("components-delete")}
            </button>
          {/if}
        </div>

        {#if deleteConfirmationOpen}
          <div class="delete-confirmation" role="alert">
            <strong>{t("components-delete-title", { name: selectedDefinition.displayName })}</strong>
            <span>{t("components-delete-description")}</span>
            <div>
              <button type="button" disabled={mutating} onclick={() => { deleteConfirmationOpen = false; }}>{t("components-cancel")}</button>
              <button class="ui-button danger" type="button" disabled={mutating} onclick={() => { void deleteSelected(); }}>
                {mutating ? t("components-checking") : t("components-remove-session")}
              </button>
            </div>
          </div>
        {/if}
      {:else}
        <div class="workspace-state">{t("components-select-help")}</div>
      {/if}
    </aside>
  </div>
</section>

<style>
  dt { color: var(--wb-text-muted); font-size: var(--font-meta); font-weight: 800; text-transform: uppercase; }
  dd { margin: 3px 0 0; color: var(--text-strong); font-size: 16px; font-weight: 750; }
  .workspace-body { display: grid; grid-template-columns: minmax(330px, 1fr) minmax(330px, .62fr); min-width: 0; min-height: 0; }
  .resource-card.shadowed { opacity: .6; }
  .resource-icon { display: grid; flex: 0 0 auto; width: 30px; height: 30px; place-items: center; border-radius: 7px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .resource-card > span:nth-child(2) { display: grid; flex: 1; gap: 3px; min-width: 0; }
  .resource-card strong { color: var(--text-strong); font-size: 12px; }
  .resource-card small { overflow: hidden; color: var(--wb-text-muted); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .resource-badges { display: grid; justify-items: end; gap: 3px; }
  .resource-badges code { padding: 2px 4px; border-radius: 4px; color: var(--wb-text-muted); background: var(--wb-surface-chrome); font-size: var(--font-meta); }
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
