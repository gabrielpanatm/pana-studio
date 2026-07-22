<script lang="ts">
  import type { MotionFamilyDefinition } from "$lib/js/motion-config";

  let {
    family,
  }: {
    family: MotionFamilyDefinition | undefined;
  } = $props();
</script>

{#if family}
  <aside class="motion-family-guide" aria-label="Ghid Motion">
    <div class="guide-row">
      <span>Rol</span>
      <p>{family.role}</p>
    </div>
    <div class="guide-row">
      <span>Când</span>
      <p>{family.when}</p>
    </div>
    <div class="guide-row">
      <span>Model mental</span>
      <p>{family.mentalModel}</p>
    </div>
  </aside>
{/if}

<style>
  .motion-family-guide {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px;
    border: 1px solid var(--border-3);
    border-radius: 7px;
    background: color-mix(in srgb, var(--brand-soft) 48%, var(--surface-4));
  }

  .guide-row {
    display: grid;
    grid-template-columns: 68px minmax(0, 1fr);
    gap: 7px;
    align-items: start;
  }

  .guide-row span {
    padding-top: 1px;
    font-size: 12px;
    font-weight: 900;
    letter-spacing: 0.06em;
    color: var(--brand-strong);
    text-transform: uppercase;
  }

  .guide-row p {
    margin: 0;
    font-size: 12px;
    line-height: 1.45;
    color: var(--text);
  }
</style>
