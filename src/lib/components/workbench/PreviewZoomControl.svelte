<script lang="ts">
  let {
    previewZoom = 100,
    disabled = false,
    setPreviewZoom,
    commitPreviewZoom,
    resetPreviewZoom,
  }: {
    previewZoom?: number;
    disabled?: boolean;
    setPreviewZoom: (value: number) => void;
    commitPreviewZoom: (value: number) => void | Promise<void>;
    resetPreviewZoom: () => void;
  } = $props();

  const zoomProgress = $derived(Math.max(0, Math.min(100, ((previewZoom - 25) / 175) * 100)));
</script>

<div class="preview-zoom-control" aria-label="Zoom previzualizare">
  <button
    type="button"
    class="ui-button compact quiet zoom-reset"
    {disabled}
    onclick={resetPreviewZoom}
  >
    Restabilește
  </button>
  <input
    class="zoom-slider"
    type="range"
    min="25"
    max="200"
    step="5"
    value={previewZoom}
    {disabled}
    style={`--zoom-progress: ${zoomProgress}%`}
    aria-label="Nivel zoom previzualizare"
    title={disabled
      ? "Zoom-ul este disponibil pentru un viewport cu lățime fixă"
      : `Zoom previzualizare ${previewZoom}%`}
    oninput={(event) => setPreviewZoom(Number(event.currentTarget.value))}
    onchange={(event) => { void commitPreviewZoom(Number(event.currentTarget.value)); }}
  />
  <span class="zoom-value">{previewZoom}%</span>
</div>

<style>
  .preview-zoom-control {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    min-width: 0;
    color: var(--wb-text-muted, var(--text-muted));
  }

  .zoom-reset {
    flex: 0 0 auto;
    color: inherit;
    font-weight: 500;
  }

  .zoom-slider {
    width: 120px;
    height: var(--control-height-compact);
    min-height: var(--control-height-compact);
    margin: 0;
    padding: 0;
    appearance: none;
    -webkit-appearance: none;
    background: transparent;
    cursor: pointer;
  }

  .zoom-slider::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: 999px;
    background: linear-gradient(
      to right,
      var(--brand) 0%,
      var(--brand) var(--zoom-progress, 0%),
      var(--border-4) var(--zoom-progress, 0%),
      var(--border-4) 100%
    );
  }

  .zoom-slider::-webkit-slider-thumb {
    width: 12px;
    height: 12px;
    margin-top: -4px;
    border: 1px solid var(--border-4);
    border-radius: 50%;
    appearance: none;
    -webkit-appearance: none;
    background: var(--surface-raised);
  }

  .zoom-slider::-moz-range-track {
    height: 4px;
    border-radius: 999px;
    background: var(--border-4);
  }

  .zoom-slider::-moz-range-progress {
    height: 4px;
    border-radius: 999px;
    background: var(--brand);
  }

  .zoom-slider::-moz-range-thumb {
    width: 12px;
    height: 12px;
    border: 1px solid var(--border-4);
    border-radius: 50%;
    background: var(--surface-raised);
  }

  .zoom-slider:disabled {
    cursor: not-allowed;
  }

  .zoom-value {
    min-width: 38px;
    color: var(--wb-text-primary, var(--text));
    font-variant-numeric: tabular-nums;
    text-align: right;
  }

  @container (max-width: 760px) {
    .zoom-reset {
      display: none;
    }

    .zoom-slider {
      width: 90px;
    }
  }
</style>
