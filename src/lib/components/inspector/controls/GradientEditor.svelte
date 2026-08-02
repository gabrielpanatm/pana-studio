<script lang="ts">
  import {
    IconCopy,
    IconPlus,
    IconRepeat,
    IconTrash,
  } from "@tabler/icons-svelte";
  import { t } from "$lib/i18n/runtime.svelte";
  import { listenForExternalReconcileInteractionBarrier } from "$lib/session/external-reconcile-barrier";
  import {
    createDefaultGradient,
    gradientStopVisualPosition,
    parseCssGradient,
    serializeCssGradient,
    splitTopLevelCssList,
    type CssGradient,
    type CssGradientHint,
    type CssGradientKind,
    type CssGradientStop,
  } from "$lib/inspector/background-model";
  import ColorInput from "./ColorInput.svelte";
  import TextWithOptions from "./TextWithOptions.svelte";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";

  let {
    gradient,
    oninput,
    oncommit,
    oncancel,
    onsourceinput,
    onsourcecommit,
  }: {
    gradient: CssGradient;
    oninput: (gradient: CssGradient) => void;
    oncommit: (gradient: CssGradient) => void;
    oncancel: () => void;
    onsourceinput: (source: string) => void;
    onsourcecommit: (source: string) => void;
  } = $props();

  let activeItemId = $state<string | null>(null);
  let ramp = $state<HTMLElement | null>(null);
  let draggingStopId = $state<string | null>(null);
  let lastDraftGradient = $state<CssGradient | null>(null);

  const stops = $derived(gradient.items.filter((item): item is CssGradientStop => item.kind === "stop"));
  const hints = $derived(gradient.items.filter((item): item is CssGradientHint => item.kind === "hint"));
  const activeStop = $derived(stops.find((stop) => stop.id === activeItemId) ?? null);
  const activeHint = $derived(hints.find((hint) => hint.id === activeItemId) ?? null);
  const preview = $derived.by(() => {
    let stopIndex = 0;
    const items = gradient.items.map((item) => {
      if (item.kind === "stop") {
        const position = gradientStopVisualPosition(item, stopIndex, stops.length);
        stopIndex += 1;
        return `${item.color} ${position}%`;
      }
      if (item.kind === "hint") return `${hintVisualPosition(item.position)}%`;
      return item.raw;
    });
    return `linear-gradient(to right, ${items.join(", ")})`;
  });
  const rawValue = $derived(serializeCssGradient(gradient));

  $effect(() => {
    if (activeItemId && gradient.items.some((item) => item.id === activeItemId)) return;
    activeItemId = gradient.items.find((item) => item.kind === "stop")?.id ?? gradient.items[0]?.id ?? null;
  });

  function withRaw(next: CssGradient): CssGradient {
    const raw = serializeCssGradient(next);
    return { ...next, raw };
  }

  function update(next: CssGradient, commit = false) {
    const normalized = withRaw(next);
    if (commit) {
      lastDraftGradient = null;
      oncommit(normalized);
    } else {
      lastDraftGradient = normalized;
      oninput(normalized);
    }
  }

  function patch(patch: Partial<CssGradient>, commit = false) {
    update({ ...(lastDraftGradient ?? gradient), ...patch }, commit);
  }

  function changeKind(kind: CssGradientKind) {
    const replacement = createDefaultGradient(kind);
    const next: CssGradient = {
      ...gradient,
      kind,
      prelude: replacement.prelude,
    };
    update(next, true);
  }

  function patchStop(id: string, patch: Partial<CssGradientStop>, commit = false) {
    const current = lastDraftGradient ?? gradient;
    update({
      ...current,
      items: current.items.map((item) => item.kind === "stop" && item.id === id
        ? { ...item, ...patch }
        : item),
    }, commit);
  }

  function patchHint(id: string, position: string, commit = false) {
    const current = lastDraftGradient ?? gradient;
    update({
      ...current,
      items: current.items.map((item) => item.kind === "hint" && item.id === id
        ? { ...item, position, raw: position }
        : item),
    }, commit);
  }

  function percentageFromClientX(clientX: number) {
    if (!ramp) return 0;
    const bounds = ramp.getBoundingClientRect();
    if (bounds.width <= 0) return 0;
    return Math.round(Math.max(0, Math.min(100, ((clientX - bounds.left) / bounds.width) * 100)));
  }

  function hintVisualPosition(value: string) {
    const normalized = value.trim().toLowerCase();
    const percentage = normalized.match(/^(-?[\d.]+)%$/);
    if (percentage) return Math.max(0, Math.min(100, Number(percentage[1])));
    const angle = normalized.match(/^(-?[\d.]+)(deg|grad|rad|turn)$/);
    if (!angle) return 50;
    const numeric = Number(angle[1]);
    const degrees = angle[2] === "turn" ? numeric * 360
      : angle[2] === "grad" ? numeric * .9
        : angle[2] === "rad" ? numeric * (180 / Math.PI)
          : numeric;
    return Math.max(0, Math.min(100, (degrees / 360) * 100));
  }

  function nearestColor(position: number) {
    if (!stops.length) return "#000000";
    return [...stops]
      .sort((left, right) => Math.abs(gradientStopVisualPosition(left, stops.indexOf(left), stops.length) - position)
        - Math.abs(gradientStopVisualPosition(right, stops.indexOf(right), stops.length) - position))[0].color;
  }

  function addStopAtPosition(position: number) {
    const stop: CssGradientStop = {
      kind: "stop",
      id: `gradient-stop-${crypto.randomUUID()}`,
      color: nearestColor(position),
      positions: [`${position}%`],
      raw: "",
    };
    activeItemId = stop.id;
    update({ ...gradient, items: [...gradient.items, stop] }, true);
  }

  function addStop(event: MouseEvent) {
    addStopAtPosition(percentageFromClientX(event.clientX));
  }

  function duplicateStop(stop: CssGradientStop) {
    const position = Math.min(100, gradientStopVisualPosition(stop, stops.indexOf(stop), stops.length) + 5);
    const duplicate: CssGradientStop = {
      ...stop,
      id: `gradient-stop-${crypto.randomUUID()}`,
      positions: [`${position}%`, ...stop.positions.slice(1)],
    };
    const index = gradient.items.findIndex((item) => item.id === stop.id);
    const items = [...gradient.items];
    items.splice(index + 1, 0, duplicate);
    activeItemId = duplicate.id;
    update({ ...gradient, items }, true);
  }

  function removeItem(id: string) {
    const item = gradient.items.find((candidate) => candidate.id === id);
    if (item?.kind === "stop" && stops.length <= 2) return;
    const items = gradient.items.filter((candidate) => candidate.id !== id);
    activeItemId = items.find((candidate) => candidate.kind === "stop")?.id ?? items[0]?.id ?? null;
    update({ ...gradient, items }, true);
  }

  function addHint() {
    const hint: CssGradientHint = {
      kind: "hint",
      id: `gradient-hint-${crypto.randomUUID()}`,
      position: "50%",
      raw: "50%",
    };
    activeItemId = hint.id;
    update({ ...gradient, items: [...gradient.items.slice(0, 1), hint, ...gradient.items.slice(1)] }, true);
  }

  function moveStopFromKeyboard(event: KeyboardEvent, stop: CssGradientStop) {
    if (event.key === "Delete" || event.key === "Backspace") {
      event.preventDefault();
      removeItem(stop.id);
      return;
    }
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    const increment = event.shiftKey ? 10 : 1;
    const direction = event.key === "ArrowRight" ? 1 : -1;
    const current = gradientStopVisualPosition(stop, stops.indexOf(stop), stops.length);
    patchStop(stop.id, { positions: [`${Math.max(0, Math.min(100, current + increment * direction))}%`, ...stop.positions.slice(1)] }, true);
  }

  function startStopDrag(event: PointerEvent, stop: CssGradientStop) {
    event.preventDefault();
    event.stopPropagation();
    activeItemId = stop.id;
    draggingStopId = stop.id;
    let stopBarrier = () => {};

    const move = (nextEvent: PointerEvent) => {
      const position = percentageFromClientX(nextEvent.clientX);
      patchStop(stop.id, { positions: [`${position}%`, ...stop.positions.slice(1)] });
    };
    const finish = (commit: boolean) => {
      draggingStopId = null;
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", accept);
      window.removeEventListener("pointercancel", cancel);
      stopBarrier();
      if (commit) {
        const finalGradient = lastDraftGradient ?? gradient;
        lastDraftGradient = null;
        oncommit(withRaw(finalGradient));
      } else {
        lastDraftGradient = null;
        oncancel();
      }
    };
    const accept = () => finish(true);
    const cancel = () => finish(false);
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", accept);
    window.addEventListener("pointercancel", cancel);
    stopBarrier = listenForExternalReconcileInteractionBarrier(accept);
  }

  function applyRawSource(source: string, commit: boolean) {
    const parsed = parseCssGradient(source);
    if (parsed) {
      if (commit) oncommit(parsed);
      else oninput(parsed);
      return;
    }
    if (commit) onsourcecommit(source);
    else onsourceinput(source);
  }

  function cancelEdit() {
    lastDraftGradient = null;
    oncancel();
  }

  function splitTopLevelPositions(value: string) {
    const trimmed = value.trim();
    if (!trimmed) return [];
    return (splitTopLevelCssList(trimmed, "space") ?? [trimmed]).slice(0, 2);
  }

  function radialPreludeParts(prelude: string) {
    const match = prelude.match(/^(.*?)(?:\s+at\s+(.+))?$/i);
    const before = match?.[1]?.trim() || "circle";
    const tokens = before.split(/\s+/);
    const shape = tokens.find((token) => token === "circle" || token === "ellipse") ?? "circle";
    const size = tokens.filter((token) => token !== "circle" && token !== "ellipse").join(" ") || "farthest-corner";
    return { shape, size, position: match?.[2]?.trim() || "center" };
  }

  function conicPreludeParts(prelude: string) {
    const match = prelude.match(/^(?:from\s+([^\s]+))?(?:\s*at\s+(.+))?$/i);
    return { angle: match?.[1] || "0deg", position: match?.[2] || "center" };
  }
</script>

<div class="gradient-editor" class:dragging={draggingStopId !== null}>
  <div class="gradient-toolbar">
    <SelectControl
      value={gradient.kind}
      options={[
        { value: "linear", label: t("inspector-background-gradient-linear") },
        { value: "radial", label: t("inspector-background-gradient-radial") },
        { value: "conic", label: t("inspector-background-gradient-conic") },
      ]}
      ariaLabel={t("inspector-background-gradient-type")}
      onchange={(value) => changeKind(value as CssGradientKind)}
    />
    <button
      type="button"
      class="icon-button repeat-button"
      class:active={gradient.repeating}
      aria-pressed={gradient.repeating}
      aria-label={t("inspector-background-gradient-repeating")}
      title={t("inspector-background-gradient-repeating")}
      onclick={() => patch({ repeating: !gradient.repeating }, true)}
    ><IconRepeat size={13} stroke={1.8} /></button>
  </div>

  {#if gradient.kind === "linear"}
    <div class="field-label">{t("inspector-background-gradient-direction")}</div>
    <TextWithOptions
      label="°"
      value={gradient.prelude || "180deg"}
      options={["0deg", "45deg", "90deg", "135deg", "180deg", "225deg", "270deg", "315deg", "to top", "to right", "to bottom", "to left", "to bottom right", "to bottom left"]}
      oninput={(value) => patch({ prelude: value })}
      oncommit={(value) => patch({ prelude: value }, true)}
      oncancel={cancelEdit}
    />
  {:else if gradient.kind === "radial"}
    {@const radial = radialPreludeParts(gradient.prelude)}
    <div class="two-fields">
      <div>
        <div class="field-label">{t("inspector-background-gradient-shape")}</div>
        <SelectControl
          value={radial.shape}
          options={[{ value: "circle", label: t("inspector-background-gradient-circle") }, { value: "ellipse", label: t("inspector-background-gradient-ellipse") }]}
          ariaLabel={t("inspector-background-gradient-shape")}
          onchange={(value) => patch({ prelude: `${value} ${radial.size} at ${radial.position}` }, true)}
        />
      </div>
      <div>
        <div class="field-label">{t("inspector-background-gradient-size")}</div>
        <SelectControl
          value={radial.size}
          options={["closest-side", "closest-corner", "farthest-side", "farthest-corner"].map((value) => ({ value, label: value }))}
          ariaLabel={t("inspector-background-gradient-size")}
          onchange={(value) => patch({ prelude: `${radial.shape} ${value} at ${radial.position}` }, true)}
        />
      </div>
    </div>
    <div class="field-label">{t("inspector-background-gradient-center")}</div>
    <TextWithOptions
      label="P"
      value={radial.position}
      options={["center", "top", "right", "bottom", "left", "top left", "top right", "bottom left", "bottom right", "50% 50%"]}
      oninput={(value) => patch({ prelude: `${radial.shape} ${radial.size} at ${value}` })}
      oncommit={(value) => patch({ prelude: `${radial.shape} ${radial.size} at ${value}` }, true)}
      oncancel={cancelEdit}
    />
  {:else}
    {@const conic = conicPreludeParts(gradient.prelude)}
    <div class="two-fields">
      <div>
        <div class="field-label">{t("inspector-background-gradient-start-angle")}</div>
        <TextWithOptions
          label="°"
          value={conic.angle}
          options={["0deg", "45deg", "90deg", "180deg", "270deg", "0.25turn", "0.5turn"]}
          oninput={(value) => patch({ prelude: `from ${value} at ${conic.position}` })}
          oncommit={(value) => patch({ prelude: `from ${value} at ${conic.position}` }, true)}
          oncancel={cancelEdit}
        />
      </div>
      <div>
        <div class="field-label">{t("inspector-background-gradient-center")}</div>
        <TextWithOptions
          label="P"
          value={conic.position}
          options={["center", "top", "right", "bottom", "left", "50% 50%"]}
          oninput={(value) => patch({ prelude: `from ${conic.angle} at ${value}` })}
          oncommit={(value) => patch({ prelude: `from ${conic.angle} at ${value}` }, true)}
          oncancel={cancelEdit}
        />
      </div>
    </div>
  {/if}

  <div class="ramp-shell">
    <div
      class="gradient-ramp"
      bind:this={ramp}
      role="group"
      aria-label={t("inspector-background-gradient-ramp")}
    >
      <button
        type="button"
        class="ramp-background"
        style:background={preview}
        aria-label={t("inspector-background-gradient-add-stop")}
        onclick={addStop}
      ></button>
      {#each stops as stop, index (stop.id)}
        <button
          type="button"
          class="gradient-stop"
          class:active={activeItemId === stop.id}
          style:left={`${gradientStopVisualPosition(stop, index, stops.length)}%`}
          style:--stop-color={stop.color}
          aria-label={t("inspector-background-gradient-stop-label", { position: stop.positions.join(" ") || t("inspector-auto") })}
          onclick={(event) => { event.stopPropagation(); activeItemId = stop.id; }}
          onpointerdown={(event) => startStopDrag(event, stop)}
          onkeydown={(event) => moveStopFromKeyboard(event, stop)}
        ></button>
      {/each}
      {#each hints as hint (hint.id)}
        {@const match = hint.position.match(/^([\d.]+)%$/)}
        <button
          type="button"
          class="gradient-hint"
          class:active={activeItemId === hint.id}
          style:left={`${match ? Math.max(0, Math.min(100, Number(match[1]))) : 50}%`}
          title={t("inspector-background-gradient-color-hint")}
          aria-label={`${t("inspector-background-gradient-color-hint")} ${hint.position}`}
          onclick={(event) => { event.stopPropagation(); activeItemId = hint.id; }}
        ></button>
      {/each}
    </div>
  </div>

  <div class="ramp-actions">
    <button type="button" class="small-button" onclick={() => addStopAtPosition(50)}>
      <IconPlus size={12} /> {t("inspector-background-gradient-add-stop")}
    </button>
    <button type="button" class="small-button" onclick={addHint}>
      <IconPlus size={12} /> {t("inspector-background-gradient-add-hint")}
    </button>
  </div>

  {#if activeStop}
    <div class="active-stop-editor">
      <div class="field-label">{t("inspector-background-gradient-stop-color")}</div>
      <div class="stop-row">
        <div class="stop-color">
          <ColorInput
            property="gradient-stop"
            value={activeStop.color}
            oninput={(value) => patchStop(activeStop.id, { color: value })}
            oncommit={(value) => patchStop(activeStop.id, { color: value }, true)}
            oncancel={cancelEdit}
          />
        </div>
        <button type="button" class="icon-button" title={t("inspector-duplicate")} aria-label={t("inspector-duplicate")} onclick={() => duplicateStop(activeStop)}><IconCopy size={13} /></button>
        <button type="button" class="icon-button danger" disabled={stops.length <= 2} title={t("inspector-delete")} aria-label={t("inspector-delete")} onclick={() => removeItem(activeStop.id)}><IconTrash size={13} /></button>
      </div>
      <div class="field-label">{t("inspector-background-gradient-stop-position")}</div>
      <TextWithOptions
        label="P"
        value={activeStop.positions.join(" ")}
        placeholder={t("inspector-auto")}
        options={["0%", "25%", "50%", "75%", "100%", "0 20%", "10px", "2rem", "calc(50% - 1rem)"]}
        oninput={(value) => patchStop(activeStop.id, { positions: splitTopLevelPositions(value) })}
        oncommit={(value) => patchStop(activeStop.id, { positions: splitTopLevelPositions(value) }, true)}
        oncancel={cancelEdit}
      />
    </div>
  {:else if activeHint}
    <div class="active-stop-editor">
      <div class="field-label">{t("inspector-background-gradient-color-hint")}</div>
      <div class="stop-row">
        <div class="stop-color">
          <TextWithOptions
            label="P"
            value={activeHint.position}
            options={["25%", "50%", "75%", "1rem", "calc(50% - 1rem)"]}
            oninput={(value) => patchHint(activeHint.id, value)}
            oncommit={(value) => patchHint(activeHint.id, value, true)}
            oncancel={cancelEdit}
          />
        </div>
        <button type="button" class="icon-button danger" title={t("inspector-delete")} aria-label={t("inspector-delete")} onclick={() => removeItem(activeHint.id)}><IconTrash size={13} /></button>
      </div>
    </div>
  {/if}

  <details class="advanced-source">
    <summary>{t("inspector-background-gradient-css-source")}</summary>
    <TextWithOptions
      value={rawValue}
      placeholder="linear-gradient(...)"
      oninput={(value) => applyRawSource(value, false)}
      oncommit={(value) => applyRawSource(value, true)}
      oncancel={cancelEdit}
    />
  </details>
</div>

<style>
  .gradient-editor { display: flex; flex-direction: column; gap: 7px; min-width: 0; }
  .gradient-toolbar { display: grid; grid-template-columns: minmax(0, 1fr) 28px; gap: 5px; }
  .field-label { margin-top: 1px; color: var(--text-muted); font-size: 11px; }
  .two-fields { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; }
  .two-fields > div { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .icon-button, .small-button {
    display: inline-flex; align-items: center; justify-content: center; gap: 4px;
    min-height: 26px; border: 1px solid var(--border-4); border-radius: 6px;
    background: var(--surface-8); color: var(--text-muted); cursor: pointer;
  }
  .icon-button { width: 28px; padding: 0; }
  .icon-button:hover, .small-button:hover { color: var(--text); border-color: var(--brand); }
  .icon-button.active { color: var(--brand); border-color: var(--brand); background: color-mix(in srgb, var(--brand) 10%, var(--surface-8)); }
  .icon-button.danger:hover { color: var(--danger, #c0392b); border-color: var(--danger, #c0392b); }
  .icon-button:disabled { opacity: .38; cursor: not-allowed; }
  .ramp-shell { padding: 11px 7px 13px; }
  .gradient-ramp {
    position: relative; height: 34px; border: 1px solid var(--border-4); border-radius: 8px;
    background: conic-gradient(#d7d7d7 25%, white 0 50%, #d7d7d7 0 75%, white 0) 0 0 / 10px 10px;
    box-shadow: inset 0 0 0 1px color-mix(in srgb, white 28%, transparent);
  }
  .ramp-background {
    position: absolute; inset: 0; width: 100%; padding: 0; border: none; border-radius: inherit;
    cursor: crosshair; overflow: hidden;
  }
  .gradient-stop {
    position: absolute; top: 100%; width: 16px; height: 20px; padding: 0;
    z-index: 2; transform: translate(-50%, -5px); border: none; background: transparent; cursor: grab;
  }
  .gradient-stop::before {
    content: ""; display: block; width: 12px; height: 12px; margin: 3px auto 0;
    transform: rotate(45deg); border: 2px solid white; border-radius: 3px;
    background: var(--stop-color, #888); box-shadow: 0 0 0 1px rgba(0, 0, 0, .42);
  }
  .gradient-stop.active { z-index: 4; }
  .gradient-stop.active::before { border-color: var(--brand); box-shadow: 0 0 0 2px color-mix(in srgb, var(--brand) 55%, transparent); }
  .gradient-hint {
    position: absolute; z-index: 2; top: 50%; width: 8px; height: 8px; padding: 0; border: 1px solid white;
    transform: translate(-50%, -50%) rotate(45deg); background: rgba(0, 0, 0, .35); cursor: pointer;
  }
  .gradient-hint.active { background: var(--brand); box-shadow: 0 0 0 1px var(--brand); }
  .dragging .gradient-stop { cursor: grabbing; }
  .ramp-actions { display: flex; gap: 5px; }
  .small-button { flex: 1; padding: 3px 6px; font-size: 11px; }
  .active-stop-editor { display: flex; flex-direction: column; gap: 5px; padding: 7px; border: 1px solid var(--border-subtle); border-radius: 8px; background: var(--surface-4); }
  .stop-row { display: flex; gap: 5px; align-items: center; }
  .stop-color { flex: 1; min-width: 0; }
  .advanced-source { border-top: 1px solid var(--border-subtle); padding-top: 5px; }
  .advanced-source summary { color: var(--text-muted); font-size: 11px; cursor: pointer; margin-bottom: 5px; }
</style>
