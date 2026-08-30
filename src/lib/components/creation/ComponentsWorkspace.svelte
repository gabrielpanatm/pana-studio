<script lang="ts">
  import {
    IconAlertTriangle,
    IconBraces,
    IconCheck,
    IconClipboard,
    IconCopy,
    IconDeviceFloppy,
    IconEdit,
    IconEye,
    IconExternalLink,
    IconLink,
    IconPlus,
    IconSearch,
    IconTrash,
    IconX,
  } from "@tabler/icons-svelte";
  import EmptyState from "$lib/components/ui/EmptyState.svelte";
  import CheckboxControl from "$lib/components/ui/CheckboxControl.svelte";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import TextAreaControl from "$lib/components/ui/TextAreaControl.svelte";
  import TextFieldControl from "$lib/components/ui/TextFieldControl.svelte";
  import { applyComponentMutation } from "$lib/creation/components-io";
  import { readFileBufferText } from "$lib/project/io/workspace";
  import { l10n, t } from "$lib/i18n/runtime.svelte";
  import type { GlobalStatusState } from "$lib/status/state.svelte";
  import type { ProjectWorkspaceMutationService } from "$lib/session/workspace-mutation-service";
  import type {
    ComponentCompanionDraft,
    ComponentMutationInput,
  } from "$lib/creation/contracts";
  import type { FileBufferRequestIdentity } from "$lib/project/workspace-contract";
  import type { ComponentDefinition } from "$lib/source-graph/contracts";
  import type { SourceGraph } from "$lib/source-graph/graph-contract";
  import { errorMessage } from "$lib/util";

  let {
    globalStatus,
    workspaceMutations,
    sourceGraph,
    openWorkspaceSource,
  }: {
    globalStatus: GlobalStatusState;
    workspaceMutations: ProjectWorkspaceMutationService;
    sourceGraph: SourceGraph | null;
    openWorkspaceSource: (
      path: string,
      options?: { surface?: "visual" | "code"; componentName?: string | null },
    ) => void | Promise<void>;
  } = $props();

  type DetailMode = "info" | "create" | "edit" | "rename";
  type WizardArgument = {
    id: number;
    name: string;
    argumentType: string;
    hasDefault: boolean;
    defaultValue: string;
  };

  let detailMode = $state<DetailMode>("info");
  let selectedDefinitionId = $state("");
  let query = $state("");
  let namespaceFilter = $state("all");
  let originFilter = $state("all");
  let formError = $state("");
  let mutating = $state(false);
  let loadingSource = $state(false);
  let deleteConfirmationOpen = $state(false);
  let copiedCall = $state(false);

  let formName = $state("card");
  let formNamespace = $state("ui");
  let formSource = $state("");
  let formRenameSymbol = $state("");
  let nextArgumentId = 2;
  let formArguments = $state<WizardArgument[]>([
    {
      id: 1,
      name: "title",
      argumentType: "string",
      hasDefault: false,
      defaultValue: "Exemplu",
    },
  ]);
  let formRestEnabled = $state(false);
  let formRestName = $state("attributes");
  let formAcceptsBody = $state(false);
  let formStylePath = $state("");
  let formStyleSource = $state("");
  let formScriptPath = $state("");
  let formScriptSource = $state("");
  let formDataPath = $state("");
  let formDataSource = $state("");

  const componentGraph = $derived(sourceGraph?.componentGraph ?? null);
  const definitions = $derived(
    (componentGraph?.definitions ?? []).filter((definition) => (
      definition.kind === "teraComponent"
    )),
  );
  const namespaces = $derived(Array.from(new Set(
    definitions.map((definition) => componentNamespace(definition)).filter(Boolean),
  )).sort((left, right) => left.localeCompare(right, l10n.locale)));
  const namespaceOptions = $derived([
    { value: "all", label: t("components-filter-all-namespaces") },
    ...namespaces.map((namespace) => ({ value: namespace, label: namespace })),
  ]);
  const originOptions = $derived([
    { value: "all", label: t("components-filter-all-origins") },
    { value: "project", label: t("components-origin-project") },
    { value: "theme", label: t("components-origin-theme") },
  ]);
  const normalizedQuery = $derived(query.trim().toLocaleLowerCase(l10n.locale));
  const filteredDefinitions = $derived(
    definitions.filter((definition) => (
      (namespaceFilter === "all" || componentNamespace(definition) === namespaceFilter)
      && (originFilter === "all" || definition.origin === originFilter)
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
  const componentInvocationCount = $derived(
    (componentGraph?.invocations ?? []).filter((invocation) => invocation.kind === "teraComponent").length,
  );
  const wizardSymbol = $derived(componentSymbol(formNamespace, formName));
  const wizardSource = $derived(componentSource(wizardSymbol, formArguments, formRestEnabled ? formRestName : "", formAcceptsBody));
  const wizardCall = $derived(componentCallExample(wizardSymbol, formArguments, formRestEnabled ? formRestName : "", formAcceptsBody));
  const selectedCall = $derived(selectedDefinition ? callExampleForDefinition(selectedDefinition) : "");

  function identity(): FileBufferRequestIdentity {
    return {
      expectedProjectRoot: workspaceMutations.snapshot?.projectRoot ?? "",
      expectedSessionId: workspaceMutations.snapshot?.runtimeSessionId ?? "",
    };
  }

  function componentNamespace(definition: ComponentDefinition) {
    return definition.symbol?.split(".").slice(0, -1).join(".") ?? "";
  }

  function componentSymbol(namespace: string, name: string) {
    return [namespace.trim(), name.trim()].filter(Boolean).join(".");
  }

  function componentPath(namespace: string, name: string) {
    return [namespace.trim().replaceAll(".", "/"), name.trim()].filter(Boolean).join("/");
  }

  function originLabel(origin: ComponentDefinition["origin"]) {
    if (origin === "project") return t("components-origin-project");
    return t("components-origin-theme");
  }

  function argumentDefault(argument: WizardArgument) {
    const value = argument.defaultValue.trim();
    if (argument.argumentType === "string") return JSON.stringify(value);
    if (argument.argumentType === "bool") return value === "false" ? "false" : "true";
    if (argument.argumentType === "array") return value || "[]";
    if (argument.argumentType === "map") return value || "{}";
    return value || (argument.argumentType === "float" ? "1.5" : "1");
  }

  function argumentExample(argument: WizardArgument) {
    if (argument.hasDefault) {
      const value = argumentDefault(argument);
      return argument.argumentType === "string" ? value : `{${value}}`;
    }
    if (argument.argumentType === "string") return JSON.stringify(t("components-call-string-example"));
    if (argument.argumentType === "bool") return "{true}";
    if (argument.argumentType === "array") return "{[]}";
    if (argument.argumentType === "map") return "{{}}";
    if (argument.argumentType === "float") return "{1.5}";
    return "{1}";
  }

  function componentSource(
    symbol: string,
    arguments_: WizardArgument[],
    restName: string,
    acceptsBody: boolean,
  ) {
    const parameters = arguments_.map((argument) => (
      `${argument.name.trim()}: ${argument.argumentType}${argument.hasDefault ? ` = ${argumentDefault(argument)}` : ""}`
    ));
    if (restName.trim()) parameters.push(`...${restName.trim()}`);
    const className = symbol.replaceAll(".", "-");
    const primary = arguments_[0]?.name.trim();
    const content = acceptsBody
      ? "    {{ body | safe }}"
      : primary
        ? `    {{ ${primary} }}`
        : `    ${t("components-new-placeholder")}`;
    return `{% component ${symbol}(${parameters.join(", ")}) %}\n  <div class="${className}">\n${content}\n  </div>\n{% endcomponent ${symbol} %}\n`;
  }

  function componentCallExample(
    symbol: string,
    arguments_: WizardArgument[],
    restName: string,
    acceptsBody: boolean,
  ) {
    const values = arguments_.map((argument) => `${argument.name.trim()}=${argumentExample(argument)}`);
    if (restName.trim()) values.push(`{...${restName.trim()}}`);
    const suffix = values.length ? ` ${values.join(" ")}` : "";
    if (acceptsBody) {
      return `{% <${symbol}${suffix}> %}\n  ${t("components-call-body-example")}\n{% </${symbol}> %}`;
    }
    return `{{ <${symbol}${suffix} /> }}`;
  }

  function semanticDefaultPreview(value: ComponentDefinition["parameters"][number]["defaultValue"]) {
    if (!value) return "";
    const semantic = value.value;
    if (semantic.kind === "string") return JSON.stringify(semantic.value);
    if (["integer", "float", "boolean", "identifier"].includes(semantic.kind)) {
      return String(semantic.value);
    }
    return semantic.kind;
  }

  function callExampleForDefinition(definition: ComponentDefinition) {
    const arguments_: WizardArgument[] = definition.parameters
      .filter((parameter) => !parameter.rest)
      .map((parameter, index) => ({
        id: index,
        name: parameter.name,
        argumentType: parameter.argumentType ?? "string",
        hasDefault: !parameter.required,
        defaultValue: semanticDefaultPreview(parameter.defaultValue),
      }));
    return componentCallExample(
      definition.symbol ?? definition.name,
      arguments_,
      definition.restParameter ?? "",
      false,
    );
  }

  function resetPanel() {
    detailMode = "info";
    formError = "";
    deleteConfirmationOpen = false;
    copiedCall = false;
    loadingSource = false;
  }

  function selectDefinition(id: string) {
    selectedDefinitionId = id;
    resetPanel();
  }

  function beginCreate() {
    formError = "";
    deleteConfirmationOpen = false;
    formName = "card";
    formNamespace = "ui";
    formArguments = [{
      id: ++nextArgumentId,
      name: "title",
      argumentType: "string",
      hasDefault: false,
      defaultValue: t("components-call-string-example"),
    }];
    formRestEnabled = false;
    formRestName = "attributes";
    formAcceptsBody = false;
    formStylePath = "";
    formStyleSource = "";
    formScriptPath = "";
    formScriptSource = "";
    formDataPath = "";
    formDataSource = "";
    detailMode = "create";
  }

  function addArgument() {
    formArguments.push({
      id: ++nextArgumentId,
      name: `argument${formArguments.length + 1}`,
      argumentType: "string",
      hasDefault: true,
      defaultValue: "",
    });
  }

  function removeArgument(id: number) {
    formArguments = formArguments.filter((argument) => argument.id !== id);
  }

  function updateArgumentType(id: number, value: string) {
    const argument = formArguments.find((candidate) => candidate.id === id);
    if (argument) argument.argumentType = value;
  }

  async function beginEdit() {
    if (!selectedDefinition?.file || selectedDefinition.origin !== "project") return;
    formError = "";
    deleteConfirmationOpen = false;
    loadingSource = true;
    detailMode = "edit";
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

  function beginRename() {
    if (!selectedDefinition?.symbol) return;
    formError = "";
    formRenameSymbol = selectedDefinition.symbol;
    detailMode = "rename";
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
    const settlement = await workspaceMutations.settle(receipt.workspace, {
      preferredRelativePath: receipt.workspace.relativePath,
      warningLabel: t("components-mutation-operation"),
    });
    const destination = receipt.plan.destinationRelativePath;
    const destinationSymbol = receipt.plan.destinationSymbol;
    if (destination || destinationSymbol) {
      selectedDefinitionId = sourceGraph?.componentGraph.definitions.find((definition) => (
        definition.kind === "teraComponent"
        && definition.active
        && (destination ? definition.file === destination : true)
        && (destinationSymbol ? definition.symbol === destinationSymbol : true)
      ))?.id ?? "";
    } else {
      selectedDefinitionId = "";
    }
    globalStatus.set(
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
        const symbol = wizardSymbol.trim();
        if (!/^[A-Za-z_][A-Za-z0-9_-]*(\.[A-Za-z_][A-Za-z0-9_-]*)*$/.test(symbol)) {
          throw new Error(t("components-invalid-symbol"));
        }
        const argumentNames = formArguments.map((argument) => argument.name.trim());
        if (argumentNames.some((name) => !/^[A-Za-z_][A-Za-z0-9_]*$/.test(name))
          || new Set(argumentNames).size !== argumentNames.length) {
          throw new Error(t("components-invalid-arguments"));
        }
        await applyMutation({
          operation: "create",
          definitionId: null,
          kind: "tera_component",
          name: componentPath(formNamespace, formName),
          symbolName: symbol,
          destinationName: null,
          contents: wizardSource,
          sourceFile: null,
          sourceRange: null,
          companions: createCompanions(),
        }, t("components-created-status", { name: symbol }));
      } else if (detailMode === "edit" && selectedDefinition) {
        await applyMutation({
          operation: "update",
          definitionId: selectedDefinition.id,
          kind: null,
          name: null,
          symbolName: selectedDefinition.symbol,
          destinationName: null,
          contents: formSource,
          sourceFile: null,
          sourceRange: null,
          companions: [],
        }, t("components-updated-status", { name: selectedDefinition.displayName }));
      }
    } catch (cause) {
      formError = errorMessage(cause);
    } finally {
      mutating = false;
    }
  }

  async function renameSelected() {
    if (!selectedDefinition || mutating) return;
    formError = "";
    mutating = true;
    try {
      await applyMutation({
        operation: "rename",
        definitionId: selectedDefinition.id,
        kind: null,
        name: null,
        symbolName: selectedDefinition.symbol,
        destinationName: formRenameSymbol.trim(),
        contents: null,
        sourceFile: null,
        sourceRange: null,
        companions: [],
      }, t("components-renamed-status", { name: formRenameSymbol.trim() }));
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
        symbolName: selectedDefinition.symbol,
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
        symbolName: selectedDefinition.symbol,
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

  async function copyCallExample() {
    if (!selectedCall) return;
    await navigator.clipboard?.writeText(selectedCall);
    copiedCall = true;
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
      <div><dt>{t("components-stat-invocations")}</dt><dd>{l10n.formatNumber(componentInvocationCount)}</dd></div>
    </dl>
  </header>

  <div class="workspace-toolbar">
    <SelectControl value={namespaceFilter} options={namespaceOptions} ariaLabel={t("components-filter-namespace")} onchange={(value) => { namespaceFilter = value; selectedDefinitionId = ""; }} />
    <SelectControl value={originFilter} options={originOptions} ariaLabel={t("components-filter-origin")} onchange={(value) => { originFilter = value; selectedDefinitionId = ""; }} />
    <label class="search-field">
      <span class="sr-only">{t("components-search-label")}</span>
      <IconSearch size={14} stroke={1.9} />
      <input class="ui-field toolbar" bind:value={query} type="search" placeholder={t("components-search-placeholder")} />
    </label>
    <button
      class="ui-button primary toolbar toolbar-action"
      type="button"
      disabled={mutating}
      onclick={beginCreate}
    >
      <IconPlus size={14} stroke={2} /> {t("components-add")}
    </button>
  </div>

  <div class="workspace-body">
    <div class="resource-list" aria-label={t("components-list-label")}>
      {#if !componentGraph}
        <EmptyState title={t("components-loading")} />
      {:else}
        {#each filteredDefinitions as definition (definition.id)}
          <button
            type="button"
            class="resource-card ui-entity-selectable"
            data-ui-selected={selectedDefinition?.id === definition.id ? "true" : undefined}
            aria-pressed={selectedDefinition?.id === definition.id}
            class:shadowed={!definition.active}
            onclick={() => selectDefinition(definition.id)}
          >
            <span class="resource-icon"><IconBraces size={17} stroke={1.8} /></span>
            <span>
              <strong>{definition.displayName}</strong>
              <small>{definition.file ?? definition.name}</small>
            </span>
            <span class="resource-badges">
              {#if componentNamespace(definition)}<code>{componentNamespace(definition)}</code>{/if}
              <code>{originLabel(definition.origin)}</code>
            </span>
          </button>
        {:else}
          <EmptyState title={t("components-empty-filter")} />
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
            <div class="wizard-name-grid">
              <TextFieldControl label={t("components-namespace")} description={t("components-namespace-help")} bind:value={formNamespace} disabled={mutating} placeholder="ui" />
              <TextFieldControl label={t("components-symbol-name")} description={t("components-symbol-name-help")} bind:value={formName} disabled={mutating} placeholder="card" />
            </div>

            <section class="wizard-section">
              <header>
                <div><strong>{t("components-typed-arguments")}</strong><span>{t("components-typed-arguments-help")}</span></div>
                <button class="ui-button compact" type="button" disabled={mutating} onclick={addArgument}><IconPlus size={13} /> {t("components-add-argument")}</button>
              </header>
              <div class="argument-list">
                {#each formArguments as argument (argument.id)}
                  <div class="argument-row">
                    <label><span>{t("components-argument-name")}</span><input class="ui-field" bind:value={argument.name} disabled={mutating} /></label>
                    <label><span>{t("components-argument-type")}</span><SelectControl value={argument.argumentType} options={[
                      { value: "string", label: "string" },
                      { value: "bool", label: "bool" },
                      { value: "integer", label: "integer" },
                      { value: "float", label: "float" },
                      { value: "array", label: "array" },
                      { value: "map", label: "map" },
                    ]} disabled={mutating} ariaLabel={t("components-argument-type")} onchange={(value) => updateArgumentType(argument.id, value)} /></label>
                    <CheckboxControl compact label={t("components-default-value")} bind:checked={argument.hasDefault} disabled={mutating} />
                    {#if argument.hasDefault}
                      <label><span>{t("components-default-value")}</span><input class="ui-field" bind:value={argument.defaultValue} disabled={mutating} /></label>
                    {/if}
                    <button class="ui-icon-button" type="button" aria-label={t("components-remove-argument", { name: argument.name })} disabled={mutating} onclick={() => removeArgument(argument.id)}><IconTrash size={13} /></button>
                  </div>
                {:else}
                  <span class="empty-inline">{t("components-no-arguments")}</span>
                {/each}
              </div>
              <div class="wizard-flags">
                <CheckboxControl compact label={t("components-rest-argument")} bind:checked={formRestEnabled} disabled={mutating} />
                {#if formRestEnabled}<input class="ui-field" bind:value={formRestName} disabled={mutating} aria-label={t("components-rest-name")} placeholder="attributes" />{/if}
                <CheckboxControl compact label={t("components-accepts-body")} bind:checked={formAcceptsBody} disabled={mutating} />
              </div>
            </section>

            <section class="code-preview">
              <strong>{t("components-generated-source")}</strong>
              <pre>{wizardSource}</pre>
              <strong>{t("components-call-example")}</strong>
              <pre>{wizardCall}</pre>
            </section>

            <details>
              <summary>{t("components-companions")}</summary>
              <div class="companion-fields">
                <section class="companion-field"><TextFieldControl label={t("components-style")} bind:value={formStylePath} disabled={mutating} placeholder={t("components-style-path-placeholder")} /><TextAreaControl label={t("components-source", { format: "SCSS" })} bind:value={formStyleSource} disabled={mutating} placeholder={".card { }"} rows={4} code spellcheck={false} /></section>
                <section class="companion-field"><TextFieldControl label={t("components-script")} bind:value={formScriptPath} disabled={mutating} placeholder="static/js/card.js" /><TextAreaControl label={t("components-source", { format: "JavaScript" })} bind:value={formScriptSource} disabled={mutating} placeholder={`// ${t("components-script-placeholder")}`} rows={4} code spellcheck={false} /></section>
                <section class="companion-field"><TextFieldControl label={t("components-canonical-data")} bind:value={formDataPath} disabled={mutating} placeholder={t("components-data-path-placeholder")} /><TextAreaControl label={t("components-source", { format: "TOML" })} bind:value={formDataSource} disabled={mutating} placeholder="[[items]]" rows={4} code spellcheck={false} /></section>
              </div>
            </details>
          {:else}
            <TextAreaControl label={t("components-source", { format: "HTML + Tera 2" })} bind:value={formSource} disabled={mutating || loadingSource} rows={18} code spellcheck={false} />
          {/if}

          {#if formError}<p class="ui-message error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>{/if}
          <div class="form-actions">
            <button class="ui-button compact" type="button" disabled={mutating} onclick={resetPanel}>{t("components-cancel")}</button>
            <button class="ui-button primary" type="submit" disabled={mutating || loadingSource || (detailMode === "create" && !wizardSymbol.trim())}>
              <IconDeviceFloppy size={14} />
              {mutating
                ? t("components-validating")
                : detailMode === "create"
                  ? t("components-create-session")
                  : t("components-save-changes")}
            </button>
          </div>
        </form>
      {:else if detailMode === "rename" && selectedDefinition}
        <form class="component-form" onsubmit={(event) => { event.preventDefault(); void renameSelected(); }}>
          <header class="detail-heading">
            <div>
              <span class="detail-kicker">{t("components-semantic-rename")}</span>
              <h2>{selectedDefinition.displayName}</h2>
              <p>{t("components-rename-description", { count: selectedInvocations.length })}</p>
            </div>
            <button class="ui-icon-button ui-close-button" type="button" aria-label={t("components-cancel")} disabled={mutating} onclick={resetPanel}><IconX size={14} /></button>
          </header>
          <TextFieldControl label={t("components-qualified-symbol")} bind:value={formRenameSymbol} disabled={mutating} placeholder="ui.card" />
          {#if formError}<p class="ui-message error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>{/if}
          <div class="form-actions">
            <button class="ui-button compact" type="button" disabled={mutating} onclick={resetPanel}>{t("components-cancel")}</button>
            <button class="ui-button primary" type="submit" disabled={mutating || !formRenameSymbol.trim()}><IconEdit size={14} /> {mutating ? t("components-validating") : t("components-rename")}</button>
          </div>
        </form>
      {:else if selectedDefinition}
        <div class="detail-kicker-row">
          <span class="detail-kicker">{selectedDefinition.symbol ?? selectedDefinition.name}</span>
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
                <code>{parameter.rest ? `...${parameter.name}` : parameter.name}</code>
                <span>
                  {parameter.argumentType ?? t("components-type-inferred")}
                  · {parameter.required ? t("components-required") : t("components-optional")}
                  {#if parameter.defaultValue} · {semanticDefaultPreview(parameter.defaultValue)}{/if}
                </span>
              </div>
            {/each}
          </section>
        {/if}

        <section class="detail-section call-example">
          <h3>{t("components-call-example")}</h3>
          <pre>{selectedCall}</pre>
          <button class="ui-button compact" type="button" disabled={!selectedCall} onclick={() => { void copyCallExample(); }}>
            {#if copiedCall}<IconCheck size={13} /> {t("components-call-copied")}{:else}<IconClipboard size={13} /> {t("components-copy-call")}{/if}
          </button>
        </section>

        <section class="detail-section">
          <h3>{t("components-find-usages", { count: selectedInvocations.length })}</h3>
          <div class="usage-list">
            {#each selectedInvocations as invocation (invocation.id)}
              <button class="usage-row" type="button" onclick={() => { void openWorkspaceSource(invocation.file, { surface: "code" }); }}>
                <IconLink size={13} />
                <span><strong>{invocation.file}</strong><small>{t("components-source-line", { line: invocation.range?.line ?? 1 })} · {invocation.status}</small></span>
                <IconExternalLink size={12} />
              </button>
            {:else}
              <span class="empty-inline">{t("components-no-usages")}</span>
            {/each}
          </div>
        </section>

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

        {#if formError}<p class="ui-message error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>{/if}
        <div class="detail-actions">
          {#if selectedDefinition.origin === "theme"}
            <button class="ui-button primary primary-action" type="button" disabled={mutating} onclick={() => { void overrideSelected(); }}>
              <IconCopy size={14} /> {t("components-create-override")}
            </button>
          {:else if selectedDefinition.file && selectedDefinition.capabilities.canEdit}
            <button class="ui-button primary primary-action" type="button" disabled={mutating} onclick={() => { void beginEdit(); }}>
              <IconEdit size={14} /> {t("components-edit")}
            </button>
          {/if}
          {#if selectedDefinition.file}
            <button class="ui-button compact" type="button" disabled={mutating} onclick={() => { void openWorkspaceSource(selectedDefinition.file!, { surface: "visual", componentName: selectedDefinition.symbol }); }}>
              <IconEye size={14} /> {t("components-preview")}
            </button>
            <button class="ui-button compact" type="button" disabled={mutating} onclick={() => { void openWorkspaceSource(selectedDefinition.file!, { surface: "code" }); }}>
              <IconExternalLink size={14} /> {t("components-open-source")}
            </button>
          {/if}
          {#if selectedDefinition.capabilities.canRename && selectedDefinition.origin === "project"}
            <button class="ui-button compact" type="button" disabled={mutating} onclick={beginRename}>
              <IconEdit size={14} /> {t("components-rename")}
            </button>
          {/if}
          {#if selectedDefinition.capabilities.canDelete && selectedDefinition.origin === "project"}
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
              <button class="ui-button compact" type="button" disabled={mutating} onclick={() => { deleteConfirmationOpen = false; }}>{t("components-cancel")}</button>
              <button class="ui-button danger" type="button" disabled={mutating} onclick={() => { void deleteSelected(); }}>
                {mutating ? t("components-checking") : t("components-remove-session")}
              </button>
            </div>
          </div>
        {/if}
      {:else}
        <EmptyState title={t("components-select-help")} />
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
  .wizard-name-grid { display: grid; grid-template-columns: minmax(0, .7fr) minmax(0, 1fr); gap: 9px; }
  .wizard-section, .code-preview { display: grid; gap: 9px; padding: 10px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .wizard-section > header { display: flex; align-items: center; justify-content: space-between; gap: 10px; }
  .wizard-section > header > div { display: grid; gap: 2px; }
  .wizard-section strong, .code-preview strong { color: var(--text-strong); font-size: 11px; }
  .wizard-section header span, .empty-inline { color: var(--wb-text-muted); font-size: 11px; line-height: 1.4; }
  .argument-list { display: grid; gap: 7px; }
  .argument-row { display: grid; grid-template-columns: minmax(90px, 1fr) minmax(90px, .8fr) auto minmax(90px, .8fr) auto; align-items: end; gap: 6px; }
  .argument-row > label { display: grid; gap: 4px; min-width: 0; color: var(--wb-text-muted); font-size: 11px; }
  .wizard-flags { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 12px; padding-top: 8px; border-top: 1px solid var(--wb-border-subtle); color: var(--wb-text-muted); font-size: 11px; }
  .wizard-flags .ui-field { width: 130px; }
  .code-preview pre, .call-example pre { overflow: auto; margin: 0; padding: 8px; border-radius: 5px; color: var(--wb-text-primary); background: var(--wb-surface-chrome); font: 11px/1.5 var(--font-mono); white-space: pre-wrap; overflow-wrap: anywhere; }
  .call-example .ui-button { margin-top: 7px; }
  .usage-list { display: grid; gap: 5px; }
  .usage-row { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: center; gap: 7px; width: 100%; padding: 7px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-muted); background: var(--wb-surface-document); text-align: left; cursor: pointer; }
  .usage-row:hover { border-color: var(--wb-border-strong); background: var(--wb-control-hover); }
  .usage-row > span { display: grid; gap: 2px; min-width: 0; }
  .usage-row strong { overflow: hidden; color: var(--text-strong); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .usage-row small { color: var(--wb-text-muted); font-size: 11px; }
  details { overflow: hidden; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  summary { min-height: 34px; padding: 9px 10px; color: var(--text-strong); font-size: 11px; font-weight: 700; cursor: pointer; }
  .companion-fields { display: grid; gap: 12px; padding: 10px; border-top: 1px solid var(--wb-border-subtle); }
  .companion-field { display: grid; gap: 8px; }
  .form-actions { justify-content: flex-end; gap: 7px; margin-top: 4px; }
  .detail-actions { flex-wrap: wrap; align-items: stretch; gap: 7px; margin-top: 14px; }
  .detail-actions button { flex: 1 1 130px; }
  .delete-confirmation { display: grid; gap: 6px; margin-top: 9px; padding: 10px; border: 1px solid color-mix(in srgb, var(--danger) 34%, var(--wb-border-subtle)); border-radius: 7px; background: var(--wb-surface-document); }
  .delete-confirmation strong { color: var(--text-strong); font-size: 12px; }
  .delete-confirmation > span { color: var(--wb-text-muted); font-size: 11px; line-height: 1.4; }
  .delete-confirmation > div { display: flex; justify-content: flex-end; gap: 7px; }
  summary:focus-visible { outline: 2px solid var(--wb-focus-ring); outline-offset: 1px; }
  .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; }
  @media (max-width: 1050px) { .workspace-header dl { grid-template-columns: repeat(2, 70px); } .argument-row { grid-template-columns: 1fr 1fr; } }
  @media (max-width: 900px) { .workspace-body { grid-template-columns: 1fr; } .resource-detail { display: none; } .resource-list { border-right: 0; } }
</style>
