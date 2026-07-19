<script lang="ts" module>
  let nextId = 0;
</script>

<script lang="ts">
  import type { ScssVariable } from "$lib/types";
  import type { CssPropertyEditController } from "$lib/inspector/css-property-edit";
  import { variablesForProperty } from "$lib/editor/controls";
  import { IconShadow, IconTrash } from "@tabler/icons-svelte";
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

<InspectorSection title="Shadow" {hasValues}>
  {#snippet icon()}<IconShadow size={13} stroke={1.7} />{/snippet}

  <!-- ── BOX SHADOW ────────────────────────────────────────────────────── -->
  <div class="sh-subheader">
    <span class="sh-label">Box Shadow</span>
    <button type="button" class="sh-add" title="Adaugă box shadow" disabled={!boxStructured} onclick={addBox}>+</button>
  </div>

  {#if !boxStructured}
    <p class="sh-empty">Valoare complexă păstrată integral; editeaz-o în modul brut.</p>
    <PropInput value={getValue("box-shadow")} placeholder="box-shadow" {...edit.continuous("box-shadow")} />
  {:else if boxShadows.length === 0}
    <p class="sh-empty">No box shadow set</p>
  {:else}
    {#each boxShadows as s (s.id)}
      <div class="sh-card">
        <div class="sh-color-row">
          <div class="sh-color">
            <ColorInput
              property="box-shadow-color-{s.id}"
              value={s.color}
              suggestions={colorSuggestions}
              oninput={(value) => patchBox(s.id, { color: value })}
              oncommit={() => edit.commit("box-shadow")}
              oncancel={() => edit.cancel("box-shadow")}
            />
          </div>
          <button
            type="button"
            class="sh-inset"
            class:active={s.inset}
            title="Inset shadow"
            onclick={() => patchBox(s.id, { inset: !s.inset }, true)}
          >INSET</button>
          <button type="button" class="sh-del" title="Șterge" onclick={() => removeBox(s.id)}>
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
  <div class="sh-subheader" style="margin-top: 4px;">
    <span class="sh-label">Text Shadow</span>
    <button type="button" class="sh-add" title="Adaugă text shadow" disabled={!textStructured} onclick={addText}>+</button>
  </div>

  {#if !textStructured}
    <p class="sh-empty">Valoare complexă păstrată integral; editeaz-o în modul brut.</p>
    <PropInput value={getValue("text-shadow")} placeholder="text-shadow" {...edit.continuous("text-shadow")} />
  {:else if textShadows.length === 0}
    <p class="sh-empty">No text shadow set</p>
  {:else}
    {#each textShadows as s (s.id)}
      <div class="sh-card">
        <div class="sh-color-row">
          <div class="sh-color">
            <ColorInput
              property="text-shadow-color-{s.id}"
              value={s.color}
              suggestions={colorSuggestions}
              oninput={(value) => patchText(s.id, { color: value })}
              oncommit={() => edit.commit("text-shadow")}
              oncancel={() => edit.cancel("text-shadow")}
            />
          </div>
          <button type="button" class="sh-del" title="Șterge" onclick={() => removeText(s.id)}>
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

  .sh-label {
    font-size: 11px;
    color: var(--text-muted);
  }

  .sh-add {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border: 1px solid var(--border-3);
    border-radius: 4px;
    background: var(--surface-4);
    color: var(--text-muted);
    font-size: 14px;
    line-height: 1;
    cursor: pointer;
    transition: color 80ms, border-color 80ms;
  }

  .sh-add:hover {
    color: var(--brand-strong);
    border-color: var(--brand);
  }

  .sh-add:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .sh-empty {
    margin: 0;
    padding: 6px 0;
    font-size: 11px;
    color: var(--text-muted);
    text-align: center;
  }

  /* ── Shadow card ─────────────────────────────────────────────────────── */

  .sh-card {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 7px 8px;
    border: 1px solid var(--border-3);
    border-radius: 8px;
    background: var(--surface-3);
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
    height: 24px;
    padding: 0 7px;
    border: 1px solid var(--border-4);
    border-radius: 5px;
    background: var(--surface-5);
    color: var(--text-muted);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.04em;
    cursor: pointer;
    transition: border-color 80ms, color 80ms, background 80ms;
    white-space: nowrap;
  }

  .sh-inset.active {
    border-color: var(--brand);
    color: var(--brand-strong);
    background: var(--brand-soft);
  }

  .sh-inset:hover:not(.active) {
    border-color: var(--border-4);
    color: var(--text);
  }

  .sh-del {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 24px;
    border: 1px solid var(--border-4);
    border-radius: 5px;
    background: var(--surface-5);
    color: var(--text-muted);
    cursor: pointer;
    transition: color 80ms, background 80ms, border-color 80ms;
  }

  .sh-del:hover {
    border-color: #cf4a4a;
    background: color-mix(in srgb, #cf4a4a 12%, transparent);
    color: #cf4a4a;
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
