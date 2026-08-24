<script lang="ts" module>
  let nextInputId = 0;
</script>

<script lang="ts">
  import type { Snippet } from "svelte";
  import type { CssPropertySuggestion } from "$lib/css/contracts";
  import { IconBolt } from "@tabler/icons-svelte";
  import VariablePopover from "./VariablePopover.svelte";
  import { t } from "$lib/i18n/runtime.svelte";

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
    suggestions?: CssPropertySuggestion[];
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

  function selectSuggestion(variable: CssPropertySuggestion) {
    const nextValue = variable.insertValue ?? `$${variable.name}`;
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

<div class="prop-field ui-control-group compact" bind:this={root} onfocusout={handleFocusOut}>
  {#if prefix || label}
    <span class="prop-prefix ui-control-affix ui-control-prefix code">
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
    class="prop-input ui-control-input code"
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
