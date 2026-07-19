<script lang="ts">
  import { onDestroy } from "svelte";
  import MoodBoardColorItemView from "$lib/components/canvas/MoodBoardColorItemView.svelte";
  import MoodBoardFrameItemView from "$lib/components/canvas/MoodBoardFrameItemView.svelte";
  import MoodBoardGroupItemView from "$lib/components/canvas/MoodBoardGroupItemView.svelte";
  import MoodBoardImageItemView from "$lib/components/canvas/MoodBoardImageItemView.svelte";
  import MoodBoardNoteItemView from "$lib/components/canvas/MoodBoardNoteItemView.svelte";
  import MoodBoardReferenceItemView from "$lib/components/canvas/MoodBoardReferenceItemView.svelte";
  import MoodBoardShapeItemView from "$lib/components/canvas/MoodBoardShapeItemView.svelte";
  import MoodBoardTextItemView from "$lib/components/canvas/MoodBoardTextItemView.svelte";
  import MoodBoardVectorGroupItemView from "$lib/components/canvas/MoodBoardVectorGroupItemView.svelte";
  import MoodBoardVectorPathItemView from "$lib/components/canvas/MoodBoardVectorPathItemView.svelte";
  import {
    type MoodBoardItem as MoodBoardItemData,
    type MoodBoardResizeHandle,
    type MoodBoardVectorHandleMode,
    type MoodBoardVectorNodeEditState,
    type MoodBoardVectorNode,
    type MoodBoardVectorTransform,
  } from "$lib/mood-board/model";
  import {
    cloneMoodBoardVectorNodes,
    moodBoardVectorGroupActiveTransformBounds,
    moodBoardVectorGroupDragData,
    moodBoardVectorGroupElementPointFromClient,
    moodBoardVectorGroupPointFromClient,
    moodBoardVectorGroupWithElementNodes,
    moodBoardVectorNodeKeyboardAction,
    moodBoardVectorPathPointFromClient,
    moodBoardVectorPathWithNodes,
    type MoodBoardVectorNodeDragState,
    type MoodBoardVectorNodeUpdater,
  } from "$lib/mood-board/item-vector-edit";
  import {
    cloneMoodBoardItem,
    moodBoardItemStyle,
    moodBoardSelectedElementTransformedBounds,
    moodBoardSvgTextEditWidth,
    moodBoardVectorGroupHandleOffset,
    moodBoardVectorNodeHandleMode,
  } from "$lib/mood-board/item-view";
  import { editableSvgElementNodes } from "$lib/mood-board/svg";
  import {
    angleBetweenPoints,
    distanceBetweenPoints,
    insertVectorNodeAtPoint,
    moveVectorNodeHandle,
    vectorBoundsCenter,
    type MoodBoardBounds,
    type MoodBoardPoint,
  } from "$lib/mood-board/vector";
  import {
    closestMoodBoardVectorNodeIndex,
    moodBoardSelectedVectorNodeSet,
    moodBoardVectorGroupNodeScope,
    moodBoardVectorHandleVisible,
    moodBoardVectorNodeSelectionState,
    moodBoardVectorPathNodeScope,
    nextMoodBoardVectorNodeSelection,
  } from "$lib/mood-board/vector-selection";
  import {
    applyMoodBoardVectorGroupElementDrag,
    updateMoodBoardVectorGroupElementDrag,
    type MoodBoardVectorGroupElementDrag,
  } from "$lib/mood-board/vector-group-drag";
  import type { MoodBoardIdentityGuard, MoodBoardRequestIdentity } from "$lib/mood-board/io";

  export let item: MoodBoardItemData;
  export let selected = false;
  export let showVectorNodes = false;
  export let attachTarget = false;
  export let parentFrameTitle = "";
  export let childItems: MoodBoardItemData[] = [];
  export let isItemSelected: (itemId: string) => boolean = () => false;
  export let onPointerDown: (event: PointerEvent, itemId: string) => void;
  export let onResizePointerDown: (event: PointerEvent, itemId: string, handle: MoodBoardResizeHandle) => void;
  export let selectedSvgElementId: string | null = null;
  export let vectorEditTargetItemId: string | null = null;
  export let vectorEditTargetSvgElementId: string | null = null;
  export let vectorNodeEditState: MoodBoardVectorNodeEditState | null = null;
  export let enterVectorEditMode: (itemId: string, svgElementId?: string | null) => void = () => undefined;
  export let onSvgElementSelect: (itemId: string, elementId: string | null) => void = () => undefined;
  export let onVectorNodeSelectionChange: (state: MoodBoardVectorNodeEditState | null) => void = () => undefined;
  export let previewItem: (item: MoodBoardItemData) => void;
  export let commitItemEdit: (beforeItem: MoodBoardItemData, nextItem: MoodBoardItemData) => void;
  export let sessionIdentity: MoodBoardRequestIdentity;
  export let isSessionCurrent: MoodBoardIdentityGuard;

  let itemEl: HTMLElement | null = null;
  let editBeforeItem: MoodBoardItemData | null = null;
  let selectedVectorNodeIndexes: number[] = [];
  let vectorNodeSelectionScope = "";
  let vectorNodeSelectionToken = "";
  let skipNextVectorPathDoubleClick = false;
  let vectorDrag: MoodBoardVectorNodeDragState | null = null;
  let vectorGroupElementDrag: MoodBoardVectorGroupElementDrag | null = null;
  const resizeHandles: MoodBoardResizeHandle[] = ["nw", "ne", "sw", "se"];

  onDestroy(() => {
    window.removeEventListener("pointermove", handleVectorDragMove);
    window.removeEventListener("pointerup", endVectorDrag);
    window.removeEventListener("pointercancel", endVectorDrag);
    window.removeEventListener("pointermove", handleVectorGroupElementDragMove);
    window.removeEventListener("pointerup", endVectorGroupElementDrag);
    window.removeEventListener("pointercancel", endVectorGroupElementDrag);
    vectorDrag = null;
    vectorGroupElementDrag = null;
    editBeforeItem = null;
  });

  $: if (!selected) {
    selectedVectorNodeIndexes = [];
    vectorNodeSelectionScope = "";
  }

  $: vectorNodeSelectionToken = [
    vectorNodeSelectionScope,
    selectedVectorNodeIndexes.join(","),
    vectorNodeEditState?.itemId ?? "",
    vectorNodeEditState?.svgElementId ?? "",
    vectorNodeEditState?.indexes.join(",") ?? "",
  ].join("|");

  function cloneItem(value: MoodBoardItemData): MoodBoardItemData {
    return cloneMoodBoardItem(value);
  }

  function beginEdit() {
    editBeforeItem ??= cloneItem(item);
  }

  function previewEdit(nextItem: MoodBoardItemData) {
    beginEdit();
    previewItem(nextItem);
  }

  function commitEdit(nextItem: MoodBoardItemData) {
    if (!editBeforeItem) return;
    const before = editBeforeItem;
    editBeforeItem = null;
    commitItemEdit(before, nextItem);
  }

  function cancelEdit() {
    if (!editBeforeItem) return;
    const before = editBeforeItem;
    editBeforeItem = null;
    previewItem(before);
  }

  function vectorPathNodeScope() {
    return moodBoardVectorPathNodeScope(item.id);
  }

  function vectorGroupNodeScope(elementId: string) {
    return moodBoardVectorGroupNodeScope(item.id, elementId);
  }

  function nodeHandleMode(node: MoodBoardVectorNode): MoodBoardVectorHandleMode {
    return moodBoardVectorNodeHandleMode(node);
  }

  function publishVectorNodeSelection() {
    onVectorNodeSelectionChange(
      moodBoardVectorNodeSelectionState(item, vectorNodeSelectionScope, selectedVectorNodeIndexes),
    );
  }

  function setVectorNodeSelection(scope: string, nodeIndex: number, additive: boolean) {
    const next = nextMoodBoardVectorNodeSelection(
      vectorNodeSelectionScope,
      selectedVectorNodeIndexes,
      scope,
      nodeIndex,
      additive,
    );
    vectorNodeSelectionScope = next.scope;
    selectedVectorNodeIndexes = next.indexes;
    publishVectorNodeSelection();
  }

  function handleVectorNodeClick(event: MouseEvent, scope: string, nodeIndex: number) {
    event.preventDefault();
    event.stopPropagation();
    focusItem();
    setVectorNodeSelection(scope, nodeIndex, event.shiftKey);
  }

  function handleVectorPathSecondPointerDown(event: PointerEvent) {
    if (item.type !== "vectorPath" || event.detail < 2) return false;
    if (!vectorPathEditActive()) skipNextVectorPathDoubleClick = true;
    event.preventDefault();
    event.stopPropagation();
    focusItem();
    enterVectorEditMode(item.id, null);
    return true;
  }

  function handleVectorNodeKeydown(event: KeyboardEvent, scope: string, nodeIndex: number) {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    event.stopPropagation();
    focusItem();
    setVectorNodeSelection(scope, nodeIndex, event.shiftKey);
  }

  function isVectorNodeSelected(scope: string, nodeIndex: number) {
    return selectedVectorNodeSet(scope).has(nodeIndex);
  }

  function selectedVectorNodeSet(scope: string) {
    return moodBoardSelectedVectorNodeSet({
      itemId: item.id,
      scope,
      currentScope: vectorNodeSelectionScope,
      currentIndexes: selectedVectorNodeIndexes,
      editState: vectorNodeEditState,
    });
  }

  function vectorHandleVisible(scope: string, nodes: MoodBoardVectorNode[], index: number, handle: "in" | "out", closed: boolean) {
    return moodBoardVectorHandleVisible(selectedVectorNodeSet(scope), nodes, index, handle, closed);
  }

  function focusItem() {
    itemEl?.focus({ preventScroll: true });
  }

  function closestNodeIndex(nodes: MoodBoardVectorNode[], point: { x: number; y: number }) {
    return closestMoodBoardVectorNodeIndex(nodes, point);
  }

  function pointerToVectorPoint(event: PointerEvent) {
    const svg = itemEl?.querySelector(".vector-path-preview");
    const rect = svg?.getBoundingClientRect();
    if (!rect || item.type !== "vectorPath") return { x: 0, y: 0 };
    return moodBoardVectorPathPointFromClient(item, event.clientX, event.clientY, rect);
  }

  function pointerToVectorGroupPoint(event: PointerEvent) {
    const svg = itemEl?.querySelector(".svg-group-preview");
    const rect = svg?.getBoundingClientRect();
    if (!rect || item.type !== "vectorGroup") return { x: 0, y: 0 };
    return moodBoardVectorGroupPointFromClient(item, event.clientX, event.clientY, rect);
  }

  function pointerToVectorGroupElementPoint(event: PointerEvent, transform: MoodBoardVectorTransform) {
    const svg = itemEl?.querySelector(".svg-group-preview");
    const rect = svg?.getBoundingClientRect();
    if (!rect || item.type !== "vectorGroup") return { x: 0, y: 0 };
    return moodBoardVectorGroupElementPointFromClient(item, event.clientX, event.clientY, rect, transform);
  }

  function vectorGroupHandleOffset() {
    if (item.type !== "vectorGroup") return 28;
    return moodBoardVectorGroupHandleOffset(item);
  }

  function svgTextEditWidth(text: string, fontSize: number) {
    return moodBoardSvgTextEditWidth(text, fontSize);
  }

  function selectedElementTransformedBounds(nodes: MoodBoardVectorNode[], transform: MoodBoardVectorTransform) {
    return moodBoardSelectedElementTransformedBounds(nodes, transform);
  }

  function vectorPathEditActive() {
    return item.type === "vectorPath" && vectorEditTargetItemId === item.id;
  }

  function vectorGroupElementEditActive(elementId: string) {
    return selected
      && item.type === "vectorGroup"
      && vectorEditTargetItemId === item.id
      && vectorEditTargetSvgElementId === elementId;
  }

  function activeSvgTransformBounds(elementId: string) {
    if (item.type !== "vectorGroup") return null;
    return moodBoardVectorGroupActiveTransformBounds(item, elementId);
  }

  function activeSvgGroupDragData(elementId: string) {
    if (item.type !== "vectorGroup") return { groupPath: null, startGroupTransforms: undefined };
    return moodBoardVectorGroupDragData(item, elementId);
  }

  function beginVectorDrag(event: PointerEvent, nodeIndex: number, handle: "node" | "in" | "out") {
    if (item.type !== "vectorPath") return;
    event.preventDefault();
    event.stopPropagation();
    focusItem();
    setVectorNodeSelection(vectorPathNodeScope(), nodeIndex, event.shiftKey && handle === "node");
    const point = pointerToVectorPoint(event);
    vectorDrag = {
      kind: "vectorPath",
      nodeIndex,
      handle,
      startX: point.x,
      startY: point.y,
      startNodes: cloneMoodBoardVectorNodes(item.nodes),
      currentNodes: cloneMoodBoardVectorNodes(item.nodes),
    };
    beginEdit();
    window.addEventListener("pointermove", handleVectorDragMove);
    window.addEventListener("pointerup", endVectorDrag, { once: true });
    window.addEventListener("pointercancel", endVectorDrag, { once: true });
  }

  function beginVectorGroupDrag(event: PointerEvent, elementId: string, nodeIndex: number, handle: "node" | "in" | "out") {
    if (item.type !== "vectorGroup") return;
    const element = item.elements.find((entry) => entry.id === elementId);
    const parsed = element ? editableSvgElementNodes(element) : null;
    if (!element || !parsed) return;
    event.preventDefault();
    event.stopPropagation();
    focusItem();
    onSvgElementSelect(item.id, elementId);
    setVectorNodeSelection(vectorGroupNodeScope(elementId), nodeIndex, event.shiftKey && handle === "node");
    const point = pointerToVectorGroupElementPoint(event, element.transform);
    vectorDrag = {
      kind: "vectorGroup",
      elementId,
      closed: parsed.closed,
      nodeIndex,
      handle,
      startX: point.x,
      startY: point.y,
      startNodes: cloneMoodBoardVectorNodes(parsed.nodes),
      currentNodes: cloneMoodBoardVectorNodes(parsed.nodes),
      elementTransform: element.transform,
    };
    beginEdit();
    window.addEventListener("pointermove", handleVectorDragMove);
    window.addEventListener("pointerup", endVectorDrag, { once: true });
    window.addEventListener("pointercancel", endVectorDrag, { once: true });
  }

  function handleVectorDragMove(event: PointerEvent) {
    if (!vectorDrag) return;
    event.preventDefault();
    const point = vectorDrag.kind === "vectorPath"
      ? pointerToVectorPoint(event)
      : pointerToVectorGroupElementPoint(event, vectorDrag.elementTransform ?? [1, 0, 0, 1, 0, 0]);
    const dx = point.x - vectorDrag.startX;
    const dy = point.y - vectorDrag.startY;
    const nodes = moveVectorNodeHandle(vectorDrag.startNodes, vectorDrag.nodeIndex, vectorDrag.handle, dx, dy);

    if (vectorDrag.kind === "vectorPath" && item.type === "vectorPath") {
      previewEdit(moodBoardVectorPathWithNodes(item, nodes));
    } else if (vectorDrag.kind === "vectorGroup" && item.type === "vectorGroup" && vectorDrag.elementId) {
      previewEdit(moodBoardVectorGroupWithElementNodes(item, vectorDrag.elementId, nodes, Boolean(vectorDrag.closed)));
    }
    vectorDrag.currentNodes = nodes;
  }

  function endVectorDrag() {
    window.removeEventListener("pointermove", handleVectorDragMove);
    window.removeEventListener("pointerup", endVectorDrag);
    window.removeEventListener("pointercancel", endVectorDrag);
    const nodes = vectorDrag?.currentNodes;
    const elementId = vectorDrag?.elementId;
    const closed = Boolean(vectorDrag?.closed);
    const kind = vectorDrag?.kind;
    vectorDrag = null;
    if (item.type === "vectorPath" && kind === "vectorPath" && nodes) {
      commitEdit(moodBoardVectorPathWithNodes(item, nodes));
    } else if (item.type === "vectorGroup" && kind === "vectorGroup" && elementId && nodes) {
      commitEdit(moodBoardVectorGroupWithElementNodes(item, elementId, nodes, closed));
    }
  }

  function beginVectorGroupElementDrag(event: PointerEvent, elementId: string) {
    if (item.type !== "vectorGroup") return;
    const element = item.elements.find((entry) => entry.id === elementId);
    if (!element) return;
    event.stopPropagation();
    onSvgElementSelect(item.id, elementId);

    if (!selected) {
      onPointerDown(event, item.id);
      return;
    }

    if (selectedSvgElementId !== elementId) {
      event.preventDefault();
      return;
    }

    event.preventDefault();
    const point = pointerToVectorGroupPoint(event);
    const { groupPath, startGroupTransforms } = activeSvgGroupDragData(elementId);
    vectorGroupElementDrag = {
      mode: "move",
      elementId,
      groupPath,
      startX: point.x,
      startY: point.y,
      startTransform: [...element.transform] as MoodBoardVectorTransform,
      startGroupTransforms,
      currentTransform: [...element.transform] as MoodBoardVectorTransform,
    };
    beginEdit();
    window.addEventListener("pointermove", handleVectorGroupElementDragMove);
    window.addEventListener("pointerup", endVectorGroupElementDrag, { once: true });
    window.addEventListener("pointercancel", endVectorGroupElementDrag, { once: true });
  }

  function beginVectorGroupElementTransformDrag(
    event: PointerEvent,
    elementId: string,
    mode: "scale" | "rotate",
    bounds: MoodBoardBounds,
  ) {
    if (item.type !== "vectorGroup") return;
    const element = item.elements.find((entry) => entry.id === elementId);
    if (!element) return;
    event.preventDefault();
    event.stopPropagation();
    onSvgElementSelect(item.id, elementId);
    const point = pointerToVectorGroupPoint(event);
    const center = vectorBoundsCenter(bounds);
    const startDistance = Math.max(0.001, distanceBetweenPoints(center, point));
    const { groupPath, startGroupTransforms } = activeSvgGroupDragData(elementId);
    vectorGroupElementDrag = {
      mode,
      elementId,
      groupPath,
      startX: point.x,
      startY: point.y,
      startTransform: [...element.transform] as MoodBoardVectorTransform,
      startGroupTransforms,
      currentTransform: [...element.transform] as MoodBoardVectorTransform,
      center,
      startDistance,
      startAngle: angleBetweenPoints(center, point),
    };
    beginEdit();
    window.addEventListener("pointermove", handleVectorGroupElementDragMove);
    window.addEventListener("pointerup", endVectorGroupElementDrag, { once: true });
    window.addEventListener("pointercancel", endVectorGroupElementDrag, { once: true });
  }

  function handleVectorGroupElementDragMove(event: PointerEvent) {
    if (!vectorGroupElementDrag || item.type !== "vectorGroup") return;
    event.preventDefault();
    const point = pointerToVectorGroupPoint(event);
    vectorGroupElementDrag = updateMoodBoardVectorGroupElementDrag(vectorGroupElementDrag, point);
    previewEdit({
      ...item,
      elements: applyMoodBoardVectorGroupElementDrag(item.elements, vectorGroupElementDrag),
    });
  }

  function endVectorGroupElementDrag() {
    window.removeEventListener("pointermove", handleVectorGroupElementDragMove);
    window.removeEventListener("pointerup", endVectorGroupElementDrag);
    window.removeEventListener("pointercancel", endVectorGroupElementDrag);
    const drag = vectorGroupElementDrag;
    vectorGroupElementDrag = null;
    if (!drag || item.type !== "vectorGroup") return;
    commitEdit({
      ...item,
      elements: applyMoodBoardVectorGroupElementDrag(item.elements, drag),
    });
  }

  function selectSvgElement(event: PointerEvent, elementId: string | null) {
    if (item.type !== "vectorGroup") return;
    event.stopPropagation();
    onSvgElementSelect(item.id, elementId);
    onPointerDown(event, item.id);
  }

  function insertVectorPathNodeAtPointer(event: MouseEvent) {
    if (item.type !== "vectorPath") return;
    event.preventDefault();
    event.stopPropagation();
    focusItem();
    const point = pointerToVectorPoint(event as unknown as PointerEvent);
    const nodes = insertVectorNodeAtPoint(item.nodes, item.closed, point);
    vectorNodeSelectionScope = vectorPathNodeScope();
    selectedVectorNodeIndexes = [closestNodeIndex(nodes, point)];
    commitItemEdit(cloneItem(item), moodBoardVectorPathWithNodes(item, nodes));
    publishVectorNodeSelection();
  }

  function handleVectorPathDoubleClick(event: MouseEvent) {
    if (item.type !== "vectorPath") return;
    if (skipNextVectorPathDoubleClick) {
      skipNextVectorPathDoubleClick = false;
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    if (vectorPathEditActive()) {
      insertVectorPathNodeAtPointer(event);
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    focusItem();
    enterVectorEditMode(item.id, null);
  }

  function insertVectorGroupNodeAtPointer(event: MouseEvent, elementId: string) {
    if (item.type !== "vectorGroup") return;
    const element = item.elements.find((entry) => entry.id === elementId);
    const parsed = element?.type === "path" ? editableSvgElementNodes(element) : null;
    if (!element || element.type !== "path" || !parsed) return;
    event.preventDefault();
    event.stopPropagation();
    focusItem();
    onSvgElementSelect(item.id, elementId);
    const point = pointerToVectorGroupElementPoint(event as unknown as PointerEvent, element.transform);
    const nodes = insertVectorNodeAtPoint(parsed.nodes, parsed.closed, point);
    vectorNodeSelectionScope = vectorGroupNodeScope(elementId);
    selectedVectorNodeIndexes = [closestNodeIndex(nodes, point)];
    commitItemEdit(cloneItem(item), moodBoardVectorGroupWithElementNodes(item, elementId, nodes, parsed.closed));
    publishVectorNodeSelection();
  }

  function handleVectorGroupPathDoubleClick(event: MouseEvent, elementId: string) {
    if (item.type !== "vectorGroup") return;
    if (vectorGroupElementEditActive(elementId)) {
      insertVectorGroupNodeAtPointer(event, elementId);
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    focusItem();
    onSvgElementSelect(item.id, elementId);
    enterVectorEditMode(item.id, elementId);
  }

  function updateSelectedVectorPathNodes(updater: MoodBoardVectorNodeUpdater) {
    if (item.type !== "vectorPath") return false;
    const selectedNodes = selectedVectorNodeSet(vectorPathNodeScope());
    if (!selectedNodes.size) return false;
    const nodes = updater(item.nodes, selectedNodes, item.closed);
    if (nodes === item.nodes) return false;
    selectedVectorNodeIndexes = selectedVectorNodeIndexes.filter((index) => index < nodes.length);
    commitItemEdit(cloneItem(item), moodBoardVectorPathWithNodes(item, nodes));
    publishVectorNodeSelection();
    return true;
  }

  function updateSelectedVectorGroupNodes(updater: MoodBoardVectorNodeUpdater) {
    if (item.type !== "vectorGroup" || !selectedSvgElementId) return false;
    const selectedNodes = selectedVectorNodeSet(vectorGroupNodeScope(selectedSvgElementId));
    if (!selectedNodes.size) return false;
    const element = item.elements.find((entry) => entry.id === selectedSvgElementId);
    const parsed = element?.type === "path" ? editableSvgElementNodes(element) : null;
    if (!element || element.type !== "path" || !parsed) return false;
    const nodes = updater(parsed.nodes, selectedNodes, parsed.closed);
    if (nodes === parsed.nodes) return false;
    selectedVectorNodeIndexes = selectedVectorNodeIndexes.filter((index) => index < nodes.length);
    commitItemEdit(cloneItem(item), moodBoardVectorGroupWithElementNodes(item, element.id, nodes, parsed.closed));
    publishVectorNodeSelection();
    return true;
  }

  function handleItemKeydown(event: KeyboardEvent) {
    const target = event.target as HTMLElement | null;
    if (target?.tagName === "INPUT" || target?.tagName === "TEXTAREA") return;
    if (!selectedVectorNodeIndexes.length) return;
    const action = moodBoardVectorNodeKeyboardAction(event);
    if (!action) return;

    let handled = false;
    if (action.type === "clearSelection") {
      selectedVectorNodeIndexes = [];
      vectorNodeSelectionScope = "";
      publishVectorNodeSelection();
      handled = true;
    } else {
      handled = updateSelectedVectorPathNodes(action.updater)
        || updateSelectedVectorGroupNodes(action.updater);
    }

    if (!handled) return;
    event.preventDefault();
    event.stopPropagation();
  }

</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<article
  role="group"
  aria-label={`Element canvas ${item.type}`}
  bind:this={itemEl}
  class="mood-item"
  class:selected
  class:attach-target={attachTarget}
  class:note-item={item.type === "note"}
  class:text-item={item.type === "text"}
  class:color-item={item.type === "color"}
  class:reference-item={item.type === "reference"}
  class:image-item={item.type === "image"}
  class:frame-item={item.type === "frame"}
  class:group-item={item.type === "group"}
  class:shape-item={item.type === "shape"}
  class:vector-path-item={item.type === "vectorPath"}
  class:svg-group-item={item.type === "vectorGroup"}
  class:inside-frame={Boolean(parentFrameTitle)}
  tabindex={selected ? 0 : -1}
  style={moodBoardItemStyle(item)}
  onkeydown={handleItemKeydown}
  onpointerdown={(event) => {
    event.stopPropagation();
    if (handleVectorPathSecondPointerDown(event)) return;
    onPointerDown(event, item.id);
  }}
>
  {#if item.type === "note"}
    <MoodBoardNoteItemView {item} {beginEdit} {previewEdit} {commitEdit} {cancelEdit} />
  {:else if item.type === "text"}
    <MoodBoardTextItemView {item} {beginEdit} {previewEdit} {commitEdit} {cancelEdit} />
  {:else if item.type === "frame"}
    <MoodBoardFrameItemView {item} hasChildren={childItems.length > 0} {beginEdit} {previewEdit} {commitEdit} {cancelEdit}>
      {#each childItems as child (child.id)}
        <svelte:self
          item={child}
          childItems={child.type === "frame" || child.type === "group" ? child.children : []}
          selected={isItemSelected(child.id)}
          showVectorNodes={child.type === "vectorPath" && vectorEditTargetItemId === child.id}
          attachTarget={false}
          parentFrameTitle={item.title}
          {isItemSelected}
          {onPointerDown}
          {onResizePointerDown}
          selectedSvgElementId={isItemSelected(child.id) ? selectedSvgElementId : null}
          {vectorEditTargetItemId}
          {vectorEditTargetSvgElementId}
          {vectorNodeEditState}
          {enterVectorEditMode}
          {onSvgElementSelect}
          {onVectorNodeSelectionChange}
          {previewItem}
          {commitItemEdit}
          {sessionIdentity}
          {isSessionCurrent}
        />
      {/each}
    </MoodBoardFrameItemView>
  {:else if item.type === "group"}
    <MoodBoardGroupItemView {item} hasChildren={childItems.length > 0} {beginEdit} {previewEdit} {commitEdit} {cancelEdit}>
      {#each childItems as child (child.id)}
        <svelte:self
          item={child}
          childItems={child.type === "frame" || child.type === "group" ? child.children : []}
          selected={isItemSelected(child.id)}
          showVectorNodes={child.type === "vectorPath" && vectorEditTargetItemId === child.id}
          attachTarget={false}
          parentFrameTitle={parentFrameTitle}
          {isItemSelected}
          {onPointerDown}
          {onResizePointerDown}
          selectedSvgElementId={isItemSelected(child.id) ? selectedSvgElementId : null}
          {vectorEditTargetItemId}
          {vectorEditTargetSvgElementId}
          {vectorNodeEditState}
          {enterVectorEditMode}
          {onSvgElementSelect}
          {onVectorNodeSelectionChange}
          {previewItem}
          {commitItemEdit}
          {sessionIdentity}
          {isSessionCurrent}
        />
      {/each}
    </MoodBoardGroupItemView>
  {:else if item.type === "shape"}
    <MoodBoardShapeItemView {item} />
  {:else if item.type === "vectorPath"}
    <MoodBoardVectorPathItemView
      {item}
      {selected}
      {showVectorNodes}
      {vectorNodeSelectionToken}
      editActive={vectorPathEditActive()}
      isNodeSelected={(index) => isVectorNodeSelected(vectorPathNodeScope(), index)}
      isHandleVisible={(index, handle) => vectorHandleVisible(vectorPathNodeScope(), item.nodes, index, handle, item.closed)}
      onPathDoubleClick={handleVectorPathDoubleClick}
      onNodePointerDown={beginVectorDrag}
      onNodeClick={(event, index) => handleVectorNodeClick(event, vectorPathNodeScope(), index)}
      onNodeKeydown={(event, index) => handleVectorNodeKeydown(event, vectorPathNodeScope(), index)}
    />
  {:else if item.type === "vectorGroup"}
    <MoodBoardVectorGroupItemView
      {item}
      {selected}
      {selectedSvgElementId}
      {vectorNodeSelectionToken}
      {beginEdit}
      {previewEdit}
      {commitEdit}
      {cancelEdit}
      {selectSvgElement}
      {onSvgElementSelect}
      {vectorGroupElementEditActive}
      {selectedElementTransformedBounds}
      {activeSvgTransformBounds}
      {beginVectorGroupElementDrag}
      {svgTextEditWidth}
      {handleVectorGroupPathDoubleClick}
      {vectorGroupHandleOffset}
      {beginVectorGroupElementTransformDrag}
      {vectorGroupNodeScope}
      {vectorHandleVisible}
      {beginVectorGroupDrag}
      {handleVectorNodeClick}
      {isVectorNodeSelected}
      {nodeHandleMode}
    />
  {:else if item.type === "color"}
    <MoodBoardColorItemView {item} {beginEdit} {previewEdit} {commitEdit} {cancelEdit} />
  {:else if item.type === "reference"}
    <MoodBoardReferenceItemView {item} {beginEdit} {previewEdit} {commitEdit} {cancelEdit} />
  {:else if item.type === "image"}
    <MoodBoardImageItemView {item} {sessionIdentity} {isSessionCurrent} />
  {/if}

  {#each resizeHandles as handle}
    <button
      class={`resize-handle ${handle}`}
      data-resize-handle
      type="button"
      title="Redimensionează"
      aria-label={`Redimensionează item ${handle}`}
      onpointerdown={(event) => onResizePointerDown(event, item.id, handle)}
    ></button>
  {/each}
</article>

<style>
  .mood-item {
    position: absolute;
    left: 0;
    top: 0;
    display: grid;
    gap: 8px;
    padding: 10px;
    border: 1px solid var(--border-3);
    border-radius: 9px;
    background: color-mix(in srgb, var(--surface-4) 94%, transparent);
    box-shadow: 0 14px 32px rgba(0, 0, 0, 0.18);
    cursor: grab;
    user-select: none;
    will-change: transform;
  }

  .mood-item.selected {
    border-color: var(--brand);
    box-shadow: 0 0 0 2px var(--brand-soft), 0 14px 32px rgba(0, 0, 0, 0.18);
  }

  .frame-item {
    grid-template-rows: auto minmax(0, 1fr) auto;
    gap: 0;
    padding: 0;
    border-color: color-mix(in srgb, var(--frame-tone) 54%, var(--border-3));
    border-radius: 6px;
    background: var(--frame-bg);
    box-shadow: 0 18px 46px rgba(0, 0, 0, 0.16);
  }

  .frame-item.attach-target {
    border-color: var(--brand);
    box-shadow: 0 0 0 3px var(--brand-soft), 0 20px 50px rgba(29, 127, 106, 0.24);
  }

  .group-item {
    display: block;
    padding: 0;
    border-color: transparent;
    border-radius: 6px;
    background: transparent;
    box-shadow: none;
    overflow: visible;
  }

  .group-item.selected {
    border-color: color-mix(in srgb, var(--brand) 72%, transparent);
    border-style: dashed;
    background: color-mix(in srgb, var(--brand) 4%, transparent);
    box-shadow: 0 0 0 2px var(--brand-soft);
  }

  .mood-item:active {
    cursor: grabbing;
  }

  .mood-item:focus-within,
  .mood-item:has(.resize-handle:active) {
    cursor: default;
  }

  .resize-handle {
    position: absolute;
    z-index: 12;
    width: 11px;
    height: 11px;
    padding: 0;
    border: 2px solid var(--surface);
    border-radius: 999px;
    background: var(--brand);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--brand) 66%, transparent);
    opacity: 0;
  }

  .resize-handle.nw {
    left: -7px;
    top: -7px;
    cursor: nwse-resize;
  }

  .resize-handle.ne {
    right: -7px;
    top: -7px;
    cursor: nesw-resize;
  }

  .resize-handle.sw {
    left: -7px;
    bottom: -7px;
    cursor: nesw-resize;
  }

  .resize-handle.se {
    right: -7px;
    bottom: -7px;
    cursor: nwse-resize;
  }

  .mood-item:hover .resize-handle,
  .mood-item.selected .resize-handle,
  .resize-handle:focus-visible {
    opacity: 1;
  }

  .resize-handle:hover,
  .resize-handle:focus-visible {
    background: var(--brand-strong);
    outline: none;
  }

  .color-item {
    grid-template-rows: minmax(0, 1fr) auto auto;
  }

  .note-item {
    grid-template-rows: minmax(0, 1fr);
  }

  .text-item {
    display: block;
    padding: 0;
    border-color: transparent;
    background: transparent;
    box-shadow: none;
  }

  .text-item.selected {
    border-color: var(--brand);
    box-shadow: 0 0 0 2px var(--brand-soft);
  }

  .reference-item {
    grid-template-rows: auto auto minmax(0, 1fr);
  }

  .image-item {
    display: block;
    gap: 0;
    padding: 0;
    border-color: transparent;
    background: transparent;
    box-shadow: none;
    overflow: visible;
  }

  .image-item.selected {
    border-color: var(--brand);
    box-shadow: 0 0 0 2px var(--brand-soft);
  }

  .shape-item {
    display: block;
    padding: 0;
    border-color: transparent;
    background: transparent;
    box-shadow: none;
  }

  .shape-item.selected {
    border-color: var(--brand);
    box-shadow: 0 0 0 2px var(--brand-soft);
  }

  .vector-path-item {
    display: block;
    padding: 0;
    border-color: transparent;
    background: transparent;
    box-shadow: none;
    overflow: visible;
  }

  .vector-path-item.selected {
    border-color: var(--brand);
    box-shadow: 0 0 0 2px var(--brand-soft);
  }

  .svg-group-item {
    display: block;
    padding: 0;
    border-color: transparent;
    background: transparent;
    box-shadow: none;
    overflow: visible;
  }

  .svg-group-item.selected {
    border-color: var(--brand);
    box-shadow: 0 0 0 2px var(--brand-soft);
  }
</style>
