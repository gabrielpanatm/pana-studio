<script lang="ts">
  import MotionTimelineCanvas from "$lib/components/inspector/js/MotionTimelineCanvas.svelte";
  import MotionTimelineControls from "$lib/components/inspector/js/MotionTimelineControls.svelte";
  import MotionTimelineTimingFields from "$lib/components/inspector/js/MotionTimelineTimingFields.svelte";
  import { buildMotionTimelineComposer } from "$lib/js/motion-timeline";
  import type { PanaMotionAnimationItem, PanaMotionItem, PanaMotionTimelineItem } from "$lib/types";

  let {
    timelineItem = null,
    motionItems = [],
    selectedStepId = null,
    emptyMessage = "Niciun clip",
    onSelectStep = undefined,
    onTimingChange = undefined,
    onAddAnimationStep = undefined,
    onAddCallbackStep = undefined,
    onAddTimerStep = undefined,
    onAddSyncStep = undefined,
    onAddSetStep = undefined,
    onAddLabel = undefined,
    onPreviewSeek = undefined,
    onPreviewPlay = undefined,
    onPreviewPause = undefined,
  }: {
    timelineItem?: PanaMotionTimelineItem | null;
    motionItems?: PanaMotionItem[];
    selectedStepId?: string | null;
    emptyMessage?: string;
    onSelectStep?: (stepId: string) => void;
    onTimingChange?: (stepIndex: number, patch: { position?: string; duration?: number }) => void;
    onAddAnimationStep?: (animationId: string) => void;
    onAddCallbackStep?: (position: string) => void;
    onAddTimerStep?: (position: string) => void;
    onAddSyncStep?: (position: string) => void;
    onAddSetStep?: (position: string) => void;
    onAddLabel?: (position: string) => void;
    onPreviewSeek?: (timeMs: number, maxMs: number) => void;
    onPreviewPlay?: (timeMs: number, maxMs: number) => void;
    onPreviewPause?: () => void;
  } = $props();

  let playheadMs = $state(0);
  let playing = $state(false);
  let playbackFrame: number | null = null;
  let seekFrame: number | null = null;
  let pendingSeekMs = 0;
  let playbackStartedAt = 0;
  let playbackStartMs = 0;

  const timeline = $derived(buildMotionTimelineComposer(timelineItem, motionItems));
  const animationItems = $derived(motionItems.filter((item): item is PanaMotionAnimationItem => item.type === "animation"));
  const animationOptions = $derived(animationItems.map((animation) => ({
    value: animation.id,
    label: animation.name,
    detail: animation.target.dataAnim || animation.target.selector || "",
  })));
  const selectedClip = $derived.by(() => {
    if (!selectedStepId) return null;
    for (const lane of timeline.lanes) {
      const clip = lane.clips.find((entry) => entry.id === selectedStepId);
      if (clip) return clip;
    }
    return null;
  });

  $effect(() => {
    if (playheadMs <= timeline.maxMs) return;
    playheadMs = timeline.maxMs;
    onPreviewSeek?.(playheadMs, timeline.maxMs);
  });

  $effect(() => {
    if ((timelineItem?.steps.length ?? 0) > 0) return;
    stopPlayback(false);
  });

  $effect(() => {
    return () => {
      stopPlayback(false);
      if (seekFrame !== null) {
        window.cancelAnimationFrame(seekFrame);
        seekFrame = null;
      }
    };
  });

  function stopPlayback(syncPreview = true) {
    const wasPlaying = playing || playbackFrame !== null;
    if (playbackFrame !== null) {
      window.cancelAnimationFrame(playbackFrame);
      playbackFrame = null;
    }
    playing = false;
    if (syncPreview && wasPlaying) onPreviewPause?.();
  }

  function updatePlayhead(value: number, syncPreview = true) {
    playheadMs = Math.max(0, Math.min(timeline.maxMs, Math.round(value)));
    if (syncPreview) onPreviewSeek?.(playheadMs, timeline.maxMs);
  }

  function playbackTick(now: number) {
    const next = Math.min(timeline.maxMs, playbackStartMs + now - playbackStartedAt);
    updatePlayhead(next, false);
    if (next >= timeline.maxMs) {
      stopPlayback(false);
      onPreviewSeek?.(timeline.maxMs, timeline.maxMs);
      return;
    }
    playbackFrame = window.requestAnimationFrame(playbackTick);
  }

  function togglePlayback() {
    if (playing) {
      stopPlayback(true);
      return;
    }
    if (!(timelineItem?.steps.length ?? 0) || !onPreviewPlay) return;
    const start = playheadMs >= timeline.maxMs ? 0 : playheadMs;
    updatePlayhead(start, false);
    playbackStartMs = start;
    playbackStartedAt = performance.now();
    playing = true;
    onPreviewPlay(start, timeline.maxMs);
    playbackFrame = window.requestAnimationFrame(playbackTick);
  }

  function handlePlayheadInput(value: string) {
    stopPlayback(true);
    updatePlayhead(parseInt(value, 10) || 0, false);
    pendingSeekMs = playheadMs;
    if (seekFrame !== null) return;
    seekFrame = window.requestAnimationFrame(() => {
      seekFrame = null;
      onPreviewSeek?.(pendingSeekMs, timeline.maxMs);
    });
  }

  function preventNativeTimelineDrag(event: DragEvent) {
    event.preventDefault();
    event.stopPropagation();
  }

</script>

<section
  class="motion-panel"
  aria-label="Motion timeline"
  draggable="false"
  ondragstart={preventNativeTimelineDrag}
>
  <MotionTimelineControls
    maxMs={timeline.maxMs}
    {playheadMs}
    {playing}
    hasSteps={(timelineItem?.steps.length ?? 0) > 0}
    {animationOptions}
    previewPlayable={Boolean(onPreviewPlay)}
    onTogglePlayback={togglePlayback}
    onPlayheadInput={handlePlayheadInput}
    {onAddAnimationStep}
    {onAddCallbackStep}
    {onAddTimerStep}
    {onAddSyncStep}
    {onAddSetStep}
    {onAddLabel}
  />

  <MotionTimelineTimingFields clip={selectedClip} {onTimingChange} />

  <MotionTimelineCanvas
    {timeline}
    {playheadMs}
    {selectedStepId}
    {emptyMessage}
    {onSelectStep}
    {onTimingChange}
  />

</section>

<style>
  .motion-panel {
    --motion-accent: color-mix(in srgb, var(--brand-strong) 62%, #7c3aed);
    --motion-track-bg: color-mix(in srgb, var(--surface-7) 90%, var(--brand-soft));
    --motion-grid: color-mix(in srgb, var(--text) 9%, transparent);
    --motion-grid-major: color-mix(in srgb, var(--text) 24%, transparent);
    --motion-grid-soft: color-mix(in srgb, var(--text) 5%, transparent);
    --motion-track-border: color-mix(in srgb, var(--border-3) 82%, transparent);
    --motion-clip-bg: color-mix(in srgb, var(--motion-accent) 24%, var(--surface-5));
    --motion-clip-bg-selected: color-mix(in srgb, var(--motion-accent) 36%, var(--surface-5));
    --motion-clip-text: var(--text-strong);
    --timeline-lane-label-width: 112px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    height: 100%;
    min-height: 0;
    padding: 10px 12px 12px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: color-mix(in srgb, var(--surface-2) 84%, var(--brand-soft));
    min-width: 0;
    overflow: hidden;
    box-shadow: var(--shadow);
    -webkit-user-drag: none;
  }

  :global(.motion-panel *) {
    -webkit-user-drag: none;
  }

</style>
