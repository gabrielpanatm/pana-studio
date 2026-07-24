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

  const views: { id: DataView; label: string }[] = [
    { id: "all", label: "Toate" },
    { id: "toml", label: "TOML" },
    { id: "other", label: "Alte formate" },
  ];
  const scalarKinds: { id: DataDraftKind; label: string }[] = [
    { id: "string", label: "Text" },
    { id: "integer", label: "Număr întreg" },
    { id: "float", label: "Număr zecimal" },
    { id: "boolean", label: "Adevărat / fals" },
    { id: "datetime", label: "Dată / oră" },
  ];

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
  const normalizedQuery = $derived(query.trim().toLocaleLowerCase("ro"));
  const filteredFiles = $derived(
    dataFiles
      .filter((file) => (
        (activeView === "all"
          || activeView === "toml" && file.format === "toml"
          || activeView === "other" && file.format !== "toml")
        && (!normalizedQuery
          || `${file.file} ${file.logicalPath} ${file.format}`
            .toLocaleLowerCase("ro")
            .includes(normalizedQuery))
      ))
      .sort((left, right) => left.logicalPath.localeCompare(right.logicalPath, "ro")),
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
    if (node.kind === "document") return "Document";
    if (node.kind === "comment") return node.valuePreview || "Comentariu";
    if (node.kind === "tableElement" || node.kind === "arrayElement") {
      const index = node.path.at(-1);
      return index?.kind === "index" ? `Element ${index.value + 1}` : "Element";
    }
    return node.key || "Valoare";
  }

  function nodeKindLabel(node: SourceDataNode) {
    if (node.kind === "document") return "Document TOML";
    if (node.kind === "table") return "Tabel";
    if (node.kind === "inlineTable") return "Tabel inline";
    if (node.kind === "arrayOfTables") return "Colecție de tabele";
    if (node.kind === "tableElement") return "Rând";
    if (node.kind === "array") return "Listă";
    if (node.kind === "arrayElement") return valueKindLabel(node.valueKind);
    if (node.kind === "comment") return "Comentariu";
    return valueKindLabel(node.valueKind);
  }

  function valueKindLabel(kind: SourceDataNode["valueKind"]) {
    if (kind === "string") return "Text";
    if (kind === "integer") return "Număr întreg";
    if (kind === "float") return "Număr zecimal";
    if (kind === "boolean") return "Boolean";
    if (kind === "datetime") return "Dată / oră";
    if (kind === "array") return "Listă";
    if (kind === "inlineTable") return "Tabel inline";
    if (kind === "table" || kind === "arrayOfTables") return "Tabel";
    return "Valoare";
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
    newFileName = "";
  }

  function beginEdit(file: SourceGraphDataFile) {
    if (file.format !== "toml" || file.parseError) return;
    detailMode = "edit";
    formError = "";
    selectedNodeId = file.nodes.find((node) => node.kind === "document")?.id ?? "";
  }

  function canonicalFilePath(value: string) {
    let normalized = value.trim().replaceAll("\\", "/").replace(/^\/+|\/+$/g, "");
    if (normalized.startsWith("date/")) normalized = normalized.slice("date/".length);
    if (!normalized.endsWith(".toml")) normalized = `${normalized}.toml`;
    return `date/${normalized}`;
  }

  function availableInsertKinds(node: SourceDataNode | null) {
    if (!node) return [];
    if (node.kind === "arrayOfTables") return [{ id: "table" as const, label: "Rând nou" }];
    const values = [...scalarKinds];
    values.push({ id: "array", label: "Listă" });
    values.push({ id: "inline_table", label: "Tabel inline" });
    if (node.kind === "document" || node.kind === "table" || node.kind === "tableElement") {
      values.push({ id: "table", label: "Tabel" });
      values.push({ id: "array_of_tables", label: "Colecție de tabele" });
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
      await app.rescanCurrentProject(receipt.workspace.relativePath, { strict: true });
      const refreshed = app.sourceGraph?.dataFiles.find((file) => file.file === receipt.plan.file);
      selectedFileId = refreshed?.id ?? "";
      selectedNodeId = refreshed?.nodes.find((node) => node.kind === "document")?.id ?? "";
      app.setGlobalStatus(
        `${successMessage} Modificarea este în sesiunea proiectului — Ctrl+S persistă pe disc.`,
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
    if (path === "date/.toml") {
      formError = "Adaugă numele fișierului.";
      return;
    }
    const applied = await applyMutation({
      operation: "create_file",
      file: path,
      nodeId: null,
      key: null,
      draftKind: null,
      value: "",
    }, `Fișierul ${path} a fost creat.`);
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
    }, `Nodul ${nodeLabel(selectedNode)} a fost actualizat.`);
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
    }, `Datele au fost adăugate în ${nodeLabel(selectedNode)}.`);
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
    }, `Nodul ${nodeLabel(selectedNode)} a fost șters.`);
    if (applied) {
      detailMode = "edit";
      deleteConfirmationOpen = false;
    }
  }

  async function openSource(file: SourceGraphDataFile) {
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

<section class="data-workspace" aria-labelledby="data-title">
  <header class="workspace-header">
    <div>
      <span class="eyebrow"><IconDatabase size={15} stroke={1.9} /> Date structurate Zola</span>
      <h1 id="data-title">Date</h1>
      <p>Fișierele reutilizabile rămân în <code>date/</code>; editarea vizuală TOML este validată de nucleul Rust.</p>
    </div>
    <dl>
      <div><dt>Fișiere</dt><dd>{dataFiles.length}</dd></div>
      <div><dt>Tabele</dt><dd>{allTableCount}</dd></div>
      <div><dt>Liste</dt><dd>{allListCount}</dd></div>
      <div><dt>Valori</dt><dd>{allValueCount}</dd></div>
    </dl>
  </header>

  <div class="workspace-toolbar">
    <div class="view-tabs" role="tablist" aria-label="Formate de date">
      {#each views as view, index (view.id)}
        <button
          id={`data-tab-${view.id}`}
          type="button"
          role="tab"
          aria-selected={activeView === view.id ? "true" : "false"}
          tabindex={activeView === view.id ? 0 : -1}
          class:active={activeView === view.id}
          onclick={() => selectView(view.id)}
          onkeydown={(event) => handleViewKeydown(event, index)}
        >{view.label}</button>
      {/each}
    </div>
    <label class="search-field">
      <IconSearch size={14} />
      <span class="sr-only">Caută fișiere de date</span>
      <input bind:value={query} type="search" placeholder="Caută date" />
    </label>
    <button class="toolbar-action" type="button" onclick={beginCreate}>
      <IconPlus size={14} /> Adaugă date
    </button>
  </div>

  <div class="workspace-body">
    <div class="data-list" role="listbox" aria-label="Fișiere de date">
      {#each filteredFiles as file (file.id)}
        <button
          class="resource-card"
          class:selected={selectedFile?.id === file.id}
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
            <small>{countNodes(file, ["value", "arrayElement"])} valori</small>
            <span>{file.format.toUpperCase()}</span>
          </span>
        </button>
      {:else}
        <div class="workspace-state">
          <IconDatabase size={24} />
          <strong>Nicio sursă de date</strong>
          <span>Adaugă primul fișier TOML în <code>date/</code>.</span>
        </div>
      {/each}
    </div>

    <aside class="detail-panel" aria-live="polite">
      {#if detailMode === "create"}
        <form class="detail-form compact-form" onsubmit={createFile}>
          <header class="detail-header">
            <div>
              <span class="detail-kicker">Fișier nou</span>
              <h2>Date TOML</h2>
              <p>Fișierul gol este creat în sesiune; apoi îi adaugi structura vizual.</p>
            </div>
            <button class="icon-button" type="button" aria-label="Închide" onclick={resetPanel}>
              <IconX size={16} />
            </button>
          </header>
          <label>
            <span>Nume sau subdirector</span>
            <div class="path-field">
              <span>date/</span>
              <input bind:value={newFileName} disabled={mutating} placeholder="meniu sau catalog/servicii" />
              <span>.toml</span>
            </div>
            <small>Folosește un nume tehnic fără diacritice; subdirectoarele sunt permise.</small>
          </label>
          {#if formError}
            <p class="ui-message error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>
          {/if}
          <div class="form-actions">
            <button type="button" disabled={mutating} onclick={resetPanel}>Renunță</button>
            <button class="primary" type="submit" disabled={mutating || !newFileName.trim()}>
              <IconPlus size={14} /> {mutating ? "Se validează…" : "Creează fișierul"}
            </button>
          </div>
        </form>
      {:else if selectedFile && detailMode === "edit"}
        <div class="visual-editor">
          <header class="detail-header editor-header">
            <div>
              <span class="detail-kicker">Editare vizuală TOML</span>
              <h2>{selectedFile.logicalPath}</h2>
              <p>Fiecare salvare validată produce o singură acțiune Undo.</p>
            </div>
            <button class="icon-button" type="button" aria-label="Închide editorul" onclick={resetPanel}>
              <IconX size={16} />
            </button>
          </header>

          <div class="editor-body">
            <div class="node-tree" role="tree" aria-label={`Structura ${selectedFile.logicalPath}`}>
              {#each selectedRows as row (row.node.id)}
                {@const NodeIcon = nodeIcon(row.node)}
                <button
                  type="button"
                  role="treeitem"
                  aria-selected={selectedNode?.id === row.node.id}
                  class:selected={selectedNode?.id === row.node.id}
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
                  <code>{selectedNode.path.map((part) => part.value).join(" › ") || "rădăcină"}</code>
                </div>

                {#if nodeEditorLoading}
                  <div class="workspace-state compact">Se citește valoarea exactă din Rust…</div>
                {:else if nodeEditor && (nodeEditor.editableKey || nodeEditor.editableValue)}
                  <form class="node-form" onsubmit={updateNode}>
                    {#if nodeEditor.editableKey}
                      <label>
                        <span>Cheie</span>
                        <input bind:value={draftKey} disabled={mutating} />
                      </label>
                    {/if}
                    {#if nodeEditor.editableValue}
                      <label>
                        <span>Tip</span>
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
                          <span>Valoare activă</span>
                        </label>
                      {:else}
                        <label>
                          <span>Valoare {draftKindLabel(draftKind).toLocaleLowerCase("ro")}</span>
                          <input bind:value={draftValue} disabled={mutating} />
                        </label>
                      {/if}
                    {/if}
                    <button class="primary full-action" type="submit" disabled={mutating || !canUpdateSelected}>
                      <IconDeviceFloppy size={14} /> {mutating ? "Se validează…" : "Salvează nodul"}
                    </button>
                  </form>
                {:else if selectedNode.kind === "comment"}
                  <p class="context-note">Comentariile sunt păstrate lossless și se modifică numai în editorul de cod.</p>
                {/if}

                {#if acceptsChildren(selectedNode)}
                  <form class="insert-form" onsubmit={insertChild}>
                    <div>
                      <span class="detail-kicker">Adaugă în selecție</span>
                      <h3>{selectedNode.kind === "arrayOfTables" ? "Rând nou" : "Element nou"}</h3>
                    </div>
                    {#if childNeedsKey(selectedNode)}
                      <label>
                        <span>Cheie</span>
                        <input bind:value={insertKey} disabled={mutating} placeholder="cheie_noua" />
                      </label>
                    {/if}
                    {#if selectedNode.kind !== "arrayOfTables"}
                      <label>
                        <span>Tip</span>
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
                          <span>Valoare activă</span>
                        </label>
                      {:else}
                        <label>
                          <span>Valoare</span>
                          <input bind:value={insertValue} disabled={mutating} />
                        </label>
                      {/if}
                    {/if}
                    <button
                      type="submit"
                      disabled={mutating || (childNeedsKey(selectedNode) && !insertKey)}
                    ><IconPlus size={14} /> Adaugă</button>
                  </form>
                {/if}

                {#if !["document", "comment", "opaque"].includes(selectedNode.kind)}
                  <div class="danger-zone">
                    {#if deleteConfirmationOpen}
                      <p>Ștergi „{nodeLabel(selectedNode)}” și toți copiii săi?</p>
                      <div>
                        <button type="button" disabled={mutating} onclick={() => { deleteConfirmationOpen = false; }}>Renunță</button>
                        <button class="danger" type="button" disabled={mutating} onclick={() => { void deleteNode(); }}>
                          <IconTrash size={14} /> {mutating ? "Se verifică…" : "Șterge"}
                        </button>
                      </div>
                    {:else}
                      <button class="danger-link" type="button" onclick={() => { deleteConfirmationOpen = true; }}>
                        <IconTrash size={14} /> Șterge nodul
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
            <span>{selectedFile.origin === "theme" ? "Temă" : "Local"}</span>
          </div>
          <header class="detail-header">
            <div>
              <h2>{selectedFile.logicalPath.split("/").at(-1)}</h2>
              <p><code>{selectedFile.file}</code></p>
            </div>
            <button type="button" onclick={() => { void openSource(selectedFile); }}>
              <IconExternalLink size={14} /> Deschide în Editor
            </button>
          </header>

          {#if selectedFile.parseError}
            <p class="ui-message error"><IconAlertTriangle size={14} /> {selectedFile.parseError}</p>
          {/if}

          <dl class="detail-stats">
            <div><dt>Tabele</dt><dd>{countNodes(selectedFile, ["table", "inlineTable", "tableElement"])}</dd></div>
            <div><dt>Liste</dt><dd>{countNodes(selectedFile, ["array", "arrayOfTables"])}</dd></div>
            <div><dt>Valori</dt><dd>{countNodes(selectedFile, ["value", "arrayElement"])}</dd></div>
            <div><dt>Legături</dt><dd>{relationCount(selectedFile)}</dd></div>
          </dl>

          <div class="info-tree">
            <h3>Structură semantică</h3>
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
            {#if selectedRows.length > 24}<small>Încă {selectedRows.length - 24} noduri sunt disponibile în editare.</small>{/if}
          </div>

          {#if selectedFile.format === "toml" && !selectedFile.parseError}
            <button class="primary full-action" type="button" onclick={() => beginEdit(selectedFile)}>
              <IconEdit size={14} /> Editează vizual
            </button>
          {:else}
            <p class="context-note">
              {selectedFile.format === "toml"
                ? "Corectează sintaxa în editorul de cod înaintea editării vizuale."
                : "Formatul este indexat semantic, dar editarea lossless este disponibilă momentan numai pentru TOML."}
            </p>
          {/if}
        </div>
      {:else}
        <div class="workspace-state">Selectează sau creează un fișier de date.</div>
      {/if}
    </aside>
  </div>
</section>

<style>
  .data-workspace { display: grid; grid-template-rows: auto 42px minmax(0, 1fr); min-width: 0; min-height: 0; height: 100%; overflow: hidden; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-panel); color: var(--wb-text-primary); background: var(--wb-surface-document); }
  .workspace-header { display: flex; align-items: center; justify-content: space-between; gap: 24px; padding: 17px 20px; border-bottom: 1px solid var(--wb-border-subtle); background: var(--wb-surface-chrome); }
  .workspace-header > div { min-width: 0; }
  .eyebrow, .detail-kicker { display: inline-flex; align-items: center; gap: 6px; color: var(--wb-accent-strong); font-size: 11px; font-weight: 800; letter-spacing: .035em; text-transform: uppercase; }
  h1 { margin: 6px 0 0; color: var(--text-strong); font-size: 20px; }
  h2 { margin: 4px 0 0; color: var(--text-strong); font-size: 20px; }
  h3 { margin: 3px 0 0; color: var(--text-strong); font-size: 14px; }
  p { margin: 4px 0 0; color: var(--wb-text-muted); font-size: 12px; line-height: 1.45; }
  code { font-family: var(--font-mono); font-size: 11px; }
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
  .toolbar-action, button.primary { display: inline-flex; align-items: center; justify-content: center; gap: 5px; min-height: 28px; padding: 0 10px; border: 1px solid var(--wb-accent); border-radius: var(--radius-control); color: #fff; background: var(--wb-accent); font-size: 12px; font-weight: 700; }
  .workspace-body { display: grid; grid-template-columns: minmax(310px, .72fr) minmax(500px, 1.28fr); min-width: 0; min-height: 0; }
  .data-list { min-width: 0; min-height: 0; overflow: auto; padding: 9px; border-right: 1px solid var(--wb-border-subtle); }
  .resource-card { display: flex; align-items: center; width: 100%; gap: 9px; min-height: 54px; padding: 7px 9px; border: 1px solid transparent; border-radius: 7px; color: var(--wb-text-primary); background: transparent; text-align: left; }
  .resource-card:hover, .resource-card.selected { border-color: var(--wb-border-subtle); background: var(--wb-control-hover); }
  .resource-card.selected { box-shadow: inset 3px 0 0 var(--wb-accent); }
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
  .node-tree > button { display: grid; grid-template-columns: auto auto minmax(0, 1fr) auto; align-items: center; width: 100%; min-height: 38px; gap: 6px; padding: 5px 7px; border: 1px solid transparent; border-radius: 6px; color: var(--wb-text-muted); background: transparent; text-align: left; }
  .node-tree > button:hover, .node-tree > button.selected { border-color: var(--wb-border-subtle); background: var(--wb-control-hover); }
  .node-tree > button.selected { box-shadow: inset 2px 0 0 var(--wb-accent); color: var(--wb-accent-strong); }
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
  label { display: grid; gap: 5px; color: var(--wb-text-muted); font-size: 11px; font-weight: 650; }
  input, select { min-width: 0; height: 31px; padding: 0 8px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font: inherit; font-weight: 500; }
  input:focus, select:focus { border-color: var(--wb-accent); outline: 2px solid var(--wb-focus-ring); outline-offset: -2px; }
  label small { color: var(--wb-text-muted); font-weight: 450; line-height: 1.4; }
  .boolean-field { display: flex; align-items: center; gap: 7px; }
  .boolean-field input { width: 16px; height: 16px; }
  .path-field { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: center; overflow: hidden; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); background: var(--wb-surface-document); }
  .path-field span { padding: 0 8px; color: var(--wb-text-muted); font-family: var(--font-mono); font-size: 11px; }
  .path-field input { border: 0; border-right: 1px solid var(--wb-border-subtle); border-left: 1px solid var(--wb-border-subtle); border-radius: 0; }
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
