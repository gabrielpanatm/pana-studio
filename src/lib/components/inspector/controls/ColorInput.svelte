<script lang="ts" module>
  let nextInputId = 0;
</script>

<script lang="ts">
  import type { ScssVariable } from "$lib/css/contracts";
  import { IconBolt } from "@tabler/icons-svelte";
  import { resolvePickerColor } from "$lib/inspector/color-picker-model";
  import PanaColorPicker from "$lib/components/ui/PanaColorPicker.svelte";
  import VariablePopover from "./VariablePopover.svelte";
  import { t } from "$lib/i18n/runtime.svelte";

  let {
    property,
    value = "",
    suggestions = [],
    resolutionVariables,
    oninput,
    oncommit,
    oncancel,
  }: {
    property: string;
    value?: string;
    suggestions?: ScssVariable[];
    resolutionVariables?: ScssVariable[];
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
  const resolvedColor = $derived(resolvePickerColor(
    draftValue,
    resolutionVariables ?? suggestions,
  ));

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

<div class="color-input ui-control-group compact" class:has-value={!!draftValue} bind:this={root} onfocusout={handleFocusOut}>
  <PanaColorPicker
    value={resolvedColor ?? "#000000"}
    empty={!resolvedColor}
    joined
    width={26}
    height={24}
    label={t("inspector-color-choose")}
    oninput={handlePickerInput}
    oncommit={(next) => oncommit?.(next)}
    oncancel={handlePickerCancel}
    onopenchange={(open) => { pickerOpen = open; }}
  />

  {#if suggestions.length}
    <button
      type="button"
      class="var-btn ui-icon-button compact quiet ui-control-action"
      title={t("inspector-insert-scss-variable")}
      onclick={() => {
        showSuggestions = !showSuggestions;
        document.getElementById(inputId)?.focus();
      }}
    ><IconBolt size={11} stroke={2} /></button>
  {/if}

  <input
    id={inputId}
    type="text"
    class="color-field ui-control-input code"
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
  }
</style>
