<script lang="ts">
  import { onDestroy, tick } from "svelte";
  import {
    IconCode,
    IconEye,
    IconFile,
    IconFileCode,
    IconLayoutColumns,
    IconLayoutOff,
    IconLayoutRows,
    IconMarkdown,
    IconX,
  } from "@tabler/icons-svelte";
  import type {
    WorkbenchDocumentSnapshot,
    WorkbenchGroupId,
    WorkbenchSnapshot,
    WorkbenchSplit,
    WorkbenchSurface,
  } from "$lib/types";

  let {
    snapshot = null,
    dirtyPaths = [],
    activateDocument = () => {},
    closeDocument = () => {},
    setSurface = () => {},
    setSplit = () => {},
    splitDisabled = false,
  }: {
    snapshot?: WorkbenchSnapshot | null;
    dirtyPaths?: string[];
    activateDocument?: (
      groupId: WorkbenchGroupId,
      document: WorkbenchDocumentSnapshot,
    ) => void | Promise<void>;
    closeDocument?: (
      groupId: WorkbenchGroupId,
      document: WorkbenchDocumentSnapshot,
    ) => void | Promise<void>;
    setSurface?: (surface: WorkbenchSurface) => void | Promise<void>;
    setSplit?: (split: WorkbenchSplit) => void | Promise<void>;
    splitDisabled?: boolean;
  } = $props();

  const activeGroup = $derived(
    snapshot?.groups.find((group) => group.groupId === snapshot?.activeGroupId)
      ?? snapshot?.groups[0]
      ?? null,
  );
  const activeDocument = $derived(
    activeGroup?.documents.find((document) => document.documentId === activeGroup.activeDocumentId)
      ?? null,
  );
  const dirtySet = $derived(new Set(dirtyPaths));
  const canCloseDocuments = $derived((activeGroup?.documents.length ?? 0) > 1);
  let documentTabsElement: HTMLDivElement;
  let lastRevealedDocumentKey = "";
  let wheelScrollTarget = 0;
  let wheelAnimationFrame: number | null = null;
  let lastWheelAnimationTime = 0;

  function iconKind(path: string): "markdown" | "code" | "file" {
    if (/\.md$/i.test(path)) return "markdown";
    if (/\.(?:html?|tera|scss|sass|css|js|ts|json|toml|ya?ml)$/i.test(path)) return "code";
    return "file";
  }

  function wheelDeltaInPixels(event: WheelEvent, element: HTMLElement): number {
    if (event.deltaMode === WheelEvent.DOM_DELTA_LINE) return event.deltaY * 16;
    if (event.deltaMode === WheelEvent.DOM_DELTA_PAGE) return event.deltaY * element.clientWidth;
    return event.deltaY;
  }

  function stopWheelAnimation() {
    if (wheelAnimationFrame !== null) cancelAnimationFrame(wheelAnimationFrame);
    wheelAnimationFrame = null;
    lastWheelAnimationTime = 0;
    wheelScrollTarget = documentTabsElement?.scrollLeft ?? 0;
  }

  function animateWheelScroll(time: number) {
    if (!documentTabsElement) {
      stopWheelAnimation();
      return;
    }

    const elapsed = lastWheelAnimationTime === 0
      ? 16
      : Math.min(32, time - lastWheelAnimationTime);
    lastWheelAnimationTime = time;
    const distance = wheelScrollTarget - documentTabsElement.scrollLeft;

    if (Math.abs(distance) < 0.5) {
      documentTabsElement.scrollLeft = wheelScrollTarget;
      wheelAnimationFrame = null;
      lastWheelAnimationTime = 0;
      return;
    }

    const progress = 1 - Math.exp(-elapsed / 72);
    documentTabsElement.scrollLeft += distance * progress;
    wheelAnimationFrame = requestAnimationFrame(animateWheelScroll);
  }

  function handleDocumentTabsWheel(event: WheelEvent) {
    const tabs = event.currentTarget as HTMLDivElement;
    if (event.ctrlKey || event.metaKey) return;
    if (Math.abs(event.deltaX) > Math.abs(event.deltaY)) {
      stopWheelAnimation();
      return;
    }

    const maxScrollLeft = Math.max(0, tabs.scrollWidth - tabs.clientWidth);
    if (maxScrollLeft === 0) return;

    const delta = wheelDeltaInPixels(event, tabs);
    if (delta === 0) return;

    const currentTarget = wheelAnimationFrame === null ? tabs.scrollLeft : wheelScrollTarget;
    const nextTarget = Math.min(maxScrollLeft, Math.max(0, currentTarget + delta));
    if (nextTarget === currentTarget) return;

    event.preventDefault();
    wheelScrollTarget = nextTarget;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      stopWheelAnimation();
      tabs.scrollLeft = nextTarget;
      wheelScrollTarget = nextTarget;
      return;
    }
    if (wheelAnimationFrame === null) {
      lastWheelAnimationTime = 0;
      wheelAnimationFrame = requestAnimationFrame(animateWheelScroll);
    }
  }

  function revealActiveDocumentTab(documentId: string) {
    const tab = Array.from(
      documentTabsElement?.querySelectorAll<HTMLElement>("[data-document-id]") ?? [],
    ).find((candidate) => candidate.dataset.documentId === documentId);
    if (!tab || !documentTabsElement) return;

    const visibleLeft = documentTabsElement.scrollLeft;
    const visibleRight = visibleLeft + documentTabsElement.clientWidth;
    const tabLeft = tab.offsetLeft;
    const tabRight = tabLeft + tab.offsetWidth;

    stopWheelAnimation();
    if (tabLeft < visibleLeft) {
      documentTabsElement.scrollLeft = tabLeft;
    } else if (tabRight > visibleRight) {
      documentTabsElement.scrollLeft = tabRight - documentTabsElement.clientWidth;
    }
  }

  $effect(() => {
    const documentId = activeGroup?.activeDocumentId ?? "";
    const documentKey = `${activeGroup?.groupId ?? ""}\u0000${documentId}`;
    if (!documentId || documentKey === lastRevealedDocumentKey) return;
    lastRevealedDocumentKey = documentKey;
    void tick().then(() => revealActiveDocumentTab(documentId));
  });

  onDestroy(stopWheelAnimation);
</script>

<header class="document-bar" aria-label="Documente deschise">
  <div
    bind:this={documentTabsElement}
    class="document-tabs"
    role="tablist"
    aria-label="Documentele spațiului de lucru"
    onwheel={handleDocumentTabsWheel}
  >
    {#if activeGroup && activeGroup.documents.length > 0}
      {#each activeGroup.documents as document (document.documentId)}
        <div
          class:active={document.documentId === activeGroup.activeDocumentId}
          class="document-tab"
          data-document-id={document.documentId}
        >
          <button
            type="button"
            class="document-select"
            role="tab"
            aria-selected={document.documentId === activeGroup.activeDocumentId ? "true" : "false"}
            tabindex={document.documentId === activeGroup.activeDocumentId ? 0 : -1}
            title={document.relativePath}
            onclick={() => { void activateDocument(activeGroup.groupId, document); }}
          >
            <span class="document-icon" aria-hidden="true">
              {#if iconKind(document.relativePath) === "markdown"}
                <IconMarkdown size={14} stroke={1.8} />
              {:else if iconKind(document.relativePath) === "code"}
                <IconFileCode size={14} stroke={1.8} />
              {:else}
                <IconFile size={14} stroke={1.8} />
              {/if}
            </span>
            <span class="document-title">{document.title}</span>
            {#if dirtySet.has(document.relativePath)}
              <span class="dirty-indicator" aria-label="Modificări nesalvate"></span>
            {/if}
          </button>
          <button
            type="button"
            class="document-close"
            disabled={!canCloseDocuments}
            aria-label={`Închide ${document.title}`}
            title={canCloseDocuments ? `Închide ${document.title}` : "Păstrează cel puțin un document deschis"}
            onclick={(event) => {
              event.stopPropagation();
              void closeDocument(activeGroup.groupId, document);
            }}
          >
            <IconX size={13} stroke={1.9} />
          </button>
        </div>
      {/each}
    {:else}
      <div class="document-empty">
        <IconFile size={14} stroke={1.8} />
        <span>Niciun document deschis</span>
      </div>
    {/if}
  </div>

  {#if snapshot?.split === "none" || !snapshot}
    <div class="surface-switcher" role="group" aria-label="Suprafața documentului">
      <button
        type="button"
        class:active={activeDocument?.surface === "visual"}
        aria-pressed={activeDocument?.surface === "visual" ? "true" : "false"}
        title="Vizual"
        onclick={() => { void setSurface("visual"); }}
      >
        <IconEye size={15} stroke={1.8} />
        <span>Vizual</span>
      </button>
      <button
        type="button"
        class:active={activeDocument?.surface === "code"}
        aria-pressed={activeDocument?.surface === "code" ? "true" : "false"}
        title="Cod"
        onclick={() => { void setSurface("code"); }}
      >
        <IconCode size={15} stroke={1.8} />
        <span>Cod</span>
      </button>
      <button
        type="button"
        class:active={activeDocument?.surface === "markdown"}
        aria-pressed={activeDocument?.surface === "markdown" ? "true" : "false"}
        disabled={!/\.md$/i.test(activeDocument?.relativePath ?? "")}
        title="Markdown"
        onclick={() => { void setSurface("markdown"); }}
      >
        <IconMarkdown size={15} stroke={1.8} />
        <span>Markdown</span>
      </button>
    </div>
  {:else}
    <div class="split-mode-label" title="Același document, două suprafețe sincronizate">
      <IconEye size={14} stroke={1.8} />
      <span>Vizual + Cod</span>
    </div>
  {/if}

  <div class="layout-switcher" role="group" aria-label="Layout editor">
    <button
      type="button"
      class:active={snapshot?.split === "vertical"}
      aria-pressed={snapshot?.split === "vertical" ? "true" : "false"}
      disabled={splitDisabled}
      title="Split alăturat: Vizual și Cod"
      aria-label="Activează split alăturat"
      onclick={() => { void setSplit("vertical"); }}
    >
      <IconLayoutColumns size={15} stroke={1.8} />
    </button>
    <button
      type="button"
      class:active={snapshot?.split === "horizontal"}
      aria-pressed={snapshot?.split === "horizontal" ? "true" : "false"}
      disabled={splitDisabled}
      title="Split stivuit: Vizual și Cod"
      aria-label="Activează split stivuit"
      onclick={() => { void setSplit("horizontal"); }}
    >
      <IconLayoutRows size={15} stroke={1.8} />
    </button>
    {#if snapshot?.split !== "none"}
      <button
        type="button"
        title="Închide split view"
        aria-label="Închide split view"
        onclick={() => { void setSplit("none"); }}
      >
        <IconLayoutOff size={15} stroke={1.8} />
      </button>
    {/if}
  </div>
</header>

<style>
  .document-bar {
    position: relative;
    z-index: 4;
    display: flex;
    align-items: stretch;
    min-width: 0;
    min-height: var(--wb-document-bar-height, 36px);
    border-bottom: 1px solid var(--wb-border-subtle, var(--border));
    background: var(--surface-panel);
  }

  .document-tabs {
    display: flex;
    align-items: stretch;
    min-width: 0;
    flex: 1;
    overflow-x: auto;
    overflow-y: hidden;
    overscroll-behavior-x: contain;
    scrollbar-width: none;
  }

  .document-tabs::-webkit-scrollbar {
    display: none;
  }

  .document-tab {
    position: relative;
    display: flex;
    align-items: stretch;
    flex: 0 1 180px;
    min-width: 112px;
    max-width: 220px;
    border-right: 1px solid var(--wb-border-subtle, var(--border));
    color: var(--wb-text-muted, var(--text-muted));
    background: transparent;
  }

  .document-tab.active {
    color: var(--wb-text-primary, var(--text));
    background: var(--surface-raised);
  }

  .document-tab.active::after {
    position: absolute;
    inset: auto 8px -1px;
    height: 2px;
    border-radius: 2px 2px 0 0;
    background: var(--wb-accent, var(--brand));
    content: "";
  }

  .document-select,
  .document-close,
  .surface-switcher button,
  .layout-switcher button {
    border: 0;
    border-radius: 0;
    color: inherit;
    background: transparent;
  }

  .document-select {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
    flex: 1;
    padding: 0 4px 0 10px;
    text-align: left;
  }

  .document-select:hover,
  .document-close:hover:not(:disabled),
  .surface-switcher button:hover:not(:disabled),
  .layout-switcher button:hover:not(:disabled) {
    color: var(--wb-text-primary, var(--text));
    background: var(--control-hover);
  }

  .document-select:focus-visible,
  .document-close:focus-visible,
  .surface-switcher button:focus-visible,
  .layout-switcher button:focus-visible {
    position: relative;
    z-index: 2;
    outline: 2px solid var(--wb-focus-ring, var(--brand-strong));
    outline-offset: -2px;
  }

  .document-icon {
    display: inline-flex;
    flex: 0 0 auto;
    color: var(--wb-text-muted, var(--text-muted));
  }

  .document-tab.active .document-icon {
    color: var(--wb-accent, var(--brand-strong));
  }

  .document-title {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--font-body);
    font-weight: 600;
  }

  .dirty-indicator {
    width: 6px;
    height: 6px;
    flex: 0 0 auto;
    border-radius: 50%;
    background: var(--wb-warning, #d97706);
  }

  .document-close {
    display: grid;
    width: 26px;
    min-width: 26px;
    padding: 0;
    place-items: center;
    opacity: 0;
  }

  .document-tab:hover .document-close,
  .document-tab.active .document-close,
  .document-close:focus-visible {
    opacity: 1;
  }

  .document-close:disabled {
    opacity: 0;
    cursor: default;
  }

  .document-empty {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 0 12px;
    color: var(--wb-text-muted, var(--text-muted));
    font-size: 12px;
  }

  .surface-switcher,
  .layout-switcher,
  .split-mode-label {
    display: flex;
    align-items: center;
    gap: 2px;
    flex: 0 0 auto;
    padding: 4px;
    border-left: 1px solid var(--wb-border-subtle, var(--border));
  }

  .layout-switcher {
    gap: 2px;
  }

  .split-mode-label {
    gap: 6px;
    min-width: max-content;
    padding: 0 9px;
    color: var(--wb-accent-strong, var(--brand-strong));
    font-size: 12px;
    font-weight: 750;
    background: var(--wb-accent-soft, var(--brand-soft));
  }

  .surface-switcher button,
  .layout-switcher button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    min-width: 30px;
    height: 28px;
    padding: 0 8px;
    border-radius: var(--wb-radius-control, 6px);
    color: var(--wb-text-muted, var(--text-muted));
    font-size: 12px;
    font-weight: 700;
  }

  .surface-switcher button.active {
    color: var(--wb-accent-strong, var(--brand-strong));
    background: var(--wb-accent-soft, var(--brand-soft));
  }

  .layout-switcher button.active {
    color: var(--wb-accent-strong, var(--brand-strong));
    background: var(--wb-accent-soft, var(--brand-soft));
  }

  .surface-switcher button:disabled,
  .layout-switcher button:disabled {
    opacity: 0.36;
    cursor: not-allowed;
  }

  @media (max-width: 1180px) {
    .surface-switcher button span,
    .split-mode-label span {
      display: none;
    }
  }
</style>
