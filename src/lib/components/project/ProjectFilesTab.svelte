<script lang="ts">
  import {
    IconChevronDown,
    IconChevronRight,
    IconFile,
    IconFileCode,
    IconFilePlus,
    IconFileText,
    IconFolder,
    IconFolderOpen,
    IconFolderPlus,
    IconPencil,
    IconPhoto,
    IconSettings,
    IconTrash,
    IconX,
  } from "@tabler/icons-svelte";
  import { tick } from "svelte";
  import type { ProjectFile } from "$lib/types";
  import {
    allProjectPaneFiles,
    buildProjectFileTree,
    type FlatProjectFileNode,
    flattenVisibleProjectFiles,
    projectFileExt,
    type ProjectTreeNode,
    resolveCreateTarget,
    type PendingCreate,
  } from "$lib/project/pane-tree";
  import {
    fileDropBlockedLabel,
    fileMoveHintLabel,
    validateFileDrop,
    type FileMoveRequest,
  } from "$lib/project/files-drag";
  import {
    validateFileRenameRequest,
    type FileRenameRequest,
  } from "$lib/project/files-rename";
  import { onDestroy, onMount } from "svelte";
  import { listenForExternalReconcileInteractionBarrier } from "$lib/session/external-reconcile-barrier";

  export let scannedProject = false;
  export let projectRoot = "";
  export let runtimeSessionId = "";
  export let allProjectFiles: ProjectFile[] = [];
  export let scannedPages: ProjectFile[] = [];
  export let scannedStyles: ProjectFile[] = [];
  export let scannedTemplates: ProjectFile[] = [];
  export let scannedScripts: ProjectFile[] = [];
  export let scannedAssets: ProjectFile[] = [];
  export let activeScannedPath: string | null = null;
  export let fileMoveBlockedReason = "";
  export let openScannedFile: (file: ProjectFile) => void;
  export let createProjectFile: (relativePath: string, content: string) => Promise<void>;
  export let moveProjectFile: (request: FileMoveRequest) => void | Promise<void>;
  export let renameProjectFile: (request: FileRenameRequest & { type: "file" | "dir" }) => boolean | void | Promise<boolean | void>;
  export let deleteProjectFile: (request: { path: string; type: "file" | "dir" }) => void | Promise<void>;

  export let collapsedDirs = new Set<string>();
  export let knownDirPaths = new Set<string>();
  let pendingCreate: PendingCreate | null = null;
  let createInputEl: HTMLInputElement | undefined;
  let createError = "";
  let hoveredPath = "";
  let dragCandidate: {
    path: string;
    pointerId: number;
    sourceType: "dir" | "file";
    startX: number;
    startY: number;
    expectedProjectRoot: string;
    expectedSessionId: string;
  } | null = null;
  let dragSourcePath: string | null = null;
  let dragTargetPath: string | null = null;
  let dragPointer = { x: 0, y: 0 };
  let dragActive = false;
  let dragDropInvalid = false;
  let dragDropMessage = "";
  let dragDropLabel = "";
  let suppressNextClick = false;
  let pendingDelete: FlatProjectFileNode | null = null;
  let deleting = false;
  let pendingRename: FlatProjectFileNode | null = null;
  let renameName = "";
  let renameError = "";
  let renaming = false;
  let renameInputEl: HTMLInputElement | undefined;

  $: allFiles = allProjectPaneFiles({
    allProjectFiles,
    scannedPages: scannedPages.filter((file) => file.kind !== "DIR"),
    scannedStyles: scannedStyles.filter((file) => file.kind !== "DIR"),
    scannedTemplates: scannedTemplates.filter((file) => file.kind !== "DIR"),
    scannedScripts: scannedScripts.filter((file) => file.kind !== "DIR"),
    scannedAssets: scannedAssets.filter((file) => file.kind !== "DIR"),
  });
  $: internalTree = buildProjectFileTree(allFiles);
  $: syncCollapsedDirectories(internalTree);
  $: flatTree = flattenVisibleProjectFiles(internalTree, collapsedDirs);
  $: projectRootDropNode = {
    type: "dir",
    name: "Rădăcina proiectului",
    path: "",
    commandPath: "",
    depth: 0,
    hasChildren: flatTree.length > 0,
  } satisfies FlatProjectFileNode;
  $: dragSourceNode = flatTree.find((item) => item.path === dragSourcePath) ?? null;
  $: dragTargetNode = targetNodeForPath(dragTargetPath);
  $: if (typeof document !== "undefined") {
    document.body.classList.toggle("files-dragging", dragActive);
  }
  $: if (
    dragCandidate
    && (
      dragCandidate.expectedProjectRoot !== projectRoot
      || dragCandidate.expectedSessionId !== runtimeSessionId
    )
  ) {
    cleanupPointerListeners();
    clearDragState();
  }

  async function startCreate(parentPath: string, depth: number, kind: "file" | "dir", event: MouseEvent) {
    event.stopPropagation();

    if (parentPath) {
      const next = new Set(collapsedDirs);
      next.delete(parentPath);
      collapsedDirs = next;
    }

    pendingCreate = {
      parentPath,
      commandParentPath: targetNodeForPath(parentPath)?.commandPath ?? "",
      depth,
      kind,
      name: "",
    };
    createError = "";
    await tick();
    createInputEl?.focus();
  }

  function toggleDirCollapse(path: string, event: MouseEvent) {
    event.stopPropagation();
    if (suppressNextClick) {
      suppressNextClick = false;
      return;
    }
    const next = new Set(collapsedDirs);
    if (next.has(path)) next.delete(path);
    else next.add(path);
    collapsedDirs = next;
  }

  function cancelCreate() {
    pendingCreate = null;
    createError = "";
  }

  async function startRename(node: FlatProjectFileNode, event: MouseEvent) {
    event.preventDefault();
    event.stopPropagation();
    if (fileMoveBlockedReason) return;
    pendingRename = node;
    renameName = node.name;
    renameError = "";
    await tick();
    renameInputEl?.focus();
    renameInputEl?.select();
  }

  function cancelRename() {
    if (renaming) return;
    pendingRename = null;
    renameName = "";
    renameError = "";
  }

  async function confirmRename() {
    if (!pendingRename || renaming) return;
    const target = pendingRename;
    const newName = renameName.trim();
    const validation = validateFileRenameRequest(allFiles, {
      path: target.commandPath,
      type: target.type,
      newName,
    });
    if (!validation.allowed) {
      renameError = validation.reason ?? "Redenumire invalidă.";
      return;
    }

    renaming = true;
    renameError = "";
    const result = await renameProjectFile({ path: target.commandPath, type: target.type, newName });
    renaming = false;
    if (result === false) {
      renameError = "Redenumirea nu a fost aplicată.";
      return;
    }
    pendingRename = null;
    renameName = "";
  }

  async function confirmCreate() {
    if (!pendingCreate) return;

    const rawName = pendingCreate.name.trim();
    if (!rawName) {
      createError = "Numele nu poate fi gol.";
      return;
    }
    if (rawName.includes("..") || rawName.startsWith("/")) {
      createError = "Nume invalid.";
      return;
    }

    try {
      createError = "";
      const target = resolveCreateTarget({ ...pendingCreate, name: rawName });
      await createProjectFile(target.filePath, target.content);
      pendingCreate = null;
    } catch (error) {
      createError = String(error);
    }
  }

  function handleCreateKeydown(event: KeyboardEvent) {
    if (event.key === "Enter") void confirmCreate();
    else if (event.key === "Escape") cancelCreate();
  }

  function handleRenameKeydown(event: KeyboardEvent) {
    if (event.key === "Enter") void confirmRename();
    else if (event.key === "Escape") cancelRename();
  }

  function isImageFile(name: string): boolean {
    return ["jpg", "jpeg", "png", "webp", "gif", "svg", "avif", "ico"].includes(projectFileExt(name));
  }

  function isCodeFile(name: string): boolean {
    return ["html", "htm", "scss", "css", "js", "ts"].includes(projectFileExt(name));
  }

  function isMarkdownFile(name: string): boolean {
    return projectFileExt(name) === "md";
  }

  function isConfigFile(name: string): boolean {
    return ["toml", "json", "yaml", "yml"].includes(projectFileExt(name));
  }

  function cleanupPointerListeners() {
    window.removeEventListener("pointermove", handleWindowPointerMove);
    window.removeEventListener("pointerup", handleWindowPointerUp);
    window.removeEventListener("pointercancel", handleWindowPointerCancel);
  }

  function clearDragState() {
    dragCandidate = null;
    dragSourcePath = null;
    dragTargetPath = null;
    dragActive = false;
    dragDropInvalid = false;
    dragDropMessage = "";
    dragDropLabel = "";
  }

  function cancelDragForExternalReconcile() {
    cleanupPointerListeners();
    clearDragState();
  }

  function targetNodeForPath(path: string | null): FlatProjectFileNode | null {
    if (path === null) return null;
    if (path === "") return projectRootDropNode;
    return flatTree.find((item) => item.path === path) ?? null;
  }

  function syncCollapsedDirectories(nodes: ProjectTreeNode[]) {
    const paths = collectDirectoryPaths(nodes);
    const nextKnown = new Set<string>();
    const nextCollapsed = new Set<string>();

    for (const path of paths) {
      nextKnown.add(path);
      if (!knownDirPaths.has(path) || collapsedDirs.has(path)) {
        nextCollapsed.add(path);
      }
    }

    if (!sameStringSet(nextKnown, knownDirPaths)) knownDirPaths = nextKnown;
    if (!sameStringSet(nextCollapsed, collapsedDirs)) collapsedDirs = nextCollapsed;
  }

  function collectDirectoryPaths(nodes: ProjectTreeNode[], paths: string[] = []) {
    for (const node of nodes) {
      if (!node.isDir) continue;
      paths.push(node.path);
      collectDirectoryPaths(node.children, paths);
    }
    return paths;
  }

  function sameStringSet(left: Set<string>, right: Set<string>) {
    if (left.size !== right.size) return false;
    for (const value of left) {
      if (!right.has(value)) return false;
    }
    return true;
  }

  function nodeForPointer(event: PointerEvent) {
    const element = document.elementFromPoint(event.clientX, event.clientY);
    const row = element instanceof HTMLElement
      ? element.closest("[data-file-drop-path]") as HTMLElement | null
      : null;
    const path = row?.dataset.fileDropPath;
    if (!row || path === undefined) return null;
    const node = targetNodeForPath(path);
    return node ? { row, node } : null;
  }

  function handlePointerDown(node: typeof flatTree[number], event: PointerEvent) {
    if (event.button !== 0) return;
    if (!projectRoot || !runtimeSessionId) return;
    if (event.target instanceof HTMLElement && event.target.closest(".icon-action, .inline-rename")) return;
    if (pendingRename) return;
    cleanupPointerListeners();
    dragCandidate = {
      path: node.path,
      pointerId: event.pointerId,
      sourceType: node.type,
      startX: event.clientX,
      startY: event.clientY,
      expectedProjectRoot: projectRoot,
      expectedSessionId: runtimeSessionId,
    };
    window.addEventListener("pointermove", handleWindowPointerMove);
    window.addEventListener("pointerup", handleWindowPointerUp);
    window.addEventListener("pointercancel", handleWindowPointerCancel);
  }

  function handleWindowPointerMove(event: PointerEvent) {
    if (!dragCandidate || event.pointerId !== dragCandidate.pointerId) return;
    const distance = Math.hypot(event.clientX - dragCandidate.startX, event.clientY - dragCandidate.startY);
    if (!dragActive && distance < 6) return;

    event.preventDefault();
    dragPointer = { x: event.clientX, y: event.clientY };
    if (!dragActive) {
      dragActive = true;
      dragSourcePath = dragCandidate.path;
    }

    const sourceNode = flatTree.find((item) => item.path === dragCandidate?.path);
    const target = nodeForPointer(event);
    if (!target) {
      dragTargetPath = null;
      dragDropInvalid = false;
      dragDropMessage = "";
      dragDropLabel = "";
      return;
    }

    const validation = validateFileDrop(sourceNode, target.node, {
      files: allFiles,
      blockedReason: fileMoveBlockedReason,
    });
    dragTargetPath = target.node.path;
    dragDropInvalid = !validation.allowed;
    dragDropLabel = validation.allowed ? "Mută aici" : fileDropBlockedLabel(validation);
    dragDropMessage = validation.allowed
      ? fileMoveHintLabel(dragCandidate.path, target.node.path)
      : validation.reason ?? "Drop invalid";
  }

  async function handleWindowPointerUp(event: PointerEvent) {
    if (!dragCandidate || event.pointerId !== dragCandidate.pointerId) return;
    cleanupPointerListeners();

    const sourcePath = dragCandidate.path;
    const sourceType = dragCandidate.sourceType;
    const expectedProjectRoot = dragCandidate.expectedProjectRoot;
    const expectedSessionId = dragCandidate.expectedSessionId;
    const targetPath = dragTargetPath;
    const wasDrag = dragActive;
    const wasInvalid = dragDropInvalid;
    clearDragState();

    if (expectedProjectRoot !== projectRoot || expectedSessionId !== runtimeSessionId) return;

    if (!wasDrag) return;
    suppressNextClick = true;
    window.setTimeout(() => {
      suppressNextClick = false;
    }, 0);
    event.preventDefault();
    const sourceNode = flatTree.find((item) => item.path === sourcePath);
    if (wasInvalid || targetPath === null || targetPath === sourcePath) return;

    const targetNode = targetNodeForPath(targetPath);
    if (!targetNode || targetNode.type !== "dir") return;
    const nextCollapsed = new Set(collapsedDirs);
    nextCollapsed.delete(targetPath);
    collapsedDirs = nextCollapsed;
    await moveProjectFile({
      sourcePath: sourceNode?.commandPath ?? sourcePath,
      sourceType,
      targetDirectory: targetNode.commandPath,
    });
  }

  function handleWindowPointerCancel(event: PointerEvent) {
    if (!dragCandidate || event.pointerId !== dragCandidate.pointerId) return;
    cleanupPointerListeners();
    clearDragState();
  }

  function handleFileClick(file: ProjectFile | undefined) {
    if (suppressNextClick) {
      suppressNextClick = false;
      return;
    }
    if (file) openScannedFile(file);
  }

  function handleDirRowClick(node: FlatProjectFileNode) {
    return (event: MouseEvent) => {
      if (!isRenaming(node)) toggleDirCollapse(node.path, event);
    };
  }

  function handleFileRowClick(node: FlatProjectFileNode) {
    return () => {
      if (!isRenaming(node)) handleFileClick(node.file);
    };
  }

  function requestDelete(node: FlatProjectFileNode, event: MouseEvent) {
    event.preventDefault();
    event.stopPropagation();
    if (fileMoveBlockedReason) return;
    pendingDelete = node;
  }

  function cancelDelete() {
    if (deleting) return;
    pendingDelete = null;
  }

  async function confirmDelete() {
    if (!pendingDelete || deleting) return;
    deleting = true;
    const target = pendingDelete;
    await deleteProjectFile({ path: target.commandPath, type: target.type });
    deleting = false;
    pendingDelete = null;
  }

  function deleteTitle(node: FlatProjectFileNode) {
    if (fileMoveBlockedReason) return fileMoveBlockedReason;
    return node.type === "dir" ? "Șterge dosar" : "Șterge fișier";
  }

  function renameTitle() {
    if (fileMoveBlockedReason) return fileMoveBlockedReason;
    return "Redenumește";
  }

  function isRenaming(node: FlatProjectFileNode) {
    return pendingRename?.path === node.path;
  }

  function isDropTarget(path: string) {
    return !dragDropInvalid && dragTargetPath === path;
  }

  function isInvalidDropTarget(path: string) {
    return dragDropInvalid && dragTargetPath === path;
  }

  onMount(() => listenForExternalReconcileInteractionBarrier(cancelDragForExternalReconcile));
  onDestroy(cleanupPointerListeners);
  onDestroy(() => {
    document.body.classList.remove("files-dragging");
  });
</script>

{#if scannedProject}
  <div class="file-tree-header">
    <span class="file-tree-title">EXPLORER</span>
    <div class="file-tree-header-actions">
      <button
        type="button"
        class="icon-action"
        title="Fișier nou în rădăcină"
        onclick={(event) => startCreate("", 0, "file", event)}
      >
        <IconFilePlus size={14} stroke={1.8} />
      </button>
      <button
        type="button"
        class="icon-action"
        title="Dosar nou în rădăcină"
        onclick={(event) => startCreate("", 0, "dir", event)}
      >
        <IconFolderPlus size={14} stroke={1.8} />
      </button>
    </div>
  </div>

  {#if pendingCreate && pendingCreate.parentPath === "" && flatTree.length > 0 === false}
    <div class="create-row" style="--depth: 0;">
      <span class="create-icon">
        {#if pendingCreate.kind === "dir"}<IconFolder size={13} stroke={1.8} />
        {:else}<IconFile size={13} stroke={1.8} />
        {/if}
      </span>
      <input
        bind:this={createInputEl}
        class="create-input"
        type="text"
        placeholder={pendingCreate.kind === "dir" ? "nume-dosar" : "name.ext"}
        bind:value={pendingCreate.name}
        onkeydown={handleCreateKeydown}
      />
      <button type="button" class="create-confirm" onclick={confirmCreate}>OK</button>
      <button type="button" class="create-cancel" onclick={cancelCreate}><IconX size={11} stroke={2} /></button>
    </div>
  {/if}

  <div class="file-tree" role="tree">
    {#if dragActive}
      <div
        class="file-row root-drop-row"
        class:drop-inside={isDropTarget("")}
        class:drop-invalid={isInvalidDropTarget("")}
        data-file-drop-path=""
        role="none"
        style="--depth: 0;"
      >
        <button class="file-row-btn" type="button">
          <span class="file-chevron"></span>
          <span class="file-icon dir"><IconFolderOpen size={14} stroke={1.6} /></span>
          <span class="file-name">Rădăcina proiectului</span>
          {#if isDropTarget("")}
            <span class="file-drop-label">Mută aici</span>
          {:else if isInvalidDropTarget("")}
            <span class="file-drop-label invalid">{dragDropLabel || "Interzis"}</span>
          {/if}
        </button>
      </div>
    {/if}

    {#if pendingCreate && pendingCreate.parentPath === "" && flatTree.length > 0}
      <div class="create-row" style="--depth: 0;">
        <span class="create-indent" style="width: 0px;"></span>
        <span class="create-icon">
          {#if pendingCreate.kind === "dir"}<IconFolder size={13} stroke={1.8} />
          {:else}<IconFile size={13} stroke={1.8} />
          {/if}
        </span>
        <input
          bind:this={createInputEl}
          class="create-input"
          type="text"
          placeholder={pendingCreate.kind === "dir" ? "nume-dosar" : "name.ext"}
          bind:value={pendingCreate.name}
          onkeydown={handleCreateKeydown}
        />
        <button type="button" class="create-confirm" onclick={confirmCreate}>OK</button>
        <button type="button" class="create-cancel" onclick={cancelCreate}><IconX size={11} stroke={2} /></button>
      </div>
    {/if}

    {#each flatTree as node}
      <div
        class="file-row"
        class:active={node.file?.relativePath === activeScannedPath}
        class:file-draggable={node.type === "file" || node.type === "dir"}
        class:dragging={dragSourcePath === node.path}
        class:drop-inside={isDropTarget(node.path)}
        class:drop-invalid={isInvalidDropTarget(node.path)}
        data-file-path={node.path}
        data-file-drop-path={node.path}
        role="none"
        style="--depth: {node.depth};"
        onmouseenter={() => { hoveredPath = node.path; }}
        onmouseleave={() => { if (hoveredPath === node.path) hoveredPath = ""; }}
        onpointerdown={(event) => handlePointerDown(node, event)}
      >
        <span class="file-indent" style="width: {node.depth * 16}px;"></span>

        {#if node.type === "dir"}
          <svelte:element
            this={isRenaming(node) ? "div" : "button"}
            class="file-row-btn"
            class:renaming-row={isRenaming(node)}
            type={isRenaming(node) ? undefined : "button"}
            role={isRenaming(node) ? "group" : undefined}
            onclick={handleDirRowClick(node)}
          >
            <span class="file-chevron">
              {#if collapsedDirs.has(node.path)}
                <IconChevronRight size={11} stroke={2} />
              {:else}
                <IconChevronDown size={11} stroke={2} />
              {/if}
            </span>
            <span class="file-icon dir">
              {#if collapsedDirs.has(node.path)}
                <IconFolder size={14} stroke={1.6} />
              {:else}
                <IconFolderOpen size={14} stroke={1.6} />
              {/if}
            </span>
            {#if isRenaming(node)}
              <span class="inline-rename">
                <input
                  bind:this={renameInputEl}
                  class="rename-input"
                  type="text"
                  bind:value={renameName}
                  onkeydown={handleRenameKeydown}
                  disabled={renaming}
                  aria-label="Nume nou pentru {node.name}"
                />
                <button type="button" class="rename-confirm" disabled={renaming} onclick={confirmRename}>OK</button>
                <button type="button" class="rename-cancel" disabled={renaming} onclick={cancelRename}><IconX size={11} stroke={2} /></button>
              </span>
            {:else}
              <span class="file-name">{node.name}</span>
            {/if}
            {#if dragTargetPath === node.path && !dragDropInvalid}
              <span class="file-drop-label">Mută aici</span>
            {:else if isInvalidDropTarget(node.path)}
              <span class="file-drop-label invalid">{dragDropLabel || "Interzis"}</span>
            {/if}
          </svelte:element>

          {#if hoveredPath === node.path && scannedProject}
            <div class="row-actions">
              <button
                type="button"
                class="icon-action small"
                disabled={Boolean(fileMoveBlockedReason) || isRenaming(node)}
                title={renameTitle()}
                onclick={(event) => startRename(node, event)}
              >
                <IconPencil size={12} stroke={1.8} />
              </button>
              <button
                type="button"
                class="icon-action small"
                disabled={isRenaming(node)}
                title="Fișier nou"
                onclick={(event) => startCreate(node.path, node.depth + 1, "file", event)}
              >
                <IconFilePlus size={12} stroke={1.8} />
              </button>
              <button
                type="button"
                class="icon-action small"
                disabled={isRenaming(node)}
                title="Dosar nou"
                onclick={(event) => startCreate(node.path, node.depth + 1, "dir", event)}
              >
                <IconFolderPlus size={12} stroke={1.8} />
              </button>
              <button
                type="button"
                class="icon-action small danger"
                disabled={Boolean(fileMoveBlockedReason) || isRenaming(node)}
                title={deleteTitle(node)}
                onclick={(event) => requestDelete(node, event)}
              >
                <IconTrash size={12} stroke={1.8} />
              </button>
            </div>
          {/if}
        {:else}
          <svelte:element
            this={isRenaming(node) ? "div" : "button"}
            class="file-row-btn"
            class:renaming-row={isRenaming(node)}
            type={isRenaming(node) ? undefined : "button"}
            role={isRenaming(node) ? "group" : undefined}
            onclick={handleFileRowClick(node)}
          >
            <span class="file-chevron"></span>
            <span class="file-icon">
              {#if isImageFile(node.name)}
                <IconPhoto size={14} stroke={1.6} />
              {:else if isCodeFile(node.name)}
                <IconFileCode size={14} stroke={1.6} />
              {:else if isMarkdownFile(node.name)}
                <IconFileText size={14} stroke={1.6} />
              {:else if isConfigFile(node.name)}
                <IconSettings size={14} stroke={1.6} />
              {:else}
                <IconFile size={14} stroke={1.6} />
              {/if}
            </span>
            {#if isRenaming(node)}
              <span class="inline-rename">
                <input
                  bind:this={renameInputEl}
                  class="rename-input"
                  type="text"
                  bind:value={renameName}
                  onkeydown={handleRenameKeydown}
                  disabled={renaming}
                  aria-label="Nume nou pentru {node.name}"
                />
                <button type="button" class="rename-confirm" disabled={renaming} onclick={confirmRename}>OK</button>
                <button type="button" class="rename-cancel" disabled={renaming} onclick={cancelRename}><IconX size={11} stroke={2} /></button>
              </span>
            {:else}
              <span class="file-name">{node.name}</span>
            {/if}
            {#if isInvalidDropTarget(node.path)}
              <span class="file-drop-label invalid">{dragDropLabel || "Interzis"}</span>
            {/if}
          </svelte:element>
          {#if hoveredPath === node.path && scannedProject}
            <div class="row-actions">
              <button
                type="button"
                class="icon-action small"
                disabled={Boolean(fileMoveBlockedReason) || isRenaming(node)}
                title={renameTitle()}
                onclick={(event) => startRename(node, event)}
              >
                <IconPencil size={12} stroke={1.8} />
              </button>
              <button
                type="button"
                class="icon-action small danger"
                disabled={Boolean(fileMoveBlockedReason) || isRenaming(node)}
                title={deleteTitle(node)}
                onclick={(event) => requestDelete(node, event)}
              >
                <IconTrash size={12} stroke={1.8} />
              </button>
            </div>
          {/if}
        {/if}
      </div>

      {#if pendingCreate && pendingCreate.parentPath === node.path && node.type === "dir" && !collapsedDirs.has(node.path)}
        <div class="create-row" style="--depth: {node.depth + 1};">
          <span class="create-indent" style="width: {(node.depth + 1) * 16}px;"></span>
          <span class="create-icon">
            {#if pendingCreate.kind === "dir"}<IconFolder size={13} stroke={1.8} />
            {:else}<IconFile size={13} stroke={1.8} />
            {/if}
          </span>
          <input
            bind:this={createInputEl}
            class="create-input"
            type="text"
            placeholder={pendingCreate.kind === "dir" ? "nume-dosar" : "name.ext"}
            bind:value={pendingCreate.name}
            onkeydown={handleCreateKeydown}
          />
          <button type="button" class="create-confirm" onclick={confirmCreate}>OK</button>
          <button type="button" class="create-cancel" onclick={cancelCreate}><IconX size={11} stroke={2} /></button>
        </div>
      {/if}
    {/each}
  </div>

  {#if createError}
    <p class="create-error">{createError}</p>
  {/if}

  {#if renameError}
    <p class="create-error">{renameError}</p>
  {/if}

  {#if dragActive}
    <div
      class="file-drag-hint"
      class:invalid={dragDropInvalid}
      style="left: {dragPointer.x + 14}px; top: {dragPointer.y + 14}px;"
    >
      {#if dragDropInvalid}
        {dragDropMessage || "Drop invalid"}
      {:else if dragTargetNode && dragSourceNode}
        {dragDropMessage || fileMoveHintLabel(dragSourceNode.path, dragTargetNode.path)}
      {:else if dragDropMessage}
        {dragDropMessage}
      {:else}
        Alege dosarul destinație
      {/if}
    </div>
  {/if}
{:else}
  <p class="no-project-hint">Deschide un proiect pentru a vedea fisierele.</p>
{/if}

{#if pendingDelete}
  <div class="delete-modal-backdrop" role="presentation">
    <div class="delete-modal" role="dialog" aria-modal="true" aria-labelledby="delete-title">
      <div class="delete-modal-icon">
        <IconTrash size={18} stroke={2} />
      </div>
      <div class="delete-modal-body">
        <h3 id="delete-title">Mutare în coș</h3>
        <p>
          {#if pendingDelete.type === "dir"}
            Dosarul <strong>{pendingDelete.path}</strong> și tot conținutul lui vor fi mutate în coșul sesiunii Pană Studio.
          {:else}
            Fișierul <strong>{pendingDelete.path}</strong> va fi mutat în coșul sesiunii Pană Studio.
          {/if}
        </p>
        <p class="delete-modal-note">Operația trece prin kernel și poate fi inversată din Undo/Redo în sesiunea curentă.</p>
      </div>
      <div class="delete-modal-actions">
        <button type="button" class="delete-cancel-btn" disabled={deleting} onclick={cancelDelete}>Anulează</button>
        <button type="button" class="delete-confirm-btn" disabled={deleting} onclick={confirmDelete}>
          {deleting ? "Se mută..." : "Mută în coș"}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .file-tree-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 2px 2px;
  }

  .file-tree-title {
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 800;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .file-tree-header-actions {
    display: flex;
    gap: 4px;
  }

  .icon-action {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    padding: 0;
    border: 1px solid transparent;
    border-radius: 5px;
    color: var(--text-muted);
    background: transparent;
    cursor: pointer;
    transition: color 80ms, background 80ms, border-color 80ms;
  }

  .icon-action:hover {
    border-color: var(--border-3);
    color: var(--text-strong);
    background: var(--surface-4);
  }

  .icon-action:disabled {
    opacity: 0.42;
    cursor: not-allowed;
  }

  .icon-action.small {
    width: 20px;
    height: 20px;
  }

  .icon-action.danger {
    color: color-mix(in srgb, #cf4a4a 82%, var(--text-muted));
  }

  .icon-action.danger:hover:not(:disabled) {
    border-color: color-mix(in srgb, #cf4a4a 42%, var(--border-3));
    color: #cf4a4a;
    background: color-mix(in srgb, #cf4a4a 12%, var(--surface-4));
  }

  .file-tree {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .file-row {
    position: relative;
    display: flex;
    align-items: center;
    min-height: 24px;
    border: 1px solid transparent;
    border-radius: 5px;
    transition: background 60ms ease;
  }

  .file-row:hover {
    background: var(--surface-4);
  }

  .file-row.active {
    border-color: var(--brand);
    background: var(--brand-soft);
  }

  .file-row.dragging {
    opacity: 0.55;
  }

  .file-row.drop-inside {
    border-color: var(--brand);
    background: color-mix(in srgb, var(--brand) 16%, var(--surface-4));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--brand) 54%, transparent);
  }

  .file-row.drop-invalid {
    border-color: color-mix(in srgb, #ef4444 52%, var(--border-3));
    background: color-mix(in srgb, #ef4444 10%, var(--surface-4));
  }

  :global(body.files-dragging),
  :global(body.files-dragging *) {
    cursor: grabbing !important;
  }

  :global(body.files-dragging) .row-actions {
    display: none;
  }

  .file-indent {
    flex: 0 0 auto;
  }

  .file-row-btn {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
    min-height: 24px;
    padding: 0 3px;
    border: none;
    color: var(--text);
    font-size: 12px;
    text-align: left;
    background: transparent;
    cursor: pointer;
  }

  .file-row.file-draggable .file-row-btn {
    cursor: grab;
  }

  .file-row.dragging .file-row-btn {
    cursor: grabbing;
  }

  .file-row-btn.renaming-row {
    cursor: default;
  }

  .file-chevron {
    display: flex;
    align-items: center;
    flex: 0 0 12px;
    color: var(--text-muted);
  }

  .file-icon {
    display: flex;
    align-items: center;
    flex: 0 0 16px;
    color: var(--text-muted);
  }

  .file-icon.dir {
    color: color-mix(in srgb, var(--brand-strong) 80%, var(--text-muted));
  }

  .file-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
  }

  .inline-rename {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 4px;
    flex: 1;
    min-width: 0;
  }

  .rename-input {
    width: 100%;
    min-width: 0;
    height: 20px;
    padding: 0 6px;
    border: 1px solid var(--brand);
    border-radius: 4px;
    color: var(--text);
    background: var(--surface);
    font: inherit;
    outline: none;
  }

  .rename-input:focus {
    box-shadow: 0 0 0 1px var(--brand);
  }

  .rename-confirm,
  .rename-cancel {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: 20px;
    min-width: 22px;
    padding: 0 6px;
    border: 1px solid var(--border-3);
    border-radius: 4px;
    color: var(--text);
    background: var(--surface-3);
    font-size: 12px;
    font-weight: 700;
    cursor: pointer;
  }

  .rename-cancel {
    min-width: 20px;
    padding: 0;
  }

  .rename-confirm:disabled,
  .rename-cancel:disabled {
    opacity: 0.55;
    cursor: wait;
  }

  .file-drop-label {
    flex: 0 0 auto;
    max-width: 72px;
    padding: 1px 5px;
    border-radius: 999px;
    color: var(--brand-strong);
    font-size: 12px;
    font-weight: 800;
    line-height: 1.3;
    text-transform: uppercase;
    background: color-mix(in srgb, var(--brand) 16%, var(--surface-3));
  }

  .file-drop-label.invalid {
    color: #dc2626;
    background: color-mix(in srgb, #ef4444 12%, var(--surface-3));
  }

  .row-actions {
    position: absolute;
    right: 4px;
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 1px;
    border-radius: 4px;
    background: var(--surface-4);
  }

  .create-row {
    display: flex;
    align-items: center;
    gap: 4px;
    min-height: 26px;
    padding: 0 4px;
    border: 1px solid var(--brand);
    border-radius: 5px;
    background: var(--brand-soft);
  }

  .create-indent {
    flex: 0 0 auto;
  }

  .create-icon {
    display: flex;
    align-items: center;
    flex: 0 0 16px;
    color: var(--brand-strong);
  }

  .create-input {
    flex: 1;
    min-width: 0;
    height: 22px;
    padding: 0 4px;
    border: none;
    border-radius: 3px;
    color: var(--text);
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
    background: var(--surface);
    outline: none;
  }

  .create-input:focus {
    box-shadow: 0 0 0 1px var(--brand);
  }

  .create-confirm {
    flex: 0 0 auto;
    height: 20px;
    padding: 0 5px;
    border: 1px solid var(--brand);
    border-radius: 3px;
    color: #ffffff;
    font-size: 12px;
    background: var(--brand);
    cursor: pointer;
  }

  .create-cancel {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border: 1px solid var(--border-3);
    border-radius: 3px;
    color: var(--text-muted);
    background: var(--surface-4);
    cursor: pointer;
  }

  .create-error {
    margin: 2px 0 0;
    color: #cf4a4a;
    font-size: 12px;
  }

  .file-drag-hint {
    position: fixed;
    z-index: 10000;
    max-width: min(260px, calc(100vw - 28px));
    padding: 6px 8px;
    border: 1px solid color-mix(in srgb, var(--brand) 48%, var(--border-3));
    border-radius: 7px;
    color: var(--text-strong);
    font-size: 12px;
    font-weight: 800;
    line-height: 1.25;
    pointer-events: none;
    background: color-mix(in srgb, var(--surface) 92%, var(--brand-soft));
    box-shadow: 0 8px 20px rgba(0, 0, 0, 0.18);
  }

  .file-drag-hint.invalid {
    border-color: color-mix(in srgb, #ef4444 52%, var(--border-3));
    color: #dc2626;
    background: color-mix(in srgb, var(--surface) 92%, #fee2e2);
  }

  .no-project-hint {
    margin: 10px 0 0;
    padding: 0;
    color: var(--text-muted);
    font-size: 12px;
    text-align: center;
  }

  .delete-modal-backdrop {
    position: fixed;
    z-index: 12000;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 18px;
    background: rgba(0, 0, 0, 0.48);
  }

  .delete-modal {
    display: grid;
    grid-template-columns: 36px minmax(0, 1fr);
    gap: 12px;
    width: min(420px, 100%);
    padding: 14px;
    border: 1px solid color-mix(in srgb, #cf4a4a 32%, var(--border-2));
    border-radius: 8px;
    background: var(--surface);
    box-shadow: 0 24px 70px rgba(0, 0, 0, 0.36);
  }

  .delete-modal-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    border-radius: 8px;
    color: #cf4a4a;
    background: color-mix(in srgb, #cf4a4a 12%, var(--surface-3));
  }

  .delete-modal-body h3 {
    margin: 0 0 7px;
    color: var(--text-strong);
    font-size: 14px;
    font-weight: 900;
  }

  .delete-modal-body p {
    margin: 0;
    color: var(--text);
    font-size: 12px;
    line-height: 1.45;
  }

  .delete-modal-body strong {
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
    overflow-wrap: anywhere;
  }

  .delete-modal-note {
    margin-top: 7px !important;
    color: #cf4a4a !important;
    font-weight: 800;
  }

  .delete-modal-actions {
    grid-column: 1 / -1;
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding-top: 2px;
  }

  .delete-cancel-btn,
  .delete-confirm-btn {
    min-width: 82px;
    height: 30px;
    padding: 0 10px;
    border-radius: 6px;
    font-size: 12px;
    font-weight: 800;
    cursor: pointer;
  }

  .delete-cancel-btn {
    border: 1px solid var(--border-3);
    color: var(--text);
    background: var(--surface-3);
  }

  .delete-confirm-btn {
    border: 1px solid color-mix(in srgb, #cf4a4a 64%, var(--border-3));
    color: #ffffff;
    background: #b91c1c;
  }

  .delete-confirm-btn:hover:not(:disabled) {
    background: #991b1b;
  }

  button:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
</style>
