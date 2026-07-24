import type { HtmlPaletteElement } from "$lib/project/html-palette";
import { listenForExternalReconcileInteractionBarrier } from "$lib/session/external-reconcile-barrier";
import type { CenterView, SaveState } from "$lib/types";

export type ElementPaletteDragHost = {
  centerView: CenterView;
  previewFrame: HTMLIFrameElement | undefined;
  previewZoom: number;
  postPreviewMessage: (payload: Record<string, unknown>) => void;
  setGlobalStatus: (text: string, kind: SaveState) => void;
  syncPreviewTeraGateState?: () => void;
};

const dragThreshold = 6;

function clampPreviewZoom(value: number) {
  return Math.max(0.1, value / 100);
}

function previewCoordinatesForPointer(host: ElementPaletteDragHost, event: PointerEvent) {
  if (host.centerView !== "preview") return null;
  const frame = host.previewFrame;
  if (!frame) return null;
  const rect = frame.getBoundingClientRect();
  const x = event.clientX - rect.left;
  const y = event.clientY - rect.top;
  if (x < 0 || y < 0 || x > rect.width || y > rect.height) return null;
  const scale = clampPreviewZoom(host.previewZoom);
  return {
    x: x / scale,
    y: y / scale,
  };
}

function previewPayloadFor(element: HtmlPaletteElement) {
  return {
    id: element.id,
    kind: element.kind,
    blockId: element.blockId,
    blockKind: element.blockKind,
    tag: element.tag,
    label: element.label,
    description: element.description,
    text: element.text,
    className: element.className,
    html: element.html,
  };
}

function createDragOverlay(element: HtmlPaletteElement) {
  const overlay = document.createElement("div");
  overlay.style.position = "fixed";
  overlay.style.inset = "0";
  overlay.style.zIndex = "2147483646";
  overlay.style.cursor = "grabbing";
  overlay.style.background = "transparent";
  overlay.style.userSelect = "none";
  overlay.style.pointerEvents = "auto";

  const ghost = document.createElement("div");
  ghost.style.position = "fixed";
  ghost.style.left = "0";
  ghost.style.top = "0";
  ghost.style.display = "inline-flex";
  ghost.style.alignItems = "center";
  ghost.style.gap = "7px";
  ghost.style.maxWidth = "220px";
  ghost.style.padding = "7px 9px";
  ghost.style.border = "1px solid rgba(47, 170, 140, 0.68)";
  ghost.style.borderRadius = "8px";
  ghost.style.color = "#eef5f0";
  ghost.style.background = "rgba(18, 24, 22, 0.94)";
  ghost.style.boxShadow = "0 12px 30px rgba(0, 0, 0, 0.28)";
  ghost.style.font = "700 12px/1.2 Inter, ui-sans-serif, system-ui, sans-serif";
  ghost.style.pointerEvents = "none";

  const tag = document.createElement("span");
  tag.textContent = `<${element.tag}>`;
  tag.style.color = "#7fe0c7";
  tag.style.fontFamily = '"JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, monospace';

  const label = document.createElement("span");
  label.textContent = element.label;
  label.style.overflow = "hidden";
  label.style.textOverflow = "ellipsis";
  label.style.whiteSpace = "nowrap";

  ghost.append(tag, label);
  overlay.appendChild(ghost);
  document.body.appendChild(overlay);

  return { overlay, ghost };
}

function moveGhost(ghost: HTMLElement, event: PointerEvent) {
  ghost.style.transform = `translate(${Math.round(event.clientX + 14)}px, ${Math.round(event.clientY + 14)}px)`;
}

function setBodyDragging(active: boolean) {
  document.body.classList.toggle("element-palette-dragging", active);
}

function trySetPointerCapture(event: PointerEvent) {
  const target = event.currentTarget;
  if (target instanceof HTMLElement && typeof target.setPointerCapture === "function") {
    try {
      target.setPointerCapture(event.pointerId);
    } catch {
      // Best-effort only. The fallback window listeners keep the drag usable.
    }
  }
}

export function startElementPaletteDrag(
  host: ElementPaletteDragHost,
  element: HtmlPaletteElement,
  event: PointerEvent,
) {
  if (event.button !== 0) return;

  trySetPointerCapture(event);

  const pointerId = event.pointerId;
  const startX = event.clientX;
  const startY = event.clientY;
  let dragActive = false;
  let overlay: HTMLDivElement | null = null;
  let ghost: HTMLDivElement | null = null;
  let wasOverPreview = false;
  let stopExternalReconcileBarrier = () => {};

  const clearPreviewIndicator = () => {
    if (wasOverPreview) {
      host.postPreviewMessage({ type: "preview-insert-drag-clear" });
      wasOverPreview = false;
    }
  };

  const cleanup = () => {
    window.removeEventListener("pointermove", handlePointerMove, true);
    window.removeEventListener("pointerup", handlePointerUp, true);
    window.removeEventListener("pointercancel", handlePointerCancel, true);
    stopExternalReconcileBarrier();
    clearPreviewIndicator();
    overlay?.remove();
    overlay = null;
    ghost = null;
    setBodyDragging(false);
  };

  const activate = (moveEvent: PointerEvent) => {
    if (dragActive) return;
    dragActive = true;
    const created = createDragOverlay(element);
    overlay = created.overlay;
    ghost = created.ghost;
    setBodyDragging(true);
    moveGhost(ghost, moveEvent);
  };

  const updatePreview = (moveEvent: PointerEvent) => {
    const coordinates = previewCoordinatesForPointer(host, moveEvent);
    if (!coordinates) {
      clearPreviewIndicator();
      return;
    }
    wasOverPreview = true;
    host.syncPreviewTeraGateState?.();
    host.postPreviewMessage({
      type: "preview-insert-drag-update",
      x: coordinates.x,
      y: coordinates.y,
      element: previewPayloadFor(element),
    });
  };

  function handlePointerMove(moveEvent: PointerEvent) {
    if (moveEvent.pointerId !== pointerId) return;
    const distance = Math.hypot(moveEvent.clientX - startX, moveEvent.clientY - startY);
    if (!dragActive && distance < dragThreshold) return;

    moveEvent.preventDefault();
    activate(moveEvent);
    if (ghost) moveGhost(ghost, moveEvent);
    updatePreview(moveEvent);
  }

  function handlePointerUp(upEvent: PointerEvent) {
    if (upEvent.pointerId !== pointerId) return;
    const wasDrag = dragActive;
    if (wasDrag) {
      upEvent.preventDefault();
      const coordinates = previewCoordinatesForPointer(host, upEvent);
      if (coordinates) {
        host.syncPreviewTeraGateState?.();
        host.postPreviewMessage({
          type: "preview-insert-drag-drop",
          x: coordinates.x,
          y: coordinates.y,
          element: previewPayloadFor(element),
        });
      } else if (host.centerView !== "preview") {
        host.setGlobalStatus("Comută pe Previzualizare ca să adaugi elemente prin tragere.", "error");
      }
    }
    cleanup();
  }

  function handlePointerCancel(cancelEvent: PointerEvent) {
    if (cancelEvent.pointerId !== pointerId) return;
    cleanup();
  }

  window.addEventListener("pointermove", handlePointerMove, true);
  window.addEventListener("pointerup", handlePointerUp, true);
  window.addEventListener("pointercancel", handlePointerCancel, true);
  stopExternalReconcileBarrier = listenForExternalReconcileInteractionBarrier(cleanup);
}
