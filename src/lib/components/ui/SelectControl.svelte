<script lang="ts">
  import { tick } from "svelte";

  export type SelectControlOption = {
    value: string;
    label: string;
    detail?: string;
    group?: string;
  };
  export type SelectControlOptionInput = SelectControlOption | string;

  const VIEWPORT_MARGIN = 8;
  const GAP = 4;
  const MAX_HEIGHT = 240;
  const MIN_HEIGHT = 80;
  const OPTION_HEIGHT = 28;
  const GROUP_HEIGHT = 24;
  const POPOVER_CHROME_HEIGHT = 12;

  let {
    value = "",
    options = [],
    placeholder = "Alege",
    disabled = false,
    ariaLabel = "Alege opțiune",
    onchange = undefined as ((value: string) => void) | undefined,
  }: {
    value?: string;
    options?: readonly SelectControlOptionInput[];
    placeholder?: string;
    disabled?: boolean;
    ariaLabel?: string;
    onchange?: (value: string) => void;
  } = $props();

  let root = $state<HTMLDivElement | null>(null);
  let open = $state(false);
  let placement = $state({ left: 0, top: 0, width: 0, maxHeight: 180 });

  const normalizedOptions = $derived(options.map((option) => (
    typeof option === "string"
      ? { value: option, label: option }
      : option
  )));
  const groupHeaderCount = $derived(normalizedOptions.filter((option, index) => (
    Boolean(option.group) && option.group !== normalizedOptions[index - 1]?.group
  )).length);
  const selected = $derived(normalizedOptions.find((option) => option.value === value) ?? null);
  const popoverStyle = $derived(
    `left: ${placement.left}px; top: ${placement.top}px; width: ${placement.width}px; max-height: ${placement.maxHeight}px;`,
  );

  function clamp(value: number, min: number, max: number): number {
    return Math.min(Math.max(value, min), max);
  }

  function horizontalTarget(): HTMLElement | null {
    if (!root) return null;
    return (
      (root.closest("[data-select-popover-scope]") as HTMLElement | null) ??
      (root.closest(".section-body") as HTMLElement | null) ??
      (root.closest("label") as HTMLElement | null) ??
      (root.closest(".inspector-pane") as HTMLElement | null) ??
      root
    );
  }

  function updatePlacement() {
    if (!root || !open) return;

    const anchorRect = root.getBoundingClientRect();
    const targetRect = (horizontalTarget() ?? root).getBoundingClientRect();
    const viewportWidth = window.innerWidth;
    const viewportHeight = window.innerHeight;
    const maxViewportWidth = viewportWidth - VIEWPORT_MARGIN * 2;
    const width = Math.max(anchorRect.width, Math.min(targetRect.width, maxViewportWidth));
    const left = clamp(
      targetRect.left,
      VIEWPORT_MARGIN,
      Math.max(VIEWPORT_MARGIN, viewportWidth - width - VIEWPORT_MARGIN),
    );
    const spaceBelow = viewportHeight - anchorRect.bottom - VIEWPORT_MARGIN;
    const spaceAbove = anchorRect.top - VIEWPORT_MARGIN;
    const openAbove = spaceBelow < 180 && spaceAbove > spaceBelow;
    const available = Math.max(MIN_HEIGHT, openAbove ? spaceAbove : spaceBelow);
    const contentHeight = normalizedOptions.length * OPTION_HEIGHT + groupHeaderCount * GROUP_HEIGHT + POPOVER_CHROME_HEIGHT;
    const maxHeight = Math.max(
      MIN_HEIGHT,
      Math.min(MAX_HEIGHT, contentHeight, Math.max(MIN_HEIGHT, available - GAP)),
    );
    const top = openAbove
      ? Math.max(VIEWPORT_MARGIN, anchorRect.top - GAP - maxHeight)
      : Math.min(anchorRect.bottom + GAP, viewportHeight - VIEWPORT_MARGIN - maxHeight);

    placement = { left, top, width, maxHeight };
  }

  function openMenu() {
    if (!normalizedOptions.length) return;
    open = true;
    tick().then(updatePlacement);
  }

  function toggle() {
    if (disabled) return;
    if (open) {
      open = false;
      return;
    }
    openMenu();
  }

  function select(option: SelectControlOption) {
    onchange?.(option.value);
    open = false;
  }

  function closeFromWindow(event: MouseEvent) {
    if (!open) return;
    const target = event.target;
    if (target instanceof Node && root?.contains(target)) return;
    open = false;
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      open = false;
      return;
    }
    if (disabled) return;
    if ((event.key === "Enter" || event.key === " ") && !open) {
      event.preventDefault();
      openMenu();
    }
  }

  function scrollParents(): HTMLElement[] {
    if (!root) return [];
    const parents = new Set<HTMLElement>();
    for (const selector of [
      ".section-body",
      ".inspector-pane",
      ".step-inspector",
      ".motion-panel",
      ".motion-timeline-pane-shell",
    ]) {
      const parent = root.closest(selector) as HTMLElement | null;
      if (parent) parents.add(parent);
    }
    return Array.from(parents);
  }

  $effect(() => {
    if (!open) return;
    tick().then(updatePlacement);
  });

  $effect(() => {
    if (!open || !root) return;
    const parents = scrollParents();
    for (const parent of parents) {
      parent.addEventListener("scroll", updatePlacement, { passive: true });
    }
    return () => {
      for (const parent of parents) {
        parent.removeEventListener("scroll", updatePlacement);
      }
    };
  });

  $effect(() => {
    if (!open) return;
    const handleWindowClick = (event: MouseEvent) => closeFromWindow(event);
    window.addEventListener("click", handleWindowClick);
    window.addEventListener("resize", updatePlacement);
    window.addEventListener("scroll", updatePlacement, { passive: true });
    return () => {
      window.removeEventListener("click", handleWindowClick);
      window.removeEventListener("resize", updatePlacement);
      window.removeEventListener("scroll", updatePlacement);
    };
  });
</script>

<div bind:this={root} class="select-control-root" class:open>
  <button
    type="button"
    class="select-control"
    {disabled}
    aria-label={ariaLabel}
    aria-haspopup="listbox"
    aria-expanded={open}
    onclick={toggle}
    onkeydown={handleKeydown}
  >
    <span class:placeholder={!selected}>{selected?.label ?? placeholder}</span>
    <span class="select-chevron" aria-hidden="true"></span>
  </button>

  {#if open && normalizedOptions.length}
    <div class="select-popover" role="listbox" aria-label={ariaLabel} style={popoverStyle}>
      {#each normalizedOptions as option, index}
        {#if option.group && option.group !== normalizedOptions[index - 1]?.group}
          <div class="select-group-label">{option.group}</div>
        {/if}
        <button
          type="button"
          class="select-option"
          class:selected={option.value === value}
          role="option"
          aria-selected={option.value === value}
          onmousedown={(event) => event.preventDefault()}
          onclick={() => select(option)}
        >
          <span>{option.label}</span>
          {#if option.detail}
            <small>{option.detail}</small>
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .select-control-root {
    position: relative;
    width: 100%;
    min-width: 0;
    box-sizing: border-box;
  }

  .select-control {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    width: 100%;
    min-width: 0;
    height: 26px;
    padding: 0 7px;
    box-sizing: border-box;
    border: 1px solid var(--border-4);
    border-radius: 6px;
    background: var(--surface-8);
    color: var(--text);
    font: inherit;
    font-size: 11px;
    line-height: 1;
    cursor: pointer;
    text-align: left;
    overflow: hidden;
  }

  .select-control-root.open .select-control,
  .select-control:focus-visible {
    border-color: var(--brand);
    outline: none;
  }

  .select-control:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .select-control span:first-child {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .select-control .placeholder {
    color: var(--text-muted);
  }

  .select-chevron {
    flex: 0 0 auto;
    width: 0;
    height: 0;
    border-right: 5px solid transparent;
    border-left: 5px solid transparent;
    border-top: 6px solid currentColor;
    opacity: 0.72;
  }

  .select-popover {
    position: fixed;
    z-index: 1000;
    overflow: auto;
    box-sizing: border-box;
    padding: 4px;
    border: 1px solid var(--border-4);
    border-radius: 7px;
    background: var(--surface-2);
    box-shadow: 0 12px 30px rgba(0, 0, 0, 0.25);
  }

  .select-option {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    width: 100%;
    height: 28px;
    min-height: 28px;
    padding: 0 8px;
    box-sizing: border-box;
    border: 0;
    border-radius: 5px;
    background: transparent;
    color: var(--text);
    text-align: left;
    cursor: pointer;
    font-size: 11px;
    line-height: 1.15;
    font-family: "JetBrains Mono", "SFMono-Regular", Consolas, monospace;
  }

  .select-option:hover,
  .select-option:focus,
  .select-option.selected {
    background: var(--brand-soft);
    color: var(--brand-strong);
    outline: none;
  }

  .select-option small {
    flex-shrink: 0;
    color: var(--text-muted);
    font-size: 9px;
  }

  .select-group-label {
    display: flex;
    align-items: center;
    height: 24px;
    padding: 4px 8px 2px;
    color: var(--text-muted);
    font-size: 9px;
    font-weight: 900;
    letter-spacing: 0.07em;
    text-transform: uppercase;
  }
</style>
