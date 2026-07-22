<script lang="ts">
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import { emptyExpression } from "$lib/js/motion-config";
  import type { PanaMotionExpression, PanaMotionItem, PanaMotionTimelineStep, PanaMotionTimelineTrack } from "$lib/types";

  let {
    step = null,
    motionItems = [],
    timelineId = "",
    tracks = [],
    blockedTimelineIds = [],
    onChange = undefined as ((step: PanaMotionTimelineStep) => void) | undefined,
    onDelete = undefined as ((stepId: string) => void) | undefined,
  }: {
    step?: PanaMotionTimelineStep | null;
    motionItems?: PanaMotionItem[];
    timelineId?: string;
    tracks?: PanaMotionTimelineTrack[];
    blockedTimelineIds?: string[];
    onChange?: (step: PanaMotionTimelineStep) => void;
    onDelete?: (stepId: string) => void;
  } = $props();

  const targetItems = $derived.by(() => motionItems.filter((item) => {
    if (!step) return item.type !== "timeline";
    if (item.id === timelineId) return false;
    if (step.type === "sync") return !blockedTimelineIds.includes(item.id);
    if (step.type === "timer") return item.type === "timer";
    if (step.type === "set") return item.type === "animation" || item.type === "waapi" || item.type === "animatable";
    if (step.type === "animation") return item.type === "animation";
    return item.type !== "timeline";
  }));
  const typeOptions = [
    { value: "animation", label: "animation" },
    { value: "timer", label: "timer" },
    { value: "callback", label: "callback" },
    { value: "set", label: "set" },
    { value: "sync", label: "sync" },
    { value: "label", label: "reper" },
  ];
  const trackOptions = $derived(tracks.map((track) => ({ value: track.id, label: track.name })));
  const targetOptions = $derived([
    { value: "", label: "fără țintă" },
    ...targetItems.map((item) => ({
      value: item.id,
      label: item.name,
      detail: targetSummary(item),
    })),
  ]);
  const callbackPlaceholder = "(timeline, anime, utils) => { ... }";

  function patch(patchValue: Partial<PanaMotionTimelineStep>) {
    if (!step) return;
    onChange?.({ ...step, ...patchValue });
  }

  function patchCallback(patchValue: Partial<PanaMotionExpression>) {
    if (!step) return;
    patch({ callback: { ...(step.callback ?? emptyExpression("Timeline callback")), ...patchValue } });
  }

  function numberValue(value: string, fallback = 0): number {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? Math.max(0, parsed) : fallback;
  }

  function targetSummary(item: PanaMotionItem): string {
    if (item.type === "animation") return item.target.dataAnim || item.target.selector || item.type;
    if (item.type === "timeline") return "timeline";
    return item.type;
  }

</script>

<aside class="step-inspector" aria-label="Inspector pas cronologie">
  {#if !step}
    <div class="empty-step">
      <strong>Pas</strong>
      <span>Selectează un clip din cronologie.</span>
    </div>
  {:else}
    <div class="step-head">
      <div>
        <span>Pas</span>
        <strong>{step.label || step.type}</strong>
      </div>
      <button type="button" class="delete-btn" onclick={() => onDelete?.(step.id)}>Șterge</button>
    </div>

    <div class="field-grid">
      <label>
        <span>Tip</span>
        <SelectControl
          value={step.type}
          options={typeOptions}
          ariaLabel="Tip pas"
          onchange={(nextValue) => patch({ type: nextValue as PanaMotionTimelineStep["type"] })}
        />
      </label>
      <label><span>Etichetă</span><input value={step.label} oninput={(event) => patch({ label: event.currentTarget.value })} /></label>
      <label><span>Poziție</span><input class="mono" value={step.position} placeholder="0, 500ms, 1.2s, +=200" oninput={(event) => patch({ position: event.currentTarget.value })} /></label>
      <label><span>Durată</span><input type="number" min="0" step="50" value={step.duration} oninput={(event) => patch({ duration: numberValue(event.currentTarget.value) })} /></label>
      <label>
        <span>Pistă</span>
        {#if tracks.length > 0}
          <SelectControl
            value={step.lane}
            options={trackOptions}
            ariaLabel="Pista pasului"
            onchange={(nextValue) => patch({ lane: nextValue })}
          />
        {:else}
          <input value={step.lane} oninput={(event) => patch({ lane: event.currentTarget.value })} />
        {/if}
      </label>
      <label>
        <span>Țintă</span>
        <SelectControl
          value={step.targetItemId}
          options={targetOptions}
          ariaLabel="Ținta pasului"
          disabled={step.type === "callback" || step.type === "label"}
          onchange={(nextValue) => patch({ targetItemId: nextValue })}
        />
      </label>
    </div>

    <div class="toggle-row">
      <button type="button" class:active={step.callback.enabled} onclick={() => patchCallback({ enabled: !step.callback.enabled })}>callback</button>
    </div>

    {#if step.callback.enabled || step.type === "callback"}
      <label>
        <span>Callback</span>
        <textarea
          value={step.callback.code}
          placeholder={callbackPlaceholder}
          oninput={(event) => patchCallback({ code: event.currentTarget.value, enabled: event.currentTarget.value.trim().length > 0 || step.type === "callback" })}
        ></textarea>
      </label>
    {/if}
  {/if}

</aside>

<style>
  .step-inspector {
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 280px;
    min-width: 240px;
    height: 100%;
    min-height: 0;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: color-mix(in srgb, var(--surface-2) 88%, var(--brand-soft));
    box-shadow: var(--shadow);
    overflow: auto;
  }

  .empty-step {
    display: flex;
    flex: 1;
    min-height: 0;
    align-items: center;
    justify-content: center;
    flex-direction: column;
    gap: 4px;
    color: var(--text-muted);
    text-align: center;
    font-size: 12px;
  }

  .empty-step strong {
    color: var(--text);
    font-size: 13px;
  }

  .step-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .step-head span,
  label span {
    display: block;
    font-size: 12px;
    font-weight: 900;
    letter-spacing: 0.07em;
    color: var(--text-muted);
    text-transform: uppercase;
  }

  .step-head strong {
    display: block;
    max-width: 150px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text);
    font-size: 13px;
  }

  .field-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 6px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  input,
  textarea,
  button {
    border: 1px solid var(--border-4);
    border-radius: 5px;
    background: var(--surface-5);
    color: var(--text);
    font-size: 12px;
  }

  input,
  button {
    min-height: 25px;
  }

  input,
  textarea {
    width: 100%;
    min-width: 0;
    padding: 0 6px;
  }

  textarea {
    min-height: 72px;
    padding-block: 6px;
    resize: vertical;
  }

  button {
    font-size: 12px;
    font-weight: 800;
    cursor: pointer;
  }

  .delete-btn {
    color: var(--danger);
  }

  .toggle-row {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 5px;
  }

  button.active {
    border-color: var(--brand);
    background: var(--brand-soft);
    color: var(--brand-strong);
  }

  .mono {
    font-family: "JetBrains Mono", monospace;
  }
</style>
