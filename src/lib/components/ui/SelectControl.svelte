<script lang="ts">
  import { tick } from "svelte";
  import { t } from "$lib/i18n/runtime.svelte";
  import {
    calculateAnchoredPopoverPlacement,
    observeAnchoredPopoverPosition,
  } from "$lib/ui/anchored-popover";

  export type SelectControlOption = {
    value: string;
    label: string;
    detail?: string;
    group?: string;
  };
  export type SelectControlOptionInput = SelectControlOption | string;

  const OPTION_HEIGHT = 28;
  const GROUP_HEIGHT = 24;
  const POPOVER_CHROME_HEIGHT = 12;

  const controlId = $props.id();

  let {
    value = "",
    options = [],
    placeholder = "",
    disabled = false,
    size = "compact",
    ariaLabel = "",
    name = undefined,
    required = false,
    onchange = undefined as ((value: string) => void) | undefined,
  }: {
    value?: string;
    options?: readonly SelectControlOptionInput[];
    placeholder?: string;
    disabled?: boolean;
    size?: "default" | "compact" | "toolbar";
    ariaLabel?: string;
    name?: string;
    required?: boolean;
    onchange?: (value: string) => void;
  } = $props();

  let root = $state<HTMLDivElement | null>(null);
  let trigger = $state<HTMLButtonElement | null>(null);
  let open = $state(false);
  let activeIndex = $state(-1);
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
  const effectivePlaceholder = $derived(placeholder || t("common-choose"));
  const effectiveAriaLabel = $derived(ariaLabel || t("common-choose-option"));
  const listboxId = `${controlId}-listbox`;
  const activeOptionId = $derived(
    open && activeIndex >= 0 ? `${controlId}-option-${activeIndex}` : undefined,
  );
  const popoverStyle = $derived(
    `left: ${placement.left}px; top: ${placement.top}px; width: ${placement.width}px; max-height: ${placement.maxHeight}px;`,
  );

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
    placement = calculateAnchoredPopoverPlacement({
      anchorRect,
      scopeRect: targetRect,
      itemCount: normalizedOptions.length,
      itemHeight: OPTION_HEIGHT,
      groupCount: groupHeaderCount,
      groupHeight: GROUP_HEIGHT,
      chromeHeight: POPOVER_CHROME_HEIGHT,
    });
  }

  function selectedIndex(): number {
    const index = normalizedOptions.findIndex((option) => option.value === value);
    return index >= 0 ? index : 0;
  }

  function revealActiveOption() {
    if (!root || activeIndex < 0) return;
    root
      .querySelector<HTMLElement>(`[data-option-index="${activeIndex}"]`)
      ?.scrollIntoView({ block: "nearest" });
  }

  function openMenu(preferredIndex = selectedIndex()) {
    if (!normalizedOptions.length) return;
    activeIndex = Math.min(Math.max(preferredIndex, 0), normalizedOptions.length - 1);
    open = true;
    tick().then(() => {
      updatePlacement();
      revealActiveOption();
    });
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
    trigger?.focus();
  }

  function selectActive() {
    const option = normalizedOptions[activeIndex];
    if (option) select(option);
  }

  function moveActive(nextIndex: number) {
    if (!normalizedOptions.length) return;
    activeIndex = (nextIndex + normalizedOptions.length) % normalizedOptions.length;
    tick().then(revealActiveOption);
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

    if (event.key === "Tab") {
      open = false;
      return;
    }

    if (event.key === "ArrowDown") {
      event.preventDefault();
      if (!open) openMenu();
      else moveActive(activeIndex + 1);
      return;
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      if (!open) openMenu(selectedIndex() || normalizedOptions.length);
      else moveActive(activeIndex - 1);
      return;
    }

    if (open && event.key === "Home") {
      event.preventDefault();
      moveActive(0);
      return;
    }

    if (open && event.key === "End") {
      event.preventDefault();
      moveActive(normalizedOptions.length - 1);
      return;
    }

    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      if (open) selectActive();
      else openMenu();
    }
  }

  $effect(() => {
    if (!open) return;
    tick().then(updatePlacement);
  });

  $effect(() => {
    if (!open) return;
    if (!normalizedOptions.length) {
      open = false;
      activeIndex = -1;
      return;
    }
    if (activeIndex >= normalizedOptions.length) {
      activeIndex = normalizedOptions.length - 1;
    }
  });

  $effect(() => {
    if (!open || !root) return;
    return observeAnchoredPopoverPosition(root, updatePlacement);
  });

  $effect(() => {
    if (!open) return;
    const handleWindowClick = (event: MouseEvent) => closeFromWindow(event);
    window.addEventListener("click", handleWindowClick);
    return () => {
      window.removeEventListener("click", handleWindowClick);
    };
  });
</script>

<div bind:this={root} class="select-control-root" class:open>
  {#if name}
    <input type="hidden" {name} {value} disabled={disabled} />
  {/if}
  <button
    bind:this={trigger}
    type="button"
    role="combobox"
    class="select-control ui-select-trigger"
    class:compact={size === "compact"}
    class:toolbar={size === "toolbar"}
    {disabled}
    aria-label={effectiveAriaLabel}
    aria-required={required}
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-controls={listboxId}
    aria-activedescendant={activeOptionId}
    onclick={toggle}
    onkeydown={handleKeydown}
  >
    <span class:placeholder={!selected}>{selected?.label ?? effectivePlaceholder}</span>
    <span class="select-chevron" aria-hidden="true"></span>
  </button>

  {#if open && normalizedOptions.length}
    <div id={listboxId} class="select-popover ui-popover" role="listbox" aria-label={effectiveAriaLabel} style={popoverStyle}>
      {#each normalizedOptions as option, index}
        {#if option.group && option.group !== normalizedOptions[index - 1]?.group}
          <div class="select-group-label">{option.group}</div>
        {/if}
        <button
          id={`${controlId}-option-${index}`}
          type="button"
          class="select-option ui-option"
          class:selected={option.value === value}
          class:active={index === activeIndex}
          role="option"
          aria-selected={option.value === value}
          tabindex="-1"
          data-option-index={index}
          onmousedown={(event) => event.preventDefault()}
          onmouseenter={() => { activeIndex = index; }}
          onclick={() => select(option)}
        >
          <span class="select-option-copy">
            <span>{option.label}</span>
            {#if option.detail}<small>{option.detail}</small>{/if}
          </span>
          <span
            class="select-option-check"
            class:visible={option.value === value}
            aria-hidden="true"
          ></span>
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
    line-height: 1;
    cursor: pointer;
    text-align: left;
    overflow: hidden;
  }

  .select-control:disabled {
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

  .select-option {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    height: 28px;
    padding: 0 8px;
    line-height: 1.15;
    font-family: var(--font-mono);
  }

  .select-option small {
    color: var(--text-muted);
    font-size: 11px;
  }

  .select-option-copy {
    display: flex;
    flex: 1 1 auto;
    min-width: 0;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
  }

  .select-option-copy > span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .select-option-check {
    flex: 0 0 auto;
    width: 6px;
    height: 11px;
    margin: -2px 4px 0 10px;
    border-right: 2px solid currentColor;
    border-bottom: 2px solid currentColor;
    opacity: 0;
    transform: rotate(45deg);
    transition: opacity 100ms ease;
  }

  .select-option-check.visible {
    opacity: 1;
  }

  .select-group-label {
    display: flex;
    align-items: center;
    height: 24px;
    padding: 4px 8px 2px;
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 900;
    letter-spacing: 0.07em;
    text-transform: uppercase;
  }
</style>
