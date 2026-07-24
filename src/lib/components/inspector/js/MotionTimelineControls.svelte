<script lang="ts">
  import { IconPlayerPause, IconPlayerPlay } from "@tabler/icons-svelte";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import type { SelectControlOption } from "$lib/components/ui/SelectControl.svelte";
  import { formatMotionTimelinePosition } from "$lib/js/motion-timeline";

  let {
    maxMs = 0,
    playheadMs = 0,
    playing = false,
    hasSteps = false,
    animationOptions = [] as SelectControlOption[],
    previewPlayable = false,
    onTogglePlayback = undefined as (() => void) | undefined,
    onPlayheadInput = undefined as ((value: string) => void) | undefined,
    onAddAnimationStep = undefined as ((animationId: string) => void) | undefined,
    onAddCallbackStep = undefined as ((position: string) => void) | undefined,
    onAddTimerStep = undefined as ((position: string) => void) | undefined,
    onAddSyncStep = undefined as ((position: string) => void) | undefined,
    onAddSetStep = undefined as ((position: string) => void) | undefined,
    onAddLabel = undefined as ((position: string) => void) | undefined,
  }: {
    maxMs?: number;
    playheadMs?: number;
    playing?: boolean;
    hasSteps?: boolean;
    animationOptions?: SelectControlOption[];
    previewPlayable?: boolean;
    onTogglePlayback?: () => void;
    onPlayheadInput?: (value: string) => void;
    onAddAnimationStep?: (animationId: string) => void;
    onAddCallbackStep?: (position: string) => void;
    onAddTimerStep?: (position: string) => void;
    onAddSyncStep?: (position: string) => void;
    onAddSetStep?: (position: string) => void;
    onAddLabel?: (position: string) => void;
  } = $props();

  let animationToAdd = $state("");

  const timelinePosition = $derived(formatMotionTimelinePosition(playheadMs));

  function addSelectedAnimation() {
    const target = animationToAdd || animationOptions[0]?.value || "";
    if (target) onAddAnimationStep?.(target);
  }
</script>

<div class="motion-control-row">
  <button
    type="button"
    class="motion-icon-btn"
    onclick={onTogglePlayback}
    disabled={!hasSteps || !previewPlayable}
    title={playing ? "Oprește linia" : "Redă linia"}
    aria-label={playing ? "Oprește linia temporală" : "Redă linia temporală"}
  >
    {#if playing}
      <IconPlayerPause size={14} stroke={1.9} />
    {:else}
      <IconPlayerPlay size={14} stroke={1.9} />
    {/if}
  </button>
  <input
    class="motion-playhead-range"
    type="range"
    min="0"
    max={maxMs}
    step="50"
    value={playheadMs}
    disabled={!hasSteps || !previewPlayable}
    title={previewPlayable ? "Poziție în previzualizare" : "Navigarea este disponibilă numai la deschiderea externă"}
    oninput={(event) => onPlayheadInput?.(event.currentTarget.value)}
  />
  <span class="motion-time">{(playheadMs / 1000).toFixed(1)}s / {(maxMs / 1000).toFixed(1)}s</span>
</div>

<div class="composer-actions">
  <SelectControl
    value={animationToAdd}
    options={animationOptions}
    placeholder="Animation"
    disabled={animationOptions.length === 0}
    ariaLabel="Alege animație"
    onchange={(nextValue) => { animationToAdd = nextValue; }}
  />
  <button type="button" onclick={addSelectedAnimation} disabled={animationOptions.length === 0 || !onAddAnimationStep}>+ clip</button>
  <button type="button" onclick={() => onAddTimerStep?.(timelinePosition)} disabled={!onAddTimerStep}>+ timer</button>
  <button type="button" onclick={() => onAddCallbackStep?.(timelinePosition)} disabled={!onAddCallbackStep}>+ callback</button>
  <button type="button" onclick={() => onAddSyncStep?.(timelinePosition)} disabled={!onAddSyncStep}>+ sync</button>
  <button type="button" onclick={() => onAddSetStep?.(timelinePosition)} disabled={!onAddSetStep}>+ set</button>
  <button type="button" onclick={() => onAddLabel?.(timelinePosition)} disabled={!onAddLabel}>+ label</button>
</div>

<style>
  .motion-control-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .motion-icon-btn {
    flex-shrink: 0;
    width: 28px;
    height: 26px;
    border: 1px solid var(--border-4);
    border-radius: 6px;
    background: var(--surface-5);
    color: var(--text);
    font-size: 12px;
    font-weight: 800;
    cursor: pointer;
  }

  .motion-icon-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .composer-actions {
    display: grid;
    grid-template-columns: minmax(160px, 220px) repeat(6, auto) minmax(0, 1fr);
    gap: 6px;
    align-items: center;
  }

  .composer-actions button {
    height: 26px;
    min-width: 0;
    border: 1px solid var(--border-4);
    border-radius: 6px;
    background: var(--surface-5);
    color: var(--text);
    font-size: 12px;
    font-weight: 800;
    padding: 0 8px;
    cursor: pointer;
  }

  .composer-actions button:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .motion-time {
    flex-shrink: 0;
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
    color: var(--text-muted);
    text-align: right;
  }

  .motion-playhead-range {
    width: 100%;
    min-width: 0;
    accent-color: var(--brand);
  }

  .motion-playhead-range:disabled {
    opacity: 0.45;
    cursor: default;
  }
</style>
