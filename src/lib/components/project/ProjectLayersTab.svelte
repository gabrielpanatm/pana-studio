<script lang="ts">
  import {
    IconAlignLeft,
    IconArticle,
    IconBox,
    IconChevronDown,
    IconChevronRight,
    IconCode,
    IconCursorText,
    IconEdit,
    IconForms,
    IconHeading,
    IconLayout,
    IconLayoutBottombar,
    IconLayoutNavbar,
    IconLayoutSidebarRight,
    IconLink,
    IconList,
    IconNavigation,
    IconPhoto,
    IconTrash,
  } from "@tabler/icons-svelte";
  import type {
    PageSection,
    PreviewSelectionState,
    SelectionInfo,
    SourceGraph,
    TemplateWorkbenchPlan,
  } from "$lib/types";
  import type { EditorLayerContextMenuRequest } from "$lib/editor-runtime/commands";
  import type { EditorActionOutcome } from "$lib/editor-runtime/action-outcome";
  import {
    canLayerReceiveChildren,
    validateLayerStructureDrop,
    type LayerMoveRequest,
  } from "$lib/project/layers-drag";
  import {
    buildLayerRows,
    type LayerRow,
    type TeraLayerNode,
  } from "$lib/project/layers-tree";
  import { activeTemplateFilesForContext, teraGateDropStatus } from "$lib/source-graph/interaction";
  import { resolveTeraMoveTarget } from "$lib/tera/move-targets";
  import type { TeraMoveRequest } from "$lib/tera/model";
  import { computeVisibleSections, type LayerNode } from "$lib/project/pane-tree";
  import { dropPositionFromPointer, type DropPosition } from "$lib/ui/drag";
  import { onDestroy, onMount } from "svelte";
  import { listenForExternalReconcileInteractionBarrier } from "$lib/session/external-reconcile-barrier";

  export let pageSections: PageSection[] = [];
  export let selectedElement: SelectionInfo | null = null;
  export let previewSelection: PreviewSelectionState = { kind: "none" };
  export let sourceGraph: SourceGraph | null = null;
  export let activeScannedPath: string | null = null;
  export let activeRenderedTemplatePath: string | null = null;
  export let templateHtmlEditSourceId: string | null = null;
  export let templateWorkbenchPlan: TemplateWorkbenchPlan | null = null;
  export let selectPageSection: (section: PageSection) => void;
  export let selectTeraSource: (section: PageSection, sourceId: string) => void;
  export let hoverPageSection: (section: PageSection | null) => void;
  export let hoverTeraSource: (section: PageSection, sourceId: string) => void;
  export let moveLayerElement: (request: LayerMoveRequest) => Promise<EditorActionOutcome>;
  export let moveTeraNode: (request: TeraMoveRequest) => void | Promise<void>;
  export let deleteLayerElement: (selector: string) => void | Promise<void>;
  export let openLayerContextMenu: (request: EditorLayerContextMenuRequest) => void;
  export let editSelectedTeraLayer: () => void | Promise<void>;
  export let deleteSelectedTeraNode: () => void | Promise<void>;
  export let openSelectedTeraSource: () => void | Promise<void>;
  export let openTemplateWorkbenchSource: (file: string) => void | Promise<void> = () => {};

  let collapsedNodes = new Set<string>();
  let dragSourceSelector: string | null = null;
  let dragSourceTeraId: string | null = null;
  let dragTargetSelector: string | null = null;
  let dragTargetKey: string | null = null;
  let dragDropPosition: DropPosition | null = null;
  let dragCandidate:
    | {
        kind: "html";
        selector: string;
        sourceId: string | null;
        templateSourceId: string | null;
        sessionId: string | null;
        pointerId: number;
        startX: number;
        startY: number;
      }
    | { kind: "tera"; sourceId: string; selector: string; pointerId: number; startX: number; startY: number }
    | null = null;
  let dragPointer = { x: 0, y: 0 };
  let dragDropInvalid = false;
  let dragDropMessage = "";
  let dragDropTargetLabel = "";
  let dragActive = false;
  let suppressNextClick = false;

  $: visibleSections = computeVisibleSections(pageSections, collapsedNodes);
  $: sourceNodesById = new Map((sourceGraph?.nodes ?? []).map((node) => [node.id, node]));
  $: activeTemplateFiles = templateWorkbenchPlan
    ? templateWorkbenchPlan.navigator.map((entry) => entry.template.file)
    : activeTemplateFilesForContext(
        sourceGraph,
        activeRenderedTemplatePath ?? activeScannedPath,
        selectedElement?.sourceLocation?.file ?? null,
      );
  $: visibleRows = buildLayerRows(visibleSections, sourceNodesById, {
    sourceGraph,
    templateWorkbenchPlan,
    gateOpenContext: {
      openedGateSourceId: templateHtmlEditSourceId,
      activeScannedPath: activeRenderedTemplatePath ?? activeScannedPath,
      activeTemplateFiles,
    },
  });
  $: if (typeof document !== "undefined") {
    document.body.classList.toggle("layers-dragging", dragActive);
  }

  function toggleNodeCollapse(selector: string, event: Event) {
    event.stopPropagation();
    const next = new Set(collapsedNodes);
    if (next.has(selector)) next.delete(selector);
    else next.add(selector);
    collapsedNodes = next;
  }

  function isLayerSelected(section: LayerNode): boolean {
    if (previewSelection.kind === "html") {
      return previewSelection.selector === section.selector
        || previewSelection.selection.domPath === section.selector
        || previewSelection.selection.cssSelector === section.selector;
    }

    if (previewSelection.kind === "tera") {
      return isLayerTeraSelected(section);
    }

    return selectedElement?.domPath === section.selector || selectedElement?.cssSelector === section.selector;
  }

  function isLayerTeraSelected(section: LayerNode): boolean {
    if (previewSelection.kind !== "tera") return false;
    return previewSelection.selector === section.selector
      || section.sourceId === previewSelection.sourceId
      || section.templateSourceId === previewSelection.sourceId
      || (previewSelection.templateSourceId !== null && section.templateSourceId === previewSelection.templateSourceId);
  }

  function isTeraRowSelected(row: TeraLayerNode): boolean {
    if (previewSelection.kind !== "tera") return false;
    if (previewSelection.sourceId || previewSelection.templateSourceId) {
      return previewSelection.sourceId === row.id || previewSelection.templateSourceId === row.id;
    }
    return previewSelection.sourceId === row.id
      || previewSelection.templateSourceId === row.id
      || previewSelection.selector === row.selector;
  }

  function layerSourceTone(section: LayerNode) {
    const sourceNode = section.sourceId ? sourceNodesById.get(section.sourceId) : null;
    const templateNode = section.templateSourceId ? sourceNodesById.get(section.templateSourceId) : null;
    const file = sourceNode?.file ?? templateNode?.file ?? section.sourceLocation?.file ?? "";
    const active = activeRenderedTemplatePath ?? activeScannedPath ?? "";
    if (active && file === active) return "current";
    const origin = sourceNode?.origin ?? templateNode?.origin;
    if (origin) return origin;
    if (file.startsWith("templates/")) return "local";
    if (/^themes\/[^/]+\/templates\//.test(file)) return "theme";
    return "unknown";
  }

  function clearDragState() {
    dragSourceSelector = null;
    dragSourceTeraId = null;
    dragTargetSelector = null;
    dragTargetKey = null;
    dragDropPosition = null;
    dragCandidate = null;
    dragDropInvalid = false;
    dragDropMessage = "";
    dragDropTargetLabel = "";
    dragActive = false;
  }

  function cleanupPointerListeners() {
    window.removeEventListener("pointermove", handleWindowPointerMove);
    window.removeEventListener("pointerup", handleWindowPointerUp);
    window.removeEventListener("pointercancel", handleWindowPointerCancel);
  }

  function cancelDragForExternalReconcile() {
    cleanupPointerListeners();
    clearDragState();
  }

  function handlePointerDown(node: LayerNode, event: PointerEvent) {
    if (event.button !== 0) return;
    if (event.target instanceof HTMLElement && event.target.closest(".toggle-btn, .tree-action-btn, .tree-delete-btn")) return;

    cleanupPointerListeners();
    dragCandidate = {
      kind: "html",
      selector: node.selector,
      sourceId: node.sourceId ?? null,
      templateSourceId: node.templateSourceId ?? null,
      sessionId: node.sessionId ?? null,
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
    };
    window.addEventListener("pointermove", handleWindowPointerMove);
    window.addEventListener("pointerup", handleWindowPointerUp);
    window.addEventListener("pointercancel", handleWindowPointerCancel);
  }

  function handleTeraPointerDown(row: TeraLayerNode, event: PointerEvent) {
    if (event.button !== 0) return;
    if (event.target instanceof HTMLElement && event.target.closest(".tree-action-btn")) return;

    cleanupPointerListeners();
    dragCandidate = {
      kind: "tera",
      sourceId: row.id,
      selector: row.selector,
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
    };
    window.addEventListener("pointermove", handleWindowPointerMove);
    window.addEventListener("pointerup", handleWindowPointerUp);
    window.addEventListener("pointercancel", handleWindowPointerCancel);
  }

  function htmlRowKey(selector: string) {
    return `html:${selector}`;
  }

  function teraRowKey(sourceId: string) {
    return `tera:${sourceId}`;
  }

  type PointerLayerRow =
    | { kind: "html"; key: string; row: HTMLElement; node: LayerNode }
    | { kind: "tera"; key: string; row: HTMLElement; tera: TeraLayerNode };

  function layerRowForPointer(event: PointerEvent): PointerLayerRow | null {
    const element = document.elementFromPoint(event.clientX, event.clientY);
    const row = element instanceof HTMLElement
      ? element.closest("[data-layer-row]") as HTMLElement | null
      : null;
    const rowKind = row?.dataset.layerRow;
    const selector = row?.dataset.layerSelector;
    if (!row || !selector) return null;
    if (rowKind === "tera") {
      const sourceId = row.dataset.teraSourceId;
      const tera = visibleRows.find((item): item is TeraLayerNode =>
        item.kind === "tera" && item.id === sourceId,
      );
      return tera ? { kind: "tera", key: teraRowKey(tera.id), row, tera } : null;
    }
    const node = visibleSections.find((item) => item.selector === selector);
    return node ? { kind: "html", key: htmlRowKey(node.selector), row, node } : null;
  }

  function dragSourceKey(candidate = dragCandidate) {
    if (!candidate) return null;
    return candidate.kind === "tera" ? teraRowKey(candidate.sourceId) : htmlRowKey(candidate.selector);
  }

  function targetLabel(target: PointerLayerRow | LayerRow) {
    if (target.kind === "tera") {
      const label = "tera" in target ? target.tera.label : target.label;
      return label.length > 26 ? `${label.slice(0, 23)}...` : label;
    }
    const label = target.node.label;
    return label.length > 26 ? `${label.slice(0, 23)}...` : label;
  }

  function validateHtmlLayerDrop(sourceSelector: string, target: PointerLayerRow, position: DropPosition) {
    if (target.kind !== "html") {
      return { allowed: false, reason: "Mutarea HTML folosește rânduri HTML ca destinație." };
    }
    const sourceNode = visibleSections.find((item) => item.selector === sourceSelector);
    const gateValidation = teraGateDropStatus(sourceGraph, {
      openedGateSourceId: templateHtmlEditSourceId,
      activeScannedPath: activeRenderedTemplatePath ?? activeScannedPath,
      activeTemplateFiles,
    }, {
      targetSourceId: target.node.sourceId,
      targetTemplateSourceId: target.node.templateSourceId,
    });
    return gateValidation.allowed
      ? validateLayerStructureDrop(sourceNode, target.node, position)
      : { allowed: false, reason: gateValidation.message };
  }

  function teraMoveRequestFor(sourceId: string, target: LayerRow | undefined, position: DropPosition): TeraMoveRequest | null {
    if (!target) return null;
    if (target.kind === "tera") {
      return {
        sourceId,
        targetSelector: target.selector,
        targetSourceId: target.section.sourceId ?? null,
        targetTemplateSourceId: target.id,
        targetTag: "tera",
        targetKind: "tera",
        position,
      };
    }
    return {
      sourceId,
      targetSelector: target.node.selector,
      targetSourceId: target.node.sourceId ?? null,
      targetTemplateSourceId: target.node.templateSourceId ?? null,
      targetTag: target.node.tag,
      targetKind: "html",
      position,
    };
  }

  function validateTeraLayerDrop(sourceId: string, target: PointerLayerRow, position: DropPosition) {
    const targetRow = target.kind === "tera"
      ? visibleRows.find((row): row is TeraLayerNode => row.kind === "tera" && row.id === target.tera.id)
      : visibleRows.find((row) => row.kind === "html" && row.node.selector === target.node.selector);
    const request = teraMoveRequestFor(sourceId, targetRow, position);
    if (!request) return { allowed: false, reason: "Nu am găsit destinația Tera." };
    const resolution = resolveTeraMoveTarget(sourceGraph, request, {
      activeScannedPath,
      activeTemplatePath: activeRenderedTemplatePath,
    });
    return resolution.allowed
      ? { allowed: true }
      : { allowed: false, reason: resolution.reason };
  }

  function handleWindowPointerMove(event: PointerEvent) {
    if (!dragCandidate || event.pointerId !== dragCandidate.pointerId) return;
    const distance = Math.hypot(event.clientX - dragCandidate.startX, event.clientY - dragCandidate.startY);
    if (!dragActive && distance < 6) return;

    event.preventDefault();
    dragPointer = { x: event.clientX, y: event.clientY };
    if (!dragActive) {
      dragActive = true;
      if (dragCandidate.kind === "html") {
        dragSourceSelector = dragCandidate.selector;
        const sourceNode = visibleSections.find((item) => item.selector === dragCandidate?.selector);
        if (sourceNode) selectPageSection(sourceNode);
      } else {
        const sourceId = dragCandidate.sourceId;
        dragSourceTeraId = sourceId;
        const sourceRow = visibleRows.find((item): item is TeraLayerNode =>
          item.kind === "tera" && item.id === sourceId,
        );
        if (sourceRow) selectTeraSource(sourceRow.section, sourceRow.id);
      }
    }

    const target = layerRowForPointer(event);
    if (!target || target.key === dragSourceKey()) {
      dragTargetSelector = null;
      dragTargetKey = null;
      dragDropPosition = null;
      dragDropInvalid = false;
      dragDropMessage = "";
      dragDropTargetLabel = "";
      return;
    }

    const position = dropPositionFromPointer(event, target.row, {
      allowInside: target.kind === "html" ? canLayerReceiveChildren(target.node.tag) : true,
    });
    const validation = dragCandidate.kind === "tera"
      ? validateTeraLayerDrop(dragCandidate.sourceId, target, position)
      : validateHtmlLayerDrop(dragCandidate.selector, target, position);
    dragTargetSelector = target.kind === "html" ? target.node.selector : target.tera.selector;
    dragTargetKey = target.key;
    dragDropPosition = position;
    dragDropInvalid = !validation.allowed;
    dragDropMessage = validation.allowed
      ? dropHintLabel(position, targetLabel(target))
      : validation.reason ?? "Drop invalid";
    dragDropTargetLabel = targetLabel(target);
  }

  async function handleWindowPointerUp(event: PointerEvent) {
    if (!dragCandidate || event.pointerId !== dragCandidate.pointerId) return;
    cleanupPointerListeners();

    const sourceSelector = dragCandidate.selector;
    const targetSelector = dragTargetSelector;
    const targetKey = dragTargetKey;
    const position = dragDropPosition;
    const wasDrag = dragActive;
    const wasInvalid = dragDropInvalid;
    const candidate = dragCandidate;
    clearDragState();

    if (!wasDrag) return;
    suppressNextClick = true;
    window.setTimeout(() => {
      suppressNextClick = false;
    }, 0);
    event.preventDefault();
    if (wasInvalid || !targetSelector || !position || !candidate || targetKey === dragSourceKey(candidate)) return;
    if (candidate.kind === "tera") {
      const target = visibleRows.find((row) =>
        row.kind === "tera" ? teraRowKey(row.id) === targetKey : htmlRowKey(row.node.selector) === targetKey,
      );
      const request = teraMoveRequestFor(candidate.sourceId, target, position);
      if (request) await moveTeraNode(request);
      return;
    }
    const target = visibleRows.find((row) =>
      row.kind === "html" && htmlRowKey(row.node.selector) === targetKey,
    );
    await moveLayerElement({
      sourceSelector,
      targetSelector,
      sourceSessionId: candidate.sessionId,
      sourceSourceId: candidate.sourceId,
      sourceTemplateSourceId: candidate.templateSourceId,
      targetSessionId: target?.kind === "html" ? target.node.sessionId ?? null : null,
      targetSourceId: target?.kind === "html" ? target.node.sourceId ?? null : null,
      targetTemplateSourceId: target?.kind === "html" ? target.node.templateSourceId ?? null : null,
      position,
    });
  }

  function handleWindowPointerCancel(event: PointerEvent) {
    if (!dragCandidate || event.pointerId !== dragCandidate.pointerId) return;
    cleanupPointerListeners();
    clearDragState();
  }

  function isDropTarget(node: LayerNode, position: DropPosition) {
    return !dragDropInvalid && dragTargetKey === htmlRowKey(node.selector) && dragDropPosition === position;
  }

  function isInvalidDropTarget(node: LayerNode) {
    return dragDropInvalid && dragTargetKey === htmlRowKey(node.selector);
  }

  function isTeraDropTarget(row: TeraLayerNode, position: DropPosition) {
    return !dragDropInvalid && dragTargetKey === teraRowKey(row.id) && dragDropPosition === position;
  }

  function isInvalidTeraDropTarget(row: TeraLayerNode) {
    return dragDropInvalid && dragTargetKey === teraRowKey(row.id);
  }

  function rowTitle(node: LayerNode) {
    if (dragActive) return undefined;
    return node.label;
  }

  function dropPositionLabel(position: DropPosition) {
    if (position === "before") return "Înainte";
    if (position === "after") return "După";
    return "Copil";
  }

  function dropHintLabel(position: DropPosition, target: string) {
    if (position === "before") return `Înainte de ${target}`;
    if (position === "after") return `După ${target}`;
    return `În interiorul ${target}`;
  }

  function handleRowClick(node: LayerNode) {
    if (suppressNextClick) {
      suppressNextClick = false;
      return;
    }
    selectPageSection(node);
  }

  function handleRowKeydown(node: LayerNode, event: KeyboardEvent) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      handleRowClick(node);
      return;
    }
    if (!node.hasChildren) return;
    if (event.key === "ArrowRight" && collapsedNodes.has(node.selector)) {
      event.preventDefault();
      toggleNodeCollapse(node.selector, event);
    }
    if (event.key === "ArrowLeft" && !collapsedNodes.has(node.selector)) {
      event.preventDefault();
      toggleNodeCollapse(node.selector, event);
    }
  }

  function handleRowMouseEnter(node: LayerNode) {
    hoverPageSection(node);
  }

  function handleRowMouseLeave() {
    hoverPageSection(null);
  }

  function canDeleteNode(node: LayerNode) {
    if (isLayerTeraSelected(node)) return false;
    return true;
  }

  function handleDeleteClick(node: LayerNode, event: MouseEvent | KeyboardEvent) {
    event.preventDefault();
    event.stopPropagation();
    if (!canDeleteNode(node)) return;
    void deleteLayerElement(node.selector);
  }

  function openHtmlLayerContextMenu(node: LayerNode, event: MouseEvent) {
    event.preventDefault();
    event.stopPropagation();
    cleanupPointerListeners();
    clearDragState();

    openLayerContextMenu({
      kind: "html",
      section: node,
      x: event.clientX,
      y: event.clientY,
      label: node.label,
    });
  }

  function isLayerTeraActionTarget(node: LayerNode) {
    if (previewSelection.kind !== "tera") return false;
    const hasSyntheticTeraRow = Boolean(node.templateSourceId);
    if (hasSyntheticTeraRow) return false;
    return previewSelection.selector === node.selector;
  }

  function isTeraRowActionTarget(row: TeraLayerNode) {
    return isTeraRowSelected(row);
  }

  function handleTeraRowClick(row: TeraLayerNode) {
    if (suppressNextClick) {
      suppressNextClick = false;
      return;
    }
    selectTeraSource(row.section, row.id);
  }

  function handleTeraRowKeydown(row: TeraLayerNode, event: KeyboardEvent) {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    handleTeraRowClick(row);
  }

  function handleTeraRowMouseEnter(row: TeraLayerNode) {
    hoverTeraSource(row.section, row.id);
  }

  function handleTeraEditClick(event: MouseEvent | KeyboardEvent) {
    event.preventDefault();
    event.stopPropagation();
    void editSelectedTeraLayer();
  }

  function handleTeraDeleteClick(event: MouseEvent | KeyboardEvent) {
    event.preventDefault();
    event.stopPropagation();
    void deleteSelectedTeraNode();
  }

  function handleTeraSourceClick(event: MouseEvent | KeyboardEvent) {
    event.preventDefault();
    event.stopPropagation();
    void openSelectedTeraSource();
  }

  function openTeraLayerContextMenu(row: TeraLayerNode, event: MouseEvent) {
    event.preventDefault();
    event.stopPropagation();
    cleanupPointerListeners();
    clearDragState();

    openLayerContextMenu({
      kind: "tera",
      section: row.section,
      sourceId: row.id,
      selector: row.selector,
      x: event.clientX,
      y: event.clientY,
      label: row.label,
      kindLabel: row.kindLabel,
      file: row.sourceNode?.file ?? null,
      origin: row.origin,
      themeName: row.themeName,
    });
  }

  onMount(() => listenForExternalReconcileInteractionBarrier(cancelDragForExternalReconcile));
  onDestroy(cleanupPointerListeners);
  onDestroy(() => {
    document.body.classList.remove("layers-dragging");
    hoverPageSection(null);
  });
</script>

{#if pageSections.length === 0}
  <div class="tab-empty-state">Selectează o pagină pentru a vedea structura.</div>
{:else}
  <div class="layers-tree" role="tree" aria-label="Structura paginii">
    {#if templateWorkbenchPlan}
      <div class="workbench-source-chain" aria-label="Ierarhia contextului de template">
        {#each templateWorkbenchPlan.navigator as entry, index}
          <button
            type="button"
            class="workbench-source-row"
            class:active-source={entry.role === "active"}
            class:source-theme={entry.template.origin === "theme"}
            class:source-local={entry.template.origin === "local"}
            style="--source-depth: {index};"
            title={entry.template.file}
            onclick={() => { void openTemplateWorkbenchSource(entry.template.file); }}
          >
            <span class="workbench-source-indent" style="width: {index * 14}px;"></span>
            <IconChevronDown size={11} stroke={2.2} />
            <IconCode size={12} stroke={2} />
            <span class="workbench-source-copy">
              <small>{entry.role === "active" ? "Template activ" : "Părinte direct"}</small>
              <strong>{entry.template.name}</strong>
            </span>
            <span class="workbench-source-origin">
              {entry.role === "active" ? "editabil" : entry.template.origin}
            </span>
          </button>
        {/each}
      </div>
    {/if}
    {#each visibleRows as row}
      {#if row.kind === "tera"}
        <div
          class="tree-row tera-node"
          class:selected={isTeraRowSelected(row)}
          class:source-local={row.origin === "local"}
          class:source-theme={row.origin === "theme"}
          class:source-unknown={row.origin === "unknown"}
          class:tera-selected={isTeraRowSelected(row)}
          class:dragging={dragSourceTeraId === row.id}
          class:drop-before={isTeraDropTarget(row, "before")}
          class:drop-after={isTeraDropTarget(row, "after")}
          class:drop-inside={isTeraDropTarget(row, "inside")}
          class:drop-invalid={isInvalidTeraDropTarget(row)}
          data-layer-row="tera"
          data-layer-selector={row.selector}
          data-tera-source-id={row.id}
          style="--depth: {row.depth};"
          role="treeitem"
          tabindex="0"
          aria-level={row.depth + 1}
          aria-selected={isTeraRowSelected(row)}
          title={`${row.kindLabel}: ${row.label}`}
          onclick={() => handleTeraRowClick(row)}
          onkeydown={(event) => handleTeraRowKeydown(row, event)}
          oncontextmenu={(event) => openTeraLayerContextMenu(row, event)}
          onmouseenter={() => handleTeraRowMouseEnter(row)}
          onmouseleave={handleRowMouseLeave}
          onpointerdown={(event) => handleTeraPointerDown(row, event)}
        >
          <span class="tree-gutter" style="width: {row.depth * 14}px;"></span>
          <span class="tree-toggle"><span class="toggle-dot tera-dot"></span></span>
          <span class="tree-icon tera-icon"><IconCode size={12} stroke={2} /></span>
          <span class="tree-label">
            <span class="tree-kicker">{row.kindLabel}</span>
            {row.label}
          </span>
          {#if isTeraRowActionTarget(row)}
            <span class="tree-actions" aria-label="Acțiuni Tera">
              <button
                type="button"
                class="tree-action-btn"
                title="Editează HTML vizual"
                onclick={handleTeraEditClick}
                onpointerdown={(event) => event.stopPropagation()}
              >
                <IconEdit size={11} stroke={2.1} />
              </button>
              <button
                type="button"
                class="tree-action-btn"
                title="Deschide sursa"
                onclick={handleTeraSourceClick}
                onpointerdown={(event) => event.stopPropagation()}
              >
                <IconCode size={11} stroke={2.1} />
              </button>
              <button
                type="button"
                class="tree-action-btn danger"
                title="Șterge nodul Tera"
                onclick={handleTeraDeleteClick}
                onpointerdown={(event) => event.stopPropagation()}
              >
                <IconTrash size={11} stroke={2.1} />
              </button>
            </span>
          {/if}
          {#if dragTargetKey === teraRowKey(row.id) && dragDropPosition && !dragDropInvalid}
            <span class="tree-drop-label">{dropPositionLabel(dragDropPosition)}</span>
          {:else if isInvalidTeraDropTarget(row)}
            <span class="tree-drop-label invalid">Interzis</span>
          {/if}
          <small class="tree-source-badge">Tera</small>
        </div>
      {:else}
        {@const node = row.node}
      <div
        class="tree-row"
        class:selected={isLayerSelected(node)}
        class:source-current={layerSourceTone(node) === "current"}
        class:source-local={layerSourceTone(node) === "local"}
        class:source-theme={layerSourceTone(node) === "theme"}
        class:source-unknown={layerSourceTone(node) === "unknown"}
        class:tera-selected={isLayerTeraSelected(node)}
        class:dragging={dragSourceSelector === node.selector}
        class:drop-before={isDropTarget(node, "before")}
        class:drop-after={isDropTarget(node, "after")}
        class:drop-inside={isDropTarget(node, "inside")}
        class:drop-invalid={isInvalidDropTarget(node)}
        data-layer-row="html"
        data-layer-selector={node.selector}
        style="--depth: {row.depth};"
        role="treeitem"
        tabindex="0"
        aria-level={row.depth + 1}
        aria-selected={isLayerSelected(node)}
        aria-expanded={node.hasChildren ? !collapsedNodes.has(node.selector) : undefined}
        title={rowTitle(node)}
        onclick={() => handleRowClick(node)}
        onkeydown={(event) => handleRowKeydown(node, event)}
        oncontextmenu={(event) => openHtmlLayerContextMenu(node, event)}
        onmouseenter={() => handleRowMouseEnter(node)}
        onmouseleave={handleRowMouseLeave}
        onpointerdown={(event) => handlePointerDown(node, event)}
      >
        <span class="tree-gutter" style="width: {row.depth * 14}px;"></span>
        <span class="tree-toggle">
          {#if node.hasChildren}
            <button
              type="button"
              class="toggle-btn"
              aria-label={collapsedNodes.has(node.selector) ? `Extinde ${node.label}` : `Restrânge ${node.label}`}
              aria-expanded={!collapsedNodes.has(node.selector)}
              onclick={(event) => toggleNodeCollapse(node.selector, event)}
              onpointerdown={(event) => event.stopPropagation()}
            >
              {#if collapsedNodes.has(node.selector)}
                <IconChevronRight size={10} stroke={2.2} />
              {:else}
                <IconChevronDown size={10} stroke={2.2} />
              {/if}
            </button>
          {:else}
            <span class="toggle-dot"></span>
          {/if}
        </span>
        <span class="tree-icon">
          {#if node.tag === "nav"}<IconNavigation size={12} stroke={1.8} />
          {:else if node.tag === "header"}<IconLayoutNavbar size={12} stroke={1.8} />
          {:else if node.tag === "footer"}<IconLayoutBottombar size={12} stroke={1.8} />
          {:else if node.tag === "main" || node.tag === "section"}<IconLayout size={12} stroke={1.8} />
          {:else if node.tag === "article"}<IconArticle size={12} stroke={1.8} />
          {:else if node.tag === "aside"}<IconLayoutSidebarRight size={12} stroke={1.8} />
          {:else if node.tag === "img"}<IconPhoto size={12} stroke={1.8} />
          {:else if node.tag === "a"}<IconLink size={12} stroke={1.8} />
          {:else if node.tag === "p" || node.tag === "span"}<IconAlignLeft size={12} stroke={1.8} />
          {:else if node.tag === "h1" || node.tag === "h2" || node.tag === "h3" || node.tag === "h4" || node.tag === "h5" || node.tag === "h6"}<IconHeading size={12} stroke={1.8} />
          {:else if node.tag === "ul" || node.tag === "ol"}<IconList size={12} stroke={1.8} />
          {:else if node.tag === "form"}<IconForms size={12} stroke={1.8} />
          {:else if node.tag === "button"}<IconCursorText size={12} stroke={1.8} />
          {:else}<IconBox size={12} stroke={1.8} />
          {/if}
        </span>
        <span class="tree-label">{node.label}</span>
        {#if dragTargetSelector === node.selector && dragDropPosition && !dragDropInvalid}
          <span class="tree-drop-label">{dropPositionLabel(dragDropPosition)}</span>
        {:else if isInvalidDropTarget(node)}
          <span class="tree-drop-label invalid">Interzis</span>
        {/if}
        {#if isLayerTeraActionTarget(node)}
          <span class="tree-actions" aria-label="Acțiuni Tera">
            <button
              type="button"
              class="tree-action-btn"
              title="Editează HTML vizual"
              onclick={handleTeraEditClick}
              onpointerdown={(event) => event.stopPropagation()}
            >
              <IconEdit size={11} stroke={2.1} />
            </button>
            <button
              type="button"
              class="tree-action-btn"
              title="Deschide sursa"
              onclick={handleTeraSourceClick}
              onpointerdown={(event) => event.stopPropagation()}
            >
              <IconCode size={11} stroke={2.1} />
            </button>
            <button
              type="button"
              class="tree-action-btn danger"
              title="Șterge nodul Tera"
              onclick={handleTeraDeleteClick}
              onpointerdown={(event) => event.stopPropagation()}
            >
              <IconTrash size={11} stroke={2.1} />
            </button>
          </span>
        {/if}
        <button
          type="button"
          class="tree-delete-btn"
          class:disabled={!canDeleteNode(node)}
          disabled={!canDeleteNode(node)}
          title={canDeleteNode(node) ? "Șterge element" : "Zona selectată este un gate Tera; folosește acțiunile Tera din Straturi sau Inspector."}
          onclick={(event) => handleDeleteClick(node, event)}
          onpointerdown={(event) => event.stopPropagation()}
        >
          <IconTrash size={11} stroke={2.1} />
        </button>
        <small class="tree-tag">{node.tag}</small>
        {#if isLayerTeraSelected(node)}
          <small class="tree-source-badge">Tera</small>
        {/if}
      </div>
      {/if}
    {/each}
  </div>

  {#if dragActive}
    <div
      class="drag-hint"
      class:invalid={dragDropInvalid}
      style="left: {dragPointer.x + 14}px; top: {dragPointer.y + 14}px;"
    >
      {#if dragDropInvalid}
        {dragDropMessage || "Drop invalid"}
      {:else if dragDropPosition && dragDropTargetLabel}
        {dragDropMessage || dropHintLabel(dragDropPosition, dragDropTargetLabel)}
      {:else}
        Alege destinația
      {/if}
    </div>
  {/if}
{/if}

<style>
  .tab-empty-state {
    position: relative;
    z-index: 1;
    margin: 2px 0 0;
    padding: 12px 10px;
    border: 1px solid var(--border-2);
    border-radius: 8px;
    background: var(--surface-2);
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.45;
    text-align: center;
  }

  .layers-tree {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .workbench-source-chain {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0 0 5px;
    padding: 4px;
    border: 1px solid color-mix(in srgb, var(--brand) 24%, var(--border-2));
    border-radius: 7px;
    background: color-mix(in srgb, var(--brand) 4%, var(--surface-2));
  }

  .workbench-source-row {
    --source-tone: var(--source-origin-local);
    display: flex;
    align-items: center;
    gap: 5px;
    width: 100%;
    min-height: 31px;
    padding: 3px 6px;
    border: 1px solid transparent;
    border-radius: 5px;
    color: var(--text);
    text-align: left;
    background: transparent;
    cursor: pointer;
  }

  .workbench-source-row.source-theme { --source-tone: var(--source-origin-theme); }
  .workbench-source-row:hover {
    border-color: color-mix(in srgb, var(--source-tone) 42%, var(--border-3));
    background: color-mix(in srgb, var(--source-tone) 7%, var(--surface-3));
  }
  .workbench-source-row.active-source {
    border-color: color-mix(in srgb, var(--source-tone) 58%, var(--border-3));
    background: color-mix(in srgb, var(--source-tone) 11%, var(--surface-3));
  }
  .workbench-source-indent { flex: 0 0 auto; }
  .workbench-source-copy {
    display: flex;
    flex: 1 1 auto;
    min-width: 0;
    flex-direction: column;
    line-height: 1.15;
  }
  .workbench-source-copy small {
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 800;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }
  .workbench-source-copy strong {
    overflow: hidden;
    font-size: 12px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .workbench-source-origin {
    color: var(--source-tone);
    font-size: 12px;
    font-weight: 800;
  }

  .tree-row {
    --layer-origin: var(--source-origin-unknown);
    --layer-origin-soft: var(--source-origin-unknown-soft);
    position: relative;
    display: flex;
    align-items: center;
    gap: 4px;
    width: 100%;
    min-height: 32px;
    padding: 0 4px 0 0;
    border: 1px solid transparent;
    border-radius: 5px;
    color: var(--text);
    font-size: 12px;
    text-align: left;
    background: transparent;
    cursor: grab;
    user-select: none;
    touch-action: none;
    transition: background 80ms ease, border-color 80ms ease;
  }

  .tree-row:not(.selected):hover {
    background: var(--surface-4);
    border-color: var(--border-3);
  }

  .tree-row.source-local {
    --layer-origin: var(--source-origin-local);
    --layer-origin-soft: var(--source-origin-local-soft);
  }

  .tree-row.source-current {
    --layer-origin: var(--source-origin-current);
    --layer-origin-soft: var(--source-origin-current-soft);
  }

  .tree-row.source-theme {
    --layer-origin: var(--source-origin-theme);
    --layer-origin-soft: var(--source-origin-theme-soft);
  }

  .tree-row.source-unknown {
    --layer-origin: var(--source-origin-unknown);
    --layer-origin-soft: var(--source-origin-unknown-soft);
  }

  .tree-row.source-current:not(.selected):hover,
  .tree-row.source-local:not(.selected):hover,
  .tree-row.source-theme:not(.selected):hover,
  .tree-row.source-unknown:not(.selected):hover {
    border-color: color-mix(in srgb, var(--layer-origin) 54%, var(--border-3));
    background: color-mix(in srgb, var(--layer-origin) 8%, var(--surface-4));
  }

  .tree-row.selected {
    border-color: var(--layer-origin);
    background: var(--layer-origin-soft);
    color: var(--text-strong);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--layer-origin) 18%, transparent);
  }

  .tree-row.tera-selected {
    --layer-origin-soft: color-mix(in srgb, var(--layer-origin) 13%, var(--surface-4));
  }

  .tree-row.tera-node {
    min-height: 32px;
    cursor: pointer;
  }

  .tree-row.tera-node:not(.selected) {
    color: color-mix(in srgb, var(--layer-origin) 78%, var(--text));
    background: color-mix(in srgb, var(--layer-origin) 5%, transparent);
  }

  .tree-row.tera-node:not(.selected):hover {
    border-color: color-mix(in srgb, var(--layer-origin) 44%, var(--border-3));
    background: color-mix(in srgb, var(--layer-origin) 9%, var(--surface-4));
  }

  .tree-row.dragging {
    opacity: 0.55;
    cursor: grabbing;
  }

  :global(body.layers-dragging),
  :global(body.layers-dragging *) {
    cursor: grabbing !important;
  }

  .tree-row.drop-inside {
    border-color: var(--brand);
    background: color-mix(in srgb, var(--brand) 18%, var(--surface-4));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--brand) 60%, transparent);
  }

  .tree-row.drop-before,
  .tree-row.drop-after {
    overflow: visible;
  }

  .tree-row.drop-before::before,
  .tree-row.drop-after::before {
    content: "";
    position: absolute;
    z-index: 3;
    left: calc(22px + (var(--depth) * 14px));
    right: 6px;
    height: 2px;
    border-radius: 999px;
    background: var(--brand);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--brand) 18%, transparent);
  }

  .tree-row.drop-before::after,
  .tree-row.drop-after::after {
    content: "";
    position: absolute;
    z-index: 4;
    left: calc(17px + (var(--depth) * 14px));
    width: 8px;
    height: 8px;
    border: 2px solid var(--brand);
    border-radius: 50%;
    background: var(--surface);
  }

  .tree-row.drop-before::before {
    top: -2px;
  }

  .tree-row.drop-before::after {
    top: -5px;
  }

  .tree-row.drop-after::before {
    bottom: -2px;
  }

  .tree-row.drop-after::after {
    bottom: -5px;
  }

  .tree-row.drop-invalid {
    border-color: color-mix(in srgb, #ef4444 55%, var(--border-3));
    background: color-mix(in srgb, #ef4444 10%, var(--surface-4));
  }

  .tree-gutter {
    display: block;
    flex: 0 0 auto;
  }

  .tree-toggle {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: 0 0 32px;
    height: 32px;
    color: var(--text-muted);
  }

  .toggle-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border-radius: 3px;
    color: var(--text-muted);
    cursor: pointer;
  }

  .toggle-btn:hover {
    color: var(--text);
    background: var(--surface-3);
  }

  .toggle-dot {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    margin: auto;
    background: var(--border-4);
  }

  .toggle-dot.tera-dot {
    width: 6px;
    height: 6px;
    background: color-mix(in srgb, var(--layer-origin) 66%, var(--border-4));
  }

  .tree-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: 0 0 16px;
    color: var(--text-muted);
  }

  .tree-row.selected .tree-icon {
    color: var(--layer-origin);
  }

  .tree-icon.tera-icon {
    color: var(--layer-origin);
  }

  .tree-label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    font-weight: 500;
  }

  .tree-kicker {
    margin-right: 5px;
    color: color-mix(in srgb, var(--layer-origin) 78%, var(--text-muted));
    font-size: 12px;
    font-weight: 900;
    text-transform: uppercase;
  }

  .tree-drop-label {
    flex: 0 0 auto;
    max-width: 58px;
    padding: 1px 5px;
    border-radius: 999px;
    color: var(--brand-strong);
    font-size: 12px;
    font-weight: 800;
    line-height: 1.3;
    text-transform: uppercase;
    background: color-mix(in srgb, var(--brand) 16%, var(--surface-3));
  }

  .tree-drop-label.invalid {
    color: #dc2626;
    background: color-mix(in srgb, #ef4444 12%, var(--surface-3));
  }

  .tree-actions {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    flex: 0 0 auto;
  }

  .tree-action-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border-radius: 5px;
    color: color-mix(in srgb, var(--layer-origin) 82%, var(--text-muted));
    cursor: pointer;
    transition: background 80ms ease, color 80ms ease;
  }

  .tree-action-btn:hover {
    color: color-mix(in srgb, var(--layer-origin) 88%, var(--text-strong));
    background: color-mix(in srgb, var(--layer-origin) 13%, var(--surface-3));
  }

  .tree-action-btn.danger {
    color: color-mix(in srgb, #cf4a4a 82%, var(--text-muted));
  }

  .tree-action-btn.danger:hover {
    color: #cf4a4a;
    background: color-mix(in srgb, #cf4a4a 14%, var(--surface-3));
  }

  .tree-delete-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: 0 0 32px;
    width: 32px;
    height: 32px;
    border-radius: 5px;
    color: color-mix(in srgb, #cf4a4a 82%, var(--text-muted));
    opacity: 0;
    cursor: pointer;
    transition: opacity 80ms ease, background 80ms ease, color 80ms ease;
  }

  .tree-row:hover .tree-delete-btn,
  .tree-row.selected .tree-delete-btn {
    opacity: 1;
  }

  .tree-delete-btn:hover:not(.disabled) {
    color: #cf4a4a;
    background: color-mix(in srgb, #cf4a4a 14%, var(--surface-3));
  }

  .tree-delete-btn.disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  .tree-tag {
    flex: 0 0 auto;
    padding: 0 4px;
    border: 1px solid var(--border-3);
    border-radius: 4px;
    color: var(--text-muted);
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
    font-weight: 700;
    background: var(--surface-3);
  }

  .tree-source-badge {
    flex: 0 0 auto;
    padding: 0 4px;
    border: 1px solid color-mix(in srgb, var(--layer-origin) 34%, var(--border-3));
    border-radius: 4px;
    color: color-mix(in srgb, var(--layer-origin) 88%, var(--text-strong));
    font-size: 12px;
    font-weight: 900;
    line-height: 1.45;
    text-transform: uppercase;
    background: color-mix(in srgb, var(--layer-origin) 10%, var(--surface-3));
  }

  .tree-row.selected .tree-tag {
    border-color: color-mix(in srgb, var(--layer-origin) 42%, var(--border-3));
    color: color-mix(in srgb, var(--layer-origin) 82%, var(--text-strong));
    background: color-mix(in srgb, var(--layer-origin) 14%, var(--surface-3));
  }

  .drag-hint {
    position: fixed;
    z-index: 10000;
    max-width: min(240px, calc(100vw - 28px));
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

  .drag-hint.invalid {
    border-color: color-mix(in srgb, #ef4444 52%, var(--border-3));
    color: #dc2626;
    background: color-mix(in srgb, var(--surface) 92%, #fee2e2);
  }
</style>
