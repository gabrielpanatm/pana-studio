<script lang="ts">
  import { onDestroy } from "svelte";
  import type { WorkbenchSplit } from "$lib/types";

  let {
    orientation,
    ratioBasisPoints,
    onCommit,
  }: {
    orientation: Exclude<WorkbenchSplit, "none">;
    ratioBasisPoints: number;
    onCommit: (ratioBasisPoints: number) => void | Promise<void>;
  } = $props();

  let active = $state(false);
  let layout: HTMLElement | null = null;
  let draftRatio = 5_000;

  const clampedRatio = $derived(Math.min(8_000, Math.max(2_000, ratioBasisPoints)));

  function applyDraft(nextRatio: number) {
    draftRatio = Math.min(8_000, Math.max(2_000, Math.round(nextRatio)));
    layout?.style.setProperty("--wb-split-ratio", `${draftRatio / 100}%`);
  }

  function ratioFromPointer(event: PointerEvent) {
    if (!layout) return draftRatio;
    const bounds = layout.getBoundingClientRect();
    const ratio = orientation === "vertical"
      ? (event.clientX - bounds.left) / bounds.width
      : (event.clientY - bounds.top) / bounds.height;
    return ratio * 10_000;
  }

  function handlePointerMove(event: PointerEvent) {
    if (!active) return;
    applyDraft(ratioFromPointer(event));
  }

  function stopPointerDrag(commit: boolean) {
    if (!active) return;
    active = false;
    document.body.classList.remove("workbench-split-resizing");
    window.removeEventListener("pointermove", handlePointerMove);
    window.removeEventListener("pointerup", handlePointerUp);
    window.removeEventListener("pointercancel", handlePointerCancel);
    if (commit) void onCommit(draftRatio);
  }

  function handlePointerUp() {
    stopPointerDrag(true);
  }

  function handlePointerCancel() {
    layout?.style.setProperty("--wb-split-ratio", `${clampedRatio / 100}%`);
    stopPointerDrag(false);
  }

  function startPointerDrag(event: PointerEvent) {
    if (event.button !== 0) return;
    event.preventDefault();
    layout = event.currentTarget instanceof HTMLElement
      ? event.currentTarget.parentElement
      : null;
    if (!layout) return;
    active = true;
    draftRatio = clampedRatio;
    document.body.classList.add("workbench-split-resizing");
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp, { once: true });
    window.addEventListener("pointercancel", handlePointerCancel, { once: true });
  }

  function handleKeydown(event: KeyboardEvent) {
    const decreasing = orientation === "vertical" ? "ArrowLeft" : "ArrowUp";
    const increasing = orientation === "vertical" ? "ArrowRight" : "ArrowDown";
    let nextRatio: number | null = null;
    if (event.key === decreasing) {
      nextRatio = clampedRatio - (event.shiftKey ? 500 : 250);
    } else if (event.key === increasing) {
      nextRatio = clampedRatio + (event.shiftKey ? 500 : 250);
    } else if (event.key === "Home") {
      nextRatio = 2_000;
    } else if (event.key === "End") {
      nextRatio = 8_000;
    }
    if (nextRatio === null) return;
    event.preventDefault();
    void onCommit(Math.min(8_000, Math.max(2_000, nextRatio)));
  }

  function resetRatio() {
    layout?.style.setProperty("--wb-split-ratio", "50%");
    void onCommit(5_000);
  }

  onDestroy(() => {
    document.body.classList.remove("workbench-split-resizing");
    window.removeEventListener("pointermove", handlePointerMove);
    window.removeEventListener("pointerup", handlePointerUp);
    window.removeEventListener("pointercancel", handlePointerCancel);
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (ARIA separator is focusable and implements pointer plus keyboard resizing) -->
<div
  class:active
  class:vertical={orientation === "vertical"}
  class:horizontal={orientation === "horizontal"}
  class="split-handle"
  role="separator"
  aria-label="Redimensionează suprafețele Vizual și Cod"
  aria-orientation={orientation}
  aria-valuemin="20"
  aria-valuemax="80"
  aria-valuenow={Math.round(clampedRatio / 100)}
  aria-valuetext={`${Math.round(clampedRatio / 100)}% Vizual`}
  tabindex="0"
  onpointerdown={startPointerDrag}
  onkeydown={handleKeydown}
  ondblclick={resetRatio}
></div>

<style>
  .split-handle {
    position: relative;
    z-index: 8;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    padding: 0;
    border: 0;
    border-radius: 0;
    background: var(--wb-border-subtle, var(--border));
    touch-action: none;
  }

  .split-handle.vertical {
    cursor: col-resize;
  }

  .split-handle.horizontal {
    cursor: row-resize;
  }

  .split-handle::after {
    position: absolute;
    inset: 0;
    margin: auto;
    border-radius: 999px;
    background: var(--wb-accent, var(--brand));
    opacity: 0;
    content: "";
    transition: opacity 120ms ease;
  }

  .split-handle.vertical::after {
    width: 3px;
    height: 100%;
  }

  .split-handle.horizontal::after {
    width: 100%;
    height: 3px;
  }

  .split-handle:hover::after,
  .split-handle:focus-visible::after,
  .split-handle.active::after {
    opacity: 1;
  }

  .split-handle:focus-visible {
    outline: 2px solid var(--wb-focus-ring, var(--brand-strong));
    outline-offset: -2px;
  }
</style>
