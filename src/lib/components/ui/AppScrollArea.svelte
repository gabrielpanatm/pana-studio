<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLAttributes } from "svelte/elements";
  import { t } from "$lib/i18n/runtime.svelte";
  import {
    appScrollbarGeometry,
    appScrollbarOffsetFromThumbDelta,
    appScrollbarOffsetFromTrackPoint,
  } from "$lib/ui/app-scrollbar";

  export type AppScrollAxis = "vertical" | "horizontal" | "both";
  export type AppScrollSnapshot = Readonly<{
    scrollTop: number;
    scrollLeft: number;
    clientWidth: number;
    clientHeight: number;
    scrollWidth: number;
    scrollHeight: number;
  }>;

  type ScrollbarAxis = "vertical" | "horizontal";
  type DragState = {
    axis: ScrollbarAxis;
    pointerId: number;
    pointerStart: number;
    scrollStart: number;
  };

  let {
    children,
    axis = "vertical",
    viewportId,
    viewportRole = undefined,
    viewportTabindex = 0,
    ariaLabel = undefined,
    class: className = "",
    style = "",
    autoHide = true,
    hideDelay = 900,
    inset = 2,
    minimumThumbSize = 28,
    resetKey = undefined,
    onscroll = undefined,
    onmeasure = undefined,
    onmouseleave = undefined,
  }: {
    children: Snippet;
    axis?: AppScrollAxis;
    viewportId: string;
    viewportRole?: HTMLAttributes<HTMLDivElement>["role"];
    viewportTabindex?: number;
    ariaLabel?: string;
    class?: string;
    style?: string;
    autoHide?: boolean;
    hideDelay?: number;
    inset?: number;
    minimumThumbSize?: number;
    resetKey?: string | number | null;
    onscroll?: (snapshot: AppScrollSnapshot) => void;
    onmeasure?: (snapshot: AppScrollSnapshot) => void;
    onmouseleave?: (event: MouseEvent) => void;
  } = $props();

  let viewport = $state<HTMLDivElement | null>(null);
  let content = $state<HTMLDivElement | null>(null);
  let snapshot = $state<AppScrollSnapshot>({
    scrollTop: 0,
    scrollLeft: 0,
    clientWidth: 0,
    clientHeight: 0,
    scrollWidth: 0,
    scrollHeight: 0,
  });
  let visible = $state(true);
  let hoveredAxis = $state<ScrollbarAxis | null>(null);
  let focusedAxis = $state<ScrollbarAxis | null>(null);
  let drag = $state<DragState | null>(null);
  let hideTimer: ReturnType<typeof setTimeout> | null = null;

  const allowsVertical = $derived(axis === "vertical" || axis === "both");
  const allowsHorizontal = $derived(axis === "horizontal" || axis === "both");
  const verticalGeometry = $derived(appScrollbarGeometry(
    snapshot.clientHeight,
    snapshot.scrollHeight,
    snapshot.scrollTop,
    Math.max(0, snapshot.clientHeight - inset * 2),
    minimumThumbSize,
  ));
  const horizontalGeometry = $derived(appScrollbarGeometry(
    snapshot.clientWidth,
    snapshot.scrollWidth,
    snapshot.scrollLeft,
    Math.max(0, snapshot.clientWidth - inset * 2),
    minimumThumbSize,
  ));

  function currentSnapshot(): AppScrollSnapshot {
    if (!viewport) return snapshot;
    return {
      scrollTop: viewport.scrollTop,
      scrollLeft: viewport.scrollLeft,
      clientWidth: viewport.clientWidth,
      clientHeight: viewport.clientHeight,
      scrollWidth: viewport.scrollWidth,
      scrollHeight: viewport.scrollHeight,
    };
  }

  function measure(notify = true) {
    snapshot = currentSnapshot();
    if (notify) onmeasure?.(snapshot);
  }

  function clearHideTimer() {
    if (hideTimer === null) return;
    clearTimeout(hideTimer);
    hideTimer = null;
  }

  function scheduleHide() {
    clearHideTimer();
    if (
      !autoHide
      || hoveredAxis
      || focusedAxis
      || drag
    ) {
      visible = true;
      return;
    }
    hideTimer = setTimeout(() => {
      visible = false;
      hideTimer = null;
    }, Math.max(0, hideDelay));
  }

  function reveal() {
    visible = true;
    scheduleHide();
  }

  function handleScroll() {
    measure(false);
    reveal();
    onscroll?.(snapshot);
  }

  function geometryFor(scrollbarAxis: ScrollbarAxis) {
    return scrollbarAxis === "vertical"
      ? verticalGeometry
      : horizontalGeometry;
  }

  function scrollOffsetFor(scrollbarAxis: ScrollbarAxis) {
    return scrollbarAxis === "vertical"
      ? snapshot.scrollTop
      : snapshot.scrollLeft;
  }

  function setScrollOffset(scrollbarAxis: ScrollbarAxis, offset: number) {
    if (!viewport) return;
    if (scrollbarAxis === "vertical") viewport.scrollTop = offset;
    else viewport.scrollLeft = offset;
    measure(false);
  }

  function pointerCoordinate(scrollbarAxis: ScrollbarAxis, event: PointerEvent) {
    return scrollbarAxis === "vertical" ? event.clientY : event.clientX;
  }

  function beginDrag(scrollbarAxis: ScrollbarAxis, event: PointerEvent) {
    event.preventDefault();
    event.stopPropagation();
    drag = {
      axis: scrollbarAxis,
      pointerId: event.pointerId,
      pointerStart: pointerCoordinate(scrollbarAxis, event),
      scrollStart: scrollOffsetFor(scrollbarAxis),
    };
    visible = true;
    clearHideTimer();
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  }

  function handleTrackPointerDown(
    scrollbarAxis: ScrollbarAxis,
    event: PointerEvent,
  ) {
    const target = event.target as HTMLElement;
    if (target.closest(".app-scrollbar-thumb")) {
      beginDrag(scrollbarAxis, event);
      return;
    }
    event.preventDefault();
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const point = scrollbarAxis === "vertical"
      ? event.clientY - rect.top
      : event.clientX - rect.left;
    setScrollOffset(
      scrollbarAxis,
      appScrollbarOffsetFromTrackPoint(geometryFor(scrollbarAxis), point),
    );
    reveal();
  }

  function moveThumb(scrollbarAxis: ScrollbarAxis, event: PointerEvent) {
    if (
      !drag
      || drag.axis !== scrollbarAxis
      || drag.pointerId !== event.pointerId
    ) return;
    event.preventDefault();
    setScrollOffset(
      scrollbarAxis,
      appScrollbarOffsetFromThumbDelta(
        geometryFor(scrollbarAxis),
        drag.scrollStart,
        pointerCoordinate(scrollbarAxis, event) - drag.pointerStart,
      ),
    );
  }

  function endDrag(scrollbarAxis: ScrollbarAxis, event: PointerEvent) {
    if (
      !drag
      || drag.axis !== scrollbarAxis
      || drag.pointerId !== event.pointerId
    ) return;
    drag = null;
    const track = event.currentTarget as HTMLElement;
    if (track.hasPointerCapture(event.pointerId)) {
      track.releasePointerCapture(event.pointerId);
    }
    scheduleHide();
  }

  function enterTrack(scrollbarAxis: ScrollbarAxis) {
    hoveredAxis = scrollbarAxis;
    visible = true;
    clearHideTimer();
  }

  function leaveTrack(scrollbarAxis: ScrollbarAxis) {
    if (hoveredAxis === scrollbarAxis) hoveredAxis = null;
    scheduleHide();
  }

  function focusTrack(scrollbarAxis: ScrollbarAxis) {
    focusedAxis = scrollbarAxis;
    visible = true;
    clearHideTimer();
  }

  function blurTrack(scrollbarAxis: ScrollbarAxis) {
    if (focusedAxis === scrollbarAxis) focusedAxis = null;
    scheduleHide();
  }

  function handleTrackKeydown(
    scrollbarAxis: ScrollbarAxis,
    event: KeyboardEvent,
  ) {
    const geometry = geometryFor(scrollbarAxis);
    const page = scrollbarAxis === "vertical"
      ? snapshot.clientHeight
      : snapshot.clientWidth;
    let next = scrollOffsetFor(scrollbarAxis);
    if (event.key === "ArrowDown" || event.key === "ArrowRight") next += 30;
    else if (event.key === "ArrowUp" || event.key === "ArrowLeft") next -= 30;
    else if (event.key === "PageDown") next += page * 0.9;
    else if (event.key === "PageUp") next -= page * 0.9;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = geometry.maxScrollOffset;
    else return;
    event.preventDefault();
    setScrollOffset(scrollbarAxis, next);
    reveal();
  }

  $effect(() => {
    if (!viewport || !content || typeof ResizeObserver === "undefined") return;
    const resizeObserver = new ResizeObserver(() => measure());
    resizeObserver.observe(viewport);
    resizeObserver.observe(content);
    const mutationObserver = new MutationObserver(() => measure());
    mutationObserver.observe(content, {
      attributes: true,
      childList: true,
      subtree: true,
    });
    measure();
    return () => {
      resizeObserver.disconnect();
      mutationObserver.disconnect();
    };
  });

  $effect(() => {
    resetKey;
    if (!viewport) return;
    viewport.scrollTop = 0;
    viewport.scrollLeft = 0;
    measure(false);
  });

  $effect(() => {
    autoHide;
    visible = !autoHide;
    if (autoHide) scheduleHide();
    else clearHideTimer();
  });

  $effect(() => {
    return () => clearHideTimer();
  });
</script>

<div
  class={`app-scroll-area ${className}`}
  class:axis-vertical={axis === "vertical"}
  class:axis-horizontal={axis === "horizontal"}
  class:axis-both={axis === "both"}
  data-app-scroll-area
  {style}
>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex (zona de scroll trebuie să primească focus pentru navigarea din tastatură) -->
  <div
    bind:this={viewport}
    id={viewportId}
    class="app-scroll-viewport"
    role={viewportRole}
    tabindex={viewportTabindex}
    aria-label={ariaLabel}
    onscroll={handleScroll}
    {onmouseleave}
  >
    <div bind:this={content} class="app-scroll-content">
      {@render children()}
    </div>
  </div>

  {#if allowsVertical && verticalGeometry.overflow}
    <div
      class="app-scrollbar vertical"
      class:visible
      class:hovered={hoveredAxis === "vertical"}
      class:dragging={drag?.axis === "vertical"}
      role="scrollbar"
      tabindex="0"
      aria-controls={viewportId}
      aria-label={t("common-scrollbar-vertical")}
      aria-orientation="vertical"
      aria-valuemin="0"
      aria-valuemax={Math.round(verticalGeometry.maxScrollOffset)}
      aria-valuenow={Math.round(snapshot.scrollTop)}
      style={`--app-scrollbar-inset:${inset}px`}
      onpointerenter={() => enterTrack("vertical")}
      onpointerleave={() => leaveTrack("vertical")}
      onpointerdown={(event) => handleTrackPointerDown("vertical", event)}
      onpointermove={(event) => moveThumb("vertical", event)}
      onpointerup={(event) => endDrag("vertical", event)}
      onpointercancel={(event) => endDrag("vertical", event)}
      onfocus={() => focusTrack("vertical")}
      onblur={() => blurTrack("vertical")}
      onkeydown={(event) => handleTrackKeydown("vertical", event)}
    >
      <div
        class="app-scrollbar-thumb"
        style={`height:${verticalGeometry.thumbSize}px;transform:translateY(${verticalGeometry.thumbOffset}px)`}
      ></div>
    </div>
  {/if}

  {#if allowsHorizontal && horizontalGeometry.overflow}
    <div
      class="app-scrollbar horizontal"
      class:visible
      class:hovered={hoveredAxis === "horizontal"}
      class:dragging={drag?.axis === "horizontal"}
      role="scrollbar"
      tabindex="0"
      aria-controls={viewportId}
      aria-label={t("common-scrollbar-horizontal")}
      aria-orientation="horizontal"
      aria-valuemin="0"
      aria-valuemax={Math.round(horizontalGeometry.maxScrollOffset)}
      aria-valuenow={Math.round(snapshot.scrollLeft)}
      style={`--app-scrollbar-inset:${inset}px`}
      onpointerenter={() => enterTrack("horizontal")}
      onpointerleave={() => leaveTrack("horizontal")}
      onpointerdown={(event) => handleTrackPointerDown("horizontal", event)}
      onpointermove={(event) => moveThumb("horizontal", event)}
      onpointerup={(event) => endDrag("horizontal", event)}
      onpointercancel={(event) => endDrag("horizontal", event)}
      onfocus={() => focusTrack("horizontal")}
      onblur={() => blurTrack("horizontal")}
      onkeydown={(event) => handleTrackKeydown("horizontal", event)}
    >
      <div
        class="app-scrollbar-thumb"
        style={`width:${horizontalGeometry.thumbSize}px;transform:translateX(${horizontalGeometry.thumbOffset}px)`}
      ></div>
    </div>
  {/if}
</div>

<style>
  .app-scroll-area {
    position: relative;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .app-scroll-viewport {
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    overflow: auto;
    scrollbar-width: none;
    outline: none;
    overscroll-behavior: contain;
  }

  .axis-vertical .app-scroll-viewport { overflow-x: hidden; }
  .axis-horizontal .app-scroll-viewport { overflow-y: hidden; }

  .app-scroll-viewport::-webkit-scrollbar {
    display: none;
    width: 0;
    height: 0;
  }

  .app-scroll-content {
    min-width: 100%;
    min-height: 100%;
    box-sizing: border-box;
    padding-inline-end: var(--app-scroll-content-inset, 0px);
  }

  .axis-horizontal .app-scroll-content,
  .axis-both .app-scroll-content {
    width: max-content;
  }

  .app-scrollbar {
    --app-scrollbar-inset: 2px;
    position: absolute;
    z-index: 20;
    border-radius: 999px;
    opacity: 0;
    outline: none;
    touch-action: none;
    transition: opacity var(--app-scrollbar-fade-duration) ease;
  }

  .app-scrollbar.vertical {
    top: var(--app-scrollbar-inset);
    right: 0;
    bottom: var(--app-scrollbar-inset);
    width: var(--app-scrollbar-hit-size);
  }

  .app-scrollbar.horizontal {
    right: var(--app-scrollbar-inset);
    bottom: 0;
    left: var(--app-scrollbar-inset);
    height: var(--app-scrollbar-hit-size);
  }

  .app-scrollbar.visible,
  .app-scrollbar:hover,
  .app-scrollbar:focus-visible,
  .app-scrollbar.dragging {
    opacity: 1;
  }

  .app-scrollbar:focus-visible {
    box-shadow: 0 0 0 1px var(--app-scrollbar-focus);
  }

  .app-scrollbar-thumb {
    position: absolute;
    border-radius: 999px;
    background: var(--app-scrollbar-thumb);
    box-shadow: inset 0 0 0 1px var(--app-scrollbar-thumb-edge);
    cursor: grab;
    transition:
      width var(--app-scrollbar-expand-duration) ease,
      height var(--app-scrollbar-expand-duration) ease,
      background var(--app-scrollbar-expand-duration) ease;
  }

  .vertical .app-scrollbar-thumb {
    top: 0;
    right: var(--app-scrollbar-edge-gap);
    width: var(--app-scrollbar-indicator-size);
  }

  .horizontal .app-scrollbar-thumb {
    bottom: var(--app-scrollbar-edge-gap);
    left: 0;
    height: var(--app-scrollbar-indicator-size);
  }

  .app-scrollbar.hovered .app-scrollbar-thumb,
  .app-scrollbar:focus-visible .app-scrollbar-thumb,
  .app-scrollbar.dragging .app-scrollbar-thumb {
    background: var(--app-scrollbar-thumb-active);
  }

  .vertical.hovered .app-scrollbar-thumb,
  .vertical:focus-visible .app-scrollbar-thumb,
  .vertical.dragging .app-scrollbar-thumb {
    width: var(--app-scrollbar-slider-size);
  }

  .horizontal.hovered .app-scrollbar-thumb,
  .horizontal:focus-visible .app-scrollbar-thumb,
  .horizontal.dragging .app-scrollbar-thumb {
    height: var(--app-scrollbar-slider-size);
  }

  .app-scrollbar.dragging .app-scrollbar-thumb {
    cursor: grabbing;
  }
</style>
