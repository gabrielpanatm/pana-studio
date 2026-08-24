<script lang="ts">
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  type IconComponent = new (...args: any[]) => any;

  export type SegmentedControlOption = {
    value: string;
    label?: string;
    icon?: IconComponent;
    title?: string;
  };

  let {
    options,
    value = "",
    compact = true,
    ariaLabel = undefined,
    onchange = undefined,
    toggleable = true,
  }: {
    options: SegmentedControlOption[];
    value?: string;
    compact?: boolean;
    ariaLabel?: string;
    onchange?: (value: string) => void;
    toggleable?: boolean;
  } = $props();

  function selectValue(nextValue: string) {
    onchange?.(toggleable && value === nextValue ? "" : nextValue);
  }
</script>

<div class="ui-segmented" class:compact role="group" aria-label={ariaLabel}>
  {#each options as option}
    <button
      type="button"
      class="ui-segmented-option"
      class:active={value === option.value}
      title={option.title ?? option.label ?? option.value}
      aria-label={option.label ?? option.title ?? option.value}
      aria-pressed={value === option.value}
      onclick={() => selectValue(option.value)}
    >
      {#if option.icon}
        <option.icon size={13} stroke={1.8} />
      {:else}
        {option.label ?? option.value}
      {/if}
    </button>
  {/each}
</div>
