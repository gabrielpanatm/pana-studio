<script lang="ts">
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import { ANIME_EASING_OPTIONS } from "$lib/js/anime-catalog";
  import type { PanaMotionPlayback, PanaMotionTimelineItem, PanaMotionTimelineTrack } from "$lib/types";

  let {
    timeline,
    onChange = undefined as ((timeline: PanaMotionTimelineItem) => void) | undefined,
    onAddTrack = undefined as (() => void) | undefined,
    onTrackChange = undefined as ((track: PanaMotionTimelineTrack) => void) | undefined,
    onDeleteTrack = undefined as ((trackId: string) => void) | undefined,
  }: {
    timeline: PanaMotionTimelineItem;
    onChange?: (timeline: PanaMotionTimelineItem) => void;
    onAddTrack?: () => void;
    onTrackChange?: (track: PanaMotionTimelineTrack) => void;
    onDeleteTrack?: (trackId: string) => void;
  } = $props();

  const booleanOptions = [
    { value: "yes", label: "da" },
    { value: "no", label: "nu" },
  ];
  const easingOptions = [{ value: "", label: "default" }, ...ANIME_EASING_OPTIONS];

  function numberValue(value: string, fallback = 0): number {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : fallback;
  }

  function patch(patchValue: Partial<PanaMotionTimelineItem>) {
    onChange?.({ ...timeline, ...patchValue });
  }

  function patchPlayback(patchValue: Partial<PanaMotionPlayback>) {
    patch({ playback: { ...timeline.playback, ...patchValue } });
  }

  function patchTrack(track: PanaMotionTimelineTrack, patchValue: Partial<PanaMotionTimelineTrack>) {
    onTrackChange?.({ ...track, ...patchValue });
  }

  function patchLabel(labelId: string, patchValue: Partial<{ name: string; position: string }>) {
    patch({
      labels: timeline.labels.map((label) => label.id === labelId ? { ...label, ...patchValue } : label),
    });
  }

  function deleteLabel(labelId: string) {
    patch({ labels: timeline.labels.filter((label) => label.id !== labelId) });
  }
</script>

<aside class="timeline-root-inspector" aria-label="Setări cronologie">
  <div class="root-head">
    <div>
      <span>Cronologia paginii</span>
      <strong>{timeline.name || "Cronologie"}</strong>
    </div>
  </div>

  <div class="field-grid">
    <label>
      <span>Nume</span>
      <input value={timeline.name} oninput={(event) => patch({ name: event.currentTarget.value })} />
    </label>
    <label>
      <span>Activ</span>
      <SelectControl value={timeline.enabled ? "yes" : "no"} options={booleanOptions} ariaLabel="Cronologie activă" onchange={(value) => patch({ enabled: value === "yes" })} />
    </label>
    <label>
      <span>Durată scenă</span>
      <input type="number" min="100" step="100" value={timeline.duration} oninput={(event) => patch({ duration: Math.max(100, numberValue(event.currentTarget.value, timeline.duration)) })} />
    </label>
    <label>
      <span>Pornire automată</span>
      <SelectControl value={timeline.playback.autoplay ? "yes" : "no"} options={booleanOptions} ariaLabel="Pornire automată cronologie" onchange={(value) => patchPlayback({ autoplay: value === "yes" })} />
    </label>
    <label>
      <span>Repetări</span>
      <input type="number" min="0" step="1" value={timeline.playback.loop} oninput={(event) => patchPlayback({ loop: Math.max(0, numberValue(event.currentTarget.value, timeline.playback.loop)) })} />
    </label>
    <label>
      <span>Întârziere repetare</span>
      <input type="number" min="0" step="50" value={timeline.playback.loopDelay} oninput={(event) => patchPlayback({ loopDelay: Math.max(0, numberValue(event.currentTarget.value, timeline.playback.loopDelay)) })} />
    </label>
    <label>
      <span>Direcție alternată</span>
      <SelectControl value={timeline.playback.alternate ? "yes" : "no"} options={booleanOptions} ariaLabel="Direcție alternată" onchange={(value) => patchPlayback({ alternate: value === "yes" })} />
    </label>
    <label>
      <span>Inversată</span>
      <SelectControl value={timeline.playback.reversed ? "yes" : "no"} options={booleanOptions} ariaLabel="Cronologie inversată" onchange={(value) => patchPlayback({ reversed: value === "yes" })} />
    </label>
    <label>
      <span>Viteză</span>
      <input type="number" min="0.1" step="0.1" value={timeline.playback.playbackRate} oninput={(event) => patchPlayback({ playbackRate: Math.max(0.1, numberValue(event.currentTarget.value, timeline.playback.playbackRate)) })} />
    </label>
    <label>
      <span>Cadre pe secundă</span>
      <input type="number" min="0" step="1" value={timeline.playback.frameRate} oninput={(event) => patchPlayback({ frameRate: Math.max(0, numberValue(event.currentTarget.value, timeline.playback.frameRate)) })} />
    </label>
    <label class="field-full">
      <span>Ease global</span>
      <SelectControl value={timeline.playback.playbackEase} options={easingOptions} ariaLabel="Easing global al cronologiei" onchange={(value) => patchPlayback({ playbackEase: value })} />
    </label>
  </div>

  <div class="toggle-row">
    <button type="button" class:active={timeline.playback.persist} onclick={() => patchPlayback({ persist: !timeline.playback.persist })}>păstrează starea</button>
  </div>

  <div class="subpanel">
    <div class="section-head">
      <strong>Piste</strong>
      <button type="button" onclick={() => onAddTrack?.()} disabled={!onAddTrack}>+ pistă</button>
    </div>
    {#each timeline.tracks as track}
      <div class="track-row">
        <input
          class="track-color"
          type="color"
          value={track.color}
          oninput={(event) => patchTrack(track, { color: event.currentTarget.value })}
          aria-label="Culoare pistă"
        />
        <input
          value={track.name}
          oninput={(event) => patchTrack(track, { name: event.currentTarget.value })}
          aria-label="Nume pistă"
        />
        <button
          type="button"
          class:active={track.collapsed}
          onclick={() => patchTrack(track, { collapsed: !track.collapsed })}
          title={track.collapsed ? "Extinde pista" : "Restrânge pista"}
        >
          {track.collapsed ? "▸" : "▾"}
        </button>
        <button
          type="button"
          class="delete-btn"
          onclick={() => onDeleteTrack?.(track.id)}
          disabled={timeline.tracks.length <= 1 || !onDeleteTrack}
          title="Șterge pista"
        >
          ×
        </button>
      </div>
    {/each}
  </div>

  <div class="subpanel">
    <div class="section-head">
      <strong>Repere</strong>
      <small>{timeline.labels.length}</small>
    </div>
    {#if timeline.labels.length === 0}
      <span class="muted">Adaugă repere de pe rigla cronologiei.</span>
    {:else}
      {#each timeline.labels as label}
        <div class="label-row">
          <input value={label.name} oninput={(event) => patchLabel(label.id, { name: event.currentTarget.value })} aria-label="Nume reper" />
          <input class="mono" value={label.position} oninput={(event) => patchLabel(label.id, { position: event.currentTarget.value })} aria-label="Poziție reper" />
          <button type="button" class="delete-btn" onclick={() => deleteLabel(label.id)} title="Șterge reperul">×</button>
        </div>
      {/each}
    {/if}
  </div>
</aside>

<style>
  .timeline-root-inspector {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }

  .root-head,
  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .root-head span,
  label span {
    display: block;
    font-size: 12px;
    font-weight: 900;
    letter-spacing: 0.07em;
    color: var(--text-muted);
    text-transform: uppercase;
  }

  .root-head strong {
    display: block;
    max-width: 210px;
    overflow: hidden;
    color: var(--text);
    font-size: 13px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .field-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 6px;
  }

  .field-full {
    grid-column: 1 / -1;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  input,
  button {
    min-height: 25px;
    border: 1px solid var(--border-4);
    border-radius: 5px;
    background: var(--surface-5);
    color: var(--text);
    font-size: 12px;
  }

  input {
    width: 100%;
    min-width: 0;
    padding: 0 6px;
  }

  button {
    font-size: 12px;
    font-weight: 800;
    cursor: pointer;
  }

  button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  button.active {
    border-color: var(--brand);
    background: var(--brand-soft);
    color: var(--brand-strong);
  }

  .toggle-row {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 5px;
  }

  .subpanel {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding-top: 6px;
    border-top: 1px solid var(--border);
  }

  .section-head strong {
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text);
  }

  .section-head button {
    padding-inline: 8px;
  }

  .section-head small,
  .muted {
    color: var(--text-muted);
    font-size: 12px;
  }

  .track-row {
    display: grid;
    grid-template-columns: 24px minmax(0, 1fr) 26px 26px;
    gap: 4px;
    align-items: center;
  }

  .track-color {
    width: 24px;
    height: 25px;
    padding: 2px;
  }

  .label-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 74px 26px;
    gap: 4px;
    align-items: center;
  }

  .delete-btn {
    color: var(--danger);
  }

  .mono {
    font-family: "JetBrains Mono", monospace;
  }
</style>
