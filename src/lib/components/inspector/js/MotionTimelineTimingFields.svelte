<script lang="ts">
  import type { MotionTimelineClip } from "$lib/js/motion-timeline";
  import {
    MOTION_TIMELINE_MIN_DURATION_MS,
    formatMotionTimelinePosition,
    quantizeMotionMs,
  } from "$lib/js/motion-timeline";
  import type { MotionTimelineTimingPatch } from "$lib/js/motion-timeline-interaction";

  let {
    clip = null,
    onTimingChange = undefined as ((stepIndex: number, patch: MotionTimelineTimingPatch) => void) | undefined,
  }: {
    clip?: MotionTimelineClip | null;
    onTimingChange?: (stepIndex: number, patch: MotionTimelineTimingPatch) => void;
  } = $props();

  function numberFromInput(value: string, fallback: number): number {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : fallback;
  }

  function commitStart(value: string) {
    if (!clip || !onTimingChange) return;
    const nextStart = quantizeMotionMs(Math.max(0, numberFromInput(value, clip.startMs)));
    onTimingChange(clip.stepIndex, { position: formatMotionTimelinePosition(nextStart) });
  }

  function commitDuration(value: string) {
    if (!clip || !onTimingChange) return;
    const nextDuration = Math.max(
      MOTION_TIMELINE_MIN_DURATION_MS,
      quantizeMotionMs(numberFromInput(value, clip.durationMs)),
    );
    onTimingChange(clip.stepIndex, { duration: nextDuration });
  }

  function preventNativeTimingDrag(event: DragEvent) {
    event.preventDefault();
    event.stopPropagation();
  }
</script>

{#if clip}
  <div
    class="timing-edit-row"
    role="group"
    aria-label="Timing clip selectat"
    draggable="false"
    ondragstart={preventNativeTimingDrag}
  >
    <span draggable="false">{clip.label}</span>
    <label draggable="false">
      Delay
      <input
        type="number"
        draggable="false"
        min="0"
        step="50"
        value={clip.startMs}
        onchange={(event) => commitStart(event.currentTarget.value)}
      />
    </label>
    <label draggable="false">
      Durată
      <input
        type="number"
        draggable="false"
        min={MOTION_TIMELINE_MIN_DURATION_MS}
        step="50"
        value={clip.durationMs}
        onchange={(event) => commitDuration(event.currentTarget.value)}
      />
    </label>
  </div>
{/if}

<style>
  .timing-edit-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) repeat(2, minmax(104px, 132px));
    align-items: center;
    gap: 8px;
    min-width: 0;
    padding: 6px 8px;
    border: 1px solid var(--border-3);
    border-radius: 7px;
    background: color-mix(in srgb, var(--surface-4) 82%, transparent);
    -webkit-user-drag: none;
  }

  .timing-edit-row * {
    -webkit-user-drag: none;
  }

  .timing-edit-row > span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text);
    font-size: 10px;
    font-weight: 900;
    user-select: none;
    -webkit-user-select: none;
  }

  .timing-edit-row label {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: center;
    gap: 6px;
    color: var(--text-muted);
    font-size: 10px;
    font-weight: 800;
    user-select: none;
    -webkit-user-select: none;
  }

  .timing-edit-row input {
    width: 100%;
    min-width: 0;
    height: 24px;
    border: 1px solid var(--border-4);
    border-radius: 5px;
    background: var(--surface-2);
    color: var(--text);
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    padding: 0 6px;
    user-select: text;
    -webkit-user-select: text;
  }
</style>
