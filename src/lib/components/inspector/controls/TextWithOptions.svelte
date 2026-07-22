<script lang="ts">
  import type { Snippet } from "svelte";
  import OptionsPopover from "./OptionsPopover.svelte";

  let {
    label = "",
    value = "",
    placeholder = "—",
    options = [],
    prefix,
    oninput,
    oncommit,
    oncancel,
  }: {
    label?: string;
    value?: string;
    placeholder?: string;
    options?: string[];
    prefix?: Snippet;
    oninput?: (value: string) => void;
    oncommit?: (value: string) => void;
    oncancel?: () => void;
  } = $props();

  let root        = $state<HTMLDivElement | null>(null);
  let draftValue  = $state("");
  let focused     = $state(false);
  let showOpts    = $state(false);
  let skipNextCommit = false;

  const filtered = $derived.by(() => {
    const q = draftValue.trim().toLowerCase();
    if (!q) return options;
    return options.filter((o) => o.toLowerCase().includes(q));
  });

  $effect(() => {
    if (!focused && value !== draftValue) {
      draftValue = value;
    }
  });

  function selectOpt(opt: string) {
    draftValue = opt;
    oninput?.(opt);
    oncommit?.(opt);
    showOpts = false;
  }

  function handleFocusOut(e: FocusEvent) {
    const next = e.relatedTarget;
    if (next instanceof Node && root?.contains(next)) return;
    showOpts = false;
    if (skipNextCommit) {
      skipNextCommit = false;
      return;
    }
    oncommit?.(draftValue);
  }

  function cancelEdit(input: HTMLInputElement) {
    skipNextCommit = true;
    draftValue = value;
    showOpts = false;
    oncancel?.();
    input.blur();
  }
</script>

<div class="tfo-field" bind:this={root} onfocusout={handleFocusOut}>
  {#if prefix || label}
    <span class="tfo-prefix">
      {#if prefix}
        {@render prefix()}
      {:else}
        {label}
      {/if}
    </span>
  {/if}

  <input
    class="tfo-input"
    type="text"
    value={draftValue}
    {placeholder}
    autocomplete="off"
    onfocus={() => {
      focused = true;
      if (filtered.length) showOpts = true;
    }}
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
      showOpts = filtered.length > 0;
    }}
  />
</div>

{#if showOpts && filtered.length}
  <OptionsPopover anchor={root} options={filtered} onselect={selectOpt} />
{/if}

<style>
  .tfo-field {
    position: relative;
    display: flex;
    align-items: center;
    border: 1px solid var(--border-4);
    border-radius: 6px;
    overflow: visible;
    background: var(--surface-8);
    min-width: 0;
  }

  .tfo-field:focus-within {
    border-color: var(--brand);
  }

  .tfo-prefix {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0 5px;
    border-right: 1px solid var(--border-4);
    color: var(--text-muted);
    font-size: 12px;
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

  .tfo-input {
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
