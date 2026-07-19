<script lang="ts">
  import { IconRefresh } from "@tabler/icons-svelte";
  import type {
    MoodBoardImageAdjustments,
    MoodBoardImageFraming,
  } from "$lib/mood-board/model";
  import { isDefaultImageAdjustments } from "$lib/mood-board/image-adjustments";
  import { isDefaultImageFraming } from "$lib/mood-board/image-framing";

  export let imageAdjustments: MoodBoardImageAdjustments;
  export let imageFraming: MoodBoardImageFraming;
  export let beginImageEdit: () => void;
  export let updateImageAdjustment: (field: keyof MoodBoardImageAdjustments, value: string) => void;
  export let commitImageAdjustment: (field: keyof MoodBoardImageAdjustments, value: string) => void;
  export let resetImageAdjustments: () => void;
  export let updateImageFraming: (field: keyof MoodBoardImageFraming, value: string) => void;
  export let commitImageFraming: (field: keyof MoodBoardImageFraming, value: string) => void;
  export let resetImageFraming: () => void;
</script>

<div class="image-adjustments-popover" role="group" aria-label="Ajustări imagine" onpointerdown={(event) => event.stopPropagation()}>
  <div class="popover-header">
    <strong>Ajustări imagine</strong>
    <button
      type="button"
      disabled={isDefaultImageAdjustments(imageAdjustments)}
      title="Resetează ajustările"
      onclick={resetImageAdjustments}
    >
      <IconRefresh size={15} stroke={2} />
    </button>
  </div>
  <label class="popover-slider">
    <span>Opacitate</span>
    <input
      type="range"
      min="0"
      max="1"
      step="0.05"
      value={imageAdjustments.opacity}
      aria-label="Opacitate imagine"
      onfocus={beginImageEdit}
      oninput={(event) => updateImageAdjustment("opacity", event.currentTarget.value)}
      onblur={(event) => commitImageAdjustment("opacity", event.currentTarget.value)}
    />
    <em>{Math.round(imageAdjustments.opacity * 100)}%</em>
  </label>
  <label class="popover-slider">
    <span>Luminozitate</span>
    <input
      type="range"
      min="0"
      max="200"
      step="5"
      value={imageAdjustments.brightness}
      aria-label="Luminozitate imagine"
      onfocus={beginImageEdit}
      oninput={(event) => updateImageAdjustment("brightness", event.currentTarget.value)}
      onblur={(event) => commitImageAdjustment("brightness", event.currentTarget.value)}
    />
    <em>{Math.round(imageAdjustments.brightness)}%</em>
  </label>
  <label class="popover-slider">
    <span>Contrast</span>
    <input
      type="range"
      min="0"
      max="200"
      step="5"
      value={imageAdjustments.contrast}
      aria-label="Contrast imagine"
      onfocus={beginImageEdit}
      oninput={(event) => updateImageAdjustment("contrast", event.currentTarget.value)}
      onblur={(event) => commitImageAdjustment("contrast", event.currentTarget.value)}
    />
    <em>{Math.round(imageAdjustments.contrast)}%</em>
  </label>
  <label class="popover-slider">
    <span>Saturație</span>
    <input
      type="range"
      min="0"
      max="250"
      step="5"
      value={imageAdjustments.saturation}
      aria-label="Saturație imagine"
      onfocus={beginImageEdit}
      oninput={(event) => updateImageAdjustment("saturation", event.currentTarget.value)}
      onblur={(event) => commitImageAdjustment("saturation", event.currentTarget.value)}
    />
    <em>{Math.round(imageAdjustments.saturation)}%</em>
  </label>
  <label class="popover-slider">
    <span>Grayscale</span>
    <input
      type="range"
      min="0"
      max="100"
      step="5"
      value={imageAdjustments.grayscale}
      aria-label="Grayscale imagine"
      onfocus={beginImageEdit}
      oninput={(event) => updateImageAdjustment("grayscale", event.currentTarget.value)}
      onblur={(event) => commitImageAdjustment("grayscale", event.currentTarget.value)}
    />
    <em>{Math.round(imageAdjustments.grayscale)}%</em>
  </label>
  <label class="popover-slider">
    <span>Blur</span>
    <input
      type="range"
      min="0"
      max="24"
      step="1"
      value={imageAdjustments.blur}
      aria-label="Blur imagine"
      onfocus={beginImageEdit}
      oninput={(event) => updateImageAdjustment("blur", event.currentTarget.value)}
      onblur={(event) => commitImageAdjustment("blur", event.currentTarget.value)}
    />
    <em>{Math.round(imageAdjustments.blur)}px</em>
  </label>
  <div class="popover-section-header">
    <strong>Încadrare</strong>
    <button
      type="button"
      disabled={isDefaultImageFraming(imageFraming)}
      title="Resetează încadrarea"
      onclick={resetImageFraming}
    >
      <IconRefresh size={15} stroke={2} />
    </button>
  </div>
  <label class="popover-slider">
    <span>Poziție X</span>
    <input
      type="range"
      min="0"
      max="100"
      step="1"
      value={imageFraming.positionX}
      aria-label="Poziție orizontală imagine"
      onfocus={beginImageEdit}
      oninput={(event) => updateImageFraming("positionX", event.currentTarget.value)}
      onblur={(event) => commitImageFraming("positionX", event.currentTarget.value)}
    />
    <em>{Math.round(imageFraming.positionX)}%</em>
  </label>
  <label class="popover-slider">
    <span>Poziție Y</span>
    <input
      type="range"
      min="0"
      max="100"
      step="1"
      value={imageFraming.positionY}
      aria-label="Poziție verticală imagine"
      onfocus={beginImageEdit}
      oninput={(event) => updateImageFraming("positionY", event.currentTarget.value)}
      onblur={(event) => commitImageFraming("positionY", event.currentTarget.value)}
    />
    <em>{Math.round(imageFraming.positionY)}%</em>
  </label>
  <label class="popover-slider">
    <span>Zoom</span>
    <input
      type="range"
      min="100"
      max="300"
      step="5"
      value={imageFraming.scale}
      aria-label="Zoom intern imagine"
      onfocus={beginImageEdit}
      oninput={(event) => updateImageFraming("scale", event.currentTarget.value)}
      onblur={(event) => commitImageFraming("scale", event.currentTarget.value)}
    />
    <em>{Math.round(imageFraming.scale)}%</em>
  </label>
</div>

<style>
  .image-adjustments-popover {
    position: absolute;
    top: calc(100% + 8px);
    left: 50%;
    z-index: 5;
    display: grid;
    gap: 10px;
    width: min(340px, calc(100vw - 64px));
    padding: 12px;
    border: 1px solid var(--border-3);
    border-radius: 10px;
    background: color-mix(in srgb, var(--surface) 97%, transparent);
    box-shadow: 0 18px 42px rgba(0, 0, 0, 0.22);
    backdrop-filter: blur(12px);
    transform: translateX(-50%);
  }

  .popover-header,
  .popover-section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .popover-section-header {
    padding-top: 4px;
    border-top: 1px solid var(--border-3);
  }

  .popover-header strong,
  .popover-section-header strong {
    color: var(--text);
    font-size: 13px;
    font-weight: 850;
  }

  button {
    display: inline-grid;
    place-items: center;
    flex: 0 0 auto;
    min-width: 31px;
    height: 30px;
    padding: 0 8px;
    border: 1px solid transparent;
    border-radius: 7px;
    color: var(--text-muted);
    background: transparent;
    font-size: 12px;
    font-weight: 850;
    white-space: nowrap;
  }

  button:hover {
    color: var(--brand);
    border-color: var(--brand);
    background: var(--brand-soft);
  }

  button:disabled {
    color: color-mix(in srgb, var(--text-muted) 42%, transparent);
    border-color: transparent;
    background: transparent;
    cursor: not-allowed;
  }

  .popover-slider {
    display: grid;
    grid-template-columns: 82px minmax(0, 1fr) 42px;
    align-items: center;
    gap: 10px;
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 800;
  }

  .popover-slider input {
    width: 100%;
    accent-color: var(--brand);
  }

  .popover-slider em {
    color: var(--text);
    font-style: normal;
    text-align: right;
  }
</style>
