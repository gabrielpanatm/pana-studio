<script lang="ts">
  import {
    IconArrowDown,
    IconArrowUp,
    IconCopy,
    IconPlus,
    IconTrash,
  } from "@tabler/icons-svelte";
  import type { EditorActionOutcome } from "$lib/editor-runtime/action-outcome";
  import { t } from "$lib/i18n/runtime.svelte";
  import type {
    NativeBlockSlotMutationRequest,
    UiBlockSourceInstance,
  } from "$lib/blocks/contracts";

  let {
    sourceInstance,
    modelRevision,
    disabled = false,
    onMutate,
  }: {
    sourceInstance: UiBlockSourceInstance;
    modelRevision: string;
    disabled?: boolean;
    onMutate: (request: NativeBlockSlotMutationRequest) => Promise<EditorActionOutcome>;
  } = $props();

  let selectedIndex = $state(0);
  let pending = $state(false);
  let status = $state("");
  const slot = $derived(sourceInstance.slots.find((candidate) => candidate.id === "slides") ?? null);
  const selected = $derived(slot?.items[selectedIndex] ?? slot?.items[0] ?? null);

  $effect(() => {
    const count = slot?.items.length ?? 0;
    if (count === 0) selectedIndex = 0;
    else if (selectedIndex >= count) selectedIndex = count - 1;
  });

  function request(
    operation: NativeBlockSlotMutationRequest["operation"],
    options: Partial<NativeBlockSlotMutationRequest> = {},
  ): NativeBlockSlotMutationRequest | null {
    if (!slot || !sourceInstance.rootSourceNodeId) return null;
    return {
      operation,
      context: {
        providerId: sourceInstance.providerId,
        slotId: slot.id,
        rootSourceId: sourceInstance.rootSourceNodeId,
        expectedModelRevision: modelRevision,
      },
      slot,
      ...options,
    };
  }

  async function mutate(next: NativeBlockSlotMutationRequest | null) {
    if (!next || pending || disabled || !slot?.editable) return;
    pending = true;
    status = t("inspector-slider-validating-rust");
    const outcome = await onMutate(next);
    pending = false;
    status = outcome.status === "committed"
      ? t("inspector-slider-committed")
      : outcome.reason ?? t("inspector-slider-failed");
  }

  function add() {
    void mutate(request("insert"));
  }

  function duplicate() {
    if (!selected) return;
    void mutate(request("duplicate", { item: selected }));
  }

  function remove() {
    if (!selected) return;
    void mutate(request("delete", { item: selected }));
  }

  function move(delta: -1 | 1) {
    if (!slot || !selected) return;
    const target = slot.items[selectedIndex + delta];
    if (!target) return;
    void mutate(request("move", {
      item: selected,
      targetItem: target,
      position: delta < 0 ? "before" : "after",
    }));
  }
</script>

<section class="slider-editor" aria-label={t("inspector-slider-slides")}>
  <div class="slider-editor__heading">
    <div>
      <strong>{t("inspector-slider-slides")}</strong>
      <small>{t("inspector-slider-count", { count: slot?.items.length ?? 0 })}</small>
    </div>
    <button
      type="button"
      title={t("inspector-slider-add")}
      aria-label={t("inspector-slider-add")}
      disabled={disabled || pending || !slot?.editable || (slot?.maximumItems != null && slot.items.length >= slot.maximumItems)}
      onclick={add}
    ><IconPlus size={15} /></button>
  </div>

  {#if !slot}
    <p class="diagnostic">{t("inspector-slider-slot-missing")}</p>
  {:else if slot.diagnostic}
    <p class="diagnostic">{slot.diagnostic}</p>
  {:else}
    <div class="slide-list" role="listbox" aria-label={t("inspector-slider-slides")}>
      {#each slot.items as item, index (item.sourceNodeId)}
        <button
          type="button"
          role="option"
          aria-selected={selectedIndex === index}
          class:active={selectedIndex === index}
          onclick={() => (selectedIndex = index)}
        >
          <span>{item.label}</span>
          <small>{index + 1}</small>
        </button>
      {/each}
    </div>
    <div class="slide-actions">
      <button type="button" title={t("inspector-slider-move-up")} aria-label={t("inspector-slider-move-up")} disabled={disabled || pending || selectedIndex <= 0} onclick={() => move(-1)}><IconArrowUp size={14} /></button>
      <button type="button" title={t("inspector-slider-move-down")} aria-label={t("inspector-slider-move-down")} disabled={disabled || pending || selectedIndex >= slot.items.length - 1} onclick={() => move(1)}><IconArrowDown size={14} /></button>
      <button type="button" title={t("inspector-slider-duplicate")} aria-label={t("inspector-slider-duplicate")} disabled={disabled || pending || !selected || (slot.maximumItems != null && slot.items.length >= slot.maximumItems)} onclick={duplicate}><IconCopy size={14} /></button>
      <button class="danger" type="button" title={t("inspector-slider-delete")} aria-label={t("inspector-slider-delete")} disabled={disabled || pending || !selected || slot.items.length <= slot.minimumItems} onclick={remove}><IconTrash size={14} /></button>
    </div>
  {/if}
  {#if status}<p class="status" aria-live="polite">{status}</p>{/if}
</section>

<style>
  .slider-editor { display: grid; gap: 7px; margin-bottom: 9px; padding-bottom: 9px; border-bottom: 1px solid var(--border-subtle); }
  .slider-editor__heading { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  .slider-editor__heading > div { display: grid; gap: 2px; }
  strong { color: var(--text-strong); font-size: 11px; }
  small { color: var(--text-muted); font-size: 11px; }
  button { display: grid; min-width: 27px; height: 27px; padding: 0 7px; place-items: center; border: 1px solid var(--border); border-radius: var(--radius-control); color: var(--text); background: var(--surface-2); }
  button:hover:not(:disabled) { background: var(--control-hover); }
  button:disabled { opacity: 0.45; }
  button.danger { color: var(--danger); }
  .slide-list { display: grid; gap: 3px; max-height: 132px; overflow: auto; }
  .slide-list button { display: flex; justify-content: space-between; width: 100%; }
  .slide-list button.active { border-color: var(--brand); background: var(--control-selected); }
  .slide-list span { font-size: 11px; }
  .slide-actions { display: flex; gap: 4px; }
  .diagnostic, .status { margin: 0; font-size: 11px; line-height: 1.35; }
  .diagnostic { color: var(--danger); }
  .status { color: var(--text-muted); }
</style>
