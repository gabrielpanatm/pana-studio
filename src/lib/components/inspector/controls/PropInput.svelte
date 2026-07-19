<script lang="ts" module>
  let nextInputId = 0;
</script>

<script lang="ts">
  import type { Snippet } from "svelte";
  import type { ScssVariable } from "$lib/types";
  import { IconBolt } from "@tabler/icons-svelte";
  import VariablePopover from "./VariablePopover.svelte";

  let {
    label = "",
    value = "",
    placeholder = "—",
    type = "text",
    suggestions = [],
    prefix,
    oninput,
    oncommit,
    oncancel,
  }: {
    label?: string;
    value?: string;
    placeholder?: string;
    type?: string;
    suggestions?: ScssVariable[];
    prefix?: Snippet;
    oninput?: (value: string) => void;
    oncommit?: (value: string) => void;
    oncancel?: () => void;
  } = $props();

  const instanceId = nextInputId++;
  const labelSlug = $derived(label ? label.replace(/[^a-z0-9]/gi, "").toLowerCase() : "x");
  const uid = $derived(`${labelSlug || "x"}-${instanceId}`);
  const inputId = $derived(`pi-input-${uid}`);

  let root = $state<HTMLDivElement | null>(null);
  let draftValue = $state("");
  let focused = $state(false);
  let showSuggestions = $state(false);
  let skipNextCommit = false;
  const filteredSuggestions = $derived.by(() => {
    const query = draftValue.trim().replace(/^\$/, "").toLowerCase();
    if (!query) return suggestions;
    return suggestions.filter((variable) => {
      const name = variable.name.toLowerCase();
      const variableValue = variable.value.toLowerCase();
      return name.includes(query) || variableValue.includes(query);
    });
  });

  $effect(() => {
    if (!focused && value !== draftValue) {
      draftValue = value;
    }
  });

  function selectSuggestion(variable: ScssVariable) {
    const nextValue = `$${variable.name}`;
    draftValue = nextValue;
    oninput?.(nextValue);
    oncommit?.(nextValue);
    showSuggestions = false;
    document.getElementById(inputId)?.focus();
  }

  function handleFocusOut(event: FocusEvent) {
    const nextTarget = event.relatedTarget;
    if (nextTarget instanceof Node && root?.contains(nextTarget)) return;
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

<div class="prop-field" bind:this={root} onfocusout={handleFocusOut}>
  {#if prefix || label}
    <span class="prop-prefix">
      {#if prefix}
        {@render prefix()}
      {:else}
        {label}
      {/if}
    </span>
  {/if}
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
    class="prop-input"
    {type}
    value={draftValue}
    {placeholder}
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
  .prop-field {
    position: relative;
    display: flex;
    align-items: center;
    border: 1px solid var(--border-4);
    border-radius: 6px;
    overflow: visible;
    background: var(--surface-8);
    min-width: 0;
  }

  .prop-field:focus-within {
    border-color: var(--brand);
  }

  .prop-prefix {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0 5px;
    border-right: 1px solid var(--border-4);
    color: var(--text-muted);
    font-size: 10px;
    font-weight: 600;
    line-height: 24px;
    white-space: nowrap;
    background: var(--surface-4);
    user-select: none;
    min-width: 24px;
    height: 24px;
    flex-shrink: 0;
    font-family: "JetBrains Mono", "SFMono-Regular", Consolas, monospace;
  }

  .var-btn {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    margin: 0 2px;
    padding: 0;
    border: 1px solid var(--border-4);
    border-radius: 4px;
    font-size: 11px;
    line-height: 1;
    background: var(--surface-4);
    cursor: pointer;
    color: var(--text-muted);
  }

  .var-btn:hover {
    border-color: var(--brand-strong);
    background: var(--brand-soft);
    color: var(--brand-strong);
  }

  .prop-input {
    flex: 1;
    min-width: 0;
    height: 24px;
    padding: 0 5px;
    border: none;
    color: var(--text);
    font-size: 12px;
    background: transparent;
    font-family: "JetBrains Mono", "SFMono-Regular", Consolas, monospace;
  }

  .prop-input:focus {
    outline: none;
  }

</style>
