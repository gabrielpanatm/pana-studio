<script lang="ts">
  import SelectControl from "$lib/components/ui/SelectControl.svelte";

  let {
    label,
    value = "",
    mode = "readonly",
    options = [],
    emptyLabel = "Nedetectat",
    title = "",
    onOpen = undefined as (() => void) | undefined,
    onChange = undefined as ((value: string) => void) | undefined,
  }: {
    label: string;
    value?: string | null;
    mode?: "readonly" | "select";
    options?: string[];
    emptyLabel?: string;
    title?: string;
    onOpen?: () => void;
    onChange?: (value: string) => void;
  } = $props();

  const displayValue = $derived(value?.trim() || emptyLabel);
  const selectOptions = $derived(value && !options.includes(value) ? [value, ...options] : options);
</script>

<div class="source-field">
  <span class="source-label">{label}</span>

  {#if mode === "select"}
    <div class="source-control-wrap" title={title || displayValue}>
      <SelectControl
      value={value ?? ""}
      options={selectOptions.length === 0 ? [{ value: "", label: emptyLabel }] : selectOptions}
      disabled={selectOptions.length === 0}
      ariaLabel={label}
      onchange={(nextValue) => onChange?.(nextValue)}
    />
    </div>
  {:else if onOpen && value}
    <button
      type="button"
      class="source-control source-button"
      title={title || displayValue}
      onclick={onOpen}
    >
      {displayValue}
    </button>
  {:else}
    <span class="source-control source-readonly" title={title || displayValue}>
      {displayValue}
    </span>
  {/if}
</div>

<style>
  .source-field {
    display: grid;
    gap: 5px;
  }

  .source-label {
    margin: 0;
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 900;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .source-control {
    width: 100%;
    min-height: 30px;
    padding: 0 8px;
    border: 1px solid var(--border-4);
    border-radius: 7px;
    color: var(--text);
    font-family: "JetBrains Mono", "SFMono-Regular", Consolas, monospace;
    font-size: 12px;
    background: var(--surface-5);
  }

  .source-control-wrap {
    width: 100%;
    min-width: 0;
  }

  .source-button,
  .source-readonly {
    display: flex;
    align-items: center;
    min-width: 0;
    color: var(--brand-strong);
    background: color-mix(in srgb, var(--brand) 8%, transparent);
    border-color: color-mix(in srgb, var(--brand) 25%, transparent);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .source-button {
    cursor: pointer;
    text-align: left;
  }

  .source-button:hover {
    background: color-mix(in srgb, var(--brand) 15%, transparent);
  }

  .source-readonly {
    color: var(--text-muted);
  }
</style>
