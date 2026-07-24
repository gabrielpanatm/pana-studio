<script lang="ts" module>
  let nextPickerInstanceId = 0;
</script>

<script lang="ts">
  import { onMount, tick } from "svelte";
  import Color from "colorjs.io";
  import { IconColorPicker, IconCopy, IconX } from "@tabler/icons-svelte";
  import { registerEditFlushHandler } from "$lib/session/edit-flush-registry";
  import {
    ColorPickerEditSession,
    inferPickerColorSpace,
    isPickerColorValue,
  } from "$lib/inspector/color-picker-model";

  type PickerFormat = "hex" | "rgb" | "hsl" | "oklch" | "display-p3";
  type EyeDropperConstructor = new () => {
    open: () => Promise<{ sRGBHex: string }>;
  };

  let {
    value = "#000000",
    size = 26,
    width = size,
    height = size,
    joined = false,
    empty = false,
    disabled = false,
    label = "Alege culoarea",
    oninput,
    oncommit,
    oncancel,
    onopenchange,
  }: {
    value?: string;
    size?: number;
    width?: number;
    height?: number;
    joined?: boolean;
    empty?: boolean;
    disabled?: boolean;
    label?: string;
    oninput?: (value: string) => void;
    oncommit?: (value: string) => void;
    oncancel?: (restoredValue: string) => void;
    onopenchange?: (open: boolean) => void;
  } = $props();

  const pickerInstanceId = nextPickerInstanceId++;
  let trigger = $state<HTMLButtonElement | null>(null);
  let panel = $state<HTMLDivElement | null>(null);
  let isOpen = $state(false);
  let panelReady = $state(false);
  let panelLeft = $state(8);
  let panelTop = $state(8);
  let hue = $state(0);
  let saturation = $state(100);
  let brightness = $state(100);
  let alpha = $state(1);
  let format = $state<PickerFormat>("hex");
  let draggingArea = false;
  let session: ColorPickerEditSession | null = null;
  let canUseEyeDropper = $state(false);

  const safeValue = $derived(isPickerColorValue(value) ? value.trim() : "#000000");
  const currentColor = $derived.by(() => new Color("hsv", [hue, saturation, brightness], alpha));
  const serializedValue = $derived.by(() => serializeColor(currentColor, format));
  const rgbChannels = $derived.by(() => {
    const coords = currentColor.to("srgb").coords;
    return coords.map((channel) => clamp((channel ?? 0) * 255, 0, 255));
  });
  const swatchValue = $derived(isOpen ? serializedValue : safeValue);
  // GTK WebKit used by the Linux Tauri shell still needs the comma syntax
  // when a CSS color is supplied through a custom property.
  const hueColor = $derived(`hsl(${hue}, 100%, 50%)`);
  const opaqueColor = $derived(new Color("hsv", [hue, saturation, brightness], 1).to("srgb").toString({ format: "rgb" }));

  onMount(() => {
    syncFromValue(safeValue);
    canUseEyeDropper = "EyeDropper" in window;
    const unregisterEditFlushHandler = registerEditFlushHandler(
      `color-picker:${pickerInstanceId}`,
      () => {
        if (isOpen) closePicker(true);
      },
    );

    const handlePointerDown = (event: PointerEvent) => {
      if (!isOpen || !(event.target instanceof Node)) return;
      if (panel?.contains(event.target) || trigger?.contains(event.target)) return;
      closePicker(true);
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (!isOpen || event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      cancelPicker();
    };
    const reposition = () => {
      if (isOpen) positionPanel();
    };

    window.addEventListener("pointerdown", handlePointerDown, true);
    window.addEventListener("keydown", handleKeyDown, true);
    window.addEventListener("resize", reposition);
    document.addEventListener("scroll", reposition, true);

    return () => {
      window.removeEventListener("pointerdown", handlePointerDown, true);
      window.removeEventListener("keydown", handleKeyDown, true);
      window.removeEventListener("resize", reposition);
      document.removeEventListener("scroll", reposition, true);
      unregisterEditFlushHandler();
    };
  });

  $effect(() => {
    const externalValue = safeValue;
    if (!isOpen) syncFromValue(externalValue);
  });

  function clamp(value: number, minimum: number, maximum: number) {
    return Math.min(maximum, Math.max(minimum, value));
  }

  function finite(value: number | null, fallback = 0) {
    return typeof value === "number" && Number.isFinite(value) ? value : fallback;
  }

  function normaliseHue(value: number) {
    return ((value % 360) + 360) % 360;
  }

  function formatForValue(candidate: string): PickerFormat {
    switch (inferPickerColorSpace(candidate)) {
      case "hex":
        return "hex";
      case "hsl":
        return "hsl";
      case "oklch":
      case "oklab":
      case "lab":
      case "lch":
        return "oklch";
      case "display-p3":
        return "display-p3";
      default:
        return "rgb";
    }
  }

  function syncFromValue(candidate: string) {
    try {
      const color = new Color(candidate);
      const hsv = color.to("hsv");
      hue = normaliseHue(finite(hsv.coords[0], 0));
      saturation = clamp(finite(hsv.coords[1], 0), 0, 100);
      brightness = clamp(finite(hsv.coords[2], 0), 0, 100);
      alpha = clamp(finite(color.alpha, 1), 0, 1);
      format = formatForValue(candidate);
    } catch {
      hue = 0;
      saturation = 0;
      brightness = 0;
      alpha = 1;
      format = "hex";
    }
  }

  function serializeColor(color: Color, targetFormat: PickerFormat) {
    switch (targetFormat) {
      case "hex":
        return color.to("srgb").toString({ format: "hex" });
      case "hsl":
        return color.to("hsl").toString({ precision: 4 });
      case "oklch":
        return color.to("oklch").toString({ precision: 4 });
      case "display-p3":
        return color.to("p3").toString({ precision: 4 });
      default:
        return color.to("srgb").toString({ format: "rgb", precision: 4 });
    }
  }

  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return {
      destroy() {
        node.remove();
      },
    };
  }

  async function openPicker() {
    if (disabled || isOpen) return;
    syncFromValue(safeValue);
    session = new ColorPickerEditSession(safeValue);
    isOpen = true;
    onopenchange?.(true);
    panelReady = false;
    await tick();
    positionPanel();
    panelReady = true;
    panel?.focus({ preventScroll: true });
  }

  function closePicker(shouldCommit: boolean) {
    if (!isOpen) return;
    if (shouldCommit) commitLatest();
    isOpen = false;
    panelReady = false;
    draggingArea = false;
    session = null;
    onopenchange?.(false);
  }

  function cancelPicker() {
    if (!isOpen) return;
    const restoredValue = session?.cancel() ?? safeValue;
    syncFromValue(restoredValue);
    oncancel?.(restoredValue);
    closePicker(false);
    trigger?.focus({ preventScroll: true });
  }

  function togglePicker() {
    if (isOpen) closePicker(true);
    else void openPicker();
  }

  function positionPanel() {
    if (!trigger || !panel) return;
    const triggerRect = trigger.getBoundingClientRect();
    const panelRect = panel.getBoundingClientRect();
    const viewportPadding = 8;
    const gap = 6;
    const preferredLeft = triggerRect.right - panelRect.width;
    panelLeft = clamp(
      preferredLeft,
      viewportPadding,
      Math.max(viewportPadding, window.innerWidth - panelRect.width - viewportPadding),
    );

    const spaceAbove = triggerRect.top - viewportPadding;
    const placeAbove = spaceAbove >= panelRect.height + gap;
    const preferredTop = placeAbove
      ? triggerRect.top - panelRect.height - gap
      : triggerRect.bottom + gap;
    panelTop = clamp(
      preferredTop,
      viewportPadding,
      Math.max(viewportPadding, window.innerHeight - panelRect.height - viewportPadding),
    );
  }

  function previewCurrent() {
    if (!session) session = new ColorPickerEditSession(safeValue);
    oninput?.(session.preview(serializedValue));
  }

  function commitLatest() {
    const committed = session?.commit();
    if (committed !== null && committed !== undefined) oncommit?.(committed);
  }

  function updateAreaFromPointer(event: PointerEvent) {
    const area = event.currentTarget as HTMLElement;
    const rect = area.getBoundingClientRect();
    saturation = clamp(((event.clientX - rect.left) / rect.width) * 100, 0, 100);
    brightness = clamp(100 - ((event.clientY - rect.top) / rect.height) * 100, 0, 100);
    previewCurrent();
  }

  function handleAreaPointerDown(event: PointerEvent) {
    const area = event.currentTarget as HTMLElement;
    area.setPointerCapture(event.pointerId);
    draggingArea = true;
    updateAreaFromPointer(event);
  }

  function handleAreaPointerMove(event: PointerEvent) {
    if (draggingArea) updateAreaFromPointer(event);
  }

  function handleAreaPointerUp(event: PointerEvent) {
    if (!draggingArea) return;
    draggingArea = false;
    updateAreaFromPointer(event);
  }

  function handleAreaKeyDown(event: KeyboardEvent) {
    const step = event.shiftKey ? 10 : 1;
    if (event.key === "ArrowLeft") saturation = clamp(saturation - step, 0, 100);
    else if (event.key === "ArrowRight") saturation = clamp(saturation + step, 0, 100);
    else if (event.key === "ArrowUp") brightness = clamp(brightness + step, 0, 100);
    else if (event.key === "ArrowDown") brightness = clamp(brightness - step, 0, 100);
    else return;
    event.preventDefault();
    previewCurrent();
  }

  function handleHueInput(event: Event) {
    hue = normaliseHue(Number((event.currentTarget as HTMLInputElement).value));
    previewCurrent();
  }

  function handleAlphaInput(event: Event) {
    alpha = clamp(Number((event.currentTarget as HTMLInputElement).value) / 100, 0, 1);
    previewCurrent();
  }

  function applyRgbChannel(index: number, rawValue: string) {
    const channels = [...rgbChannels];
    channels[index] = clamp(Number(rawValue), 0, 255);
    const color = new Color(
      "srgb",
      [channels[0] / 255, channels[1] / 255, channels[2] / 255],
      alpha,
    ).to("hsv");
    hue = normaliseHue(finite(color.coords[0], hue));
    saturation = clamp(finite(color.coords[1], saturation), 0, 100);
    brightness = clamp(finite(color.coords[2], brightness), 0, 100);
    previewCurrent();
  }

  function applyAlphaPercent(rawValue: string) {
    alpha = clamp(Number(rawValue) / 100, 0, 1);
    previewCurrent();
  }

  function handleFormatChange(event: Event) {
    format = (event.currentTarget as HTMLSelectElement).value as PickerFormat;
    previewCurrent();
  }

  async function copyValue() {
    await navigator.clipboard?.writeText(serializedValue);
  }

  async function pickFromScreen() {
    const EyeDropper = (window as unknown as { EyeDropper?: EyeDropperConstructor }).EyeDropper;
    if (!EyeDropper) return;
    try {
      const result = await new EyeDropper().open();
      syncFromValue(result.sRGBHex);
      previewCurrent();
    } catch {
      // Closing the system eyedropper is an intentional cancel action.
    }
  }
</script>

<span
  class="picker-shell"
  class:joined
  class:empty
  class:disabled
  style:--picker-width={`${width}px`}
  style:--picker-height={`${height}px`}
>
  <button
    bind:this={trigger}
    class="picker-trigger"
    type="button"
    disabled={disabled}
    title={label}
    aria-label={label}
    aria-haspopup="dialog"
    aria-expanded={isOpen}
    onclick={togglePicker}
  >
    <span class="checker" aria-hidden="true">
      <span class="swatch" style:background={empty ? "transparent" : swatchValue}></span>
    </span>
  </button>
</span>

{#if isOpen}
  <div
    use:portal
    bind:this={panel}
    class="color-popover"
    class:ready={panelReady}
    style={`left: ${panelLeft}px; top: ${panelTop}px;`}
    role="dialog"
    aria-label="Editor de culoare"
    tabindex="-1"
  >
    <div class="popover-header">
      <strong>Culoare</strong>
      <button class="icon-button close-button" type="button" aria-label="Închide" onclick={() => closePicker(true)}>
        <IconX size={16} stroke={1.9} />
      </button>
    </div>

    <!-- svelte-ignore a11y_no_static_element_interactions (custom two-axis slider with complete keyboard controls) -->
    <div
      class="color-area"
      style:--area-hue={hueColor}
      role="slider"
      aria-label="Saturație și luminozitate"
      aria-valuemin="0"
      aria-valuemax="100"
      aria-valuenow={Math.round(brightness)}
      aria-valuetext={`${Math.round(saturation)}% saturație, ${Math.round(brightness)}% luminozitate`}
      tabindex="0"
      onpointerdown={handleAreaPointerDown}
      onpointermove={handleAreaPointerMove}
      onpointerup={handleAreaPointerUp}
      onpointercancel={() => { draggingArea = false; }}
      onkeydown={handleAreaKeyDown}
    >
      <span
        class="area-thumb"
        style={`left: ${saturation}%; top: ${100 - brightness}%; background: ${serializedValue};`}
        aria-hidden="true"
      ></span>
    </div>

    <div class="sliders">
      <label class="slider-row">
        <span>H</span>
        <input
          class="hue-slider"
          type="range"
          min="0"
          max="360"
          step="1"
          value={hue}
          aria-label="Nuanță"
          oninput={handleHueInput}
        />
        <output>{Math.round(hue)}°</output>
      </label>
      <label class="slider-row">
        <span>A</span>
        <span class="alpha-track" style:--opaque-color={opaqueColor}>
          <input
            class="alpha-slider"
            type="range"
            min="0"
            max="100"
            step="1"
            value={alpha * 100}
            aria-label="Opacitate"
            oninput={handleAlphaInput}
          />
        </span>
        <output>{Math.round(alpha * 100)}%</output>
      </label>
    </div>

    <div class="channel-grid" aria-label="Canale RGBA">
      {#each ["R", "G", "B"] as channel, index}
        <label>
          <span>{channel}</span>
          <input
            type="number"
            min="0"
            max="255"
            step="1"
            value={Math.round(rgbChannels[index])}
            oninput={(event) => applyRgbChannel(index, event.currentTarget.value)}
          />
        </label>
      {/each}
      <label>
        <span>A</span>
        <input
          type="number"
          min="0"
          max="100"
          step="1"
          value={Math.round(alpha * 100)}
          oninput={(event) => applyAlphaPercent(event.currentTarget.value)}
        />
      </label>
    </div>

    <div class="value-row">
      <select value={format} aria-label="Format culoare" onchange={handleFormatChange}>
        <option value="hex">HEX</option>
        <option value="rgb">RGB</option>
        <option value="hsl">HSL</option>
        <option value="oklch">OKLCH</option>
        <option value="display-p3">P3</option>
      </select>
      <input class="color-value" aria-label="Valoare culoare" value={serializedValue} readonly />
      {#if canUseEyeDropper}
        <button class="icon-button" type="button" aria-label="Preia culoarea de pe ecran" title="Preia de pe ecran" onclick={pickFromScreen}>
          <IconColorPicker size={14} stroke={1.9} />
        </button>
      {/if}
      <button class="icon-button" type="button" aria-label="Copiază valoarea" title="Copiază" onclick={copyValue}>
        <IconCopy size={14} stroke={1.9} />
      </button>
    </div>
  </div>
{/if}

<style>
  .picker-shell {
    display: inline-flex;
    flex: 0 0 auto;
    width: var(--picker-width);
    height: var(--picker-height);
    min-width: 0;
  }

  .picker-shell.disabled {
    opacity: 0.45;
  }

  .picker-trigger {
    box-sizing: border-box;
    width: 100%;
    height: 100%;
    min-width: 0;
    padding: 2px;
    border: 1px solid var(--border-4);
    border-radius: 6px;
    background: var(--surface-5);
    overflow: hidden;
    cursor: pointer;
  }

  .picker-trigger:focus-visible,
  .color-popover :is(button, input, select):focus-visible,
  .color-area:focus-visible {
    outline: 2px solid var(--focus-ring, var(--brand));
    outline-offset: 1px;
  }

  .joined .picker-trigger {
    border: 0;
    border-right: 1px solid var(--border-4);
    border-radius: 5px 0 0 5px;
    background: var(--surface-4);
  }

  .checker,
  .alpha-track {
    background-color: #fff;
    background-image:
      linear-gradient(45deg, #c8cecb 25%, transparent 25%),
      linear-gradient(-45deg, #c8cecb 25%, transparent 25%),
      linear-gradient(45deg, transparent 75%, #c8cecb 75%),
      linear-gradient(-45deg, transparent 75%, #c8cecb 75%);
    background-position: 0 0, 0 4px, 4px -4px, -4px 0;
    background-size: 8px 8px;
  }

  .checker,
  .swatch {
    display: block;
    width: 100%;
    height: 100%;
    border-radius: 3px;
  }

  .color-popover {
    position: fixed;
    z-index: 10020;
    box-sizing: border-box;
    width: min(344px, calc(100vw - 16px));
    padding: 10px;
    border: 1px solid var(--border-4);
    border-radius: 10px;
    background: var(--surface-1);
    color: var(--text);
    box-shadow: var(--shadow-float, var(--shadow));
    font-family: Inter, ui-sans-serif, system-ui, sans-serif;
    font-size: 12px;
    visibility: hidden;
  }

  .color-popover.ready {
    visibility: visible;
  }

  .popover-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 24px;
    margin-bottom: 8px;
  }

  .popover-header strong {
    font-size: 12px;
    font-weight: 650;
  }

  .icon-button {
    display: inline-grid;
    flex: 0 0 28px;
    width: 28px;
    height: 28px;
    place-items: center;
    padding: 0;
    border: 1px solid var(--border-3);
    border-radius: var(--radius-control, 4px);
    background: var(--surface-4);
    color: var(--text-muted);
    font: inherit;
    cursor: pointer;
  }

  .icon-button:hover {
    border-color: var(--border-4);
    background: var(--control-hover);
    color: var(--text);
  }

  .close-button {
    width: 24px;
    height: 24px;
    flex-basis: 24px;
    border-color: transparent;
    background: transparent;
    font-size: 18px;
  }

  .color-area {
    position: relative;
    width: 100%;
    aspect-ratio: 16 / 9;
    border: 1px solid rgb(0 0 0 / 18%);
    border-radius: 6px;
    background:
      linear-gradient(to top, #000, transparent),
      linear-gradient(to right, #fff, transparent),
      var(--area-hue);
    cursor: crosshair;
    touch-action: none;
    user-select: none;
  }

  .area-thumb {
    position: absolute;
    width: 14px;
    height: 14px;
    border: 2px solid #fff;
    border-radius: 50%;
    box-shadow: 0 0 0 1px rgb(0 0 0 / 55%), 0 1px 3px rgb(0 0 0 / 35%);
    transform: translate(-50%, -50%);
    pointer-events: none;
  }

  .sliders {
    display: grid;
    gap: 6px;
    margin-top: 10px;
  }

  .slider-row {
    display: grid;
    grid-template-columns: 12px minmax(0, 1fr) 38px;
    align-items: center;
    gap: 7px;
    color: var(--text-muted);
  }

  .slider-row output {
    color: var(--text);
    font-family: var(--font-mono);
    font-size: 11px;
    text-align: right;
  }

  .slider-row input[type="range"] {
    width: 100%;
    height: 12px;
    margin: 0;
    border-radius: 999px;
    background: transparent;
    appearance: none;
    cursor: pointer;
  }

  .slider-row input[type="range"].hue-slider {
    background: linear-gradient(
      90deg,
      #f00,
      #ff0,
      #0f0,
      #0ff,
      #00f,
      #f0f,
      #f00
    );
  }

  .alpha-track {
    position: relative;
    height: 12px;
    border-radius: 999px;
    overflow: hidden;
  }

  .alpha-track::after {
    position: absolute;
    inset: 0;
    background: linear-gradient(to right, transparent, var(--opaque-color));
    content: "";
    pointer-events: none;
  }

  .alpha-slider {
    position: relative;
    z-index: 1;
  }

  .slider-row input[type="range"]::-webkit-slider-thumb {
    width: 13px;
    height: 13px;
    border: 2px solid #fff;
    border-radius: 50%;
    background: transparent;
    box-shadow: 0 0 0 1px rgb(0 0 0 / 55%);
    appearance: none;
  }

  .slider-row input[type="range"]::-moz-range-thumb {
    width: 10px;
    height: 10px;
    border: 2px solid #fff;
    border-radius: 50%;
    background: transparent;
    box-shadow: 0 0 0 1px rgb(0 0 0 / 55%);
  }

  .channel-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 6px;
    margin-top: 10px;
  }

  .channel-grid label {
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr);
    align-items: center;
    border: 1px solid var(--border-3);
    border-radius: var(--radius-control, 4px);
    background: var(--surface-2);
    overflow: hidden;
  }

  .channel-grid label:focus-within {
    border-color: var(--brand);
  }

  .channel-grid span {
    padding-left: 6px;
    color: var(--text-muted);
    font-size: 11px;
  }

  .channel-grid input,
  .color-value,
  .value-row select {
    min-width: 0;
    height: 28px;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
  }

  .channel-grid input {
    width: 100%;
    padding: 0 4px 0 0;
    text-align: right;
    appearance: textfield;
  }

  .channel-grid input::-webkit-inner-spin-button,
  .channel-grid input::-webkit-outer-spin-button {
    appearance: none;
  }

  .channel-grid input:focus,
  .color-value:focus,
  .value-row select:focus {
    outline: 0;
  }

  .value-row {
    display: grid;
    grid-template-columns: 68px minmax(0, 1fr) auto auto;
    gap: 6px;
    margin-top: 10px;
  }

  .value-row select,
  .color-value {
    box-sizing: border-box;
    height: 30px;
    border: 1px solid var(--border-3);
    border-radius: var(--radius-control, 4px);
    background: var(--surface-2);
  }

  .value-row select {
    padding: 0 7px;
  }

  .color-value {
    padding: 0 8px;
    font-family: var(--font-mono);
    font-size: 11px;
  }

  @media (prefers-reduced-motion: reduce) {
    .color-popover {
      scroll-behavior: auto;
    }
  }
</style>
