<script lang="ts" module>
  let nextInputId = 0;
</script>

<script lang="ts">
  import type { ScssVariable } from "$lib/types";
  import { IconBolt } from "@tabler/icons-svelte";
  import { resolvePickerColor } from "$lib/inspector/color-picker-model";
  import PanaColorPicker from "$lib/components/ui/PanaColorPicker.svelte";
  import VariablePopover from "./VariablePopover.svelte";

  let {
    property,
    value = "",
    suggestions = [],
    oninput,
    oncommit,
    oncancel,
  }: {
    property: string;
    value?: string;
    suggestions?: ScssVariable[];
    oninput?: (value: string) => void;
    oncommit?: (value: string) => void;
    oncancel?: () => void;
  } = $props();

  const instanceId = nextInputId++;
  const uid        = $derived(`${property.replace(/[^a-z0-9]/g, "-")}-${instanceId}`);
  const inputId    = $derived(`ci-input-${uid}`);

  let draftValue   = $state("");
  let focused      = $state(false);
  let pickerOpen   = $state(false);

  let root             = $state<HTMLDivElement | null>(null);
  let showSuggestions  = $state(false);
  let skipNextCommit = false;

  // Variabilele sunt rezolvate numai pentru swatch; sursa rămâne `$token`.
  const resolvedColor = $derived(resolvePickerColor(draftValue, suggestions));

  // ── Sync effects ─────────────────────────────────────────────────────────

  $effect(() => {
    if (!focused && value !== draftValue) {
      draftValue = value;
    }
  });

  // ── Handlers ─────────────────────────────────────────────────────────────

  function handlePickerInput(next: string) {
    draftValue = next;
    oninput?.(next);
  }

  function handlePickerCancel(restoredValue: string) {
    draftValue = restoredValue;
    oncancel?.();
  }

  // ── Variable suggestions ─────────────────────────────────────────────────

  const filteredSuggestions = $derived.by(() => {
    const query = draftValue.trim().replace(/^\$/, "").toLowerCase();
    if (!query) return suggestions;
    return suggestions.filter((s) =>
      s.name.toLowerCase().includes(query) || s.value.toLowerCase().includes(query)
    );
  });

  function selectSuggestion(variable: ScssVariable) {
    const next = `$${variable.name}`;
    draftValue  = next;
    oninput?.(next);
    oncommit?.(next);
    showSuggestions = false;
    document.getElementById(inputId)?.focus();
  }

  function handleFocusOut(event: FocusEvent) {
    const next = event.relatedTarget;
    if (next instanceof Node && root?.contains(next)) return;
    if (pickerOpen) return;
    showSuggestions = false;
    if (skipNextCommit) {
      skipNextCommit = false;
      return;
    }
    oncommit?.(draftValue);
  }

  function cancelEdit(input: HTMLInputElement) {
    skipNextCommit = true;
    draftValue = value;
    showSuggestions = false;
    oncancel?.();
    input.blur();
  }
</script>

<div class="color-input" class:has-value={!!draftValue} bind:this={root} onfocusout={handleFocusOut}>
  <PanaColorPicker
    value={resolvedColor ?? "#000000"}
    empty={!resolvedColor}
    joined
    width={26}
    height={24}
    label="Alege culoarea"
    oninput={handlePickerInput}
    oncommit={(next) => oncommit?.(next)}
    oncancel={handlePickerCancel}
    onopenchange={(open) => { pickerOpen = open; }}
  />

  {#if suggestions.length}
    <button
      type="button"
      class="var-btn"
      title="Inserează variabilă SCSS"
      onclick={() => {
        showSuggestions = !showSuggestions;
        document.getElementById(inputId)?.focus();
      }}
    ><IconBolt size={11} stroke={2} /></button>
  {/if}

  <input
    id={inputId}
    type="text"
    class="color-field"
    value={draftValue}
    placeholder="—"
    autocomplete="off"
    onfocus={() => { focused = true; if (filteredSuggestions.length) showSuggestions = true; }}
    onblur={() => { focused = false; }}
    onkeydown={(e) => {
      if (e.key === "Escape") {
        e.preventDefault();
        cancelEdit(e.currentTarget);
      } else if (e.key === "Enter") {
        e.preventDefault();
        e.currentTarget.blur();
      }
    }}
    oninput={(e) => {
      draftValue = e.currentTarget.value;
      oninput?.(draftValue);
    }}
  />

  {#if showSuggestions && filteredSuggestions.length}
    <VariablePopover anchor={root} suggestions={filteredSuggestions} onselect={selectSuggestion} />
  {/if}
</div>

<style>
  .color-input {
    position: relative;
    display: flex;
    align-items: center;
    border: 1px solid var(--border-4);
    border-radius: 6px;
    background: var(--surface-8);
    overflow: visible;
    min-width: 0;
  }

  .color-input:focus-within {
    border-color: var(--brand);
  }

  /* ── Variable button ────────────────────────────────────────────────── */

  .var-btn {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 24px;
    padding: 0;
    border: none;
    border-right: 1px solid var(--border-4);
    font-size: 12px;
    line-height: 1;
    background: var(--surface-4);
    cursor: pointer;
    color: var(--text-muted);
  }

  .var-btn:hover {
    background: var(--brand-soft);
    color: var(--brand-strong);
  }

  /* ── Text field ─────────────────────────────────────────────────────── */

  .color-field {
    flex: 1;
    min-width: 0;
    height: 24px;
    padding: 0 6px;
    border: none;
    color: var(--text);
    font-size: 12px;
    background: transparent;
    font-family: "JetBrains Mono", "SFMono-Regular", Consolas, monospace;
    outline: none;
  }
</style>
