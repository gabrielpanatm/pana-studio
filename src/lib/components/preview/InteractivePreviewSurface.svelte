<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import {
    parseInteractivePreviewMessage,
    type InteractivePreviewDomNode,
  } from "$lib/preview/interactive";
  import type { WorkbenchCanvasMode } from "$lib/types";
  import { UI_TERMS } from "$lib/i18n/ui-terms";

  type FrameSlot = {
    id: number;
    url: string;
    previewRevision: string;
    startedAt: number;
  };

  export let desiredUrl = "";
  export let canvasMode: WorkbenchCanvasMode = "fit";
  export let canvasWidthPx = 1_440;
  export let previewZoom = 100;
  export let onDomSnapshot: (nodes: InteractivePreviewDomNode[]) => void = () => {};
  export let onLifecycleError: (message: string) => void = () => {};
  export let onRealmRestarted: (previewRevision: string, durationMs: number) => void = () => {};
  export let onRealmFailed: (previewRevision: string, durationMs: number, diagnostic: string) => void = () => {};

  let frames: FrameSlot[] = [];
  let activeId: number | null = null;
  let observedDesiredUrl = "";
  let nextId = 1;
  let status = `${UI_TERMS.interactivePreview} se pregătește…`;
  const frameNodes = new Map<number, HTMLIFrameElement>();
  const timeouts = new Map<number, number>();

  $: if (desiredUrl !== observedDesiredUrl) {
    observedDesiredUrl = desiredUrl;
    stageDesiredUrl(desiredUrl);
  }

  function previewRevisionFromUrl(url: string) {
    try {
      return new URL(url).searchParams.get("__pana_preview_revision") ?? "";
    } catch {
      return "";
    }
  }

  function stageDesiredUrl(url: string) {
    if (!url) {
      clearAllFrames();
      status = `${UI_TERMS.interactivePreview} este indisponibilă.`;
      return;
    }
    const active = frames.find((slot) => slot.id === activeId);
    if (active?.url === url || frames.some((slot) => slot.id !== activeId && slot.url === url)) return;
    const previewRevision = previewRevisionFromUrl(url);
    if (!previewRevision) {
      status = `${UI_TERMS.interactivePreview} a refuzat un URL fără revizie canonică.`;
      return;
    }

    for (const slot of frames.filter((entry) => entry.id !== activeId)) removeFrame(slot.id);
    const slot = { id: nextId++, url, previewRevision, startedAt: performance.now() };
    frames = [...frames.filter((entry) => entry.id === activeId), slot];
    status = activeId === null
      ? `${UI_TERMS.interactivePreview} pornește într-un mediu izolat…`
      : "Se verifică noua revizie interactivă…";
    const timeout = window.setTimeout(() => {
      if (slot.id === activeId) return;
      removeFrame(slot.id);
      status = activeId === null
        ? `${UI_TERMS.interactivePreview} nu a confirmat pornirea în 15 secunde.`
        : "Noua revizie JS a eșuat; ultima revizie interactivă validă rămâne activă.";
      onRealmFailed(
        slot.previewRevision,
        Math.max(0, performance.now() - slot.startedAt),
        status,
      );
    }, 15_000);
    timeouts.set(slot.id, timeout);
  }

  function registerFrame(node: HTMLIFrameElement, id: number) {
    frameNodes.set(id, node);
    return {
      destroy() {
        if (frameNodes.get(id) === node) frameNodes.delete(id);
      },
    };
  }

  function handleMessage(event: MessageEvent) {
    const slot = frames.find((entry) => {
      const frame = frameNodes.get(entry.id);
      return frame?.contentWindow && event.source === frame.contentWindow;
    });
    if (!slot) return;
    const message = parseInteractivePreviewMessage(
      frameNodes.get(slot.id),
      event,
      slot.previewRevision,
    );
    if (!message) return;

    if (message.type === "ready") {
      clearFrameTimeout(slot.id);
      activeId = slot.id;
      status = `${UI_TERMS.interactivePreview} activă · JavaScript izolat`;
      onRealmRestarted(
        slot.previewRevision,
        Math.max(0, performance.now() - slot.startedAt),
      );
      window.requestAnimationFrame(() => {
        for (const previous of frames.filter((entry) => entry.id !== slot.id)) {
          removeFrame(previous.id);
        }
        frames = frames.filter((entry) => entry.id === slot.id);
      });
      return;
    }
    if (message.type === "dom-snapshot" && slot.id === activeId) {
      onDomSnapshot(message.nodes);
      return;
    }
    if (message.type === "lifecycle-error") {
      onLifecycleError(
        `${message.componentId || "componentă"} · ${message.phase || "runtime"}: ${message.message}`,
      );
    }
  }

  function clearFrameTimeout(id: number) {
    const timeout = timeouts.get(id);
    if (timeout !== undefined) window.clearTimeout(timeout);
    timeouts.delete(id);
  }

  function removeFrame(id: number) {
    clearFrameTimeout(id);
    frameNodes.delete(id);
    frames = frames.filter((entry) => entry.id !== id);
    if (activeId === id) activeId = null;
  }

  function clearAllFrames() {
    for (const id of timeouts.keys()) clearFrameTimeout(id);
    frames = [];
    frameNodes.clear();
    activeId = null;
  }

  onMount(() => {
    window.addEventListener("message", handleMessage);
    return () => window.removeEventListener("message", handleMessage);
  });
  onDestroy(clearAllFrames);
</script>

<div
  class:canvas-fit={canvasMode === "fit"}
  class:canvas-fixed={canvasMode === "fixed"}
  class="interactive-stage"
  style={`--preview-zoom-scale: ${previewZoom / 100}; --canvas-width-px: ${canvasWidthPx}px;`}
  aria-label="Previzualizare interactivă izolată"
>
  {#each frames as slot (slot.id)}
    <iframe
      use:registerFrame={slot.id}
      class:active={slot.id === activeId}
      class="interactive-frame"
      title="Previzualizare interactivă izolată"
      src={slot.url}
      sandbox="allow-scripts"
      referrerpolicy="no-referrer"
    ></iframe>
  {/each}
  {#if activeId === null}
    <div class="interactive-status" role="status">{status}</div>
  {:else if frames.length > 1}
    <div class="interactive-restart" role="status">{status}</div>
  {/if}
</div>

<style>
  .interactive-stage {
    position: absolute;
    inset: 0;
    overflow: hidden;
    background: #f6f8f7;
  }

  .interactive-frame {
    position: absolute;
    inset: 0;
    z-index: 1;
    width: 100%;
    height: 100%;
    border: 0;
    opacity: 0;
    pointer-events: none;
    transform-origin: top left;
    background: transparent;
  }

  .interactive-frame.active {
    z-index: 2;
    opacity: 1;
    pointer-events: auto;
  }

  .canvas-fixed .interactive-frame {
    width: var(--canvas-width-px);
    height: calc(100% / var(--preview-zoom-scale));
    transform: scale(var(--preview-zoom-scale));
  }

  .interactive-status,
  .interactive-restart {
    position: absolute;
    z-index: 4;
    left: 50%;
    top: 50%;
    max-width: min(440px, calc(100% - 32px));
    padding: 12px 16px;
    border: 1px solid #c8d8d2;
    border-radius: 10px;
    color: #31433d;
    background: rgba(255, 255, 255, 0.94);
    box-shadow: 0 12px 32px rgba(25, 45, 38, 0.12);
    transform: translate(-50%, -50%);
    text-align: center;
    font-size: 13px;
  }

  .interactive-restart {
    top: auto;
    bottom: 14px;
    transform: translateX(-50%);
  }
</style>
