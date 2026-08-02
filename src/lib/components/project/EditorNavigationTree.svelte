<script lang="ts">
  import {
    IconArrowBackUp,
    IconArrowUpRight,
    IconArticle,
    IconBox,
    IconBraces,
    IconChevronDown,
    IconChevronRight,
    IconCode,
    IconCodeDots,
    IconColumns,
    IconFileCode,
    IconFileImport,
    IconFilter,
    IconForms,
    IconFunction,
    IconGitBranch,
    IconHeading,
    IconLayout,
    IconLayoutBottombar,
    IconLayoutDashboard,
    IconLayoutNavbar,
    IconLink,
    IconList,
    IconListDetails,
    IconListNumbers,
    IconLogin2,
    IconMarkdown,
    IconNavigation,
    IconPhoto,
    IconPilcrow,
    IconPointer,
    IconQuote,
    IconRepeat,
    IconSection,
    IconTable,
    IconTag,
    IconTextCaption,
    IconTrash,
    IconTypography,
    IconVector,
    IconVideo,
    IconVolume,
  } from "@tabler/icons-svelte";
  import { t } from "$lib/i18n/runtime.svelte";
  import type {
    EditorMovePlan,
    EditorNavigationNode,
    EditorNavigationSnapshot,
    EditorNavigationViewNode,
    ProjectMovePosition,
  } from "$lib/types";

  type NavigationRow = {
    node: EditorNavigationViewNode;
    depth: number;
  };

  let {
    snapshot,
    loading = false,
    error = "",
    selectedNodeId = null,
    hoveredNodeId = null,
    openScopeId = null,
    selectNode,
    hoverNode,
    enterScope,
    exitScope,
    previewMove,
    moveNode,
    deleteNode,
    openDocument,
    activeDocumentPath = null,
    callerDocumentPath = null,
    callerTargetDocumentPath = null,
    returnToCaller,
  }: {
    snapshot: EditorNavigationSnapshot | null;
    loading?: boolean;
    error?: string;
    selectedNodeId?: string | null;
    hoveredNodeId?: string | null;
    openScopeId?: string | null;
    selectNode: (node: EditorNavigationNode) => void;
    hoverNode: (node: EditorNavigationNode | null) => void;
    enterScope: (scopeId: string) => void | Promise<unknown>;
    exitScope: () => void;
    previewMove: (
      sourceNodeId: string,
      targetNodeId: string,
      position: ProjectMovePosition,
    ) => Promise<EditorMovePlan>;
    moveNode: (
      sourceNodeId: string,
      targetNodeId: string,
      position: ProjectMovePosition,
    ) => void | Promise<unknown>;
    deleteNode: (node: EditorNavigationNode) => void | Promise<unknown>;
    openDocument: (
      documentPath: string,
      rememberCaller?: boolean,
    ) => void | Promise<unknown>;
    activeDocumentPath?: string | null;
    callerDocumentPath?: string | null;
    callerTargetDocumentPath?: string | null;
    returnToCaller: () => void | Promise<unknown>;
  } = $props();

  const rowHeight = 30;
  const overscan = 10;
  let collapsed = $state(new Set<string>());
  let scrollTop = $state(0);
  let viewportHeight = $state(500);
  let treeViewport = $state<HTMLDivElement>();
  let snapshotKey = $state("");
  let dragSourceId = $state<string | null>(null);
  let dragTargetId = $state<string | null>(null);
  let dragPosition = $state<ProjectMovePosition | null>(null);
  let dragPlan = $state<EditorMovePlan | null>(null);
  let dragPlanError = $state("");
  let dragPlanSerial = 0;

  const focusedView = $derived(snapshot?.focusedView ?? null);
  const viewNodesById = $derived(
    new Map((focusedView?.nodes ?? []).map((node) => [node.id, node])),
  );
  const editorNodesById = $derived(
    new Map((snapshot?.nodes ?? []).map((node) => [node.id, node])),
  );
  const openScope = $derived(
    openScopeId
      ? focusedView?.nodes.find((node) => node.editorNodeId === openScopeId) ?? null
      : null,
  );
  const callerContextReady = $derived(Boolean(
    callerDocumentPath
      && callerTargetDocumentPath
      && focusedView?.activeDocumentPath === callerTargetDocumentPath
      && activeDocumentPath === callerTargetDocumentPath,
  ));
  const enteredContext = $derived.by(() => {
    if (openScope && focusedView) {
      return {
        documentName: fileName(focusedView.activeDocumentPath),
        scopeName: openScope.label,
        title: `${focusedView.activeDocumentPath} · ${openScope.label}`,
      };
    }
    if (callerContextReady && focusedView) {
      return {
        documentName: fileName(focusedView.activeDocumentPath),
        scopeName: null,
        title: focusedView.activeDocumentPath,
      };
    }
    return null;
  });
  const rows = $derived.by(() => {
    const result: NavigationRow[] = [];
    const visited = new Set<string>();
    const visit = (nodeId: string, depth: number) => {
      if (visited.has(nodeId)) return;
      visited.add(nodeId);
      const node = viewNodesById.get(nodeId);
      if (!node) return;
      result.push({ node, depth });
      if (node.kind === "boundary") return;
      if (collapsed.has(node.id)) return;
      for (const childId of node.children) visit(childId, depth + 1);
    };
    const rootNodeIds = openScope?.children ?? focusedView?.rootNodeIds ?? [];
    for (const rootId of rootNodeIds) visit(rootId, 0);
    return result;
  });
  const visibleStart = $derived(
    Math.max(0, Math.floor(scrollTop / rowHeight) - overscan),
  );
  const visibleEnd = $derived(
    Math.min(
      rows.length,
      Math.ceil((scrollTop + viewportHeight) / rowHeight) + overscan,
    ),
  );
  const visibleRows = $derived(rows.slice(visibleStart, visibleEnd));
  const topSpacer = $derived(visibleStart * rowHeight);
  const bottomSpacer = $derived((rows.length - visibleEnd) * rowHeight);

  $effect(() => {
    const key = [
      snapshot?.identity.transactionId ?? "",
      focusedView?.activeDocumentPath ?? "",
      openScopeId ?? "",
    ].join(":");
    if (key === snapshotKey) return;
    snapshotKey = key;
    collapsed = new Set();
    scrollTop = 0;
    if (treeViewport) treeViewport.scrollTop = 0;
    clearDrag();
  });

  function editorNode(node: EditorNavigationViewNode) {
    if (node.editorNodeId) {
      const exact = editorNodesById.get(node.editorNodeId);
      if (exact) return exact;
    }
    for (const renderInstanceId of node.renderInstanceIds) {
      const rendered = editorNodesById.get(`editor_render:${renderInstanceId}`);
      if (rendered) return rendered;
    }
    return null;
  }

  function isSelected(node: EditorNavigationViewNode) {
    if (!selectedNodeId) return false;
    if (node.editorNodeId === selectedNodeId) return true;
    return node.renderInstanceIds.some(
      (renderInstanceId) => `editor_render:${renderInstanceId}` === selectedNodeId,
    );
  }

  function isCoordinatorHovered(node: EditorNavigationViewNode) {
    if (!hoveredNodeId) return false;
    if (node.editorNodeId === hoveredNodeId) return true;
    return node.renderInstanceIds.some(
      (renderInstanceId) => `editor_render:${renderInstanceId}` === hoveredNodeId,
    );
  }

  function isScopeOpen(node: EditorNavigationViewNode) {
    return Boolean(node.editorNodeId && node.editorNodeId === openScopeId);
  }

  function toggleCollapsed(node: EditorNavigationViewNode, event: Event) {
    event.stopPropagation();
    const next = new Set(collapsed);
    if (next.has(node.id)) next.delete(node.id);
    else next.add(node.id);
    collapsed = next;
  }

  function hasVisibleChildren(node: EditorNavigationViewNode) {
    return node.kind !== "boundary" && node.children.length > 0;
  }

  function canDrag(node: EditorNavigationViewNode) {
    if (!node.editorNodeId || node.kind === "relation") return false;
    if (node.kind === "boundary" || node.kind === "slot") {
      return node.capabilities.canMoveAtomic;
    }
    return node.capabilities.canMove
      || (
        node.capabilities.requiresEditScopeId !== null
        && node.capabilities.requiresEditScopeId === openScopeId
      );
  }

  function htmlElementIcon(tag: string | null) {
    switch (tag?.toLowerCase()) {
      case "main": return IconLayoutDashboard;
      case "header": return IconLayoutNavbar;
      case "footer": return IconLayoutBottombar;
      case "nav": return IconNavigation;
      case "section": return IconSection;
      case "article": return IconArticle;
      case "aside": return IconColumns;
      case "div": return IconBox;
      case "h1":
      case "h2":
      case "h3":
      case "h4":
      case "h5":
      case "h6":
        return IconHeading;
      case "p": return IconPilcrow;
      case "span":
      case "small":
      case "strong":
      case "em":
        return IconTypography;
      case "a": return IconLink;
      case "button": return IconPointer;
      case "img":
      case "picture":
      case "figure":
        return IconPhoto;
      case "figcaption":
      case "caption":
        return IconTextCaption;
      case "ul":
      case "menu":
        return IconList;
      case "ol": return IconListNumbers;
      case "li": return IconListDetails;
      case "form":
      case "fieldset":
      case "legend":
      case "label":
      case "input":
      case "textarea":
      case "select":
      case "option":
        return IconForms;
      case "table":
      case "thead":
      case "tbody":
      case "tfoot":
      case "tr":
      case "th":
      case "td":
        return IconTable;
      case "video": return IconVideo;
      case "audio": return IconVolume;
      case "blockquote":
      case "q":
        return IconQuote;
      case "svg":
      case "canvas":
        return IconVector;
      case "code":
      case "pre":
        return IconCode;
      default:
        return IconTag;
    }
  }

  function teraBoundaryIcon(node: EditorNavigationViewNode) {
    switch (node.sourceKind) {
      case "include": return IconFileImport;
      case "macro": return IconFunction;
      case "for": return IconRepeat;
      case "if": return IconGitBranch;
      case "filter": return IconFilter;
      case "raw": return IconCodeDots;
      default: return IconBraces;
    }
  }

  function navigationNodeIcon(node: EditorNavigationViewNode) {
    if (isMarkdownBoundary(node)) return IconMarkdown;
    switch (node.kind) {
      case "htmlElement": return htmlElementIcon(node.tag);
      case "boundary": return teraBoundaryIcon(node);
      case "relation": return IconLink;
      case "slot": return IconLayout;
      case "source": return IconFileCode;
    }
  }

  function isMarkdownBoundary(node: EditorNavigationViewNode) {
    return editorNode(node)?.kind === "markdownBoundary";
  }

  function canDelete(node: EditorNavigationViewNode) {
    const resolved = editorNode(node);
    const scopeAllowsEdit = node.capabilities.requiresEditScopeId !== null
      && node.capabilities.requiresEditScopeId === openScopeId;
    if (
      !resolved
      || (node.capabilities.readOnly && !scopeAllowsEdit)
    ) return false;
    if (resolved.kind === "htmlElement") {
      return Boolean(resolved.sourceNodeId && resolved.tag);
    }
    return resolved.kind === "teraBoundary"
      && Boolean(resolved.sourceNodeId && resolved.boundary)
      && node.capabilities.canMoveAtomic;
  }

  function deleteViewNode(node: EditorNavigationViewNode, event: Event) {
    event.preventDefault();
    event.stopPropagation();
    if (!canDelete(node)) return;
    const resolved = editorNode(node);
    if (resolved) void deleteNode(resolved);
  }

  function clearDrag() {
    dragSourceId = null;
    dragTargetId = null;
    dragPosition = null;
    dragPlan = null;
    dragPlanError = "";
    dragPlanSerial += 1;
  }

  function fileName(path: string) {
    return path.replaceAll("\\", "/").split("/").filter(Boolean).at(-1) ?? path;
  }

  function selectViewNode(node: EditorNavigationViewNode) {
    if (node.kind === "relation") {
      const target = node.relation?.targetDocumentPath;
      if (target) void openDocument(target);
      return;
    }
    const resolved = editorNode(node);
    if (resolved) selectNode(resolved);
  }

  function hoverViewNode(node: EditorNavigationViewNode | null) {
    hoverNode(node ? editorNode(node) : null);
  }

  async function enterNodeContext(node: EditorNavigationViewNode) {
    if (
      node.relation?.kind === "include"
      && node.relation.targetDocumentPath
    ) {
      await openDocument(node.relation.targetDocumentPath, true);
      return;
    }
    const scopeId = node.editorNodeId;
    if (!scopeId) return;
    await enterScope(scopeId);
  }

  function returnFromEnteredContext() {
    if (openScope) {
      exitScope();
      return;
    }
    void returnToCaller();
  }

  function startDrag(node: EditorNavigationViewNode, event: DragEvent) {
    if (!canDrag(node) || !node.editorNodeId) {
      event.preventDefault();
      return;
    }
    dragSourceId = node.editorNodeId;
    event.dataTransfer?.setData(
      "application/x-pana-editor-node",
      node.editorNodeId,
    );
    if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
  }

  function pointerPosition(event: DragEvent): ProjectMovePosition {
    const row = event.currentTarget as HTMLElement;
    const rect = row.getBoundingClientRect();
    const ratio = rect.height > 0
      ? (event.clientY - rect.top) / rect.height
      : 0.5;
    if (ratio < 0.28) return "before";
    if (ratio > 0.72) return "after";
    return "inside";
  }

  function updateDragTarget(node: EditorNavigationViewNode, event: DragEvent) {
    const targetId = node.editorNodeId;
    if (!dragSourceId || !targetId || targetId === dragSourceId) return;
    event.preventDefault();
    const position = pointerPosition(event);
    if (dragTargetId === targetId && dragPosition === position) return;
    dragTargetId = targetId;
    dragPosition = position;
    dragPlan = null;
    dragPlanError = "";
    const serial = ++dragPlanSerial;
    void previewMove(dragSourceId, targetId, position)
      .then((plan) => {
        if (serial === dragPlanSerial) dragPlan = plan;
      })
      .catch((planError) => {
        if (serial !== dragPlanSerial) return;
        dragPlanError = planError instanceof Error
          ? planError.message
          : String(planError);
      });
  }

  async function dropOnNode(
    node: EditorNavigationViewNode,
    event: DragEvent,
  ) {
    event.preventDefault();
    const sourceId = dragSourceId;
    const targetId = node.editorNodeId;
    const position = dragPosition ?? pointerPosition(event);
    let plan = dragPlan;
    if (!sourceId || !targetId || sourceId === targetId) {
      clearDrag();
      return;
    }
    if (
      !plan
      || plan.sourceNodeId !== sourceId
      || plan.targetNodeId !== targetId
      || plan.position !== position
    ) {
      try {
        plan = await previewMove(sourceId, targetId, position);
      } catch {
        clearDrag();
        return;
      }
    }
    if (plan.allowed) await moveNode(sourceId, targetId, position);
    clearDrag();
  }

  function rowDropClass(
    node: EditorNavigationViewNode,
    position: ProjectMovePosition,
  ) {
    return dragTargetId === node.editorNodeId && dragPosition === position;
  }

  function handleRowKeydown(
    node: EditorNavigationViewNode,
    event: KeyboardEvent,
  ) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      selectViewNode(node);
      return;
    }
    if (!hasVisibleChildren(node)) return;
    if (event.key === "ArrowRight" && collapsed.has(node.id)) {
      toggleCollapsed(node, event);
    } else if (event.key === "ArrowLeft" && !collapsed.has(node.id)) {
      toggleCollapsed(node, event);
    }
  }

</script>

<section class="navigation-tree" aria-label={t("project-navigation-semantic-tree")}>
  {#if focusedView}
    <nav class="source-context" aria-label={t("project-navigation-semantic-tree")}>
      {#if enteredContext}
        <button
          type="button"
          class="context-return"
          title={t("project-navigation-exit-scope")}
          onclick={returnFromEnteredContext}
        >
          <IconArrowBackUp size={13} stroke={1.8} />
        </button>
        <div class="entered-context" title={enteredContext.title}>
          <span>{enteredContext.documentName}</span>
          {#if enteredContext.scopeName}
            <span class="context-separator">·</span>
            <strong>{enteredContext.scopeName}</strong>
          {/if}
        </div>
      {:else}
        <div class="breadcrumbs">
          {#each focusedView.breadcrumbs as crumb, index (crumb.sourceNodeId)}
            {#if index > 0}<span class="crumb-separator">/</span>{/if}
            {#if crumb.current}
              <span
                class="current-crumb"
                title={crumb.documentPath}
                aria-current="page"
              >
                {fileName(crumb.templateName)}
              </span>
            {:else}
              <button
                type="button"
                title={crumb.documentPath}
                onclick={() => { void openDocument(crumb.documentPath); }}
              >
                {fileName(crumb.templateName)}
              </button>
            {/if}
          {/each}
        </div>
      {/if}
    </nav>
  {/if}

  {#if loading && !snapshot}
    <div class="tree-state">{t("project-navigation-loading")}</div>
  {:else if error}
    <div class="tree-state error">{error}</div>
  {:else if !focusedView || rows.length === 0}
    <div class="tree-state">
      {openScope
        ? t("project-navigation-scope-empty")
        : t("project-navigation-empty")}
    </div>
  {:else}
    <div
      bind:this={treeViewport}
      id="project-navigation-tree-viewport"
      class="tree-viewport"
      bind:clientHeight={viewportHeight}
      onscroll={(event) => { scrollTop = event.currentTarget.scrollTop; }}
      onmouseleave={() => hoverViewNode(null)}
      role="tree"
      tabindex="0"
      aria-label={t("project-navigation-semantic-tree")}
    >
        <div style={`height:${topSpacer}px`} aria-hidden="true"></div>
        {#each visibleRows as row (row.node.id)}
          {@const NodeIcon = navigationNodeIcon(row.node)}
          <div
          class="navigation-row ui-entity-selectable"
          class:selected={isSelected(row.node)}
          class:coordinator-hover={isCoordinatorHovered(row.node)}
          data-ui-selected={isSelected(row.node) ? "true" : undefined}
          data-ui-hovered={isCoordinatorHovered(row.node) ? "true" : undefined}
          class:boundary={row.node.kind === "boundary"}
          class:markdown={isMarkdownBoundary(row.node)}
          class:relation={row.node.kind === "relation"}
          class:slot={row.node.kind === "slot"}
          class:scope-open={isScopeOpen(row.node)}
          class:readonly={row.node.capabilities.readOnly}
          class:drop-before={rowDropClass(row.node, "before")}
          class:drop-after={rowDropClass(row.node, "after")}
          class:drop-inside={rowDropClass(row.node, "inside")}
          class:drop-allowed={dragTargetId === row.node.editorNodeId && dragPlan?.allowed === true}
          class:drop-invalid={dragTargetId === row.node.editorNodeId && (dragPlan?.allowed === false || Boolean(dragPlanError))}
          style={`--tree-depth:${row.depth}`}
          role="treeitem"
          tabindex={isSelected(row.node) ? 0 : -1}
          aria-level={row.depth + 1}
          aria-selected={isSelected(row.node)}
          aria-expanded={hasVisibleChildren(row.node)
            ? !collapsed.has(row.node.id)
            : undefined}
          draggable={canDrag(row.node)}
          ondragstart={(event) => startDrag(row.node, event)}
          ondragover={(event) => updateDragTarget(row.node, event)}
          ondrop={(event) => { void dropOnNode(row.node, event); }}
          ondragend={clearDrag}
          onmouseenter={() => hoverViewNode(row.node)}
          onclick={() => selectViewNode(row.node)}
          onkeydown={(event) => handleRowKeydown(row.node, event)}
          >
          <span class="depth-rail" aria-hidden="true"></span>
          <button
            type="button"
            class="disclosure"
            class:invisible={!hasVisibleChildren(row.node)}
            aria-label={collapsed.has(row.node.id)
              ? t("project-navigation-expand")
              : t("project-navigation-collapse")}
            onclick={(event) => toggleCollapsed(row.node, event)}
          >
            {#if collapsed.has(row.node.id)}
              <IconChevronRight size={12} stroke={1.8} />
            {:else}
              <IconChevronDown size={12} stroke={1.8} />
            {/if}
          </button>
          <span class="node-icon" aria-hidden="true">
            <NodeIcon size={13} stroke={1.8} />
          </span>
          <span class="node-copy">
            <span class="node-label">{row.node.label}</span>
          </span>
          {#if row.node.kind === "relation" && row.node.relation?.targetDocumentPath}
            <span class="relation-open" aria-hidden="true">
              <IconArrowUpRight size={13} stroke={1.8} />
            </span>
          {:else if row.node.relation?.targetDocumentPath
            && !row.node.capabilities.canEnterBoundary}
            <button
              type="button"
              class="relation-action"
              title={row.node.relation.targetDocumentPath}
              onclick={(event) => {
                event.stopPropagation();
                void openDocument(row.node.relation?.targetDocumentPath ?? "");
              }}
            >
              <IconArrowUpRight size={13} stroke={1.8} />
            </button>
          {/if}
          {#if row.node.kind === "boundary" && row.node.capabilities.canEnterBoundary}
            <button
              type="button"
              class="scope-action"
              title={t("project-navigation-enter-scope")}
              onclick={(event) => {
                event.stopPropagation();
                void enterNodeContext(row.node);
              }}
            >
              <IconLogin2 size={13} stroke={1.8} />
              <span>{t("project-navigation-enter")}</span>
            </button>
          {:else if row.node.capabilities.readOnly}
            <span
              class="readonly-mark"
              title={t("project-navigation-readonly")}
            >•</span>
          {/if}
          {#if canDelete(row.node)}
            <button
              type="button"
              class="delete-action"
              title={row.node.kind === "boundary"
                ? t("project-navigation-delete-tera")
                : t("project-navigation-delete-element")}
              aria-label={row.node.kind === "boundary"
                ? t("project-navigation-delete-tera")
                : t("project-navigation-delete-element")}
              onclick={(event) => deleteViewNode(row.node, event)}
              onpointerdown={(event) => event.stopPropagation()}
            >
              <IconTrash size={12} stroke={1.9} />
            </button>
          {/if}
          </div>
        {/each}
        <div style={`height:${bottomSpacer}px`} aria-hidden="true"></div>
    </div>
  {/if}
</section>

<style>
  .navigation-tree {
    display: flex;
    flex: 1;
    min-height: 0;
    flex-direction: column;
    color: var(--text);
  }

  .source-context {
    display: flex;
    min-height: 29px;
    align-items: center;
    gap: 4px;
    margin: 0 2px 6px;
    padding: 3px;
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
    background: var(--material-control);
    box-shadow: var(--shadow-control);
  }

  .breadcrumbs {
    display: flex;
    min-width: 0;
    flex: 1;
    align-items: center;
    overflow: hidden;
  }

  .breadcrumbs button,
  .current-crumb,
  .context-return {
    display: inline-flex;
    min-width: 0;
    height: 21px;
    align-items: center;
    border: 0;
    border-radius: 5px;
    color: var(--text-muted);
    background: transparent;
    font-size: var(--font-meta);
  }

  .breadcrumbs button,
  .current-crumb {
    max-width: 78px;
    padding: 0 4px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .breadcrumbs button:hover,
  .context-return:hover {
    color: var(--text-strong);
    background: var(--material-control-hover);
  }

  .current-crumb {
    color: var(--brand-strong);
  }

  .crumb-separator {
    flex: 0 0 auto;
    color: var(--text-faint);
    font-size: var(--font-meta);
  }

  .entered-context {
    display: inline-flex;
    min-width: 0;
    flex: 1;
    align-items: center;
    gap: 3px;
    padding: 0 4px;
    color: var(--brand-strong);
    font-size: var(--font-meta);
    overflow: hidden;
    white-space: nowrap;
  }

  .entered-context span,
  .entered-context strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .entered-context strong {
    font-weight: 700;
  }

  .context-separator {
    flex: 0 0 auto;
    color: var(--text-faint);
  }

  .context-return {
    flex: 0 0 22px;
    justify-content: center;
    padding: 0;
    border-right: 1px solid var(--border);
    border-radius: 5px 0 0 5px;
  }

  .scope-action,
  .relation-action,
  .delete-action,
  .disclosure {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 0;
    color: inherit;
    background: transparent;
  }

  .tree-viewport {
    position: relative;
    width: calc(100% + var(--project-pane-padding));
    min-height: 0;
    flex: 1;
    box-sizing: border-box;
    padding-right: var(--project-pane-padding);
    overflow-x: hidden;
    overflow-y: auto;
    outline: none;
  }

  .navigation-row {
    --indent: calc(var(--tree-depth) * 13px);
    position: relative;
    display: flex;
    height: 30px;
    align-items: center;
    gap: 2px;
    padding: 0 5px 0 calc(1px + var(--indent));
    border: 1px solid transparent;
    border-radius: 7px;
    color: var(--text);
    font-size: 11px;
    user-select: none;
  }

  .navigation-row.boundary,
  .navigation-row.relation,
  .navigation-row.slot {
    color: color-mix(in srgb, var(--text) 88%, var(--brand));
  }

  .navigation-row.boundary:not(.scope-open)::before {
    position: absolute;
    inset: 4px auto 4px calc(var(--indent) + 1px);
    width: 2px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--brand) 65%, var(--border));
    content: "";
  }

  .navigation-row.markdown {
    color: color-mix(in srgb, var(--text) 76%, var(--markdown));
    background: var(--markdown-soft);
  }

  .navigation-row.markdown:not(.scope-open)::before {
    background: color-mix(in srgb, var(--markdown) 76%, var(--border));
  }

  .navigation-row.scope-open {
    background: color-mix(in srgb, var(--brand-soft) 60%, var(--surface-panel));
  }

  .navigation-row.scope-open:hover,
  .navigation-row.scope-open.coordinator-hover,
  .navigation-row.scope-open.selected {
    background: transparent;
  }

  .navigation-row.readonly {
    color: var(--text-muted);
  }

  .navigation-row.drop-before::before,
  .navigation-row.drop-after::after {
    position: absolute;
    right: 5px;
    left: calc(5px + var(--indent));
    z-index: 2;
    height: 2px;
    border-radius: 999px;
    background: var(--brand);
    content: "";
  }

  .navigation-row.drop-before::before { top: -1px; }
  .navigation-row.drop-after::after { bottom: -1px; }

  .navigation-row.drop-inside {
    border-color: var(--brand);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--brand) 30%, transparent);
  }

  .navigation-row.drop-allowed.drop-before::before,
  .navigation-row.drop-allowed.drop-after::after {
    background: var(--success);
  }

  .navigation-row.drop-allowed.drop-inside {
    border-color: var(--success);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--success) 30%, transparent);
  }

  .navigation-row.drop-invalid {
    border-color: var(--danger);
    background: color-mix(in srgb, var(--danger) 7%, var(--material-panel));
  }

  .navigation-row.drop-invalid.drop-before::before,
  .navigation-row.drop-invalid.drop-after::after {
    background: var(--danger);
  }

  .navigation-row.drop-invalid.drop-inside {
    border-color: var(--danger);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--danger) 30%, transparent);
  }

  .disclosure {
    position: relative;
    width: 12px;
    flex: 0 0 12px;
    height: 22px;
    padding: 0;
    border-radius: 5px;
  }

  .disclosure::before {
    position: absolute;
    inset: 0 -4px;
    content: "";
  }

  .disclosure:hover,
  .scope-action:hover {
    background: var(--material-control-selected);
  }

  .disclosure.invisible { visibility: hidden; }

  .node-icon {
    display: inline-flex;
    flex: 0 0 14px;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
  }

  .boundary .node-icon,
  .relation .node-icon,
  .slot .node-icon,
  .selected .node-icon {
    color: var(--brand);
  }

  .markdown .node-icon,
  .markdown.selected .node-icon {
    color: var(--markdown);
  }

  .node-copy {
    display: flex;
    min-width: 0;
    flex: 1;
    align-items: baseline;
    gap: 6px;
  }

  .node-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .node-label { flex: 0 1 auto; font-weight: 560; }

  .relation-open {
    display: inline-flex;
    flex: 0 0 auto;
    align-items: center;
    color: var(--text-muted);
  }

  .scope-action {
    min-width: 26px;
    height: 23px;
    gap: 3px;
    padding: 0 5px;
    border: 1px solid color-mix(in srgb, var(--brand) 30%, var(--border));
    border-radius: 6px;
    color: var(--brand-strong);
    font-size: var(--font-meta);
  }

  .relation-action {
    width: 23px;
    height: 23px;
    padding: 0;
    border-radius: 6px;
    color: var(--text-muted);
  }

  .relation-action:hover {
    color: var(--brand-strong);
    background: var(--material-control-selected);
  }

  .readonly-mark {
    display: inline-flex;
    flex: 0 0 auto;
    align-items: center;
    color: var(--text-muted);
  }

  .delete-action {
    width: 20px;
    min-width: 20px;
    height: 20px;
    min-height: 20px;
    flex: 0 0 20px;
    padding: 0;
    border-radius: 5px;
    color: color-mix(in srgb, var(--danger) 82%, var(--text-muted));
    opacity: 0;
    pointer-events: none;
    transition:
      opacity 80ms ease,
      color 80ms ease,
      background 80ms ease;
  }

  .navigation-row:hover .delete-action,
  .navigation-row.coordinator-hover .delete-action,
  .navigation-row.selected .delete-action,
  .navigation-row:focus-within .delete-action {
    opacity: 1;
    pointer-events: auto;
  }

  .delete-action:hover,
  .delete-action:focus-visible {
    color: var(--danger);
    background: color-mix(in srgb, var(--danger) 12%, var(--material-control));
    outline: none;
  }

  .tree-state {
    margin: 2px;
    padding: 12px 9px;
    border: 1px solid var(--border);
    border-radius: 8px;
    color: var(--text-muted);
    background: var(--material-control);
    font-size: 11px;
    line-height: 1.4;
  }

  .tree-state.error {
    border-color: color-mix(in srgb, var(--danger) 35%, var(--border));
    color: var(--danger);
  }

</style>
