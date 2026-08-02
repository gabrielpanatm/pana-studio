<script lang="ts">
  import {
    IconArrowDown,
    IconArrowUp,
    IconCopy,
    IconLayoutGrid,
    IconLink,
    IconPlus,
    IconTrash,
    IconUnlink,
  } from "@tabler/icons-svelte";
  import { untrack } from "svelte";
  import { t } from "$lib/i18n/runtime.svelte";
  import type { ScssVariable } from "$lib/types";
  import type { CssPropertyEditController } from "$lib/inspector/css-property-edit";
  import {
    GRID_PROPERTIES,
    GRID_OPAQUE_PROPERTIES,
    cloneGridTrack,
    createGridTrack,
    gridAreasEditorText,
    gridFromProperties,
    gridToProperties,
    parseGridAreasText,
    parseGridTrack,
    serializeGridAreasRows,
    serializeGridTrackList,
    validateGridAreasRows,
    type CssGrid,
    type CssGridTrack,
    type CssGridTrackKind,
    type CssGridTrackList,
  } from "$lib/inspector/grid-model";
  import PropInput from "./PropInput.svelte";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import { variablesForProperty } from "$lib/editor/controls";

  let {
    pendingValues,
    rulesMap,
    canonicalGrid = null,
    scssVariables = [],
    viewport = "desktop",
    hasBaseRule = false,
    hasViewportRule = false,
    overlayEnabled = false,
    onOverlayChange,
    edit,
  }: {
    pendingValues: Record<string, string>;
    rulesMap: Record<string, string>;
    canonicalGrid?: CssGrid | null;
    scssVariables?: ScssVariable[];
    viewport?: "desktop" | "tablet" | "mobile";
    hasBaseRule?: boolean;
    hasViewportRule?: boolean;
    overlayEnabled?: boolean;
    onOverlayChange?: (enabled: boolean) => void;
    edit: CssPropertyEditController;
  } = $props();

  const trackKinds = $derived([
    { value: "flex", label: "fr" },
    { value: "keyword", label: t("inspector-grid-track-auto") },
    { value: "length", label: t("inspector-grid-track-fixed") },
    { value: "minmax", label: "minmax()" },
    { value: "fit_content", label: "fit-content()" },
    { value: "repeat", label: "repeat()" },
    { value: "line_names", label: t("inspector-grid-track-line") },
    { value: "opaque", label: t("inspector-grid-track-advanced") },
  ]);
  const flowOptions = ["row", "column", "row dense", "column dense"];
  const contentOptions = ["normal", "start", "center", "end", "stretch", "space-between", "space-around", "space-evenly"];
  const itemOptions = ["normal", "start", "center", "end", "stretch", "baseline"];

  function getValue(property: string) {
    return pendingValues[property] ?? rulesMap[property] ?? "";
  }

  function getOpaqueValue(property: string) {
    if (Object.prototype.hasOwnProperty.call(pendingValues, property)) return pendingValues[property] ?? "";
    if (Object.prototype.hasOwnProperty.call(rulesMap, property)) return rulesMap[property] ?? "";
    return canonicalGrid?.opaqueProperties[property] ?? "";
  }

  function cloneGrid(value: CssGrid): CssGrid {
    return JSON.parse(JSON.stringify(value)) as CssGrid;
  }

  function inputGrid() {
    const hasPending = GRID_PROPERTIES.some((property) => Object.prototype.hasOwnProperty.call(pendingValues, property));
    if (!hasPending && canonicalGrid?.schemaVersion === 1) return cloneGrid(canonicalGrid);
    const canonical = canonicalGrid?.schemaVersion === 1 ? gridToProperties(canonicalGrid) : {};
    return gridFromProperties({ ...canonical, ...rulesMap, ...pendingValues });
  }

  let grid = $state<CssGrid>(gridFromProperties({}));
  let locallyEmittedFingerprint = "";
  let advancedOpen = $state(false);
  let gapsLinked = $state(true);
  let areasDraft = $state("");
  let areasFocused = $state(false);

  const sourceFingerprint = $derived(GRID_PROPERTIES.map((property) => `${property}\u0000${getValue(property)}`).join("\u0001"));
  const viewportState = $derived(
    viewport === "desktop"
      ? t("inspector-grid-source-base")
      : hasViewportRule
        ? t("inspector-grid-source-local")
        : hasBaseRule
          ? t("inspector-grid-source-inherited")
          : t("inspector-grid-source-new"),
  );
  const areasDynamic = $derived(isDynamicGridValue(areasDraft));
  const areasRows = $derived(areasDynamic ? [] : parseGridAreasText(areasDraft));
  const shorthandBlocked = $derived(Object.keys(grid.opaqueProperties).length > 0);
  const columnsBlocked = $derived(shorthandBlocked || !grid.templateColumns.structurallyEditable);
  const rowsBlocked = $derived(shorthandBlocked || !grid.templateRows.structurallyEditable);
  const hasPreservedExpression = $derived(
    shorthandBlocked || columnsBlocked || rowsBlocked || !grid.templateAreas.structurallyEditable,
  );
  const areasError = $derived.by(() => {
    if (!areasDraft.trim() || areasDraft.trim() === "none") return "";
    if (areasDynamic) return "";
    const error = validateGridAreasRows(areasRows);
    if (error === "name") return t("inspector-grid-areas-name-error");
    if (error === "contiguous") return t("inspector-grid-areas-contiguous-error");
    return error === "rectangular" ? t("inspector-grid-areas-rectangular-error") : "";
  });
  const visualColumnCount = $derived(trackCount(grid.templateColumns, 3));
  const visualRowCount = $derived(Math.max(grid.templateAreas.rows.length, trackCount(grid.templateRows, 2)));
  const visualCells = $derived(Array.from({ length: Math.min(36, visualColumnCount * visualRowCount) }, (_, index) => index));
  const visualClass = $derived(`grid-visual cols-${Math.min(6, visualColumnCount)}`);

  $effect(() => {
    const fingerprint = sourceFingerprint;
    if (fingerprint === locallyEmittedFingerprint) {
      // Confirmarea proiecției locale se consumă o singură dată. Dacă am
      // păstra amprenta, un Undo urmat de Redo către aceeași valoare ar fi
      // confundat cu vechea confirmare și builderul ar rămâne pe starea Undo.
      locallyEmittedFingerprint = "";
      return;
    }
    locallyEmittedFingerprint = "";
    const next = inputGrid();
    grid = next;
    if (!untrack(() => areasFocused)) areasDraft = gridAreasEditorText(next.templateAreas);
  });

  function fingerprintWith(changes: Readonly<Record<string, string>>) {
    return GRID_PROPERTIES.map((property) => `${property}\u0000${changes[property] ?? getValue(property)}`).join("\u0001");
  }

  function emit(properties: Record<string, string>, commit = false) {
    locallyEmittedFingerprint = fingerprintWith(properties);
    if (commit) edit.commitMany(properties);
    else edit.draftMany(properties);
  }

  function updateTrackList(axis: "columns" | "rows", list: CssGridTrackList, commit = false) {
    const normalizedList = {
      ...list,
      structurallyEditable: list.mode !== "tracks"
        ? list.structurallyEditable
        : list.tracks.every((track) => track.structurallyEditable),
    };
    const property = axis === "columns" ? "grid-template-columns" : "grid-template-rows";
    grid = axis === "columns" ? { ...grid, templateColumns: normalizedList } : { ...grid, templateRows: normalizedList };
    emit({ [property]: serializeGridTrackList(normalizedList) }, commit);
  }

  function editableList(axis: "columns" | "rows") {
    const current = axis === "columns" ? grid.templateColumns : grid.templateRows;
    return current.mode === "tracks"
      ? current
      : { raw: null, mode: "tracks", tracks: [], structurallyEditable: true } satisfies CssGridTrackList;
  }

  function addTrack(axis: "columns" | "rows", kind: CssGridTrackKind = axis === "columns" ? "flex" : "keyword") {
    if (shorthandBlocked || (axis === "columns" ? columnsBlocked : rowsBlocked)) return;
    const list = editableList(axis);
    updateTrackList(axis, { ...list, tracks: [...list.tracks, createGridTrack(kind, axis)] }, true);
  }

  function patchTrack(axis: "columns" | "rows", trackId: string, nextTrack: CssGridTrack, commit = false) {
    const list = editableList(axis);
    updateTrackList(axis, {
      ...list,
      tracks: list.tracks.map((track) => track.id === trackId ? { ...nextTrack, id: trackId } : track),
    }, commit);
  }

  function patchTrackRaw(axis: "columns" | "rows", track: CssGridTrack, raw: string, commit = false) {
    patchTrack(axis, track.id, { ...parseGridTrack(raw), id: track.id }, commit);
  }

  function changeTrackKind(axis: "columns" | "rows", track: CssGridTrack, kind: CssGridTrackKind) {
    if (shorthandBlocked || (axis === "columns" ? columnsBlocked : rowsBlocked)) return;
    patchTrack(axis, track.id, { ...createGridTrack(kind, axis), id: track.id }, true);
  }

  function removeTrack(axis: "columns" | "rows", trackId: string) {
    if (shorthandBlocked || (axis === "columns" ? columnsBlocked : rowsBlocked)) return;
    const list = editableList(axis);
    updateTrackList(axis, { ...list, tracks: list.tracks.filter((track) => track.id !== trackId) }, true);
  }

  function duplicateTrack(axis: "columns" | "rows", trackId: string) {
    if (shorthandBlocked || (axis === "columns" ? columnsBlocked : rowsBlocked)) return;
    const list = editableList(axis);
    const index = list.tracks.findIndex((track) => track.id === trackId);
    if (index < 0) return;
    const tracks = [...list.tracks];
    tracks.splice(index + 1, 0, cloneGridTrack(tracks[index]));
    updateTrackList(axis, { ...list, tracks }, true);
  }

  function moveTrack(axis: "columns" | "rows", trackId: string, direction: -1 | 1) {
    if (shorthandBlocked || (axis === "columns" ? columnsBlocked : rowsBlocked)) return;
    const list = editableList(axis);
    const index = list.tracks.findIndex((track) => track.id === trackId);
    const target = index + direction;
    if (index < 0 || target < 0 || target >= list.tracks.length) return;
    const tracks = [...list.tracks];
    [tracks[index], tracks[target]] = [tracks[target], tracks[index]];
    updateTrackList(axis, { ...list, tracks }, true);
  }

  function patchRepeat(axis: "columns" | "rows", track: CssGridTrack, count: string, pattern: string, commit = false) {
    patchTrackRaw(axis, track, `repeat(${count || "2"}, ${pattern || "1fr"})`, commit);
  }

  function commitAreas() {
    if (areasError) return;
    const value = areasDynamic
      ? areasDraft.trim()
      : areasDraft.trim() ? serializeGridAreasRows(areasRows) : "";
    edit.commit("grid-template-areas", value);
  }

  function isDynamicGridValue(value: string) {
    const trimmed = value.trim();
    return trimmed.startsWith("$") || trimmed.startsWith("#{") || trimmed.startsWith("var(");
  }

  function updateGap(property: "row-gap" | "column-gap", value: string, commit = false) {
    const properties: Record<string, string> = { [property]: value };
    if (gapsLinked) properties[property === "row-gap" ? "column-gap" : "row-gap"] = value;
    grid = {
      ...grid,
      rowGap: properties["row-gap"] ?? grid.rowGap,
      columnGap: properties["column-gap"] ?? grid.columnGap,
    };
    emit(properties, commit);
  }

  function trackCount(list: CssGridTrackList, fallback: number) {
    if (list.mode !== "tracks" || !list.tracks.length) return fallback;
    return Math.max(1, Math.min(6, list.tracks.reduce((count, track) => {
      if (track.kind !== "repeat") return count + (track.kind === "line_names" ? 0 : 1);
      const repeat = Number(track.repeatCount);
      return count + (Number.isSafeInteger(repeat) && repeat > 0 ? Math.min(repeat, 6) : 3);
    }, 0)));
  }

  function visualCellLabel(index: number) {
    const row = Math.floor(index / visualColumnCount);
    const column = index % visualColumnCount;
    const label = grid.templateAreas.rows[row]?.[column];
    return label && label !== "." ? label : "";
  }
</script>

<div class="grid-builder" data-grid-builder>
  <div class="grid-builder-heading">
    <div>
      <strong>{t("inspector-grid-builder-title")}</strong>
      <span>{viewportState}</span>
    </div>
    <button
      type="button"
      class="overlay-toggle"
      class:active={overlayEnabled}
      aria-pressed={overlayEnabled}
      title={overlayEnabled ? t("inspector-grid-hide-overlay") : t("inspector-grid-show-overlay")}
      onclick={() => onOverlayChange?.(!overlayEnabled)}
    ><IconLayoutGrid size={15} stroke={1.8} /></button>
  </div>

  {#if hasPreservedExpression}
    <div class="grid-compatibility" role="status">
      <strong>{t("inspector-grid-preserved-title")}</strong>
      <span>{t("inspector-grid-preserved-description")}</span>
    </div>
  {/if}

  <div class={visualClass} aria-label={t("inspector-grid-diagram-label")}>
    {#each visualCells as cell}
      <span><small>{visualCellLabel(cell)}</small></span>
    {/each}
  </div>

  <div class="axis-block">
    <div class="axis-heading">
      <strong>{t("inspector-grid-columns")}</strong>
      <button type="button" disabled={columnsBlocked} title={t("inspector-grid-add-column")} onclick={() => addTrack("columns")}>
        <IconPlus size={13} stroke={2} />
      </button>
    </div>
    {#if grid.templateColumns.mode === "tracks" && grid.templateColumns.tracks.length}
      <div class="track-list">
        {#each grid.templateColumns.tracks as track, index (track.id)}
          <div class="track-card">
            <div class="track-index">C{index + 1}</div>
            <SelectControl
              value={track.kind}
              options={trackKinds}
              disabled={columnsBlocked}
              ariaLabel={t("inspector-grid-track-type")}
              onchange={(kind) => changeTrackKind("columns", track, kind as CssGridTrackKind)}
            />
            <div class="track-actions">
              <button type="button" disabled={columnsBlocked || index === 0} title={t("inspector-grid-move-before")} onclick={() => moveTrack("columns", track.id, -1)}><IconArrowUp size={12} /></button>
              <button type="button" disabled={columnsBlocked || index === grid.templateColumns.tracks.length - 1} title={t("inspector-grid-move-after")} onclick={() => moveTrack("columns", track.id, 1)}><IconArrowDown size={12} /></button>
              <button type="button" disabled={columnsBlocked} title={t("inspector-grid-duplicate-track")} onclick={() => duplicateTrack("columns", track.id)}><IconCopy size={12} /></button>
              <button type="button" disabled={columnsBlocked} title={t("inspector-grid-remove-track")} onclick={() => removeTrack("columns", track.id)}><IconTrash size={12} /></button>
            </div>
            {#if track.kind === "repeat"}
              <div class="repeat-fields">
                <PropInput label="×" value={track.repeatCount ?? "2"} placeholder="2 / auto-fit" oninput={(value) => patchRepeat("columns", track, value, track.repeatTracks.map((child) => child.raw).join(" "))} oncommit={(value) => patchRepeat("columns", track, value, track.repeatTracks.map((child) => child.raw).join(" "), true)} />
                <PropInput label="T" value={track.repeatTracks.map((child) => child.raw).join(" ")} placeholder="minmax(0, 1fr)" oninput={(value) => patchRepeat("columns", track, track.repeatCount ?? "2", value)} oncommit={(value) => patchRepeat("columns", track, track.repeatCount ?? "2", value, true)} />
              </div>
            {:else}
              <PropInput label="V" value={track.raw} suggestions={variablesForProperty("grid-template-columns", scssVariables)} oninput={(value) => patchTrackRaw("columns", track, value)} oncommit={(value) => patchTrackRaw("columns", track, value, true)} />
            {/if}
          </div>
        {/each}
      </div>
    {:else}
      <div class="empty-axis">
        <span>{grid.templateColumns.raw || t("inspector-grid-no-explicit-tracks")}</span>
        <button type="button" disabled={columnsBlocked} onclick={() => addTrack("columns")}>{t("inspector-grid-add-column")}</button>
      </div>
    {/if}
  </div>

  <div class="axis-block">
    <div class="axis-heading">
      <strong>{t("inspector-grid-rows")}</strong>
      <button type="button" disabled={rowsBlocked} title={t("inspector-grid-add-row")} onclick={() => addTrack("rows", "keyword")}>
        <IconPlus size={13} stroke={2} />
      </button>
    </div>
    {#if grid.templateRows.mode === "tracks" && grid.templateRows.tracks.length}
      <div class="track-list">
        {#each grid.templateRows.tracks as track, index (track.id)}
          <div class="track-card">
            <div class="track-index">R{index + 1}</div>
            <SelectControl value={track.kind} options={trackKinds} disabled={rowsBlocked} ariaLabel={t("inspector-grid-track-type")} onchange={(kind) => changeTrackKind("rows", track, kind as CssGridTrackKind)} />
            <div class="track-actions">
              <button type="button" disabled={rowsBlocked || index === 0} title={t("inspector-grid-move-before")} onclick={() => moveTrack("rows", track.id, -1)}><IconArrowUp size={12} /></button>
              <button type="button" disabled={rowsBlocked || index === grid.templateRows.tracks.length - 1} title={t("inspector-grid-move-after")} onclick={() => moveTrack("rows", track.id, 1)}><IconArrowDown size={12} /></button>
              <button type="button" disabled={rowsBlocked} title={t("inspector-grid-duplicate-track")} onclick={() => duplicateTrack("rows", track.id)}><IconCopy size={12} /></button>
              <button type="button" disabled={rowsBlocked} title={t("inspector-grid-remove-track")} onclick={() => removeTrack("rows", track.id)}><IconTrash size={12} /></button>
            </div>
            {#if track.kind === "repeat"}
              <div class="repeat-fields">
                <PropInput label="×" value={track.repeatCount ?? "2"} oninput={(value) => patchRepeat("rows", track, value, track.repeatTracks.map((child) => child.raw).join(" "))} oncommit={(value) => patchRepeat("rows", track, value, track.repeatTracks.map((child) => child.raw).join(" "), true)} />
                <PropInput label="T" value={track.repeatTracks.map((child) => child.raw).join(" ")} oninput={(value) => patchRepeat("rows", track, track.repeatCount ?? "2", value)} oncommit={(value) => patchRepeat("rows", track, track.repeatCount ?? "2", value, true)} />
              </div>
            {:else}
              <PropInput label="V" value={track.raw} suggestions={variablesForProperty("grid-template-rows", scssVariables)} oninput={(value) => patchTrackRaw("rows", track, value)} oncommit={(value) => patchTrackRaw("rows", track, value, true)} />
            {/if}
          </div>
        {/each}
      </div>
    {:else}
      <div class="empty-axis">
        <span>{grid.templateRows.raw || t("inspector-grid-no-explicit-tracks")}</span>
        <button type="button" disabled={rowsBlocked} onclick={() => addTrack("rows", "keyword")}>{t("inspector-grid-add-row")}</button>
      </div>
    {/if}
  </div>

  <div class="field-heading">
    <strong>{t("inspector-grid-gaps")}</strong>
    <button type="button" class:active={gapsLinked} title={gapsLinked ? t("inspector-grid-unlink-gaps") : t("inspector-grid-link-gaps")} onclick={() => { gapsLinked = !gapsLinked; }}>
      {#if gapsLinked}<IconLink size={13} />{:else}<IconUnlink size={13} />{/if}
    </button>
  </div>
  <div class="two-fields">
    <PropInput label="C" value={grid.columnGap ?? ""} suggestions={variablesForProperty("column-gap", scssVariables)} oninput={(value) => updateGap("column-gap", value)} oncommit={(value) => updateGap("column-gap", value, true)} />
    <PropInput label="R" value={grid.rowGap ?? ""} suggestions={variablesForProperty("row-gap", scssVariables)} oninput={(value) => updateGap("row-gap", value)} oncommit={(value) => updateGap("row-gap", value, true)} />
  </div>

  <div class="field-label">{t("inspector-grid-auto-flow")}</div>
  <SelectControl value={grid.autoFlow ?? ""} options={flowOptions} placeholder={t("inspector-default-value")} onchange={(value) => edit.commit("grid-auto-flow", value)} />

  <div class="two-fields">
    <div><div class="field-label">{t("inspector-grid-auto-columns")}</div><PropInput label="C" value={grid.autoColumns ?? ""} suggestions={variablesForProperty("grid-auto-columns", scssVariables)} {...edit.continuous("grid-auto-columns")} /></div>
    <div><div class="field-label">{t("inspector-grid-auto-rows")}</div><PropInput label="R" value={grid.autoRows ?? ""} suggestions={variablesForProperty("grid-auto-rows", scssVariables)} {...edit.continuous("grid-auto-rows")} /></div>
  </div>

  <div class="alignment-grid">
    <label><span>{t("inspector-grid-justify-content")}</span><SelectControl value={grid.justifyContent ?? ""} options={contentOptions} onchange={(value) => edit.commit("justify-content", value)} /></label>
    <label><span>{t("inspector-grid-align-content")}</span><SelectControl value={grid.alignContent ?? ""} options={contentOptions} onchange={(value) => edit.commit("align-content", value)} /></label>
    <label><span>{t("inspector-grid-justify-items")}</span><SelectControl value={grid.justifyItems ?? ""} options={itemOptions} onchange={(value) => edit.commit("justify-items", value)} /></label>
    <label><span>{t("inspector-grid-align-items")}</span><SelectControl value={grid.alignItems ?? ""} options={itemOptions} onchange={(value) => edit.commit("align-items", value)} /></label>
  </div>

  <div class="areas-block">
    <div class="field-heading"><strong>{t("inspector-grid-areas")}</strong><span>{t("inspector-grid-areas-hint")}</span></div>
    <textarea
      class:invalid={Boolean(areasError)}
      value={areasDraft}
      placeholder={'hero hero side\nmain main side'}
      aria-label={t("inspector-grid-areas")}
      onfocus={() => { areasFocused = true; }}
      oninput={(event) => {
        areasDraft = event.currentTarget.value;
        if (!areasError) edit.draft(
          "grid-template-areas",
          isDynamicGridValue(areasDraft)
            ? areasDraft.trim()
            : areasDraft.trim() ? serializeGridAreasRows(parseGridAreasText(areasDraft)) : "",
        );
      }}
      onblur={() => { areasFocused = false; commitAreas(); }}
      onkeydown={(event) => {
        if (event.key === "Escape") {
          event.preventDefault();
          edit.cancel("grid-template-areas");
          areasDraft = gridAreasEditorText(grid.templateAreas);
          event.currentTarget.blur();
        } else if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
          event.preventDefault();
          event.currentTarget.blur();
        }
      }}
    ></textarea>
    {#if areasError}<span class="field-error" role="alert">{areasError}</span>{/if}
  </div>

  <button type="button" class="advanced-toggle" aria-expanded={advancedOpen} onclick={() => { advancedOpen = !advancedOpen; }}>
    {t("inspector-grid-advanced")}
  </button>
  {#if advancedOpen}
    <div class="advanced-fields">
      <label><span>grid-template-columns</span><PropInput label="C" value={getValue("grid-template-columns")} suggestions={variablesForProperty("grid-template-columns", scssVariables)} {...edit.continuous("grid-template-columns")} /></label>
      <label><span>grid-template-rows</span><PropInput label="R" value={getValue("grid-template-rows")} suggestions={variablesForProperty("grid-template-rows", scssVariables)} {...edit.continuous("grid-template-rows")} /></label>
      {#each GRID_OPAQUE_PROPERTIES as property}
        <label><span>{property}</span><PropInput label="A" value={getOpaqueValue(property)} {...edit.continuous(property)} /></label>
      {/each}
    </div>
  {/if}
</div>

<style>
  .grid-builder { display: flex; flex-direction: column; gap: 8px; min-width: 0; }
  .grid-builder-heading, .axis-heading, .field-heading { display: flex; align-items: center; justify-content: space-between; gap: 6px; min-width: 0; }
  .grid-builder-heading { padding: 7px 8px; border: 1px solid var(--border-subtle); border-radius: 8px; color: var(--brand-strong); background: var(--brand-soft); }
  .grid-builder-heading > div { display: grid; min-width: 0; gap: 1px; }
  .grid-builder-heading strong, .axis-heading strong, .field-heading strong { font-size: 11px; font-weight: 750; }
  .grid-builder-heading span, .field-heading span { color: var(--text-muted); font-size: 11px; }
  .overlay-toggle { display: grid; width: 27px; height: 27px; flex: 0 0 auto; padding: 0; place-items: center; border: 1px solid color-mix(in srgb, var(--brand) 38%, var(--border-subtle)); border-radius: 6px; color: var(--brand-strong); background: var(--surface); cursor: pointer; }
  .overlay-toggle.active { border-color: #7c3aed; color: #7c3aed; background: color-mix(in srgb, #7c3aed 12%, var(--surface)); }
  .grid-compatibility { display: grid; gap: 3px; padding: 7px 8px; border: 1px solid color-mix(in srgb, var(--warning) 45%, var(--border-subtle)); border-radius: 7px; color: var(--text-muted); background: color-mix(in srgb, var(--warning) 9%, var(--surface-panel)); font-size: 11px; line-height: 1.4; }
  .grid-compatibility strong { color: var(--text); font-size: 11px; }
  .grid-visual { display: grid; gap: 3px; min-height: 92px; padding: 6px; border: 1px solid color-mix(in srgb, var(--brand) 54%, var(--border-subtle)); border-radius: 8px; background: repeating-linear-gradient(135deg, transparent 0 7px, color-mix(in srgb, var(--brand-soft) 40%, transparent) 7px 8px); pointer-events: none; }
  .grid-visual.cols-1 { grid-template-columns: 1fr; }
  .grid-visual.cols-2 { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .grid-visual.cols-3 { grid-template-columns: repeat(3, minmax(0, 1fr)); }
  .grid-visual.cols-4 { grid-template-columns: repeat(4, minmax(0, 1fr)); }
  .grid-visual.cols-5 { grid-template-columns: repeat(5, minmax(0, 1fr)); }
  .grid-visual.cols-6 { grid-template-columns: repeat(6, minmax(0, 1fr)); }
  .grid-visual > span { display: grid; min-height: 26px; place-items: center; overflow: hidden; border: 1px solid color-mix(in srgb, var(--brand) 38%, var(--border-subtle)); border-radius: 4px; background: color-mix(in srgb, var(--surface) 84%, var(--brand-soft)); }
  .grid-visual small { overflow: hidden; max-width: 100%; padding: 0 3px; color: var(--brand-strong); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .axis-block, .areas-block, .advanced-fields { display: grid; gap: 6px; padding: 7px; border: 1px solid var(--border-subtle); border-radius: 8px; background: var(--surface-4); }
  .axis-heading > button, .field-heading > button, .track-actions button { display: inline-grid; width: 23px; height: 23px; padding: 0; place-items: center; border: 1px solid var(--border-4); border-radius: 5px; color: var(--text-muted); background: var(--surface-2); cursor: pointer; }
  .axis-heading > button:disabled, .empty-axis button:disabled { opacity: .38; cursor: default; }
  .field-heading > button.active { border-color: var(--brand); color: var(--brand-strong); background: var(--brand-soft); }
  .track-list { display: grid; gap: 5px; }
  .track-card { display: grid; grid-template-columns: 24px minmax(76px, .72fr) minmax(0, 1fr); align-items: center; gap: 5px; padding: 5px; border: 1px solid var(--border-4); border-radius: 6px; background: var(--surface); }
  .track-index { display: grid; height: 24px; place-items: center; border-radius: 4px; color: var(--brand-strong); background: var(--brand-soft); font: 700 11px/1 "JetBrains Mono", monospace; }
  .track-actions { grid-column: 1 / -1; display: flex; justify-content: flex-end; gap: 3px; }
  .track-actions button:disabled { opacity: .35; cursor: default; }
  .track-card > :global(.prop-field) { grid-column: 3; grid-row: 1; }
  .repeat-fields { grid-column: 1 / -1; display: grid; grid-template-columns: minmax(70px, .45fr) minmax(0, 1fr); gap: 5px; }
  .empty-axis { display: flex; align-items: center; justify-content: space-between; gap: 6px; color: var(--text-muted); font-size: 11px; }
  .empty-axis span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .empty-axis button, .advanced-toggle { min-height: 25px; padding: 0 8px; border: 1px solid var(--border-4); border-radius: 6px; color: var(--text); background: var(--surface-2); cursor: pointer; font-size: 11px; font-weight: 650; }
  .two-fields, .alignment-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 6px; }
  .two-fields > div, .alignment-grid label, .advanced-fields label { display: grid; min-width: 0; gap: 4px; }
  .field-label, .alignment-grid label > span, .advanced-fields label > span { color: var(--text-muted); font-size: 11px; }
  .areas-block textarea { min-height: 64px; resize: vertical; padding: 6px 7px; border: 1px solid var(--border-4); border-radius: 6px; color: var(--text); background: var(--surface-8); font: 11px/1.5 "JetBrains Mono", monospace; }
  .areas-block textarea:focus { outline: none; border-color: var(--brand); }
  .areas-block textarea.invalid { border-color: var(--danger); }
  .field-error { color: var(--danger); font-size: 11px; line-height: 1.35; }
  .advanced-toggle { width: 100%; }
</style>
