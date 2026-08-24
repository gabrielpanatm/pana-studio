<script lang="ts">
  import type { Snippet } from "svelte";
  import OptionsPopover from "./OptionsPopover.svelte";

  let {
    label = "",
    value = "",
    placeholder = "—",
    options = [],
    disabled = false,
    prefix,
    oninput,
    oncommit,
    oncancel,
  }: {
    label?: string;
    value?: string;
    placeholder?: string;
    options?: string[];
    disabled?: boolean;
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

<div class="tfo-field ui-control-group compact" class:disabled bind:this={root} onfocusout={handleFocusOut}>
  {#if prefix || label}
    <span class="tfo-prefix ui-control-affix ui-control-prefix code">
      {#if prefix}
        {@render prefix()}
      {:else}
        {label}
      {/if}
    </span>
  {/if}

  <input
    class="tfo-input ui-control-input code"
    type="text"
    value={draftValue}
    {placeholder}
    autocomplete="off"
    {disabled}
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
