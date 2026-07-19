<script lang="ts">
  import { tick } from "svelte";
  import { IconChevronDown, IconPhoto } from "@tabler/icons-svelte";
  import type { ProjectFile } from "$lib/types";
  import {
    assetEditLeaseMatches,
    cancelledAssetEditValue,
    captureAssetEditLease,
    type AssetEditLease,
  } from "$lib/html/asset-edit-session";

  let {
    value = "",
    assets = [],
    assetUrl,
    assetMeta,
    oninput,
    oncommit,
    oncancel,
    commitOnInputMs = 0,
    contextKey = "",
    disabled = false,
  }: {
    value?: string;
    assets?: ProjectFile[];
    assetUrl: (asset: ProjectFile) => string;
    assetMeta?: (asset: ProjectFile) => string | null | undefined;
    oninput: (value: string) => void;
    oncommit?: (value: string) => void | Promise<void>;
    oncancel?: (baselineValue: string, contextKey: string) => void;
    commitOnInputMs?: number;
    contextKey?: string;
    disabled?: boolean;
  } = $props();

  const VIEWPORT_MARGIN = 8;
  const GAP = 4;
  const MAX_HEIGHT = 260;
  const MIN_HEIGHT = 96;
  const OPTION_HEIGHT = 28;
  const POPOVER_PADDING = 8;

  let root = $state<HTMLDivElement | null>(null);
  let open = $state(false);
  let placement = $state({ left: 0, top: 0, width: 0, maxHeight: 180 });
  let commitTimer: number | null = null;
  let skipNextCommit = false;
  let editLease: AssetEditLease | null = null;
  let lastCommittedKey = "";
  let lastCommittedValue = "";

  const filteredAssets = $derived.by(() => {
    const query = value.trim().toLowerCase();
    if (!query) return assets;
    return assets.filter((asset) => {
      const url = assetUrl(asset).toLowerCase();
      return asset.name.toLowerCase().includes(query)
        || asset.relativePath.toLowerCase().includes(query)
        || url.includes(query);
    });
  });

  const popoverStyle = $derived(
    `left: ${placement.left}px; top: ${placement.top}px; width: ${placement.width}px; max-height: ${placement.maxHeight}px;`,
  );

  function clamp(value: number, min: number, max: number) {
    return Math.min(Math.max(value, min), max);
  }

  function updatePlacement() {
    if (!root) return;

    const anchorRect = root.getBoundingClientRect();
    const group = root.closest(".inspector-group") as HTMLElement | null;
    const form = root.closest(".edit-form") as HTMLElement | null;
    const targetRect = (form ?? group ?? root).getBoundingClientRect();
    const viewportWidth = window.innerWidth;
    const viewportHeight = window.innerHeight;
    const width = Math.max(anchorRect.width, Math.min(targetRect.width, viewportWidth - VIEWPORT_MARGIN * 2));
    const left = clamp(targetRect.left, VIEWPORT_MARGIN, Math.max(VIEWPORT_MARGIN, viewportWidth - width - VIEWPORT_MARGIN));
    const spaceBelow = viewportHeight - anchorRect.bottom - VIEWPORT_MARGIN;
    const spaceAbove = anchorRect.top - VIEWPORT_MARGIN;
    const openAbove = spaceBelow < 180 && spaceAbove > spaceBelow;
    const availableSpace = Math.max(MIN_HEIGHT, openAbove ? spaceAbove : spaceBelow);
    const contentHeight = filteredAssets.length * OPTION_HEIGHT + POPOVER_PADDING;
    const maxHeight = Math.min(MAX_HEIGHT, contentHeight, availableSpace - GAP);
    const top = openAbove
      ? Math.max(VIEWPORT_MARGIN, anchorRect.top - GAP - maxHeight)
      : Math.min(anchorRect.bottom + GAP, viewportHeight - VIEWPORT_MARGIN - maxHeight);

    placement = { left, top, width, maxHeight };
  }

  function showPicker() {
    if (disabled || !assets.length) return;
    beginEdit();
    open = true;
    tick().then(updatePlacement);
  }

  function beginEdit() {
    if (assetEditLeaseMatches(editLease, contextKey)) return;
    editLease = captureAssetEditLease(contextKey, value);
    lastCommittedKey = "";
    lastCommittedValue = "";
  }

  function selectAsset(asset: ProjectFile) {
    const url = assetUrl(asset);
    oninput(url);
    commitNow(url);
    open = false;
    root?.querySelector("input")?.focus();
  }

  function clearCommitTimer() {
    if (commitTimer === null) return;
    window.clearTimeout(commitTimer);
    commitTimer = null;
  }

  function commitNow(nextValue: string) {
    clearCommitTimer();
    if (disabled || !assetEditLeaseMatches(editLease, contextKey)) return;
    if (lastCommittedKey === contextKey && lastCommittedValue === nextValue) return;
    lastCommittedKey = contextKey;
    lastCommittedValue = nextValue;
    oncommit?.(nextValue);
  }

  function scheduleCommit(nextValue: string) {
    if (!commitOnInputMs || !oncommit) return;
    clearCommitTimer();
    const scheduledLease = editLease;
    commitTimer = window.setTimeout(() => {
      commitTimer = null;
      if (scheduledLease !== editLease || !assetEditLeaseMatches(scheduledLease, contextKey)) return;
      commitNow(nextValue);
    }, commitOnInputMs);
  }

  function handleFocusOut(event: FocusEvent) {
    const nextTarget = event.relatedTarget;
    if (nextTarget instanceof Node && root?.contains(nextTarget)) return;
    open = false;
  }

  $effect(() => {
    if (!open || !root) return;
    tick().then(updatePlacement);
    const pane = root.closest(".inspector-pane");
    pane?.addEventListener("scroll", updatePlacement, { passive: true });

    return () => {
      pane?.removeEventListener("scroll", updatePlacement);
    };
  });

  $effect(() => {
    const currentContextKey = contextKey;
    if (!editLease || assetEditLeaseMatches(editLease, currentContextKey)) return;
    oncancel?.(cancelledAssetEditValue(editLease), editLease.contextKey);
    clearCommitTimer();
    open = false;
    editLease = null;
  });

  $effect(() => () => clearCommitTimer());
</script>

<svelte:window onresize={updatePlacement} onscroll={updatePlacement} />

<div class="asset-picker" bind:this={root} onfocusout={handleFocusOut}>
  <span class="asset-icon">
    <IconPhoto size={13} stroke={1.8} />
  </span>
  <input
    type="text"
    class="asset-input"
    {value}
    placeholder="/imagini/exemplu.png"
    autocomplete="off"
    {disabled}
    onfocus={showPicker}
    oninput={(event) => {
      beginEdit();
      oninput(event.currentTarget.value);
      scheduleCommit(event.currentTarget.value);
      showPicker();
    }}
    onchange={(event) => {
      if (skipNextCommit) return;
      commitNow(event.currentTarget.value);
    }}
    onblur={(event) => {
      if (skipNextCommit) {
        skipNextCommit = false;
        return;
      }
      commitNow(event.currentTarget.value);
    }}
    onkeydown={(event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        clearCommitTimer();
        skipNextCommit = true;
        open = false;
        if (editLease) {
          const baselineValue = cancelledAssetEditValue(editLease);
          oninput(baselineValue);
          oncancel?.(baselineValue, editLease.contextKey);
        }
        event.currentTarget.blur();
      }
      else if (event.key === "Enter") {
        event.preventDefault();
        open = false;
        commitNow(event.currentTarget.value);
        event.currentTarget.blur();
      }
    }}
  />
  <button
    type="button"
    class="asset-toggle"
    title="Alege imagine din proiect"
    disabled={disabled || !assets.length}
    onclick={() => {
      open = !open;
      if (open) tick().then(updatePlacement);
      root?.querySelector("input")?.focus();
    }}
  >
    <IconChevronDown size={12} stroke={1.8} />
  </button>
</div>

{#if open && filteredAssets.length}
  <div class="asset-popover" role="listbox" aria-label="Imagini din proiect" style={popoverStyle}>
    {#each filteredAssets as asset}
      {@const url = assetUrl(asset)}
      {@const meta = assetMeta?.(asset) ?? ""}
      <button
        type="button"
        class="asset-option"
        title={`${asset.relativePath} -> ${url}`}
        onmousedown={(event) => event.preventDefault()}
        onclick={() => selectAsset(asset)}
      >
        <span class="asset-name">{asset.name}</span>
        <span class="asset-path">{url}</span>
        {#if meta}<span class="asset-origin">{meta}</span>{/if}
      </button>
    {/each}
  </div>
{/if}

<style>
  .asset-picker {
    display: flex;
    align-items: center;
    min-width: 0;
    border: 1px solid var(--border-4);
    border-radius: 6px;
    background: var(--surface-8);
  }

  .asset-picker:focus-within {
    border-color: var(--brand);
  }

  .asset-picker:has(.asset-input:disabled) {
    opacity: 0.55;
  }

  .asset-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    margin: 0 6px;
    color: var(--text-muted);
  }

  .asset-input {
    flex: 1;
    min-width: 0;
    height: 27px;
    padding: 0 6px 0 0;
    border: none;
    color: var(--text);
    background: transparent;
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    outline: none;
  }

  .asset-toggle {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 27px;
    border: none;
    border-left: 1px solid var(--border-4);
    color: var(--text-muted);
    background: var(--surface-4);
    cursor: pointer;
  }

  .asset-toggle:hover {
    color: var(--brand-strong);
    background: var(--brand-soft);
  }

  .asset-toggle:disabled {
    cursor: default;
    opacity: 0.45;
  }

  .asset-popover {
    position: fixed;
    z-index: 1000;
    overflow: auto;
    padding: 4px;
    border: 1px solid var(--border-4);
    border-radius: 7px;
    background: var(--surface-2);
    box-shadow: 0 12px 30px rgba(0, 0, 0, 0.25);
  }

  .asset-option {
    display: grid;
    grid-template-columns: minmax(0, 0.72fr) minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: 28px;
    padding: 4px 7px;
    border: 0;
    border-radius: 5px;
    color: var(--text);
    background: transparent;
    text-align: left;
    cursor: pointer;
  }

  .asset-option:hover,
  .asset-option:focus {
    background: var(--brand-soft);
    outline: none;
  }

  .asset-name,
  .asset-path,
  .asset-origin {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
  }

  .asset-name {
    font-weight: 700;
    color: var(--brand-strong);
  }

  .asset-path {
    color: var(--text-muted);
  }

  .asset-origin {
    max-width: 92px;
    padding: 2px 5px;
    border: 1px solid var(--border-3);
    border-radius: 999px;
    color: var(--text-muted);
    background: var(--surface-4);
    font-size: 9px;
    font-weight: 800;
  }
</style>
