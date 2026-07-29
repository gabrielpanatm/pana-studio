<script lang="ts">
  import {
    IconAlertTriangle,
    IconBraces,
    IconChevronRight,
    IconDatabase,
    IconDeviceFloppy,
    IconEdit,
    IconExternalLink,
    IconFileDatabase,
    IconHash,
    IconList,
    IconMessage,
    IconPlus,
    IconSearch,
    IconTrash,
    IconX,
  } from "@tabler/icons-svelte";
  import { applyDataMutation, readDataNodeEditor } from "$lib/project/io";
  import { t } from "$lib/i18n/runtime.svelte";
  import { sourceCapabilityReason } from "$lib/source-graph/capabilities";
  import { settleProjectWorkspaceMutation } from "$lib/session/workspace-mutation-coordinator";
  import type { AppState } from "$lib/state/app.svelte";
  import type {
    DataDraftKind,
    DataMutationInput,
    DataNodeEditorSnapshot,
    FileBufferRequestIdentity,
    SourceDataNode,
    SourceGraphDataFile,
  } from "$lib/types";
  import { errorMessage } from "$lib/util";

  let {
    app,
    openWorkspaceSource,
  }: {
    app: AppState;
    openWorkspaceSource: (path: string) => void | Promise<void>;
  } = $props();

  type DataView = "all" | "toml" | "other";
  type DetailMode = "info" | "create" | "edit";
  type NodeRow = { node: SourceDataNode; depth: number };

  const views = $derived<{ id: DataView; label: string }[]>([
    { id: "all", label: t("data-all") },
    { id: "toml", label: t("data-toml") },
    { id: "other", label: t("data-other-formats") },
  ]);
  const scalarKinds = $derived<{ id: DataDraftKind; label: string }[]>([
    { id: "string", label: t("data-kind-text") },
    { id: "integer", label: t("data-kind-integer") },
    { id: "float", label: t("data-kind-decimal") },
    { id: "boolean", label: t("data-kind-boolean") },
    { id: "datetime", label: t("data-kind-datetime") },
  ]);

  let activeView = $state<DataView>("all");
  let detailMode = $state<DetailMode>("info");
  let query = $state("");
  let selectedFileId = $state("");
  let selectedNodeId = $state("");
  let newFileName = $state("");
  let mutating = $state(false);
  let formError = $state("");
  let nodeEditor = $state<DataNodeEditorSnapshot | null>(null);
  let nodeEditorLoading = $state(false);
  let nodeLoadSequence = 0;
  let draftKey = $state("");
  let draftKind = $state<DataDraftKind>("string");
  let draftValue = $state("");
  let insertKey = $state("");
  let insertKind = $state<DataDraftKind>("string");
  let insertValue = $state("");
  let deleteConfirmationOpen = $state(false);

  const dataFiles = $derived(app.sourceGraph?.dataFiles ?? []);
  const normalizedQuery = $derived(query.trim().toLocaleLowerCase(app.uiLocale));
  const filteredFiles = $derived(
    dataFiles
      .filter((file) => (
        (activeView === "all"
          || activeView === "toml" && file.format === "toml"
          || activeView === "other" && file.format !== "toml")
        && (!normalizedQuery
          || `${file.file} ${file.logicalPath} ${file.loadPaths.join(" ")} ${file.location} ${file.format}`
            .toLocaleLowerCase(app.uiLocale)
            .includes(normalizedQuery))
      ))
      .sort((left, right) => left.logicalPath.localeCompare(right.logicalPath, app.uiLocale)),
  );
  const selectedFile = $derived(
    dataFiles.find((file) => file.id === selectedFileId)
      ?? filteredFiles[0]
      ?? null,
  );
  const selectedRows = $derived(selectedFile ? flattenNodes(selectedFile) : []);
  const selectedNode = $derived(
    selectedFile?.nodes.find((node) => node.id === selectedNodeId)
      ?? selectedFile?.nodes.find((node) => node.kind === "document")
      ?? null,
  );
  const allTableCount = $derived(
    dataFiles.reduce((count, file) => (
      count + file.nodes.filter((node) => (
        node.kind === "table" || node.kind === "inlineTable" || node.kind === "tableElement"
      )).length
    ), 0),
  );
  const allListCount = $derived(
    dataFiles.reduce((count, file) => (
      count + file.nodes.filter((node) => node.kind === "array" || node.kind === "arrayOfTables").length
    ), 0),
  );
  const allValueCount = $derived(
    dataFiles.reduce((count, file) => (
      count + file.nodes.filter((node) => node.kind === "value" || node.kind === "arrayElement").length
    ), 0),
  );
  const insertKinds = $derived(availableInsertKinds(selectedNode));
  const canUpdateSelected = $derived(Boolean(
    nodeEditor
      && (nodeEditor.editableKey || nodeEditor.editableValue)
      && (!nodeEditor.editableKey || draftKey.length > 0)
      && (
        nodeEditor.editableKey && draftKey !== (nodeEditor.key ?? "")
        || nodeEditor.editableValue && (
          draftKind !== nodeEditor.draftKind
          || draftValue !== (nodeEditor.value ?? "")
        )
      )
  ));

  $effect(() => {
    if (!filteredFiles.some((file) => file.id === selectedFileId)) {
      selectedFileId = filteredFiles[0]?.id ?? "";
    }
  });

  $effect(() => {
    const file = selectedFile;
    if (!file) {
      selectedNodeId = "";
      return;
    }
    if (!file.nodes.some((node) => node.id === selectedNodeId)) {
      selectedNodeId = file.nodes.find((node) => node.kind === "document")?.id ?? "";
    }
  });

  $effect(() => {
    const file = selectedFile;
    const node = selectedNode;
    const projectRoot = app.sessionProjectRoot;
    const sessionId = app.kernelProjectSessionId;
    if (
      detailMode !== "edit"
      || !file
      || file.format !== "toml"
      || !file.capabilities.canEditVisual
      || !node
      || !projectRoot
      || !sessionId
    ) {
      nodeEditor = null;
      nodeEditorLoading = false;
      return;
    }
    const requestId = ++nodeLoadSequence;
    nodeEditorLoading = true;
    formError = "";
    void readDataNodeEditor(file.file, node.id, identity())
      .then((snapshot) => {
        if (
          requestId !== nodeLoadSequence
          || app.sessionProjectRoot !== projectRoot
          || app.kernelProjectSessionId !== sessionId
        ) return;
        nodeEditor = snapshot;
        draftKey = snapshot.key ?? "";
        draftKind = snapshot.draftKind ?? "string";
        draftValue = snapshot.value ?? "";
      })
      .catch((cause) => {
        if (requestId === nodeLoadSequence) {
          nodeEditor = null;
          formError = errorMessage(cause);
        }
      })
      .finally(() => {
        if (requestId === nodeLoadSequence) nodeEditorLoading = false;
      });
  });

  function identity(): FileBufferRequestIdentity {
    return {
      expectedProjectRoot: app.sessionProjectRoot,
      expectedSessionId: app.kernelProjectSessionId,
    };
  }

  function flattenNodes(file: SourceGraphDataFile): NodeRow[] {
    const byId = new Map(file.nodes.map((node) => [node.id, node]));
    const root = file.nodes.find((node) => node.kind === "document");
    if (!root) return file.nodes.map((node) => ({ node, depth: 0 }));
    const rows: NodeRow[] = [{ node: root, depth: 0 }];
    const append = (node: SourceDataNode, depth: number) => {
      for (const childId of node.children) {
        const child = byId.get(childId);
        if (!child) continue;
        rows.push({ node: child, depth });
        append(child, depth + 1);
      }
    };
    append(root, 1);
    return rows;
  }

  function nodeLabel(node: SourceDataNode) {
    if (node.kind === "document") return t("data-node-document");
    if (node.kind === "comment") return node.valuePreview || t("data-node-comment");
    if (node.kind === "tableElement" || node.kind === "arrayElement") {
      const index = node.path.at(-1);
      return index?.kind === "index"
        ? t("data-node-element-index", { index: index.value + 1 })
        : t("data-node-element");
    }
    return node.key || t("data-node-value");
  }

  function nodeKindLabel(node: SourceDataNode) {
    if (node.kind === "document") return t("data-kind-document");
    if (node.kind === "table") return t("data-kind-table");
    if (node.kind === "inlineTable") return t("data-kind-inline-table");
    if (node.kind === "arrayOfTables") return t("data-kind-table-collection");
    if (node.kind === "tableElement") return t("data-kind-row");
    if (node.kind === "array") return t("data-kind-list");
    if (node.kind === "arrayElement") return valueKindLabel(node.valueKind);
    if (node.kind === "comment") return t("data-node-comment");
    return valueKindLabel(node.valueKind);
  }

  function valueKindLabel(kind: SourceDataNode["valueKind"]) {
    if (kind === "string") return t("data-kind-text");
    if (kind === "integer") return t("data-kind-integer");
    if (kind === "float") return t("data-kind-decimal");
    if (kind === "boolean") return t("data-kind-boolean");
    if (kind === "datetime") return t("data-kind-datetime");
    if (kind === "array") return t("data-kind-list");
    if (kind === "inlineTable") return t("data-kind-inline-table");
    if (kind === "table" || kind === "arrayOfTables") return t("data-kind-table");
    return t("data-node-value");
  }

  function nodeIcon(node: SourceDataNode) {
    if (node.kind === "comment") return IconMessage;
    if (node.kind === "array" || node.kind === "arrayOfTables") return IconList;
    if (
      node.kind === "table"
      || node.kind === "tableElement"
      || node.kind === "inlineTable"
      || node.kind === "document"
    ) return IconBraces;
    return IconHash;
  }

  function relationCount(file: SourceGraphDataFile) {
    return (app.sourceGraph?.relations ?? []).filter(
      (relation) => relation.from === file.nodeId || relation.to === file.nodeId,
    ).length;
  }

  function countNodes(file: SourceGraphDataFile, kinds: SourceDataNode["kind"][]) {
    return file.nodes.filter((node) => kinds.includes(node.kind)).length;
  }

  function selectView(view: DataView) {
    activeView = view;
    resetPanel();
  }

  function selectFile(file: SourceGraphDataFile) {
    selectedFileId = file.id;
    selectedNodeId = file.nodes.find((node) => node.kind === "document")?.id ?? "";
    resetPanel();
  }

  function selectNode(node: SourceDataNode) {
    selectedNodeId = node.id;
    formError = "";
    deleteConfirmationOpen = false;
    insertKey = "";
    insertKind = node.kind === "arrayOfTables" ? "table" : "string";
    insertValue = "";
  }

  function resetPanel() {
    detailMode = "info";
    formError = "";
    deleteConfirmationOpen = false;
    nodeEditor = null;
    newFileName = "";
  }

  function beginCreate() {
    detailMode = "create";
    formError = "";
    newFileName = "date/";
  }

  function beginEdit(file: SourceGraphDataFile) {
    if (!file.capabilities.canEditVisual || file.format !== "toml" || file.parseError) return;
    detailMode = "edit";
    formError = "";
    selectedNodeId = file.nodes.find((node) => node.kind === "document")?.id ?? "";
  }

  function canonicalFilePath(value: string) {
    let normalized = value.trim().replaceAll("\\", "/").replace(/^\/+/, "");
    if (normalized.endsWith("/")) return "";
    if (!normalized) return "";
    if (!normalized.endsWith(".toml")) normalized = `${normalized}.toml`;
    return normalized;
  }

  function locationLabel(file: SourceGraphDataFile) {
    if (file.location === "date") return t("data-location-date");
    if (file.location === "static") return t("data-location-static");
    if (file.location === "content") return t("data-location-content");
    if (file.location === "output") return t("data-location-output");
    if (file.location === "theme") return t("data-location-theme");
    return t("data-location-project-root");
  }

  function originLabel(file: SourceGraphDataFile) {
    if (file.origin === "theme") {
      return file.themeName
        ? t("data-origin-theme", { theme: file.themeName })
        : t("data-origin-active-theme");
    }
    return t("data-origin-local");
  }

  function availableInsertKinds(node: SourceDataNode | null) {
    if (!node) return [];
    if (node.kind === "arrayOfTables") {
      return [{ id: "table" as const, label: t("data-kind-new-row") }];
    }
    const values = [...scalarKinds];
    values.push({ id: "array", label: t("data-kind-list") });
    values.push({ id: "inline_table", label: t("data-kind-inline-table") });
    if (node.kind === "document" || node.kind === "table" || node.kind === "tableElement") {
      values.push({ id: "table", label: t("data-kind-table") });
      values.push({ id: "array_of_tables", label: t("data-kind-table-collection") });
    }
    return values;
  }

  function acceptsChildren(node: SourceDataNode | null) {
    return Boolean(node && [
      "document",
      "table",
      "tableElement",
      "inlineTable",
      "array",
      "arrayOfTables",
    ].includes(node.kind));
  }

  function childNeedsKey(node: SourceDataNode | null) {
    return Boolean(node && [
      "document",
      "table",
      "tableElement",
      "inlineTable",
    ].includes(node.kind));
  }

  function draftNeedsValue(kind: DataDraftKind) {
    return scalarKinds.some((entry) => entry.id === kind);
  }

  function draftKindLabel(kind: DataDraftKind) {
    return [...scalarKinds, ...availableInsertKinds(selectedNode)]
      .find((entry) => entry.id === kind)?.label ?? kind;
  }

  function updateDraftKind(kind: DataDraftKind) {
    draftKind = kind;
    if (kind === "boolean" && draftValue !== "true" && draftValue !== "false") {
      draftValue = "false";
    }
  }

  function updateInsertKind(kind: DataDraftKind) {
    insertKind = kind;
    if (kind === "boolean" && insertValue !== "true" && insertValue !== "false") {
      insertValue = "false";
    }
  }

  async function applyMutation(input: DataMutationInput, successMessage: string) {
    if (mutating) return;
    mutating = true;
    formError = "";
    try {
      const receipt = await applyDataMutation(input, identity());
      const settlement = await settleProjectWorkspaceMutation(app, receipt.workspace, {
        preferredRelativePath: receipt.workspace.relativePath,
        warningLabel: t("data-mutation-label"),
      });
      const refreshed = app.sourceGraph?.dataFiles.find((file) => file.file === receipt.plan.file);
      selectedFileId = refreshed?.id ?? "";
      selectedNodeId = refreshed?.nodes.find((node) => node.kind === "document")?.id ?? "";
      app.setGlobalStatus(
        settlement.warnings.length > 0
          ? t("data-mutation-needs-resync", { success: successMessage })
          : t("data-mutation-session-only", { success: successMessage }),
        "unsaved",
      );
      return true;
    } catch (cause) {
      formError = errorMessage(cause);
      return false;
    } finally {
      mutating = false;
    }
  }

  async function createFile(event: SubmitEvent) {
    event.preventDefault();
    const path = canonicalFilePath(newFileName);
    if (!path || path === "date.toml") {
      formError = t("data-file-path-required");
      return;
    }
    const applied = await applyMutation({
      operation: "create_file",
      file: path,
      nodeId: null,
      key: null,
      draftKind: null,
      value: "",
    }, t("data-file-created", { path }));
    if (applied) detailMode = "info";
  }

  async function updateNode(event: SubmitEvent) {
    event.preventDefault();
    if (!selectedFile || !selectedNode || !nodeEditor) return;
    await applyMutation({
      operation: "update_node",
      file: selectedFile.file,
      nodeId: selectedNode.id,
      key: nodeEditor.editableKey ? draftKey : null,
      draftKind: nodeEditor.editableValue ? draftKind : null,
      value: nodeEditor.editableValue ? draftValue : null,
    }, t("data-node-updated", { node: nodeLabel(selectedNode) }));
  }

  async function insertChild(event: SubmitEvent) {
    event.preventDefault();
    if (!selectedFile || !selectedNode) return;
    const applied = await applyMutation({
      operation: "insert_child",
      file: selectedFile.file,
      nodeId: selectedNode.id,
      key: childNeedsKey(selectedNode) ? insertKey : null,
      draftKind: selectedNode.kind === "arrayOfTables" ? "table" : insertKind,
      value: draftNeedsValue(insertKind) ? insertValue : null,
    }, t("data-node-inserted", { node: nodeLabel(selectedNode) }));
    if (applied) {
      detailMode = "edit";
      insertKey = "";
      insertValue = "";
    }
  }

  async function deleteNode() {
    if (!selectedFile || !selectedNode) return;
    const applied = await applyMutation({
      operation: "delete_node",
      file: selectedFile.file,
      nodeId: selectedNode.id,
      key: null,
      draftKind: null,
      value: null,
    }, t("data-node-deleted", { node: nodeLabel(selectedNode) }));
    if (applied) {
      detailMode = "edit";
      deleteConfirmationOpen = false;
    }
  }

  async function openSource(file: SourceGraphDataFile) {
    if (!file.capabilities.canOpenInCode) return;
    await openWorkspaceSource(file.file);
    await app.setWorkbenchActivity("editor");
  }

  function handleViewKeydown(event: KeyboardEvent, index: number) {
    let nextIndex: number | null = null;
    if (event.key === "ArrowLeft") nextIndex = (index - 1 + views.length) % views.length;
    if (event.key === "ArrowRight") nextIndex = (index + 1) % views.length;
    if (event.key === "Home") nextIndex = 0;
    if (event.key === "End") nextIndex = views.length - 1;
    if (nextIndex === null) return;
    event.preventDefault();
    const next = views[nextIndex];
    if (!next) return;
    selectView(next.id);
    requestAnimationFrame(() => document.getElementById(`data-tab-${next.id}`)?.focus());
  }
</script>

<section class="activity-workspace data-workspace" aria-labelledby="data-title">
  <header class="workspace-header">
    <div>
      <span class="eyebrow"><IconDatabase size={15} stroke={1.9} /> {t("data-eyebrow")}</span>
      <h1 id="data-title">{t("data-title")}</h1>
      <p>{t("data-description")}</p>
    </div>
    <dl>
      <div><dt>{t("data-files")}</dt><dd>{dataFiles.length}</dd></div>
      <div><dt>{t("data-tables")}</dt><dd>{allTableCount}</dd></div>
      <div><dt>{t("data-lists")}</dt><dd>{allListCount}</dd></div>
      <div><dt>{t("data-values")}</dt><dd>{allValueCount}</dd></div>
    </dl>
  </header>

  <div class="workspace-toolbar">
    <div class="ui-tabs view-tabs" role="tablist" aria-label={t("data-formats-label")}>
      {#each views as view, index (view.id)}
        <button
          id={`data-tab-${view.id}`}
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
      <IconSearch size={14} />
      <span class="sr-only">{t("data-search-files")}</span>
      <input class="ui-field toolbar" bind:value={query} type="search" placeholder={t("data-search")} />
    </label>
    <button class="ui-button primary toolbar toolbar-action" type="button" onclick={beginCreate}>
      <IconPlus size={14} /> {t("data-add")}
    </button>
  </div>

  <div class="workspace-body">
    <div class="data-list" role="listbox" aria-label={t("data-files-label")}>
      {#each filteredFiles as file (file.id)}
        <button
          class="resource-card ui-entity-selectable"
          data-ui-selected={selectedFile?.id === file.id ? "true" : undefined}
          type="button"
          role="option"
          aria-selected={selectedFile?.id === file.id}
          onclick={() => selectFile(file)}
        >
          <span class="resource-icon"><IconFileDatabase size={17} /></span>
          <span class="resource-main">
            <strong>{file.logicalPath.split("/").at(-1)}</strong>
            <small>{file.file}</small>
          </span>
          <span class="resource-meta">
            <small>{t("data-values-count", { count: countNodes(file, ["value", "arrayElement"]) })}</small>
            <span title={locationLabel(file)}>{file.format.toUpperCase()}</span>
          </span>
        </button>
      {:else}
        <div class="workspace-state">
          <IconDatabase size={24} />
          <strong>{t("data-empty-title")}</strong>
          <span>{t("data-empty-description")}</span>
        </div>
      {/each}
    </div>

    <aside class="detail-panel" aria-live="polite">
      {#if detailMode === "create"}
        <form class="detail-form compact-form" onsubmit={createFile}>
          <header class="detail-header">
            <div>
              <span class="detail-kicker">{t("data-new-file")}</span>
              <h2>{t("data-toml-data")}</h2>
              <p>{t("data-new-file-description")}</p>
            </div>
            <button class="ui-icon-button ui-close-button icon-button" type="button" aria-label={t("data-close")} onclick={resetPanel}>
              <IconX size={16} />
            </button>
          </header>
          <label>
            <span>{t("data-project-relative-path")}</span>
            <input bind:value={newFileName} disabled={mutating} placeholder={t("data-new-file-placeholder")} />
            <small>
              {t("data-new-file-path-help")}
            </small>
          </label>
          {#if formError}
            <p class="ui-message error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>
          {/if}
          <div class="form-actions">
            <button type="button" disabled={mutating} onclick={resetPanel}>{t("data-cancel")}</button>
            <button class="ui-button primary" type="submit" disabled={mutating || !canonicalFilePath(newFileName)}>
              <IconPlus size={14} /> {mutating ? t("data-validating") : t("data-create-file")}
            </button>
          </div>
        </form>
      {:else if selectedFile && detailMode === "edit"}
        <div class="visual-editor">
          <header class="detail-header editor-header">
            <div>
              <span class="detail-kicker">{t("data-visual-editing")}</span>
              <h2>{selectedFile.logicalPath}</h2>
              <p>{t("data-visual-editing-description")}</p>
            </div>
            <button class="ui-icon-button ui-close-button icon-button" type="button" aria-label={t("data-close-editor")} onclick={resetPanel}>
              <IconX size={16} />
            </button>
          </header>

          <div class="editor-body">
            <div class="node-tree" role="tree" aria-label={t("data-structure-label", { file: selectedFile.logicalPath })}>
              {#each selectedRows as row (row.node.id)}
                {@const NodeIcon = nodeIcon(row.node)}
                <button
                  type="button"
                  class="ui-entity-selectable"
                  role="treeitem"
                  aria-selected={selectedNode?.id === row.node.id}
                  data-ui-selected={selectedNode?.id === row.node.id ? "true" : undefined}
                  class:comment={row.node.kind === "comment"}
                  onclick={() => selectNode(row.node)}
                >
                  <span class="tree-indent" aria-hidden="true">{"· ".repeat(row.depth)}</span>
                  <NodeIcon size={14} />
                  <span>
                    <strong>{nodeLabel(row.node)}</strong>
                    <small>{nodeKindLabel(row.node)}{row.node.valuePreview ? ` · ${row.node.valuePreview}` : ""}</small>
                  </span>
                  <IconChevronRight class="row-chevron" size={13} />
                </button>
              {/each}
            </div>

            <div class="node-editor">
              {#if selectedNode}
                <div class="node-editor-title">
                  <div>
                    <span class="detail-kicker">{nodeKindLabel(selectedNode)}</span>
                    <h3>{nodeLabel(selectedNode)}</h3>
                  </div>
                  <code>{selectedNode.path.map((part) => part.value).join(" › ") || t("data-root")}</code>
                </div>

                {#if nodeEditorLoading}
                  <div class="workspace-state compact">{t("data-loading-exact-value")}</div>
                {:else if nodeEditor && (nodeEditor.editableKey || nodeEditor.editableValue)}
                  <form class="node-form" onsubmit={updateNode}>
                    {#if nodeEditor.editableKey}
                      <label>
                        <span>{t("data-key")}</span>
                        <input bind:value={draftKey} disabled={mutating} />
                      </label>
                    {/if}
                    {#if nodeEditor.editableValue}
                      <label>
                        <span>{t("data-type")}</span>
                        <select
                          value={draftKind}
                          disabled={mutating}
                          onchange={(event) => updateDraftKind(event.currentTarget.value as DataDraftKind)}
                        >
                          {#each scalarKinds as kind (kind.id)}
                            <option value={kind.id}>{kind.label}</option>
                          {/each}
                        </select>
                      </label>
                      {#if draftKind === "boolean"}
                        <label class="boolean-field">
                          <input
                            type="checkbox"
                            checked={draftValue === "true"}
                            disabled={mutating}
                            onchange={(event) => { draftValue = event.currentTarget.checked ? "true" : "false"; }}
                          />
                          <span>{t("data-active-value")}</span>
                        </label>
                      {:else}
                        <label>
                          <span>{t("data-value-with-kind", { kind: draftKindLabel(draftKind).toLocaleLowerCase(app.uiLocale) })}</span>
                          <input bind:value={draftValue} disabled={mutating} />
                        </label>
                      {/if}
                    {/if}
                    <button class="primary full-action" type="submit" disabled={mutating || !canUpdateSelected}>
                      <IconDeviceFloppy size={14} /> {mutating ? t("data-validating") : t("data-save-node")}
                    </button>
                  </form>
                {:else if selectedNode.kind === "comment"}
                  <p class="context-note">{t("data-comments-code-only")}</p>
                {/if}

                {#if acceptsChildren(selectedNode)}
                  <form class="insert-form" onsubmit={insertChild}>
                    <div>
                      <span class="detail-kicker">{t("data-add-to-selection")}</span>
                      <h3>{selectedNode.kind === "arrayOfTables" ? t("data-kind-new-row") : t("data-new-element")}</h3>
                    </div>
                    {#if childNeedsKey(selectedNode)}
                      <label>
                        <span>{t("data-key")}</span>
                        <input bind:value={insertKey} disabled={mutating} placeholder={t("data-new-key-placeholder")} />
                      </label>
                    {/if}
                    {#if selectedNode.kind !== "arrayOfTables"}
                      <label>
                        <span>{t("data-type")}</span>
                        <select
                          value={insertKind}
                          disabled={mutating}
                          onchange={(event) => updateInsertKind(event.currentTarget.value as DataDraftKind)}
                        >
                          {#each insertKinds as kind (kind.id)}
                            <option value={kind.id}>{kind.label}</option>
                          {/each}
                        </select>
                      </label>
                    {/if}
                    {#if draftNeedsValue(insertKind) && selectedNode.kind !== "arrayOfTables"}
                      {#if insertKind === "boolean"}
                        <label class="boolean-field">
                          <input
                            type="checkbox"
                            checked={insertValue === "true"}
                            disabled={mutating}
                            onchange={(event) => { insertValue = event.currentTarget.checked ? "true" : "false"; }}
                          />
                          <span>{t("data-active-value")}</span>
                        </label>
                      {:else}
                        <label>
                          <span>{t("data-value")}</span>
                          <input bind:value={insertValue} disabled={mutating} />
                        </label>
                      {/if}
                    {/if}
                    <button
                      type="submit"
                      disabled={mutating || (childNeedsKey(selectedNode) && !insertKey)}
                    ><IconPlus size={14} /> {t("data-add-action")}</button>
                  </form>
                {/if}

                {#if !["document", "comment", "opaque"].includes(selectedNode.kind)}
                  <div class="danger-zone">
                    {#if deleteConfirmationOpen}
                      <p>{t("data-delete-confirmation", { node: nodeLabel(selectedNode) })}</p>
                      <div>
                        <button type="button" disabled={mutating} onclick={() => { deleteConfirmationOpen = false; }}>{t("data-cancel")}</button>
                        <button class="ui-button danger" type="button" disabled={mutating} onclick={() => { void deleteNode(); }}>
                          <IconTrash size={14} /> {mutating ? t("data-checking") : t("data-delete")}
                        </button>
                      </div>
                    {:else}
                      <button class="danger-link" type="button" onclick={() => { deleteConfirmationOpen = true; }}>
                        <IconTrash size={14} /> {t("data-delete-node")}
                      </button>
                    {/if}
                  </div>
                {/if}
              {/if}

              {#if formError}
                <p class="ui-message error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>
              {/if}
            </div>
          </div>
        </div>
      {:else if selectedFile}
        <div class="file-details">
          <div class="detail-kicker-row">
            <span class="detail-kicker">{selectedFile.format.toUpperCase()}</span>
            <span>{locationLabel(selectedFile)}</span>
            <span>{t("data-origin-label", { origin: originLabel(selectedFile) })}</span>
            <span>{selectedFile.capabilities.canEditVisual ? t("data-visually-editable") : t("data-read-only")}</span>
          </div>
          <header class="detail-header">
            <div>
              <h2>{selectedFile.logicalPath.split("/").at(-1)}</h2>
              <p><code>{selectedFile.file}</code></p>
            </div>
            {#if selectedFile.capabilities.canOpenInCode}
              <button type="button" onclick={() => { void openSource(selectedFile); }}>
                <IconExternalLink size={14} /> {t("data-open-in-editor")}
              </button>
            {/if}
          </header>

          {#if selectedFile.parseError}
            <p class="ui-message error"><IconAlertTriangle size={14} /> {selectedFile.parseError}</p>
          {/if}

          <dl class="detail-stats">
            <div><dt>{t("data-tables")}</dt><dd>{countNodes(selectedFile, ["table", "inlineTable", "tableElement"])}</dd></div>
            <div><dt>{t("data-lists")}</dt><dd>{countNodes(selectedFile, ["array", "arrayOfTables"])}</dd></div>
            <div><dt>{t("data-values")}</dt><dd>{countNodes(selectedFile, ["value", "arrayElement"])}</dd></div>
            <div><dt>{t("data-links")}</dt><dd>{relationCount(selectedFile)}</dd></div>
          </dl>

          {#if selectedFile.loadPaths.length > 0}
            <div class="load-paths">
              <h3>{t("data-load-data-paths")}</h3>
              {#each selectedFile.loadPaths as loadPath (loadPath)}
                <code>{loadPath}</code>
              {/each}
            </div>
          {/if}

          <div class="info-tree">
            <h3>{t("data-semantic-structure")}</h3>
            {#each selectedRows.slice(0, 24) as row (row.node.id)}
              {@const NodeIcon = nodeIcon(row.node)}
              <div class="info-node">
                <span class="tree-indent" aria-hidden="true">{"· ".repeat(row.depth)}</span>
                <NodeIcon size={13} />
                <strong>{nodeLabel(row.node)}</strong>
                <span>{nodeKindLabel(row.node)}</span>
                {#if row.node.valuePreview}<code>{row.node.valuePreview}</code>{/if}
              </div>
            {/each}
            {#if selectedRows.length > 24}<small>{t("data-more-nodes", { count: selectedRows.length - 24 })}</small>{/if}
          </div>

          {#if selectedFile.capabilities.canEditVisual && selectedFile.format === "toml" && !selectedFile.parseError}
            <button class="primary full-action" type="button" onclick={() => beginEdit(selectedFile)}>
              <IconEdit size={14} /> {t("data-edit-visually")}
            </button>
          {:else}
            <p class="context-note">
              {selectedFile.parseError && selectedFile.format === "toml"
                ? t("data-fix-syntax-before-visual")
                : sourceCapabilityReason(selectedFile.capabilities, "data-read-only-reason")}
            </p>
          {/if}
        </div>
      {:else}
        <div class="workspace-state">{t("data-select-or-create")}</div>
      {/if}
    </aside>
  </div>
</section>

<style>
  .detail-kicker { display: inline-flex; align-items: center; gap: 6px; color: var(--wb-accent-strong); font-size: 11px; font-weight: 800; letter-spacing: .035em; text-transform: uppercase; }
  h2 { margin: 4px 0 0; color: var(--text-strong); font-size: 20px; }
  h3 { margin: 3px 0 0; color: var(--text-strong); font-size: 14px; }
  p { margin: 4px 0 0; color: var(--wb-text-muted); font-size: 12px; line-height: 1.45; }
  code { font-family: var(--font-mono); font-size: 11px; }
  dt { color: var(--wb-text-muted); font-size: var(--font-meta); font-weight: 800; text-transform: uppercase; }
  dd { margin: 3px 0 0; color: var(--text-strong); font-size: 16px; font-weight: 750; }
  .workspace-body { display: grid; grid-template-columns: minmax(310px, .72fr) minmax(500px, 1.28fr); min-width: 0; min-height: 0; }
  .data-list { min-width: 0; min-height: 0; overflow: auto; padding: 9px; border-right: 1px solid var(--wb-border-subtle); }
  .resource-icon { display: grid; flex: 0 0 auto; width: 30px; height: 30px; place-items: center; border-radius: 7px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .resource-main { display: grid; min-width: 0; gap: 3px; }
  .resource-main strong, .resource-main small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .resource-main strong { color: var(--text-strong); font-size: 12px; }
  .resource-main small { color: var(--wb-text-muted); font-size: 11px; }
  .resource-meta { display: grid; flex: 0 0 auto; justify-items: end; gap: 4px; margin-left: auto; }
  .resource-meta small { color: var(--wb-text-muted); font-size: 11px; }
  .resource-meta span, .detail-kicker-row > span:last-child { padding: 2px 6px; border-radius: 4px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); font-size: 11px; font-weight: 750; }
  .detail-panel { min-width: 0; min-height: 0; overflow: auto; background: var(--wb-surface-document); }
  .file-details, .detail-form { display: grid; align-content: start; gap: 14px; padding: 17px; }
  .detail-header { display: flex; align-items: flex-start; justify-content: space-between; gap: 14px; }
  .detail-header > div { min-width: 0; }
  .detail-header > button:not(.icon-button) { display: inline-flex; flex: 0 0 auto; align-items: center; gap: 5px; min-height: 28px; padding: 0 9px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 11px; }
  .detail-kicker-row { display: flex; align-items: center; justify-content: space-between; }
  .detail-stats { display: grid; grid-template-columns: repeat(4, 1fr); gap: 7px; margin: 0; }
  .detail-stats div { padding: 9px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-chrome); }
  .info-tree { display: grid; gap: 3px; min-height: 0; }
  .info-tree > h3 { margin-bottom: 5px; }
  .info-node { display: grid; grid-template-columns: auto auto minmax(100px, .7fr) minmax(80px, .45fr) minmax(80px, 1fr); align-items: center; gap: 6px; min-height: 28px; padding: 3px 7px; border-radius: 5px; color: var(--wb-text-muted); font-size: 11px; }
  .info-node:hover { background: var(--wb-control-hover); }
  .info-node strong { overflow: hidden; color: var(--wb-text-primary); text-overflow: ellipsis; white-space: nowrap; }
  .info-node code { overflow: hidden; color: var(--wb-text-muted); text-align: right; text-overflow: ellipsis; white-space: nowrap; }
  .tree-indent { color: var(--wb-border-strong); font-family: var(--font-mono); white-space: pre; }
  .full-action { width: 100%; }
  .context-note { padding: 10px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-chrome); }
  .visual-editor { display: grid; grid-template-rows: auto minmax(0, 1fr); height: 100%; min-height: 0; }
  .editor-header { padding: 14px 16px; border-bottom: 1px solid var(--wb-border-subtle); }
  .icon-button { display: grid; width: 28px; height: 28px; place-items: center; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-muted); background: var(--wb-surface-document); }
  .editor-body { display: grid; grid-template-columns: minmax(250px, .82fr) minmax(270px, 1fr); min-width: 0; min-height: 0; }
  .node-tree { min-width: 0; min-height: 0; overflow: auto; padding: 8px; border-right: 1px solid var(--wb-border-subtle); }
  .node-tree > button { --ui-entity-color: var(--wb-text-muted); display: grid; grid-template-columns: auto auto minmax(0, 1fr) auto; align-items: center; width: 100%; min-height: 38px; gap: 6px; padding: 5px 7px; border: 1px solid transparent; border-radius: 6px; color: var(--wb-text-muted); background: transparent; text-align: left; }
  .node-tree > button.comment { opacity: .72; }
  .node-tree > button > span:nth-of-type(2) { display: grid; min-width: 0; gap: 2px; }
  .node-tree strong, .node-tree small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .node-tree strong { color: var(--wb-text-primary); font-size: 11px; }
  .node-tree small { color: var(--wb-text-muted); font-size: 11px; }
  .node-tree :global(.row-chevron) { color: var(--wb-border-strong); }
  .node-editor { display: grid; align-content: start; gap: 14px; min-width: 0; min-height: 0; overflow: auto; padding: 14px; }
  .node-editor-title { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
  .node-editor-title code { max-width: 48%; overflow: hidden; color: var(--wb-text-muted); text-overflow: ellipsis; white-space: nowrap; }
  .node-form, .insert-form { display: grid; gap: 9px; padding: 11px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-chrome); }
  .insert-form { border-style: dashed; }
  :is(.detail-form, .node-form, .insert-form) label { display: grid; gap: 5px; color: var(--wb-text-muted); font-size: 11px; font-weight: 650; }
  :is(.detail-form, .node-form, .insert-form) :is(input, select) { min-width: 0; height: 31px; padding: 0 8px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font: inherit; font-weight: 500; }
  :is(.detail-form, .node-form, .insert-form) :is(input, select):focus { border-color: var(--wb-accent); outline: 2px solid var(--wb-focus-ring); outline-offset: -2px; }
  .detail-form label small { color: var(--wb-text-muted); font-weight: 450; line-height: 1.4; }
  .boolean-field { display: flex; align-items: center; gap: 7px; }
  .boolean-field input { width: 16px; height: 16px; }
  .load-paths { display: grid; gap: 6px; padding: 10px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-chrome); }
  .load-paths code { overflow: hidden; color: var(--wb-text-muted); text-overflow: ellipsis; white-space: nowrap; }
  .form-actions { display: flex; justify-content: flex-end; gap: 7px; padding-top: 3px; }
  .form-actions > button:not(.primary), .insert-form > button, .danger-zone button { display: inline-flex; align-items: center; justify-content: center; gap: 5px; min-height: 28px; padding: 0 9px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 11px; }
  .danger-zone { display: grid; gap: 7px; padding-top: 10px; border-top: 1px solid var(--wb-border-subtle); }
  .danger-zone > p { color: var(--danger); }
  .danger-zone > div { display: flex; justify-content: flex-end; gap: 7px; }
  .danger-zone .danger, .danger-zone .danger-link { color: var(--danger); border-color: color-mix(in srgb, var(--danger), transparent 60%); }
  .danger-zone .danger-link { justify-self: start; border-color: transparent; background: transparent; }
  .ui-message { display: flex; align-items: flex-start; gap: 6px; padding: 8px 9px; border-radius: 6px; font-size: 11px; line-height: 1.4; }
  .ui-message.error { border: 1px solid color-mix(in srgb, var(--danger), transparent 58%); color: var(--danger); background: color-mix(in srgb, var(--danger), transparent 92%); }
  .workspace-state { display: grid; min-height: 180px; place-content: center; justify-items: center; gap: 7px; padding: 18px; color: var(--wb-text-muted); text-align: center; font-size: 12px; }
  .workspace-state.compact { min-height: 70px; }
  button:focus-visible { outline: 2px solid var(--wb-focus-ring); outline-offset: -2px; }
  button:disabled { cursor: default; opacity: .5; }
  @media (max-width: 1180px) {
    .workspace-body { grid-template-columns: minmax(260px, .65fr) minmax(430px, 1.35fr); }
    .workspace-header dl div:nth-child(2), .workspace-header dl div:nth-child(3) { display: none; }
    .workspace-header dl { grid-template-columns: repeat(2, minmax(68px, auto)); }
    .editor-body { grid-template-columns: minmax(210px, .7fr) minmax(250px, 1.3fr); }
  }
</style>
