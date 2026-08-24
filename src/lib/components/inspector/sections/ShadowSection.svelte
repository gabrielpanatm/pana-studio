<script lang="ts" module>
  let nextId = 0;
</script>

<script lang="ts">
  import { t } from "$lib/i18n/runtime.svelte";
  import type { ScssVariable } from "$lib/css/contracts";
  import type { CssPropertyEditController } from "$lib/inspector/css-property-edit";
  import { variablesForProperty } from "$lib/editor/controls";
  import { IconPlus, IconShadow, IconTrash } from "@tabler/icons-svelte";
  import InspectorSection from "../InspectorSection.svelte";
  import ColorInput from "../controls/ColorInput.svelte";
  import PropInput from "../controls/PropInput.svelte";
  import {
    parseBoxShadowList,
    parseTextShadowList,
    serializeBoxShadowList,
    serializeTextShadowList,
  } from "$lib/inspector/shadow-value";

  let {
    pendingValues,
    rulesMap,
    scssVariables = [],
    edit,
  }: {
    pendingValues: Record<string, string>;
    rulesMap: Record<string, string>;
    scssVariables?: ScssVariable[];
    edit: CssPropertyEditController;
  } = $props();

  function getValue(prop: string): string {
    return pendingValues[prop] ?? rulesMap[prop] ?? "";
  }

  const PROPS = ["box-shadow", "text-shadow"];
  const hasValues = $derived(PROPS.some((p) => {
    const v = getValue(p);
    return v !== "" && v !== "none";
  }));

  // ── Types ────────────────────────────────────────────────────────────────

  type BoxShadow  = { id: number; x: string; y: string; blur: string; spread: string; color: string; inset: boolean; };
  type TextShadow = { id: number; x: string; y: string; blur: string; color: string; };

  // ── State ────────────────────────────────────────────────────────────────

  let boxShadows  = $state<BoxShadow[]>([]);
  let textShadows = $state<TextShadow[]>([]);
  let lastBox  = "";
  let lastText = "";
  let boxStructured = $state(true);
  let textStructured = $state(true);

  $effect(() => {
    const v = getValue("box-shadow");
    if (v === lastBox) return;
    lastBox = v;
    const parsed = parseBoxShadowList(v);
    boxStructured = parsed !== null;
    boxShadows = (parsed ?? []).map((layer) => ({ id: nextId++, ...layer }));
  });

  $effect(() => {
    const v = getValue("text-shadow");
    if (v === lastText) return;
    lastText = v;
    const parsed = parseTextShadowList(v);
    textStructured = parsed !== null;
    textShadows = (parsed ?? []).map((layer) => ({ id: nextId++, ...layer }));
  });

  function emitBox(commit = false) {
    const css = serializeBoxShadowList(boxShadows);
    lastBox = css;
    if (commit) edit.commit("box-shadow", css);
    else edit.draft("box-shadow", css);
  }

  function emitText(commit = false) {
    const css = serializeTextShadowList(textShadows);
    lastText = css;
    if (commit) edit.commit("text-shadow", css);
    else edit.draft("text-shadow", css);
  }

  // ── Box shadow actions ───────────────────────────────────────────────────

  function addBox() {
    boxShadows = [...boxShadows, { id: nextId++, x: "0px", y: "4px", blur: "8px", spread: "0px", color: "rgba(0, 0, 0, 0.15)", inset: false }];
    emitBox(true);
  }

  function patchBox(id: number, patch: Partial<BoxShadow>, commit = false) {
    boxShadows = boxShadows.map((s) => s.id === id ? { ...s, ...patch } : s);
    emitBox(commit);
  }

  function removeBox(id: number) {
    boxShadows = boxShadows.filter((s) => s.id !== id);
    emitBox(true);
  }

  // ── Text shadow actions ──────────────────────────────────────────────────

  function addText() {
    textShadows = [...textShadows, { id: nextId++, x: "0px", y: "2px", blur: "4px", color: "rgba(0, 0, 0, 0.3)" }];
    emitText(true);
  }

  function patchText(id: number, patch: Partial<TextShadow>, commit = false) {
    textShadows = textShadows.map((s) => s.id === id ? { ...s, ...patch } : s);
    emitText(commit);
  }

  function removeText(id: number) {
    textShadows = textShadows.filter((s) => s.id !== id);
    emitText(true);
  }

  const colorSuggestions = $derived(variablesForProperty("color", scssVariables));
</script>

<InspectorSection title={t("inspector-shadow-title")} {hasValues}>
  {#snippet icon()}<IconShadow size={13} stroke={1.7} />{/snippet}

  <!-- ── BOX SHADOW ────────────────────────────────────────────────────── -->
  <div class="sh-subheader">
    <span class="sh-label">{t("inspector-shadow-box")}</span>
    <button type="button" class="sh-add ui-icon-button mini" title={t("inspector-shadow-add-box")} aria-label={t("inspector-shadow-add-box")} disabled={!boxStructured} onclick={addBox}>
      <IconPlus size={13} stroke={1.9} />
    </button>
  </div>

  {#if !boxStructured}
    <p class="sh-empty">{t("inspector-shadow-complex")}</p>
    <PropInput value={getValue("box-shadow")} placeholder="box-shadow" {...edit.continuous("box-shadow")} />
  {:else if boxShadows.length === 0}
    <p class="sh-empty">{t("inspector-shadow-no-box")}</p>
  {:else}
    {#each boxShadows as s (s.id)}
      <div class="sh-card ui-card">
        <div class="sh-color-row">
          <div class="sh-color">
            <ColorInput
              property="box-shadow-color-{s.id}"
              value={s.color}
              suggestions={colorSuggestions}
              resolutionVariables={scssVariables}
              oninput={(value) => patchBox(s.id, { color: value })}
              oncommit={() => edit.commit("box-shadow")}
              oncancel={() => edit.cancel("box-shadow")}
            />
          </div>
          <button
            type="button"
            class="sh-inset ui-button compact quiet"
            aria-pressed={s.inset}
            title={t("inspector-shadow-inset")}
            onclick={() => patchBox(s.id, { inset: !s.inset }, true)}
          >{t("inspector-shadow-inset-short")}</button>
          <button type="button" class="sh-del ui-icon-button mini danger" title={t("inspector-delete")} aria-label={t("inspector-delete")} onclick={() => removeBox(s.id)}>
            <IconTrash size={11} stroke={1.8} />
          </button>
        </div>
        <div class="sh-dims">
          <PropInput label="X" value={s.x} placeholder="0px" oninput={(value) => patchBox(s.id, { x: value })} oncommit={() => edit.commit("box-shadow")} oncancel={() => edit.cancel("box-shadow")} />
          <PropInput label="Y" value={s.y} placeholder="0px" oninput={(value) => patchBox(s.id, { y: value })} oncommit={() => edit.commit("box-shadow")} oncancel={() => edit.cancel("box-shadow")} />
        </div>
        <div class="sh-dims">
          <PropInput label="Bl" value={s.blur} placeholder="0px" oninput={(value) => patchBox(s.id, { blur: value })} oncommit={() => edit.commit("box-shadow")} oncancel={() => edit.cancel("box-shadow")} />
          <PropInput label="Sp" value={s.spread} placeholder="0px" oninput={(value) => patchBox(s.id, { spread: value })} oncommit={() => edit.commit("box-shadow")} oncancel={() => edit.cancel("box-shadow")} />
        </div>
      </div>
    {/each}
  {/if}

  <!-- ── TEXT SHADOW ───────────────────────────────────────────────────── -->
  <div class="sh-subheader spaced">
    <span class="sh-label">{t("inspector-shadow-text")}</span>
    <button type="button" class="sh-add ui-icon-button mini" title={t("inspector-shadow-add-text")} aria-label={t("inspector-shadow-add-text")} disabled={!textStructured} onclick={addText}>
      <IconPlus size={13} stroke={1.9} />
    </button>
  </div>

  {#if !textStructured}
    <p class="sh-empty">{t("inspector-shadow-complex")}</p>
    <PropInput value={getValue("text-shadow")} placeholder="text-shadow" {...edit.continuous("text-shadow")} />
  {:else if textShadows.length === 0}
    <p class="sh-empty">{t("inspector-shadow-no-text")}</p>
  {:else}
    {#each textShadows as s (s.id)}
      <div class="sh-card ui-card">
        <div class="sh-color-row">
          <div class="sh-color">
            <ColorInput
              property="text-shadow-color-{s.id}"
              value={s.color}
              suggestions={colorSuggestions}
              resolutionVariables={scssVariables}
              oninput={(value) => patchText(s.id, { color: value })}
              oncommit={() => edit.commit("text-shadow")}
              oncancel={() => edit.cancel("text-shadow")}
            />
          </div>
          <button type="button" class="sh-del ui-icon-button mini danger" title={t("inspector-delete")} aria-label={t("inspector-delete")} onclick={() => removeText(s.id)}>
            <IconTrash size={11} stroke={1.8} />
          </button>
        </div>
        <div class="sh-dims">
          <PropInput label="X" value={s.x} placeholder="0px" oninput={(value) => patchText(s.id, { x: value })} oncommit={() => edit.commit("text-shadow")} oncancel={() => edit.cancel("text-shadow")} />
          <PropInput label="Y" value={s.y} placeholder="0px" oninput={(value) => patchText(s.id, { y: value })} oncommit={() => edit.commit("text-shadow")} oncancel={() => edit.cancel("text-shadow")} />
        </div>
        <div class="sh-dims single">
          <PropInput label="Bl" value={s.blur} placeholder="0px" oninput={(value) => patchText(s.id, { blur: value })} oncommit={() => edit.commit("text-shadow")} oncancel={() => edit.cancel("text-shadow")} />
        </div>
      </div>
    {/each}
  {/if}
</InspectorSection>

<style>
  .sh-subheader {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .sh-subheader.spaced {
    margin-top: 4px;
  }

  .sh-label {
    font-size: 12px;
    color: var(--text-muted);
  }

  .sh-empty {
    margin: 0;
    padding: 6px 0;
    font-size: 12px;
    color: var(--text-muted);
    text-align: center;
  }

  /* ── Shadow card ─────────────────────────────────────────────────────── */

  .sh-card {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 7px 8px;
  }

  .sh-color-row {
    display: flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
  }

  .sh-color {
    flex: 1;
    min-width: 0;
  }

  .sh-inset {
    flex-shrink: 0;
    letter-spacing: 0.04em;
    white-space: nowrap;
  }

  .sh-dims {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 5px;
  }

  .sh-dims.single {
    grid-template-columns: 1fr;
  }
</style>
