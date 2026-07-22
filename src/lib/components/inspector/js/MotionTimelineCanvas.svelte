<script lang="ts">
  import type { MotionTimelineClip, MotionTimelineLane, MotionTimelineModel } from "$lib/js/motion-timeline";
  import {
    beginMotionTimelineClipDrag,
    motionTimelineClipStyle,
    type MotionTimelineDragController,
    type MotionTimelineDragMode,
    type MotionTimelineTiming,
    type MotionTimelineTimingPatch,
  } from "$lib/js/motion-timeline-interaction";

  let {
    timeline,
    playheadMs = 0,
    selectedStepId = null,
    emptyMessage = "Niciun clip",
    onSelectStep = undefined as ((stepId: string) => void) | undefined,
    onTimingChange = undefined as ((stepIndex: number, patch: MotionTimelineTimingPatch) => void) | undefined,
  }: {
    timeline: MotionTimelineModel;
    playheadMs?: number;
    selectedStepId?: string | null;
    emptyMessage?: string;
    onSelectStep?: (stepId: string) => void;
    onTimingChange?: (stepIndex: number, patch: MotionTimelineTimingPatch) => void;
  } = $props();

  let activeDrag: MotionTimelineDragController | null = null;
  let draggingClipId = $state<string | null>(null);
  let dragPreview = $state<{ stepId: string; timing: MotionTimelineTiming } | null>(null);
  let suppressClipClickUntil = 0;

  function laneStyle(lane: MotionTimelineLane): string {
    const height = Math.max(26, lane.collapsed ? 26 : lane.height);
    return `--lane-color:${lane.color || "var(--motion-accent)"};flex-basis:${height}px;min-height:${height}px;`;
  }

  function tickStyle(ms: number): string {
    return `left:${(ms / Math.max(1, timeline.maxMs)) * 100}%;`;
  }

  function clipTiming(clip: MotionTimelineClip): MotionTimelineTiming {
    return dragPreview?.stepId === clip.id
      ? dragPreview.timing
      : { startMs: clip.startMs, durationMs: clip.durationMs };
  }

  function clipStyle(clip: MotionTimelineClip): string {
    return motionTimelineClipStyle(clipTiming(clip), timeline.maxMs, clip.rowIndex);
  }

  function playheadStyle(): string {
    return `left:${(Math.min(timeline.maxMs, playheadMs) / Math.max(1, timeline.maxMs)) * 100}%;`;
  }

  function stopActiveDrag() {
    activeDrag?.cancel();
    activeDrag = null;
    draggingClipId = null;
    dragPreview = null;
  }

  function clipDragModeFromEvent(event: PointerEvent): MotionTimelineDragMode {
    return (event.target as HTMLElement | null)?.closest(".clip-resize-handle") ? "resize" : "move";
  }

  function trackWidthForClip(clipElement: HTMLElement): number {
    const track = clipElement.closest(".lane-track") as HTMLElement | null;
    return track?.getBoundingClientRect().width ?? 1;
  }

  function pointerCaptureTargetForClip(clipElement: HTMLElement): HTMLElement {
    return (clipElement.closest(".timeline-shell") as HTMLElement | null) ?? clipElement;
  }

  function preventNativeClipDrag(event: DragEvent) {
    event.preventDefault();
    event.stopPropagation();
    stopActiveDrag();
  }

  function preventNativeClipSelection(event: Event) {
    event.preventDefault();
    event.stopPropagation();
  }

  function startClipDrag(event: PointerEvent, clip: MotionTimelineClip) {
    if (!onTimingChange || event.button !== 0) return;
    const clipElement = event.currentTarget as HTMLElement;
    stopActiveDrag();
    draggingClipId = clip.id;
    dragPreview = { stepId: clip.id, timing: { startMs: clip.startMs, durationMs: clip.durationMs } };
    activeDrag = beginMotionTimelineClipDrag({
      event,
      clip,
      // Capture belongs to the stable canvas, never to the clip whose left/width
      // are mutated while the native pointer stream is active.
      captureTarget: pointerCaptureTargetForClip(clipElement),
      mode: clipDragModeFromEvent(event),
      maxMs: timeline.maxMs,
      trackWidthPx: trackWidthForClip(clipElement),
      onPreview: (timing) => {
        dragPreview = timing ? { stepId: clip.id, timing } : null;
      },
      onSelect: (stepId) => onSelectStep?.(stepId),
      onCommit: (commit) => {
        suppressClipClickUntil = performance.now() + 250;
        onSelectStep?.(commit.stepId);
        onTimingChange?.(commit.stepIndex, commit.patch);
      },
      onFinish: () => {
        activeDrag = null;
        draggingClipId = null;
        dragPreview = null;
      },
    });
  }

  function handleClipClick(clipId: string) {
    if (performance.now() < suppressClipClickUntil) return;
    onSelectStep?.(clipId);
  }

  $effect(() => {
    return () => {
      stopActiveDrag();
    };
  });
</script>

<div
  class="timeline-shell"
  role="group"
  aria-label="Suprafață de editare a cronologiei"
  draggable="false"
  ondragstart={preventNativeClipDrag}
  onselectstart={preventNativeClipSelection}
>
  <div class="timeline-canvas">
    <div class="timeline-ruler">
      <div class="timeline-ruler-track">
        {#each timeline.ticks as tick (tick.ms)}
          <div class:major={tick.major} class="timeline-tick" style={tickStyle(tick.ms)}>
            {#if tick.major}<span>{tick.label}</span>{/if}
          </div>
        {/each}
        {#each timeline.labels as label (label.id)}
          <div class="timeline-label" style={tickStyle(label.ms)} title={label.name}>
            <span>{label.name}</span>
          </div>
        {/each}
      </div>
    </div>

    {#if timeline.lanes.length === 0}
      <div class="timeline-empty">
        <span>{emptyMessage}</span>
      </div>
    {:else}
      {#each timeline.lanes as lane (lane.id)}
        <div class="timeline-lane" class:collapsed={lane.collapsed} style={laneStyle(lane)}>
          <span class="lane-label">{lane.label}</span>
          <div class="lane-track">
            {#each lane.clips as clip (clip.id)}
              <button
                type="button"
                class="timeline-clip"
                class:selected={clip.id === selectedStepId || clip.id === draggingClipId}
                class:dragging={clip.id === draggingClipId}
                class:scroll={clip.trigger === "scroll"}
                class:click={clip.trigger === "click"}
                class:hovering={clip.trigger === "hover"}
                class:callback={clip.type === "callback"}
                class:timer={clip.type === "timer"}
                class:sync={clip.type === "sync"}
                style={clipStyle(clip)}
                draggable="false"
                onpointerdown={(event) => startClipDrag(event, clip)}
                ondragstart={preventNativeClipDrag}
                onselectstart={preventNativeClipSelection}
                onclick={() => handleClipClick(clip.id)}
                title={clip.id === draggingClipId ? undefined : `${clip.label} · ${clipTiming(clip).durationMs}ms`}
                aria-label={`Editează poziția și durata pentru ${clip.label}`}
              >
                <span class="clip-label" draggable="false">{clip.label}</span>
                <span class="clip-meta" draggable="false">{clip.trigger}</span>
                <span class="clip-resize-handle" draggable="false" aria-hidden="true"></span>
              </button>
            {/each}
          </div>
        </div>
      {/each}
      <div class="timeline-fill" aria-hidden="true"></div>
    {/if}
    <div class="timeline-track-overlay">
      <div class="timeline-playhead" style={playheadStyle()}>
        <span>{(playheadMs / 1000).toFixed(1)}s</span>
      </div>
    </div>
  </div>
</div>

<style>
  .timeline-shell {
    flex: 1;
    min-height: 0;
    overflow-x: hidden;
    overflow-y: auto;
    border: 1px solid var(--motion-track-border);
    border-radius: 7px;
    background: var(--motion-track-bg);
    user-select: none;
    -webkit-user-select: none;
    -webkit-user-drag: none;
  }

  .timeline-shell * {
    user-select: none;
    -webkit-user-select: none;
    -webkit-user-drag: none;
  }

  .timeline-canvas {
    display: flex;
    flex-direction: column;
    position: relative;
    width: 100%;
    min-height: 100%;
    color: var(--text);
  }

  .timeline-fill {
    flex: 1 1 auto;
    min-height: 0;
    background:
      repeating-linear-gradient(
        to right,
        var(--motion-grid-soft) 0,
        var(--motion-grid-soft) 1px,
        transparent 1px,
        transparent 5%
      );
  }

  .timeline-ruler {
    position: relative;
    height: 28px;
    border-bottom: 1px solid var(--motion-grid);
  }

  .timeline-ruler-track {
    position: relative;
    width: 100%;
    height: 100%;
    min-width: 0;
  }

  .timeline-tick {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 1px;
    background: var(--motion-grid);
  }

  .timeline-tick.major {
    background: var(--motion-grid-major);
  }

  .timeline-tick span {
    position: absolute;
    top: 4px;
    left: 4px;
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
    color: var(--text-muted);
  }

  .timeline-label {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    background: color-mix(in srgb, #f59e0b 76%, var(--motion-accent));
    z-index: 2;
  }

  .timeline-label span {
    position: absolute;
    top: 15px;
    left: 4px;
    max-width: 86px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
    font-weight: 800;
    color: color-mix(in srgb, #f59e0b 72%, var(--text));
  }

  .timeline-playhead {
    position: absolute;
    top: 0;
    height: 100%;
    min-height: 100%;
    width: 2px;
    background: var(--motion-accent);
    z-index: 3;
    pointer-events: none;
  }

  .timeline-track-overlay {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    left: 0;
    pointer-events: none;
    z-index: 3;
  }

  .timeline-playhead span {
    position: absolute;
    top: 2px;
    left: -12px;
    padding: 1px 4px;
    border-radius: 4px;
    background: var(--motion-accent);
    color: var(--surface-2);
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
  }

  .timeline-lane {
    flex: 0 0 38px;
    position: relative;
    min-height: 38px;
    border-bottom: 1px solid var(--motion-grid-soft);
  }

  .lane-label {
    position: absolute;
    z-index: 2;
    top: 6px;
    left: 6px;
    max-width: 118px;
    padding: 2px 5px;
    border: 1px solid color-mix(in srgb, var(--lane-color, var(--border-4)) 42%, var(--border-4));
    border-radius: 4px;
    background: color-mix(in srgb, var(--surface-5) 86%, transparent);
    color: color-mix(in srgb, var(--lane-color, var(--text-muted)) 56%, var(--text));
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
    font-weight: 800;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    pointer-events: none;
  }

  .timeline-lane:last-child {
    border-bottom: none;
  }

  .lane-track {
    position: relative;
    height: 100%;
    min-height: 38px;
    background:
      repeating-linear-gradient(
        to right,
        var(--motion-grid-soft) 0,
        var(--motion-grid-soft) 1px,
        transparent 1px,
        transparent 5%
      );
  }

  .timeline-lane.collapsed .lane-track {
    min-height: 26px;
  }

  .timeline-clip {
    position: absolute;
    top: 5px;
    height: 26px;
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 18px;
    border: 1px solid var(--motion-accent);
    border-radius: 5px;
    background: var(--motion-clip-bg);
    color: var(--motion-clip-text);
    cursor: pointer;
    padding: 0 16px 0 8px;
    overflow: hidden;
    user-select: none;
    -webkit-user-select: none;
    -webkit-user-drag: none;
    touch-action: none;
  }

  .timeline-clip * {
    user-select: none;
    -webkit-user-select: none;
    -webkit-user-drag: none;
  }

  .clip-label,
  .clip-meta {
    pointer-events: none;
  }

  :global(body.motion-timeline-dragging) {
    user-select: none;
    -webkit-user-select: none;
  }

  :global(body.motion-timeline-dragging iframe) {
    pointer-events: none;
  }

  .timeline-clip:active,
  .timeline-clip.dragging {
    cursor: grabbing;
  }

  .timeline-clip.dragging {
    z-index: 4;
    will-change: left, width;
  }

  .timeline-clip.selected {
    border-color: color-mix(in srgb, var(--motion-accent) 68%, var(--text-strong));
    background: var(--motion-clip-bg-selected);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--motion-accent) 28%, transparent);
  }

  .timeline-clip.scroll {
    border-color: #10b981;
    background: color-mix(in srgb, #10b981 24%, var(--surface-5));
  }

  .timeline-clip.click {
    border-color: #f59e0b;
    background: color-mix(in srgb, #f59e0b 24%, var(--surface-5));
  }

  .timeline-clip.hovering {
    border-color: #38bdf8;
    background: color-mix(in srgb, #38bdf8 24%, var(--surface-5));
  }

  .timeline-clip.callback {
    border-color: #a855f7;
    background: color-mix(in srgb, #a855f7 24%, var(--surface-5));
  }

  .timeline-clip.timer {
    border-color: #14b8a6;
    background: color-mix(in srgb, #14b8a6 24%, var(--surface-5));
  }

  .timeline-clip.sync {
    border-color: #6366f1;
    background: color-mix(in srgb, #6366f1 24%, var(--surface-5));
  }

  .clip-label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
    font-weight: 800;
  }

  .clip-meta {
    font-size: 12px;
    font-weight: 800;
    opacity: 0.75;
    flex-shrink: 0;
  }

  .clip-resize-handle {
    position: absolute;
    top: 2px;
    right: 2px;
    bottom: 2px;
    width: 12px;
    border-left: 1px solid color-mix(in srgb, var(--motion-accent) 38%, transparent);
    border-radius: 0 4px 4px 0;
    background: color-mix(in srgb, var(--surface-2) 36%, transparent);
    cursor: ew-resize;
    opacity: 0.9;
  }

  .clip-resize-handle::before,
  .clip-resize-handle::after {
    content: "";
    position: absolute;
    top: 5px;
    bottom: 5px;
    width: 2px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--motion-accent) 72%, var(--text));
  }

  .clip-resize-handle::before {
    left: 3px;
  }

  .clip-resize-handle::after {
    right: 3px;
  }

  .timeline-empty {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 76px;
    color: var(--text-muted);
    font-size: 12px;
  }
</style>
