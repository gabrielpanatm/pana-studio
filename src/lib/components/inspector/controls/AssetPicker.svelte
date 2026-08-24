<script lang="ts">
  import { tick } from "svelte";
  import { IconChevronDown, IconPhoto } from "@tabler/icons-svelte";
  import type { ProjectFile } from "$lib/project/lifecycle-contract";
  import { t } from "$lib/i18n/runtime.svelte";
  import {
    calculateAnchoredPopoverPlacement,
    observeAnchoredPopoverPosition,
  } from "$lib/ui/anchored-popover";
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

  const OPTION_HEIGHT = 28;

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

  function updatePlacement() {
    if (!root) return;

    const anchorRect = root.getBoundingClientRect();
    const group = root.closest(".inspector-group") as HTMLElement | null;
    const form = root.closest(".edit-form") as HTMLElement | null;
    const targetRect = (form ?? group ?? root).getBoundingClientRect();
    placement = calculateAnchoredPopoverPlacement({
      anchorRect,
      scopeRect: targetRect,
      itemCount: filteredAssets.length,
      itemHeight: OPTION_HEIGHT,
      minHeight: 96,
      maxHeight: 260,
    });
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
    return observeAnchoredPopoverPosition(root, updatePlacement);
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

<div class="asset-picker ui-control-group compact" class:disabled bind:this={root} onfocusout={handleFocusOut}>
  <span class="asset-icon ui-control-affix ui-control-prefix">
    <IconPhoto size={13} stroke={1.8} />
  </span>
  <input
    type="text"
    class="asset-input ui-control-input code"
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
    class="asset-toggle ui-icon-button compact quiet ui-control-action"
    title={t("inspector-asset-choose-project")}
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
  <div class="asset-popover ui-popover" role="listbox" aria-label={t("inspector-asset-project-images")} style={popoverStyle}>
    {#each filteredAssets as asset}
      {@const url = assetUrl(asset)}
      {@const meta = assetMeta?.(asset) ?? ""}
      <button
        type="button"
        class="asset-option ui-option"
        title={`${asset.relativePath} -> ${url}`}
        onmousedown={(event) => event.preventDefault()}
        onclick={() => selectAsset(asset)}
      >
        <span class="asset-name">{asset.name}</span>
        <span class="asset-path">{url}</span>
        {#if meta}<span class="asset-origin ui-badge">{meta}</span>{/if}
      </button>
    {/each}
  </div>
{/if}

<style>
  .asset-picker {
    position: relative;
  }

  .asset-icon {
    padding-inline: 6px;
  }

  .asset-input {
    padding-left: 6px;
  }

  .asset-toggle {
    border-left-color: var(--border-subtle);
  }

  .asset-option {
    display: grid;
    grid-template-columns: minmax(0, 0.72fr) minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    padding: 4px 7px;
  }

  .asset-name,
  .asset-path,
  .asset-origin {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: 12px;
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
  }
</style>
