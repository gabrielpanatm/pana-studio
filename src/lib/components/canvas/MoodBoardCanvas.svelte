<script lang="ts">
  import { onMount } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import MoodBoardCanvasScene from "$lib/components/canvas/MoodBoardCanvasScene.svelte";
  import MoodBoardContextBar from "$lib/components/canvas/MoodBoardContextBar.svelte";
  import MoodBoardDock from "$lib/components/canvas/MoodBoardDock.svelte";
  import {
    addMoodBoardVisualAssetAtPath,
    applyMoodBoardPaletteColors,
    applyMoodBoardVisualAssetItem,
    exportMoodBoardCompositionWebpWorkflow,
    exportMoodBoardImageWebpWorkflow,
    exportMoodBoardVectorPathWorkflow,
    extractMoodBoardPaletteItems,
    pasteMoodBoardClipboardImageToDesign,
    saveMoodBoardClipboardEventImageToDesign,
  } from "$lib/mood-board/canvas-assets";
  import {
    clearMoodBoardDragOverlays,
    moodBoardItemDragSelection,
    moodBoardMarqueeSelectionStart,
  } from "$lib/mood-board/canvas-pointer";
  import { createMoodBoardBoardScheduler } from "$lib/mood-board/canvas-scheduler";
  import {
    moodBoardAcceptedVectorNodeEditState,
    moodBoardActiveVectorNodeEditState,
    moodBoardApplyVectorNodeHandleMode,
    moodBoardEnterVectorEditMode,
    moodBoardSelectedItems,
    moodBoardSelectionState,
    moodBoardSelectOnly,
    moodBoardSetSelection,
    moodBoardSetSvgElementSelection,
    moodBoardToggleSelection,
    moodBoardVisibleSelectedIds,
    type MoodBoardSelectionState,
    type MoodBoardVectorEditTarget,
  } from "$lib/mood-board/canvas-selection";
  import {
    createMoodBoardColor,
    createMoodBoardFrame,
    createMoodBoardNote,
    createMoodBoardReference,
    createMoodBoardShape,
    createMoodBoardText,
    createMoodBoardVectorPath as createDefaultMoodBoardVectorPath,
    type FramePreset,
  } from "$lib/mood-board/factory";
  import { detachChildFromFrameIfNeeded } from "$lib/mood-board/frame-actions";
  import {
    createMoodBoardItemDragState,
    createMoodBoardMarqueeDragState,
    createMoodBoardPanDragState,
    createMoodBoardResizeDragState,
    finalizeMoodBoardItemDragAttachment,
    isMoodBoardCanvasPanTarget,
    isMoodBoardSecondaryPointer,
    moodBoardMarqueeDragUpdate,
    moodBoardPointerDragUpdate,
    shouldIgnoreMoodBoardItemDrag,
    type MoodBoardDragState,
    type MoodBoardMarqueeBox,
  } from "$lib/mood-board/drag";
  import {
    isMoodBoardEditableTarget,
    moodBoardKeyboardIntent,
  } from "$lib/mood-board/keyboard";
  import { createMoodBoardHeavyAssetGate } from "$lib/mood-board/heavy-asset-gate";
  import type { MoodBoardAlignMode, MoodBoardDistributeMode } from "$lib/mood-board/layout";
  import {
    alignItems as alignMoodBoardItemsAction,
    bringItemsToFront as bringItemsToFrontAction,
    distributeItems as distributeMoodBoardItemsAction,
    duplicateItems as duplicateItemsAction,
    groupItems as groupMoodBoardItemsAction,
    nudgeItems as nudgeMoodBoardItems,
    sendItemsToBack as sendItemsToBackAction,
    ungroupGroup as ungroupMoodBoardGroupAction,
    type MoodBoardItemActionResult,
  } from "$lib/mood-board/item-actions";
  import {
    cloneMoodBoard,
    findMoodBoardItem,
    flattenMoodBoardItems,
    mapMoodBoardItems,
    removeMoodBoardItems,
    type MoodBoard,
    type MoodBoardItem as MoodBoardItemData,
    type MoodBoardResizeHandle,
    type MoodBoardSaveState,
    type MoodBoardTool,
    type MoodBoardVectorHandleMode,
    type MoodBoardVectorNode,
    type MoodBoardVectorNodeEditState,
  } from "$lib/mood-board/model";
  import {
    isMoodBoardStaleSessionError,
    requireCurrentMoodBoardIdentity,
    type MoodBoardIdentityGuard,
    type MoodBoardRequestIdentity,
  } from "$lib/mood-board/io";
  import type { MoodBoardSnapGuide } from "$lib/mood-board/snap";
  import { parentFrameTitle as moodBoardParentFrameTitle } from "$lib/mood-board/tree";
  import {
    applyVectorMaskToImage as applyVectorMaskToImageAction,
    canApplyVectorMask as canApplyVectorMaskAction,
    clearVectorMaskFromImage as clearVectorMaskFromImageAction,
    convertShapeToPath as convertShapeToPathAction,
    extractAllSvgSubPaths as extractAllSvgSubPathsAction,
    extractSvgSubPath as extractSvgSubPathAction,
    ungroupVectorGroup as ungroupVectorGroupAction,
    type MoodBoardVectorActionResult,
  } from "$lib/mood-board/vector-actions";
  import {
    appendMoodBoardPenNode,
    createMoodBoardVectorPathFromPen,
    isMoodBoardPenCloseHit,
  } from "$lib/mood-board/pen";
  import { errorMessage } from "$lib/util";
  import {
    boardWithZoomAtPoint as moodBoardWithZoomAtPoint,
    clampMoodBoardZoom,
    moodBoardItemBounds,
    moodBoardViewportForBounds,
    screenPointToCanvas,
  } from "$lib/mood-board/viewport";
  import {
    isNativeCanvasZoomBegin,
    isNativeCanvasZoomEnd,
    isNativeCanvasZoomUpdate,
    moodBoardPanViewportByWheel,
    moodBoardWheelIntent,
    type NativeCanvasZoomPayload,
  } from "$lib/mood-board/viewport-input";
  import type { ScssVariable } from "$lib/types";

  let {
    board,
    tool = "select",
    canUndo = false,
    canRedo = false,
    saveState = "idle",
    saveStatus = "",
    scssVariables = [],
    sessionIdentity,
    isSessionCurrent,
    setTool,
    commitBoard,
    setTransientBoard,
    undo,
    redo,
    applyImageToSelectedElement,
    applyColorToScssVariable,
    onStatusUpdate = () => undefined,
  }: {
    board: MoodBoard;
    tool?: MoodBoardTool;
    canUndo?: boolean;
    canRedo?: boolean;
    saveState?: MoodBoardSaveState;
    saveStatus?: string;
    sessionIdentity: MoodBoardRequestIdentity;
    isSessionCurrent: MoodBoardIdentityGuard;
    setTool: (tool: MoodBoardTool) => void;
    commitBoard: (board: MoodBoard) => void;
    setTransientBoard: (board: MoodBoard) => void;
    undo: () => void;
    redo: () => void;
    scssVariables?: ScssVariable[];
    applyImageToSelectedElement?: (path: string) => void | Promise<void>;
    applyColorToScssVariable?: (color: string, label: string, variableName?: string) => void | Promise<void>;
    onStatusUpdate?: (text: string, kind: "idle" | "saved" | "error") => void;
  } = $props();

  let stageEl: HTMLElement | null = $state(null);
  let selectedItemIds = $state<string[]>([]);
  let selectedSvgElementId = $state<string | null>(null);
  let vectorEditTarget = $state<MoodBoardVectorEditTarget | null>(null);
  let vectorNodeEditState = $state<MoodBoardVectorNodeEditState | null>(null);
  let marqueeBox = $state<MoodBoardMarqueeBox | null>(null);
  let snapGuides = $state<MoodBoardSnapGuide[]>([]);
  let attachFrameId = $state<string | null>(null);
  let penDraftNodes = $state<MoodBoardVectorNode[]>([]);
  let gestureStartBoard: MoodBoard | null = null;
  let nativeZoomStartBoard: MoodBoard | null = null;
  let nativeZoomCommitTimer: ReturnType<typeof setTimeout> | null = null;
  let clipboardFallbackTimer: ReturnType<typeof setTimeout> | null = null;
  let lastClipboardPasteAt = 0;
  let componentDisposed = false;
  let heavyAssetBusy = $state(false);
  let dragState = $state<MoodBoardDragState | null>(null);
  const heavyAssetGate = createMoodBoardHeavyAssetGate();
  const viewportScheduler = createMoodBoardBoardScheduler((nextBoard) => setTransientBoard(nextBoard));
  const interactionScheduler = createMoodBoardBoardScheduler((nextBoard) => setTransientBoard(nextBoard));

  const visibleItems = $derived(board.items);

  const allItems = $derived(flattenMoodBoardItems(board.items));
  const selectedItems = $derived(moodBoardSelectedItems(board, selectedItemIds));

  const visibleItemIds = $derived(allItems.map((item) => item.id));
  const activeVectorNodeEditState = $derived(
    moodBoardActiveVectorNodeEditState(vectorNodeEditState, vectorEditTarget),
  );

  type MoodBoardImageDropDetail = {
    relativePath: string;
    clientX: number;
    clientY: number;
    expectedProjectRoot: string;
    expectedSessionId: string;
  };

  function currentSessionGuard(identity: MoodBoardRequestIdentity) {
    return !componentDisposed && isSessionCurrent(identity);
  }

  function capturedSessionIdentity(): MoodBoardRequestIdentity {
    const identity = { ...sessionIdentity };
    requireCurrentMoodBoardIdentity(identity, currentSessionGuard);
    return identity;
  }

  function staleAsyncCompletion(error: unknown, identity: MoodBoardRequestIdentity) {
    return isMoodBoardStaleSessionError(error) || !currentSessionGuard(identity);
  }

  async function runHeavyAssetOperation(operation: () => Promise<void>) {
    const permit = heavyAssetGate.tryAcquire();
    if (!permit) return false;
    heavyAssetBusy = true;
    try {
      await operation();
      return true;
    } finally {
      permit.release();
      heavyAssetBusy = heavyAssetGate.isBusy();
    }
  }

  function setCanvasTool(nextTool: MoodBoardTool) {
    if (nextTool !== "pen") penDraftNodes = [];
    if (nextTool !== "select") {
      vectorEditTarget = null;
      vectorNodeEditState = null;
    }
    setTool(nextTool);
  }

  function currentSelectionState(): MoodBoardSelectionState {
    return moodBoardSelectionState(selectedItemIds, selectedSvgElementId, vectorEditTarget, vectorNodeEditState);
  }

  function applySelectionState(next: MoodBoardSelectionState) {
    selectedItemIds = next.selectedItemIds;
    selectedSvgElementId = next.selectedSvgElementId;
    vectorEditTarget = next.vectorEditTarget;
    vectorNodeEditState = next.vectorNodeEditState;
  }

  function setSelectedSvgElement(itemId: string, elementId: string | null) {
    applySelectionState(moodBoardSetSvgElementSelection(board, currentSelectionState(), itemId, elementId));
  }

  function enterVectorEditMode(itemId: string, svgElementId: string | null = null) {
    const next = moodBoardEnterVectorEditMode(board, itemId, svgElementId);
    if (!next) return;
    applySelectionState(next);
    setCanvasTool("select");
  }

  function exitVectorEditMode() {
    vectorEditTarget = null;
    vectorNodeEditState = null;
  }

  function screenToCanvas(clientX: number, clientY: number) {
    return screenPointToCanvas(board, stageEl?.getBoundingClientRect(), clientX, clientY);
  }

  function screenToCanvasWithBoard(sourceBoard: MoodBoard, clientX: number, clientY: number) {
    return screenPointToCanvas(sourceBoard, stageEl?.getBoundingClientRect(), clientX, clientY);
  }

  function clampZoom(value: number) {
    return clampMoodBoardZoom(value);
  }

  function boardWithZoomAtPoint(sourceBoard: MoodBoard, nextZoomValue: number, clientX: number, clientY: number) {
    return moodBoardWithZoomAtPoint(sourceBoard, stageEl?.getBoundingClientRect(), nextZoomValue, clientX, clientY);
  }

  function scheduleViewportUpdate(next: MoodBoard) {
    viewportScheduler.schedule(next);
  }

  function scheduleInteractionUpdate(next: MoodBoard) {
    interactionScheduler.schedule(next);
  }

  function flushInteractionUpdate() {
    return interactionScheduler.flush();
  }

  function setBoardItemTransient(nextItem: MoodBoardItemData) {
    const next = cloneMoodBoard(board);
    next.items = mapMoodBoardItems(next.items, (item) => item.id === nextItem.id ? nextItem : item);
    setTransientBoard(next);
  }

  function commitBoardItemEdit(beforeItem: MoodBoardItemData, nextItem: MoodBoardItemData) {
    if (JSON.stringify(beforeItem) === JSON.stringify(nextItem)) return;
    const current = cloneMoodBoard(board);
    const before = cloneMoodBoard(current);
    before.items = mapMoodBoardItems(before.items, (item) => item.id === beforeItem.id ? beforeItem : item);
    current.items = mapMoodBoardItems(current.items, (item) => item.id === nextItem.id ? nextItem : item);
    setTransientBoard(before);
    commitBoard(current);
  }

  function setVectorNodeEditState(state: MoodBoardVectorNodeEditState | null) {
    vectorNodeEditState = moodBoardAcceptedVectorNodeEditState(state, vectorEditTarget);
  }

  function applySelectedVectorNodeHandleMode(mode: MoodBoardVectorHandleMode) {
    const edit = moodBoardApplyVectorNodeHandleMode(board, activeVectorNodeEditState, mode);
    if (!edit) return;
    commitBoardItemEdit(edit.beforeItem, edit.nextItem);
    vectorNodeEditState = edit.vectorNodeEditState;
  }

  function isItemSelected(itemId: string) {
    return selectedItemIds.includes(itemId);
  }

  function selectOnly(itemId: string | null) {
    applySelectionState(moodBoardSelectOnly(itemId));
  }

  function setSelection(itemIds: string[]) {
    applySelectionState(moodBoardSetSelection(itemIds, allItems));
  }

  function visibleSelectedIds() {
    return moodBoardVisibleSelectedIds(selectedItemIds, visibleItemIds);
  }

  function toggleSelection(itemId: string) {
    applySelectionState(moodBoardToggleSelection(currentSelectionState(), itemId));
  }

  function deleteSelectedItems() {
    const ids = visibleSelectedIds();
    if (ids.length === 0) return;
    const selectedSet = new Set(ids);
    const next = cloneMoodBoard(board);
    next.items = removeMoodBoardItems(next.items, selectedSet);
    selectOnly(null);
    commitBoard(next);
  }

  function parentFrameTitle(itemId: string) {
    return moodBoardParentFrameTitle(board, itemId);
  }

  function applyMoodBoardItemActionResult(result: MoodBoardItemActionResult | null) {
    if (!result) return;
    if (result.selectedItemIds) {
      if (result.selectionMode === "selectOnly") {
        selectOnly(result.selectedItemIds[0] ?? null);
      } else if (result.selectionMode === "setSelection") {
        setSelection(result.selectedItemIds);
      } else {
        selectedItemIds = result.selectedItemIds;
      }
    }
    if (result.selectedSvgElementId !== undefined) selectedSvgElementId = result.selectedSvgElementId;
    if (result.board) commitBoard(result.board);
    if (result.status) onStatusUpdate(result.status.text, result.status.kind);
  }

  function bringSelectedItemsToFront() {
    applyMoodBoardItemActionResult(bringItemsToFrontAction(board, visibleSelectedIds()));
  }

  function sendSelectedItemsToBack() {
    applyMoodBoardItemActionResult(sendItemsToBackAction(board, visibleSelectedIds()));
  }

  function duplicateSelectedItems() {
    applyMoodBoardItemActionResult(duplicateItemsAction(board, visibleSelectedIds()));
  }

  function alignSelectedItems(mode: MoodBoardAlignMode) {
    applyMoodBoardItemActionResult(alignMoodBoardItemsAction(board, visibleSelectedIds(), mode));
  }

  function distributeSelectedItems(mode: MoodBoardDistributeMode) {
    applyMoodBoardItemActionResult(distributeMoodBoardItemsAction(board, visibleSelectedIds(), mode));
  }

  function groupSelectedItems() {
    applyMoodBoardItemActionResult(groupMoodBoardItemsAction(board, visibleSelectedIds()));
  }

  function ungroupSelectedGroup() {
    const groupId = selectedItemIds.length === 1 ? selectedItemIds[0] : null;
    applyMoodBoardItemActionResult(ungroupMoodBoardGroupAction(board, groupId));
  }

  function nudgeSelectedItems(dx: number, dy: number) {
    const next = nudgeMoodBoardItems(board, visibleSelectedIds(), dx, dy);
    if (next) commitBoard(next);
  }

  function canApplyVectorMask(itemId: string) {
    return canApplyVectorMaskAction(board, visibleSelectedIds(), itemId);
  }

  function applyVectorMaskToImage(itemId: string) {
    applyMoodBoardVectorActionResult(applyVectorMaskToImageAction(board, visibleSelectedIds(), itemId));
  }

  function clearVectorMaskFromImage(itemId: string) {
    const next = clearVectorMaskFromImageAction(board, itemId);
    if (next) commitBoard(next);
  }

  function addItem(item: MoodBoardItemData) {
    const next = cloneMoodBoard(board);
    next.items = [...next.items, item];
    selectOnly(item.id);
    commitBoard(next);
  }

  function centerPoint() {
    const rect = stageEl?.getBoundingClientRect();
    return screenToCanvas((rect?.left ?? 0) + (rect?.width ?? 0) / 2, (rect?.top ?? 0) + (rect?.height ?? 0) / 2);
  }

  function addNote() {
    addItem(createMoodBoardNote(centerPoint()));
  }

  function addText() {
    addItem(createMoodBoardText(centerPoint()));
  }

  function addColor() {
    addItem(createMoodBoardColor(centerPoint()));
  }

  function addReference() {
    addItem(createMoodBoardReference(centerPoint()));
  }

  function addFrame(preset: FramePreset = "desktop") {
    addItem(createMoodBoardFrame(centerPoint(), preset));
  }

  function addShape() {
    addItem(createMoodBoardShape(centerPoint()));
  }

  function addVectorPath() {
    addItem(createDefaultMoodBoardVectorPath(centerPoint()));
  }

  function applyMoodBoardVectorActionResult(result: MoodBoardVectorActionResult | null) {
    if (!result) return;
    if (result.selectedItemIds) selectedItemIds = result.selectedItemIds;
    if (result.selectedSvgElementId !== undefined) selectedSvgElementId = result.selectedSvgElementId;
    if (result.board) commitBoard(result.board);
    if (result.status) onStatusUpdate(result.status.text, result.status.kind);
  }

  function extractSvgSubPath(itemId: string, elementId: string) {
    applyMoodBoardVectorActionResult(extractSvgSubPathAction(board, itemId, elementId));
  }

  function extractAllSvgSubPaths(itemId: string) {
    applyMoodBoardVectorActionResult(extractAllSvgSubPathsAction(board, itemId));
  }

  function ungroupVectorGroup(itemId: string) {
    applyMoodBoardVectorActionResult(ungroupVectorGroupAction(board, itemId));
  }

  function convertShapeToPath(itemId: string) {
    applyMoodBoardVectorActionResult(convertShapeToPathAction(board, itemId));
  }

  async function exportVectorPath(itemId: string) {
    const identity = capturedSessionIdentity();
    await exportMoodBoardVectorPathWorkflow(
      board,
      itemId,
      window.prompt,
      onStatusUpdate,
      identity,
      currentSessionGuard,
    );
  }

  async function exportImageWebp(itemId: string) {
    await runHeavyAssetOperation(async () => {
      const identity = capturedSessionIdentity();
      await exportMoodBoardImageWebpWorkflow(
        board,
        itemId,
        window.prompt,
        onStatusUpdate,
        identity,
        currentSessionGuard,
      );
    });
  }

  async function exportCompositionWebp(itemId: string) {
    await runHeavyAssetOperation(async () => {
      const identity = capturedSessionIdentity();
      await exportMoodBoardCompositionWebpWorkflow(
        board,
        itemId,
        window.prompt,
        onStatusUpdate,
        identity,
        currentSessionGuard,
      );
    });
  }

  function createVectorPathFromPen(closed: boolean) {
    const vectorPath = createMoodBoardVectorPathFromPen(penDraftNodes, closed);
    if (!vectorPath) {
      penDraftNodes = [];
      return;
    }
    const next = cloneMoodBoard(board);
    next.items = [...next.items, vectorPath];
    penDraftNodes = [];
    selectOnly(vectorPath.id);
    setTool("select");
    commitBoard(next);
  }

  function handlePenPointerDown(event: PointerEvent) {
    if (event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    stageEl?.focus();
    const point = screenToCanvas(event.clientX, event.clientY);
    if (isMoodBoardPenCloseHit(penDraftNodes, point, board.viewport.zoom)) {
      createVectorPathFromPen(true);
      return;
    }
    penDraftNodes = appendMoodBoardPenNode(penDraftNodes, point);
  }

  async function addVisualAssetAtPath(
    path: string,
    point = centerPoint(),
    operationIdentity = capturedSessionIdentity(),
  ) {
    requireCurrentMoodBoardIdentity(operationIdentity, currentSessionGuard);
    const result = await addMoodBoardVisualAssetAtPath(
      path,
      point,
      operationIdentity,
      currentSessionGuard,
    );
    requireCurrentMoodBoardIdentity(operationIdentity, currentSessionGuard);
    if (result.item) {
      const applied = applyMoodBoardVisualAssetItem(board, result.item);
      if (applied.selectedItemId) selectOnly(applied.selectedItemId);
      commitBoard(applied.board);
    }
    if (result.status) onStatusUpdate(result.status.text, result.status.kind);
  }

  async function handlePaste(event: ClipboardEvent) {
    if (event.defaultPrevented) return;
    if (isMoodBoardEditableTarget(event.target)) return;
    await runHeavyAssetOperation(async () => {
      const identity = capturedSessionIdentity();
      const relativePathPromise = saveMoodBoardClipboardEventImageToDesign(
        event,
        identity,
        currentSessionGuard,
      );
      if (!relativePathPromise) return;

      event.preventDefault();
      lastClipboardPasteAt = Date.now();
      stageEl?.focus();
      const point = centerPoint();

      try {
        onStatusUpdate("Se salvează imaginea lipită în design/imagini...", "idle");
        const relativePath = await relativePathPromise;
        requireCurrentMoodBoardIdentity(identity, currentSessionGuard);
        await addVisualAssetAtPath(relativePath, point, identity);
        requireCurrentMoodBoardIdentity(identity, currentSessionGuard);
        onStatusUpdate(`Imagine lipită: ${relativePath}`, "saved");
      } catch (error) {
        if (staleAsyncCompletion(error, identity)) return;
        onStatusUpdate(`Lipirea imaginii a eșuat: ${errorMessage(error)}`, "error");
      }
    });
  }

  async function pasteClipboardImageFromNavigator() {
    await runHeavyAssetOperation(async () => {
      const identity = capturedSessionIdentity();
      stageEl?.focus();
      const point = centerPoint();

      try {
        onStatusUpdate("Se salvează imaginea lipită în design/imagini...", "idle");
        const relativePath = await pasteMoodBoardClipboardImageToDesign(
          identity,
          currentSessionGuard,
        );
        requireCurrentMoodBoardIdentity(identity, currentSessionGuard);
        await addVisualAssetAtPath(relativePath, point, identity);
        requireCurrentMoodBoardIdentity(identity, currentSessionGuard);
        lastClipboardPasteAt = Date.now();
        onStatusUpdate(`Imagine lipită: ${relativePath}`, "saved");
      } catch (error) {
        if (staleAsyncCompletion(error, identity)) return;
        onStatusUpdate(`Lipirea imaginii a eșuat: ${errorMessage(error)}`, "error");
      }
    });
  }

  function addImage() {
    const path = window.prompt("Path imagine în proiect", "static/");
    if (!path) return;
    const identity = capturedSessionIdentity();
    void addVisualAssetAtPath(path, centerPoint(), identity).catch((error) => {
      if (staleAsyncCompletion(error, identity)) return;
      onStatusUpdate(`Importul asset-ului a eșuat: ${errorMessage(error)}`, "error");
    });
  }

  async function extractPaletteFromImage(itemId: string, path: string) {
    await runHeavyAssetOperation(async () => {
      const identity = capturedSessionIdentity();
      try {
        const result = await extractMoodBoardPaletteItems(
          board,
          itemId,
          path,
          identity,
          currentSessionGuard,
        );
        requireCurrentMoodBoardIdentity(identity, currentSessionGuard);
        if (!result) return;
        const applied = applyMoodBoardPaletteColors(board, itemId, path, result.colors);
        if (!applied) {
          onStatusUpdate(
            "Paleta a fost ignorată deoarece imaginea sursă s-a schimbat în timpul analizei.",
            "error",
          );
          return;
        }
        selectOnly(applied.selectedItemId);
        commitBoard(applied.board);
      } catch (error) {
        if (staleAsyncCompletion(error, identity)) return;
        window.alert(error instanceof Error ? error.message : String(error));
      }
    });
  }

  function handleExternalImageDrop(event: Event) {
    const detail = (event as CustomEvent<MoodBoardImageDropDetail>).detail;
    if (!detail?.relativePath) return;
    const identity = {
      expectedProjectRoot: detail.expectedProjectRoot,
      expectedSessionId: detail.expectedSessionId,
    };
    if (
      identity.expectedProjectRoot !== sessionIdentity.expectedProjectRoot
      || identity.expectedSessionId !== sessionIdentity.expectedSessionId
      || !currentSessionGuard(identity)
    ) return;
    void addVisualAssetAtPath(
      detail.relativePath,
      screenToCanvas(detail.clientX, detail.clientY),
      identity,
    ).catch((error) => {
      if (staleAsyncCompletion(error, identity)) return;
      onStatusUpdate(`Importul asset-ului a eșuat: ${errorMessage(error)}`, "error");
    });
  }

  function handleGestureStart(event: Event) {
    event.preventDefault();
    gestureStartBoard = cloneMoodBoard(viewportScheduler.current() ?? board);
  }

  function handleGestureChange(event: Event) {
    const gestureEvent = event as Event & { scale?: number; clientX?: number; clientY?: number };
    if (!gestureStartBoard || !gestureEvent.scale) return;
    event.preventDefault();
    const rect = stageEl?.getBoundingClientRect();
    const clientX = gestureEvent.clientX ?? (rect ? rect.left + rect.width / 2 : 0);
    const clientY = gestureEvent.clientY ?? (rect ? rect.top + rect.height / 2 : 0);
    const next = boardWithZoomAtPoint(
      gestureStartBoard,
      (gestureStartBoard.viewport.zoom || 1) * gestureEvent.scale,
      clientX,
      clientY,
    );
    scheduleViewportUpdate(next);
  }

  function handleGestureEnd(event: Event) {
    event.preventDefault();
    gestureStartBoard = null;
  }

  function handleNativeCanvasZoom(detail: NativeCanvasZoomPayload) {
    if (!stageEl) return;
    if (isNativeCanvasZoomBegin(detail.phase)) {
      nativeZoomStartBoard = cloneMoodBoard(viewportScheduler.current() ?? board);
      return;
    }
    if (isNativeCanvasZoomEnd(detail.phase)) {
      nativeZoomStartBoard = null;
      return;
    }
    if (!isNativeCanvasZoomUpdate(detail.phase)) return;
    if (!Number.isFinite(detail.scale) || detail.scale <= 0) return;
    if (!nativeZoomStartBoard) return;
    const rect = stageEl.getBoundingClientRect();
    const clientX = rect.left + detail.x;
    const clientY = rect.top + detail.y;
    const next = boardWithZoomAtPoint(
      nativeZoomStartBoard,
      (nativeZoomStartBoard.viewport.zoom || 1) * detail.scale,
      clientX,
      clientY,
    );

    scheduleViewportUpdate(next);
    if (nativeZoomCommitTimer) clearTimeout(nativeZoomCommitTimer);
    nativeZoomCommitTimer = setTimeout(() => {
      nativeZoomCommitTimer = null;
      nativeZoomStartBoard = null;
    }, 180);
  }

  function beginCanvasPan(event: PointerEvent) {
    stageEl?.focus();
    event.preventDefault();
    event.stopPropagation();
    stageEl?.setPointerCapture?.(event.pointerId);
    dragState = createMoodBoardPanDragState(board, event);
  }

  function beginItemDrag(event: PointerEvent, itemId: string) {
    if (shouldIgnoreMoodBoardItemDrag(event)) return;
    if (isMoodBoardSecondaryPointer(event) || tool === "pan") {
      beginCanvasPan(event);
      return;
    }
    if (event.button !== 0) return;
    stageEl?.focus();
    const item = findMoodBoardItem(board.items, itemId);
    if (!item) return;

    const additive = event.shiftKey || event.ctrlKey || event.metaKey;
    const selection = moodBoardItemDragSelection(selectedItemIds, visibleItemIds, itemId, additive);
    applySelectionState(moodBoardSetSelection(selection.selectedItemIds, allItems));
    if (!selection.shouldStartDrag) return;

    event.preventDefault();
    (event.currentTarget as HTMLElement).setPointerCapture?.(event.pointerId);
    dragState = createMoodBoardItemDragState({
      board,
      event,
      item,
      itemId,
      itemIds: selection.dragItemIds,
      allItems,
    });
  }

  function beginItemResize(event: PointerEvent, itemId: string, resizeHandle: MoodBoardResizeHandle) {
    selectOnly(itemId);
    if (tool === "pan") return;
    const item = findMoodBoardItem(board.items, itemId);
    if (!item) return;
    event.preventDefault();
    event.stopPropagation();
    snapGuides = [];
    (event.currentTarget as HTMLElement).setPointerCapture?.(event.pointerId);
    dragState = createMoodBoardResizeDragState(board, event, item, resizeHandle);
  }

  function beginPan(event: PointerEvent) {
    if (tool === "pen") {
      handlePenPointerDown(event);
      return;
    }
    if (!isMoodBoardCanvasPanTarget(event)) return;
    if (isMoodBoardSecondaryPointer(event) || tool === "pan") {
      beginCanvasPan(event);
      return;
    }
    if (event.button === 0) {
      stageEl?.focus();
      beginMarquee(event);
      return;
    }
  }

  function beginMarquee(event: PointerEvent) {
    const additive = event.shiftKey || event.ctrlKey || event.metaKey;
    const selection = moodBoardMarqueeSelectionStart(selectedItemIds, additive);
    applySelectionState(moodBoardSetSelection(selection.selectedItemIds, allItems));
    event.preventDefault();
    stageEl?.setPointerCapture?.(event.pointerId);
    dragState = createMoodBoardMarqueeDragState(board, event, selection.initialSelection, selection.additive);
    updateMarquee(event.clientX, event.clientY);
  }

  function updateMarquee(clientX: number, clientY: number) {
    if (!dragState || !stageEl) return;
    const rect = stageEl.getBoundingClientRect();
    const start = screenToCanvas(dragState.startX, dragState.startY);
    const end = screenToCanvas(clientX, clientY);
    const update = moodBoardMarqueeDragUpdate(dragState, rect, start, end, visibleItems, clientX, clientY);
    if (!update) return;
    marqueeBox = update.marqueeBox;
    setSelection(update.selectedItemIds);
  }

  function handlePointerMove(event: PointerEvent) {
    if (!dragState) return;
    event.preventDefault();

    if (dragState.kind === "marquee") {
      const overlays = clearMoodBoardDragOverlays();
      snapGuides = overlays.snapGuides;
      attachFrameId = overlays.attachFrameId;
      updateMarquee(event.clientX, event.clientY);
      return;
    }

    const update = moodBoardPointerDragUpdate(dragState, event);
    if (!update) return;
    snapGuides = update.snapGuides;
    attachFrameId = update.attachFrameId;
    if (dragState.kind === "resize") dragState.resizeAspectAxis = update.resizeAspectAxis;
    scheduleInteractionUpdate(update.board);
  }

  function detachItem(itemId: string) {
    const next = detachChildFromFrameIfNeeded(cloneMoodBoard(board), itemId, true);
    if (JSON.stringify(next) === JSON.stringify(board)) return;
    selectOnly(itemId);
    commitBoard(next);
  }

  function handlePointerUp() {
    if (!dragState) return;
    if (dragState.kind === "marquee") {
      const overlays = clearMoodBoardDragOverlays();
      dragState = null;
      marqueeBox = overlays.marqueeBox;
      snapGuides = overlays.snapGuides;
      attachFrameId = overlays.attachFrameId;
      return;
    }
    const dragKind = dragState.kind;
    let current = flushInteractionUpdate() ?? cloneMoodBoard(board);
    const before = dragState.before;
    current = finalizeMoodBoardItemDragAttachment(current, dragState);
    const overlays = clearMoodBoardDragOverlays();
    dragState = null;
    snapGuides = overlays.snapGuides;
    attachFrameId = overlays.attachFrameId;
    if (dragKind === "pan") return;
    if (JSON.stringify(current) !== JSON.stringify(before)) {
      setTransientBoard(before);
      commitBoard(current);
    }
  }

  function setZoom(nextZoom: number) {
    const next = cloneMoodBoard(viewportScheduler.current() ?? board);
    next.viewport.zoom = clampZoom(nextZoom);
    scheduleViewportUpdate(next);
  }

  function zoomIn() {
    setZoom((viewportScheduler.current() ?? board).viewport.zoom + 0.1);
  }

  function zoomOut() {
    setZoom((viewportScheduler.current() ?? board).viewport.zoom - 0.1);
  }

  function itemBounds() {
    return moodBoardItemBounds(visibleItems);
  }

  function fit() {
    const next = cloneMoodBoard(board);
    const rect = stageEl?.getBoundingClientRect();
    const bounds = itemBounds();
    next.viewport = moodBoardViewportForBounds(rect, bounds);
    scheduleViewportUpdate(next);
  }

  function handleWheel(event: WheelEvent) {
    event.preventDefault();
    const baseBoard = viewportScheduler.current() ?? board;
    const intent = moodBoardWheelIntent(event);

    if (intent.kind === "pan") {
      scheduleViewportUpdate(moodBoardPanViewportByWheel(baseBoard, intent.deltaX, intent.deltaY));
      return;
    }

    const next = boardWithZoomAtPoint(
      baseBoard,
      (baseBoard.viewport.zoom || 1) * intent.zoomFactor,
      event.clientX,
      event.clientY,
    );
    scheduleViewportUpdate(next);
  }

  function handleKeydown(event: KeyboardEvent) {
    const intent = moodBoardKeyboardIntent(event, {
      tool,
      hasSelection: selectedItemIds.length > 0,
      hasVectorEditTarget: Boolean(vectorEditTarget),
      penNodeCount: penDraftNodes.length,
    });

    if (intent.kind === "none") return;

    if (intent.kind === "pasteClipboardFallback") {
      const pasteStartedAt = Date.now();
      const identity = capturedSessionIdentity();
      if (clipboardFallbackTimer) clearTimeout(clipboardFallbackTimer);
      clipboardFallbackTimer = setTimeout(() => {
        clipboardFallbackTimer = null;
        if (!currentSessionGuard(identity)) return;
        if (lastClipboardPasteAt >= pasteStartedAt) return;
        void pasteClipboardImageFromNavigator();
      }, 80);
      return;
    }

    event.preventDefault();

    if (intent.kind === "finishPenPath") {
      createVectorPathFromPen(intent.closed);
    } else if (intent.kind === "clearPenDraft") {
      penDraftNodes = [];
    } else if (intent.kind === "exitVectorEdit") {
      exitVectorEditMode();
    } else if (intent.kind === "deleteSelection") {
      deleteSelectedItems();
    } else if (intent.kind === "duplicateSelection") {
      duplicateSelectedItems();
    } else if (intent.kind === "groupSelection") {
      groupSelectedItems();
    } else if (intent.kind === "ungroupSelection") {
      ungroupSelectedGroup();
    } else if (intent.kind === "bringSelectionToFront") {
      bringSelectedItemsToFront();
    } else if (intent.kind === "sendSelectionToBack") {
      sendSelectedItemsToBack();
    } else if (intent.kind === "nudgeSelection") {
      nudgeSelectedItems(intent.dx, intent.dy);
    }
  }

  onMount(() => {
    let disposed = false;
    let unlistenNativeZoom: UnlistenFn | null = null;
    window.addEventListener("mood-board-image-drop", handleExternalImageDrop);
    window.addEventListener("paste", handlePaste);
    window.addEventListener("keydown", handleKeydown);
    stageEl?.addEventListener("gesturestart", handleGestureStart as EventListener);
    stageEl?.addEventListener("gesturechange", handleGestureChange as EventListener);
    stageEl?.addEventListener("gestureend", handleGestureEnd as EventListener);
    void listen<NativeCanvasZoomPayload>("native-canvas-zoom", (event) => {
      handleNativeCanvasZoom(event.payload);
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlistenNativeZoom = unlisten;
    });
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp);
    window.addEventListener("pointercancel", handlePointerUp);

    return () => {
      disposed = true;
      componentDisposed = true;
      unlistenNativeZoom?.();
      window.removeEventListener("mood-board-image-drop", handleExternalImageDrop);
      window.removeEventListener("paste", handlePaste);
      window.removeEventListener("keydown", handleKeydown);
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
      window.removeEventListener("pointercancel", handlePointerUp);
      stageEl?.removeEventListener("gesturestart", handleGestureStart as EventListener);
      stageEl?.removeEventListener("gesturechange", handleGestureChange as EventListener);
      stageEl?.removeEventListener("gestureend", handleGestureEnd as EventListener);
      viewportScheduler.cancel();
      interactionScheduler.cancel();
      if (nativeZoomCommitTimer) clearTimeout(nativeZoomCommitTimer);
      if (clipboardFallbackTimer) clearTimeout(clipboardFallbackTimer);
      nativeZoomStartBoard = null;
    };
  });

  function preventContextMenu(event: MouseEvent) {
    event.preventDefault();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<section
  bind:this={stageEl}
  class="mood-board-canvas"
  data-mood-board-canvas
  role="application"
  tabindex="0"
  aria-label="Mood board canvas"
  aria-busy={heavyAssetBusy}
  class:pen-tool={tool === "pen"}
  onpointerdown={beginPan}
  onwheel={handleWheel}
  oncontextmenu={preventContextMenu}
>
  <MoodBoardContextBar
    {selectedItems}
    {selectedSvgElementId}
    parentFrameTitle={selectedItems.length === 1 ? parentFrameTitle(selectedItems[0].id) : ""}
    previewItem={setBoardItemTransient}
    commitItemEdit={commitBoardItemEdit}
    {duplicateSelectedItems}
    {groupSelectedItems}
    {ungroupSelectedGroup}
    {alignSelectedItems}
    {distributeSelectedItems}
    {bringSelectedItemsToFront}
    {sendSelectedItemsToBack}
    {deleteSelectedItems}
    {detachItem}
    {applyImageToSelectedElement}
    {extractPaletteFromImage}
    {canApplyVectorMask}
    applyVectorMask={applyVectorMaskToImage}
    clearVectorMask={clearVectorMaskFromImage}
    {exportVectorPath}
    {extractSvgSubPath}
    {extractAllSvgSubPaths}
    {ungroupVectorGroup}
    {convertShapeToPath}
    {exportImageWebp}
    {exportCompositionWebp}
    {heavyAssetBusy}
    {scssVariables}
    {applyColorToScssVariable}
    vectorEditTargetItemId={vectorEditTarget?.itemId ?? null}
    vectorEditTargetSvgElementId={vectorEditTarget?.svgElementId ?? null}
    vectorNodeEditState={activeVectorNodeEditState}
    setSelectedVectorNodeHandleMode={applySelectedVectorNodeHandleMode}
    {enterVectorEditMode}
    {exitVectorEditMode}
  />

  <MoodBoardCanvasScene
    {board}
    {visibleItems}
    {attachFrameId}
    {selectedSvgElementId}
    vectorEditTargetItemId={vectorEditTarget?.itemId ?? null}
    vectorEditTargetSvgElementId={vectorEditTarget?.svgElementId ?? null}
    vectorNodeEditState={activeVectorNodeEditState}
    {penDraftNodes}
    {snapGuides}
    {marqueeBox}
    {isItemSelected}
    {parentFrameTitle}
    onPointerDown={beginItemDrag}
    onResizePointerDown={beginItemResize}
    {enterVectorEditMode}
    onSvgElementSelect={setSelectedSvgElement}
    onVectorNodeSelectionChange={setVectorNodeEditState}
    previewItem={setBoardItemTransient}
    commitItemEdit={commitBoardItemEdit}
    {sessionIdentity}
    {isSessionCurrent}
  />
  <MoodBoardDock
    {tool}
    zoom={board.viewport.zoom}
    {canUndo}
    {canRedo}
    {saveState}
    {saveStatus}
    setTool={setCanvasTool}
    {undo}
    {redo}
    {zoomIn}
    {zoomOut}
    {fit}
    {addNote}
    {addText}
    {addColor}
    {addReference}
    {addImage}
    {addFrame}
    {addShape}
  />
</section>

<style>
  .mood-board-canvas {
    position: relative;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: 10px;
    background:
      linear-gradient(color-mix(in srgb, var(--border-3) 28%, transparent) 1px, transparent 1px),
      linear-gradient(90deg, color-mix(in srgb, var(--border-3) 28%, transparent) 1px, transparent 1px),
      radial-gradient(circle at 50% 40%, color-mix(in srgb, var(--brand) 9%, transparent), transparent 36%),
      var(--surface-8);
    background-size: 40px 40px, 40px 40px, auto, auto;
    box-shadow: var(--shadow);
    touch-action: none;
  }

  .mood-board-canvas:focus {
    outline: none;
  }

  .mood-board-canvas.pen-tool {
    cursor: crosshair;
  }

</style>
