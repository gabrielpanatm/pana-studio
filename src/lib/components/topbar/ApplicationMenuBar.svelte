<script lang="ts">
  import {
    legacyTranslator,
    localeRevision,
  } from "$lib/i18n/runtime.svelte";
  import type { CommandCenterAction, CommandCenterAppCommand } from "$lib/workbench/contracts";

  $: t = legacyTranslator($localeRevision);

  export let noProject = false;
  export let canUndo = false;
  export let canRedo = false;
  export let sidebarsAvailable = true;
  export let executeAction: (action: CommandCenterAction) => void | Promise<void> = () => {};
  export let openCommandCenter: () => void = () => {};

  let menuBar: HTMLElement;
  let previousEditingTarget: HTMLElement | null = null;

  function detailsElements() {
    return Array.from(menuBar?.querySelectorAll<HTMLDetailsElement>("details.application-menu") ?? []);
  }

  function closeMenus(except: HTMLDetailsElement | null = null) {
    for (const details of detailsElements()) {
      if (details !== except) details.open = false;
    }
  }

  function rememberEditingTarget() {
    const active = document.activeElement;
    if (!(active instanceof HTMLElement) || menuBar?.contains(active)) return;
    previousEditingTarget = active;
  }

  function handleToggle(event: Event) {
    const details = event.currentTarget;
    if (details instanceof HTMLDetailsElement && details.open) closeMenus(details);
  }

  function handleWindowClick(event: MouseEvent) {
    if (!(event.target instanceof Node) || !menuBar?.contains(event.target)) closeMenus();
  }

  function enabledMenuItems(details: HTMLDetailsElement) {
    return Array.from(details.querySelectorAll<HTMLButtonElement>("button[role='menuitem']:not(:disabled)"));
  }

  function focusSiblingMenu(current: HTMLDetailsElement, offset: number) {
    const menus = detailsElements();
    const index = menus.indexOf(current);
    if (index < 0) return;
    const next = menus[(index + offset + menus.length) % menus.length];
    closeMenus(next);
    next.open = true;
    next.querySelector<HTMLElement>("summary")?.focus();
  }

  function handleKeydown(event: KeyboardEvent) {
    const target = event.target;
    if (!(target instanceof HTMLElement)) return;
    const details = target.closest<HTMLDetailsElement>("details.application-menu");
    if (!details) return;

    if (event.key === "Escape") {
      event.preventDefault();
      details.open = false;
      details.querySelector<HTMLElement>("summary")?.focus();
      return;
    }
    if (event.key === "ArrowRight" || event.key === "ArrowLeft") {
      event.preventDefault();
      focusSiblingMenu(details, event.key === "ArrowRight" ? 1 : -1);
      return;
    }
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;

    event.preventDefault();
    details.open = true;
    const items = enabledMenuItems(details);
    if (items.length === 0) return;
    const current = items.indexOf(target as HTMLButtonElement);
    if (current < 0) {
      items[event.key === "ArrowDown" ? 0 : items.length - 1]?.focus();
      return;
    }
    const offset = event.key === "ArrowDown" ? 1 : -1;
    items[(current + offset + items.length) % items.length]?.focus();
  }

  function runCommand(command: CommandCenterAppCommand) {
    closeMenus();
    void executeAction({ kind: "app_command", command });
  }

  function showCommandCenter() {
    closeMenus();
    openCommandCenter();
  }

  function runEditingCommand(command: "cut" | "copy" | "paste" | "selectAll") {
    closeMenus();
    previousEditingTarget?.focus();
    document.execCommand(command);
  }
</script>

<svelte:window onclick={handleWindowClick} />

<div
  class="application-menu-bar"
  role="menubar"
  tabindex="-1"
  aria-label={t("application-menu-label")}
  bind:this={menuBar}
  onkeydown={handleKeydown}
  onpointerdown={rememberEditingTarget}
>
  <details class="application-menu" ontoggle={handleToggle}>
    <summary aria-haspopup="menu">{t("application-menu-application")}</summary>
    <div class="application-menu-popover" role="menu">
      <button role="menuitem" type="button" onclick={() => runCommand("open_settings")}>
        <span>{t("application-menu-settings")}</span><kbd>Ctrl ,</kbd>
      </button>
      <hr />
      <button role="menuitem" type="button" onclick={() => runCommand("close_application")}>
        <span>{t("application-menu-close-window")}</span><kbd>Ctrl Q</kbd>
      </button>
    </div>
  </details>

  <details class="application-menu" ontoggle={handleToggle}>
    <summary aria-haspopup="menu">{t("application-menu-file")}</summary>
    <div class="application-menu-popover" role="menu">
      <button role="menuitem" type="button" onclick={() => runCommand("open_project")}>
        <span>{t("application-menu-open-project")}</span><kbd>Ctrl O</kbd>
      </button>
      <hr />
      <button role="menuitem" type="button" disabled={noProject} onclick={() => runCommand("save")}>
        <span>{t("application-menu-save")}</span><kbd>Ctrl S</kbd>
      </button>
      <button role="menuitem" type="button" disabled={noProject} onclick={() => runCommand("close_project")}>
        <span>{t("application-menu-close-project")}</span>
      </button>
    </div>
  </details>

  <details class="application-menu" ontoggle={handleToggle}>
    <summary aria-haspopup="menu">{t("application-menu-edit")}</summary>
    <div class="application-menu-popover" role="menu">
      <button role="menuitem" type="button" disabled={!canUndo} onclick={() => runCommand("undo")}>
        <span>{t("application-menu-undo")}</span><kbd>Ctrl Z</kbd>
      </button>
      <button role="menuitem" type="button" disabled={!canRedo} onclick={() => runCommand("redo")}>
        <span>{t("application-menu-redo")}</span><kbd>Ctrl Shift Z</kbd>
      </button>
      <hr />
      <button role="menuitem" type="button" onclick={() => runEditingCommand("cut")}>
        <span>{t("application-menu-cut")}</span><kbd>Ctrl X</kbd>
      </button>
      <button role="menuitem" type="button" onclick={() => runEditingCommand("copy")}>
        <span>{t("application-menu-copy")}</span><kbd>Ctrl C</kbd>
      </button>
      <button role="menuitem" type="button" onclick={() => runEditingCommand("paste")}>
        <span>{t("application-menu-paste")}</span><kbd>Ctrl V</kbd>
      </button>
      <button role="menuitem" type="button" onclick={() => runEditingCommand("selectAll")}>
        <span>{t("application-menu-select-all")}</span><kbd>Ctrl A</kbd>
      </button>
    </div>
  </details>

  <details class="application-menu" ontoggle={handleToggle}>
    <summary aria-haspopup="menu">{t("application-menu-view")}</summary>
    <div class="application-menu-popover" role="menu">
      <button role="menuitem" type="button" onclick={showCommandCenter}>
        <span>{t("application-menu-command-center")}</span><kbd>Ctrl K</kbd>
      </button>
      <hr />
      <button role="menuitem" type="button" disabled={noProject} onclick={() => runCommand("toggle_terminal")}>
        <span>{t("application-menu-terminal")}</span><kbd>Ctrl `</kbd>
      </button>
      <button role="menuitem" type="button" disabled={noProject || !sidebarsAvailable} onclick={() => runCommand("toggle_left_sidebar")}>
        <span>{t("application-menu-left-sidebar")}</span><kbd>Ctrl B</kbd>
      </button>
      <button role="menuitem" type="button" disabled={noProject || !sidebarsAvailable} onclick={() => runCommand("toggle_inspector")}>
        <span>{t("application-menu-inspector")}</span>
      </button>
      <hr />
      <button role="menuitem" type="button" onclick={() => runCommand("toggle_theme")}>
        <span>{t("application-menu-theme")}</span>
      </button>
    </div>
  </details>

  <details class="application-menu" ontoggle={handleToggle}>
    <summary aria-haspopup="menu">{t("application-menu-help")}</summary>
    <div class="application-menu-popover align-left" role="menu">
      <button role="menuitem" type="button" onclick={() => runCommand("open_about")}>
        <span>{t("application-menu-about")}</span>
      </button>
    </div>
  </details>
</div>

<style>
  .application-menu-bar {
    display: flex;
    align-items: center;
    min-width: 0;
    gap: 1px;
  }

  .application-menu {
    position: relative;
  }

  .application-menu > summary {
    display: flex;
    align-items: center;
    min-height: 30px;
    padding: 0 8px;
    border-radius: var(--radius-control);
    color: var(--text-muted);
    cursor: default;
    font-size: var(--font-body);
    list-style: none;
    user-select: none;
  }

  .application-menu > summary::-webkit-details-marker {
    display: none;
  }

  .application-menu > summary:hover,
  .application-menu[open] > summary {
    color: var(--text-strong);
    background: var(--material-control);
    box-shadow: var(--shadow-control);
  }

  .application-menu > summary:focus-visible {
    outline: 2px solid var(--brand-strong);
    outline-offset: 1px;
  }

  .application-menu-popover {
    position: absolute;
    z-index: 1200;
    top: calc(100% + 5px);
    left: 0;
    display: grid;
    min-width: 230px;
    padding: 5px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-panel);
    background: var(--material-panel);
    box-shadow: var(--shadow-float);
  }

  .application-menu-popover button {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 20px;
    min-height: 30px;
    padding: 0 9px;
    border: 0;
    border-radius: var(--radius-control);
    color: var(--text);
    text-align: left;
    background: transparent;
  }

  .application-menu-popover button:hover:not(:disabled),
  .application-menu-popover button:focus-visible {
    color: var(--text-strong);
    background: color-mix(in srgb, var(--brand) 12%, var(--material-control));
    outline: none;
  }

  .application-menu-popover button:disabled {
    color: color-mix(in srgb, var(--text-muted) 48%, transparent);
  }

  .application-menu-popover kbd {
    color: var(--text-muted);
    font-family: inherit;
    font-size: var(--font-meta);
    white-space: nowrap;
  }

  .application-menu-popover hr {
    width: 100%;
    height: 1px;
    margin: 4px 0;
    border: 0;
    background: var(--border-subtle);
  }

  @media (max-width: 1080px) {
    .application-menu > summary {
      padding-inline: 6px;
    }
  }
</style>
