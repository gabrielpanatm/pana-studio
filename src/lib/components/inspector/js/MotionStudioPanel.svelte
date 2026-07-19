<script lang="ts">
  import {
    MOTION_FAMILIES,
    createMotionItem,
    emptyMotionConfig,
    normalizeMotionConfig,
  } from "$lib/js/motion-config";
  import type { PanaMotionConfig, PanaMotionFamily, PanaMotionItem } from "$lib/types";
  import InspectorSection from "$lib/components/inspector/InspectorSection.svelte";
  import MotionFamilyGroups from "$lib/components/inspector/js/MotionFamilyGroups.svelte";
  import MotionFamilyGuide from "$lib/components/inspector/js/MotionFamilyGuide.svelte";
  import MotionItemEditor from "$lib/components/inspector/js/MotionItemEditor.svelte";

  let {
    motion = emptyMotionConfig(),
    dataAnim = "",
    onChange = undefined as ((motion: PanaMotionConfig) => void) | undefined,
  }: {
    motion?: PanaMotionConfig;
    dataAnim?: string;
    onChange?: (motion: PanaMotionConfig) => void;
  } = $props();

  let activeFamily = $state<PanaMotionFamily>("animation");

  const normalized = $derived(normalizeMotionConfig(motion));
  const visibleItems = $derived(normalized.items.filter((item) => item.type !== "timeline"));
  const elementItems = $derived(visibleItems.filter((item) => itemBelongsToSelectedElement(item)));
  const counts = $derived.by(() => {
    const result: Partial<Record<PanaMotionFamily, number>> = {};
    for (const item of elementItems) {
      result[item.type] = (result[item.type] ?? 0) + 1;
    }
    return result;
  });
  const familyItems = $derived(elementItems.filter((item) => item.type === activeFamily));
  const activeItem = $derived.by(() => {
    const active = elementItems.find((item) => item.id === normalized.activeItemId);
    if (active?.type === activeFamily) return active;
    return familyItems[0] ?? null;
  });
  const activeDefinition = $derived(MOTION_FAMILIES.find((family) => family.type === activeFamily));

  function emit(next: PanaMotionConfig) {
    onChange?.(normalizeMotionConfig(next));
  }

  function selectorTargetsDataAnim(selector: string, value: string): boolean {
    const trimmed = selector.trim();
    if (!trimmed || !value) return false;
    return trimmed.includes(`[data-anim="${value}"]`)
      || trimmed.includes(`[data-anim='${value}']`)
      || trimmed.includes(`[data-anim=${value}]`);
  }

  function itemBelongsToSelectedElement(item: PanaMotionItem): boolean {
    if (!dataAnim) return false;
    const target = item.target;
    if (target?.dataAnim === dataAnim) return true;
    return selectorTargetsDataAnim(target?.selector ?? "", dataAnim);
  }

  function setActiveFamily(family: PanaMotionFamily) {
    if (family === "timeline") return;
    activeFamily = family;
    const first = elementItems.find((item) => item.type === family);
    emit({ ...normalized, activeItemId: first?.id ?? null });
  }

  function addItem(type = activeFamily) {
    if (type === "timeline") return;
    const item = createMotionItem(type, dataAnim);
    emit({
      ...normalized,
      activeItemId: item.id,
      items: [...normalized.items, item],
    });
  }

  function updateItem(item: PanaMotionItem) {
    emit({
      ...normalized,
      activeItemId: item.id,
      items: normalized.items.map((entry) => entry.id === item.id ? item : entry),
    });
  }

  function deleteItem(id: string) {
    const items = normalized.items.filter((item) => item.id !== id);
    const nextElementItems = items
      .filter((item) => item.type !== "timeline")
      .filter((item) => itemBelongsToSelectedElement(item));
    emit({
      ...normalized,
      activeItemId: nextElementItems.find((item) => item.type === activeFamily)?.id ?? nextElementItems[0]?.id ?? null,
      items,
    });
  }

  function selectItem(id: string) {
    const item = elementItems.find((entry) => entry.id === id);
    if (!item) return;
    emit({ ...normalized, activeItemId: id });
  }
</script>

<InspectorSection title="Efecte Motion" hasValues={normalized.items.length > 0}>
  <div class="motion-studio">
    <div class="studio-meta">
      <span>Anime.js {normalized.animeVersion}</span>
      <strong>{elementItems.length} efect{elementItems.length === 1 ? "" : "e"} pe element</strong>
    </div>

    <MotionFamilyGroups {activeFamily} {counts} onSelect={setActiveFamily} />

    <div class="family-card">
      <div>
        <span>{activeDefinition?.label ?? activeFamily}</span>
        <p>{activeDefinition?.description}</p>
        {#if activeFamily === "animation"}
          <small>Pentru scroll scrub: setează Trigger = scroll și Scroll mode = scrub.</small>
        {:else if activeFamily === "scroll"}
          <small>onScroll este pentru observer/callback-uri avansate. Scrub-ul simplu stă în Animation.</small>
        {/if}
      </div>
      <button type="button" onclick={() => addItem()}>+ adaugă</button>
    </div>

    <MotionFamilyGuide family={activeDefinition} />

    {#if familyItems.length > 0}
      <div class="item-list">
        {#each familyItems as item}
          <button
            type="button"
            class:active={activeItem?.id === item.id}
            onclick={() => selectItem(item.id)}
          >
            <span>{item.name}</span>
            <small>{item.enabled ? "activ" : "oprit"}</small>
          </button>
        {/each}
      </div>
    {/if}

    {#if activeItem}
      <MotionItemEditor item={activeItem} onChange={updateItem} onDelete={deleteItem} />
    {:else}
      <div class="empty-family">
        <span>Elementul selectat nu are {activeDefinition?.label ?? activeFamily}.</span>
        <button type="button" onclick={() => addItem()}>Creează {activeDefinition?.shortLabel ?? activeFamily}</button>
      </div>
    {/if}
  </div>
</InspectorSection>

<style>
  .motion-studio {
    display: flex;
    flex-direction: column;
    gap: 9px;
    min-width: 0;
  }

  .studio-meta,
  .family-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .studio-meta {
    font-size: 10px;
    color: var(--text-muted);
  }

  .studio-meta strong {
    color: var(--brand-strong);
  }

  .family-card {
    padding: 8px;
    border: 1px solid var(--border-3);
    border-radius: 7px;
    background: var(--surface-3);
  }

  .family-card span {
    display: block;
    font-size: 11px;
    font-weight: 900;
    color: var(--text);
  }

  .family-card p {
    margin: 3px 0 0;
    font-size: 10px;
    line-height: 1.4;
    color: var(--text-muted);
  }

  .family-card small {
    display: block;
    margin-top: 5px;
    font-size: 10px;
    line-height: 1.35;
    color: var(--brand-strong);
  }

  .family-card button,
  .empty-family button {
    min-height: 25px;
    border: 1px solid var(--brand);
    border-radius: 6px;
    background: var(--brand-soft);
    color: var(--brand-strong);
    font-size: 10px;
    font-weight: 900;
    cursor: pointer;
    white-space: nowrap;
  }

  .item-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .item-list button {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    min-height: 28px;
    border: 1px solid var(--border-3);
    border-radius: 6px;
    background: var(--surface-5);
    color: var(--text);
    cursor: pointer;
    text-align: left;
  }

  .item-list button.active {
    border-color: var(--brand);
    background: var(--brand-soft);
  }

  .item-list span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11px;
    font-weight: 800;
  }

  .item-list small {
    color: var(--text-muted);
    font-size: 10px;
  }

  .empty-family {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 14px 8px;
    border: 1px dashed var(--border-4);
    border-radius: 7px;
    color: var(--text-muted);
    font-size: 11px;
    text-align: center;
  }
</style>
