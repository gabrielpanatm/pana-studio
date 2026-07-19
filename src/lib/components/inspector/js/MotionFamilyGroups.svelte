<script lang="ts">
  import { MOTION_ELEMENT_FAMILY_GROUPS, MOTION_FAMILIES } from "$lib/js/motion-config";
  import type { MotionFamilyDefinition, MotionFamilyGroup } from "$lib/js/motion-config";
  import type { PanaMotionFamily } from "$lib/types";

  let {
    activeFamily = "animation" as PanaMotionFamily,
    groups = MOTION_ELEMENT_FAMILY_GROUPS,
    counts = {} as Partial<Record<PanaMotionFamily, number>>,
    onSelect = undefined as ((family: PanaMotionFamily) => void) | undefined,
  }: {
    activeFamily?: PanaMotionFamily;
    groups?: MotionFamilyGroup[];
    counts?: Partial<Record<PanaMotionFamily, number>>;
    onSelect?: (family: PanaMotionFamily) => void;
  } = $props();

  let advancedOpen = $state(false);
  const activeGroupId = $derived(groups.find((group) => group.families.includes(activeFamily))?.id ?? "");
  const showAdvanced = $derived(advancedOpen || activeGroupId === "advanced");

  function definition(type: PanaMotionFamily): MotionFamilyDefinition | undefined {
    return MOTION_FAMILIES.find((family) => family.type === type);
  }
</script>

<div class="motion-family-groups" aria-label="Efecte Motion">
  {#each groups as group}
    {#if group.id === "advanced"}
      <section class="family-group advanced-group">
        <button type="button" class="advanced-toggle" class:active={activeGroupId === "advanced"} onclick={() => { advancedOpen = !advancedOpen; }}>
          <span>{group.label}</span>
          <strong>{showAdvanced ? "▴" : "▾"}</strong>
        </button>
        {#if showAdvanced}
          <div class="group-grid">
            {#each group.families as type}
              {@const family = definition(type)}
              {#if family}
                <button
                  type="button"
                  class:active={activeFamily === family.type}
                  title={family.when}
                  onclick={() => onSelect?.(family.type)}
                >
                  <span>{family.label}</span>
                  {#if counts[family.type]}
                    <em>{counts[family.type]}</em>
                  {/if}
                </button>
              {/if}
            {/each}
          </div>
        {/if}
      </section>
    {:else}
      <section class="family-group" class:primary-group={group.id === "primary"}>
        <span class="group-label">{group.label}</span>
        <div class="group-grid">
          {#each group.families as type}
            {@const family = definition(type)}
            {#if family}
              <button
                type="button"
                class:active={activeFamily === family.type}
                class:primary-family={group.id === "primary"}
                title={family.when}
                onclick={() => onSelect?.(family.type)}
              >
                <span>{family.label}</span>
                {#if counts[family.type]}
                  <em>{counts[family.type]}</em>
                {/if}
              </button>
            {/if}
          {/each}
        </div>
      </section>
    {/if}
  {/each}
</div>

<style>
  .motion-family-groups {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }

  .family-group {
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
  }

  .group-label {
    font-size: 9px;
    font-weight: 900;
    letter-spacing: 0.07em;
    color: var(--text-muted);
    text-transform: uppercase;
  }

  .group-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 5px;
  }

  .primary-group .group-grid {
    grid-template-columns: 1fr;
  }

  .group-grid button {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: flex-start;
    min-width: 0;
    min-height: 31px;
    padding: 0 12px;
    border: 1px solid var(--border-4);
    border-radius: 7px;
    background: var(--surface-5);
    color: var(--text);
    cursor: pointer;
    overflow: hidden;
  }

  .group-grid button.active {
    border-color: var(--brand);
    background: var(--brand-soft);
    color: var(--brand-strong);
  }

  .group-grid button.primary-family {
    min-height: 38px;
    border-color: color-mix(in srgb, var(--brand) 64%, var(--border-4));
    background: color-mix(in srgb, var(--brand-soft) 58%, var(--surface-5));
  }

  .group-grid span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 10px;
    font-weight: 850;
  }

  .group-grid em {
    position: absolute;
    top: -1px;
    right: -1px;
    min-width: 15px;
    height: 15px;
    border-radius: 999px;
    background: var(--brand);
    color: white;
    font-size: 9px;
    font-style: normal;
    line-height: 15px;
    text-align: center;
  }

  .advanced-toggle {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    min-height: 29px;
    padding: 0 10px;
    border: 1px solid var(--border-4);
    border-radius: 7px;
    background: var(--surface-5);
    color: var(--text-muted);
    cursor: pointer;
  }

  .advanced-toggle.active {
    border-color: var(--brand);
    background: var(--brand-soft);
    color: var(--brand-strong);
  }

  .advanced-toggle span {
    font-size: 9px;
    font-weight: 900;
    letter-spacing: 0.07em;
    text-transform: uppercase;
  }

  .advanced-toggle strong {
    font-size: 11px;
  }
</style>
