<script lang="ts">
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  type IconComponent = new (...args: any[]) => any;

  type Option = {
    value: string;
    label?: string;
    icon?: IconComponent;
    title?: string;
  };

  let {
    options,
    value = "",
    onchange,
    toggleable = true,
  }: {
    options: Option[];
    value?: string;
    onchange?: (value: string) => void;
    toggleable?: boolean;
  } = $props();

  function selectValue(nextValue: string) {
    onchange?.(toggleable && value === nextValue ? "" : nextValue);
  }
</script>

<div class="segmented">
  {#each options as opt}
    <button
      type="button"
      class="seg-btn"
      class:active={value === opt.value}
      title={opt.title ?? opt.label ?? opt.value}
      aria-pressed={value === opt.value}
      onclick={() => selectValue(opt.value)}
    >
      {#if opt.icon}
        <opt.icon size={13} stroke={1.8} />
      {:else}
        {opt.label ?? opt.value}
      {/if}
    </button>
  {/each}
</div>

<style>
  .segmented {
    display: flex;
    width: 100%;
    gap: 2px;
    padding: 2px;
    background: var(--surface-4);
    border: 1px solid var(--border-4);
    border-radius: 7px;
    min-width: 0;
    overflow: hidden;
    box-sizing: border-box;
  }

  .seg-btn {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 22px;
    min-width: 0;
    padding: 0 3px;
    border: 1px solid transparent;
    border-radius: 5px;
    color: var(--text-muted);
    font-size: 12px;
    background: transparent;
    cursor: pointer;
    line-height: 1;
    transition: color 0.1s;
    white-space: nowrap;
  }

  .seg-btn:hover {
    color: var(--text);
  }

  .seg-btn.active {
    background: var(--surface-2);
    border-color: var(--border-3);
    color: var(--text);
    font-weight: 600;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.15);
  }
</style>
