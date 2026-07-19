<script lang="ts">
  import { MOTION_FAMILIES, type MotionFamilyDefinition } from "$lib/js/motion-config";
  import type { PanaMotionFamily } from "$lib/types";

  let {
    activeFamily = "animation" as PanaMotionFamily,
    families = MOTION_FAMILIES,
    counts = {} as Partial<Record<PanaMotionFamily, number>>,
    onSelect = undefined as ((family: PanaMotionFamily) => void) | undefined,
  }: {
    activeFamily?: PanaMotionFamily;
    families?: MotionFamilyDefinition[];
    counts?: Partial<Record<PanaMotionFamily, number>>;
    onSelect?: (family: PanaMotionFamily) => void;
  } = $props();
</script>

<div class="family-rail" aria-label="Familii Anime.js">
  {#each families as family}
    <button
      type="button"
      class:active={activeFamily === family.type}
      title={`${family.label}: ${family.when}`}
      onclick={() => onSelect?.(family.type)}
    >
      <span>{family.shortLabel}</span>
      {#if counts[family.type]}
        <strong>{counts[family.type]}</strong>
      {/if}
    </button>
  {/each}
</div>

<style>
  .family-rail {
    display: flex;
    gap: 4px;
    overflow-x: auto;
    padding: 2px 0 8px;
  }

  .family-rail button {
    position: relative;
    min-width: 54px;
    min-height: 28px;
    border: 1px solid var(--border-4);
    border-radius: 6px;
    background: var(--surface-5);
    color: var(--text-muted);
    font-size: 10px;
    font-weight: 800;
    cursor: pointer;
    white-space: nowrap;
  }

  .family-rail button.active {
    border-color: var(--brand);
    background: var(--brand-soft);
    color: var(--brand-strong);
  }

  .family-rail strong {
    position: absolute;
    top: -5px;
    right: -3px;
    min-width: 14px;
    height: 14px;
    border-radius: 999px;
    background: var(--brand);
    color: white;
    font-size: 9px;
    line-height: 14px;
  }
</style>
