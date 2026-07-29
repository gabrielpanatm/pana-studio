<script lang="ts">
  import { onMount, tick } from "svelte";
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
  import { t } from "$lib/i18n/runtime.svelte";
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
  let lastMeasuredDocumentLayoutKey = "";
  let canScrollDocumentsLeft = $state(false);
  let canScrollDocumentsRight = $state(false);

  function updateDocumentScrollCues() {
    if (!documentTabsElement) return;
    const maximumScrollLeft = Math.max(
      0,
      documentTabsElement.scrollWidth - documentTabsElement.clientWidth,
    );
    canScrollDocumentsLeft = maximumScrollLeft > 1 && documentTabsElement.scrollLeft > 1;
    canScrollDocumentsRight = maximumScrollLeft > 1
      && documentTabsElement.scrollLeft < maximumScrollLeft - 1;
  }

  function iconKind(path: string): "markdown" | "code" | "file" {
    if (/\.md$/i.test(path)) return "markdown";
    if (/\.(?:html?|tera|scss|sass|css|js|ts|json|toml|ya?ml)$/i.test(path)) return "code";
    return "file";
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

    let nextScrollLeft: number | null = null;
    if (tabLeft < visibleLeft) {
      nextScrollLeft = tabLeft;
    } else if (tabRight > visibleRight) {
      nextScrollLeft = tabRight - documentTabsElement.clientWidth;
    }
    if (nextScrollLeft !== null) {
      documentTabsElement.scrollTo({
        left: nextScrollLeft,
        behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches
          ? "auto"
          : "smooth",
      });
    }
  }

  $effect(() => {
    const documentId = activeGroup?.activeDocumentId ?? "";
    const documentKey = `${activeGroup?.groupId ?? ""}\u0000${documentId}`;
    if (!documentId || documentKey === lastRevealedDocumentKey) return;
    lastRevealedDocumentKey = documentKey;
    void tick().then(() => revealActiveDocumentTab(documentId));
  });

  $effect(() => {
    const layoutKey = [
      activeGroup?.groupId ?? "",
      ...(activeGroup?.documents.map((document) => `${document.documentId}:${document.title}`) ?? []),
      ...dirtyPaths,
    ].join("\u0000");
    if (layoutKey === lastMeasuredDocumentLayoutKey) return;
    lastMeasuredDocumentLayoutKey = layoutKey;
    void tick().then(updateDocumentScrollCues);
  });

  onMount(() => {
    const resizeObserver = new ResizeObserver(updateDocumentScrollCues);
    resizeObserver.observe(documentTabsElement);
    updateDocumentScrollCues();
    return () => resizeObserver.disconnect();
  });
</script>

<header class="document-bar" aria-label={t("workbench-open-documents")}>
  <div
    class="document-tabs-shell"
    class:can-scroll-left={canScrollDocumentsLeft}
    class:can-scroll-right={canScrollDocumentsRight}
  >
    <div
      bind:this={documentTabsElement}
      class="ui-document-tabs document-tabs"
      role="tablist"
      aria-label={t("workbench-workspace-documents")}
      onscroll={updateDocumentScrollCues}
    >
      {#if activeGroup && activeGroup.documents.length > 0}
        {#each activeGroup.documents as document (document.documentId)}
          <div
            class:active={document.documentId === activeGroup.activeDocumentId}
            class="ui-document-tab document-tab"
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
                <span class="dirty-indicator" aria-label={t("workbench-unsaved-changes")}></span>
              {/if}
            </button>
            <button
              type="button"
              class="ui-icon-button ui-close-button document-close"
              disabled={!canCloseDocuments}
              aria-label={t("workbench-close-document", { document: document.title })}
              title={canCloseDocuments
                ? t("workbench-close-document", { document: document.title })
                : t("workbench-keep-one-document")}
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
          <span>{t("workbench-no-open-document")}</span>
        </div>
      {/if}
    </div>
  </div>

  {#if snapshot?.split === "none" || !snapshot}
    <div class="ui-tabs compact surface-switcher" role="group" aria-label={t("workbench-document-surface")}>
      <button
        type="button"
        class="ui-tab"
        class:active={activeDocument?.surface === "visual"}
        aria-pressed={activeDocument?.surface === "visual" ? "true" : "false"}
        title={t("workbench-visual")}
        onclick={() => { void setSurface("visual"); }}
      >
        <IconEye size={15} stroke={1.8} />
        <span>{t("workbench-visual")}</span>
      </button>
      <button
        type="button"
        class="ui-tab"
        class:active={activeDocument?.surface === "code"}
        aria-pressed={activeDocument?.surface === "code" ? "true" : "false"}
        title={t("workbench-code")}
        onclick={() => { void setSurface("code"); }}
      >
        <IconCode size={15} stroke={1.8} />
        <span>{t("workbench-code")}</span>
      </button>
      <button
        type="button"
        class="ui-tab"
        class:active={activeDocument?.surface === "markdown"}
        aria-pressed={activeDocument?.surface === "markdown" ? "true" : "false"}
        disabled={!/\.md$/i.test(activeDocument?.relativePath ?? "")}
        title={t("workbench-markdown")}
        onclick={() => { void setSurface("markdown"); }}
      >
        <IconMarkdown size={15} stroke={1.8} />
        <span>{t("workbench-markdown")}</span>
      </button>
    </div>
  {:else}
    <div class="split-mode-label" title={t("workbench-synchronized-surfaces")}>
      <IconEye size={14} stroke={1.8} />
      <span>{t("workbench-visual-code")}</span>
    </div>
  {/if}

  <div class="layout-switcher" role="group" aria-label={t("workbench-editor-layout")}>
    <button
      type="button"
      class:active={snapshot?.split === "vertical"}
      aria-pressed={snapshot?.split === "vertical" ? "true" : "false"}
      disabled={splitDisabled}
      title={t("workbench-split-side-title")}
      aria-label={t("workbench-split-side-enable")}
      onclick={() => { void setSplit("vertical"); }}
    >
      <IconLayoutColumns size={15} stroke={1.8} />
    </button>
    <button
      type="button"
      class:active={snapshot?.split === "horizontal"}
      aria-pressed={snapshot?.split === "horizontal" ? "true" : "false"}
      disabled={splitDisabled}
      title={t("workbench-split-stack-title")}
      aria-label={t("workbench-split-stack-enable")}
      onclick={() => { void setSplit("horizontal"); }}
    >
      <IconLayoutRows size={15} stroke={1.8} />
    </button>
    {#if snapshot?.split !== "none"}
      <button
        type="button"
        title={t("workbench-split-close")}
        aria-label={t("workbench-split-close")}
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

  .document-tabs-shell {
    position: relative;
    min-width: 0;
    flex: 1;
    overflow: hidden;
  }

  .document-tabs-shell::before,
  .document-tabs-shell::after {
    position: absolute;
    z-index: 3;
    top: 4px;
    bottom: 0;
    width: 20px;
    pointer-events: none;
    opacity: 0;
    content: "";
    transition: opacity 140ms ease;
  }

  .document-tabs-shell::before {
    left: 0;
    background: linear-gradient(
      90deg,
      color-mix(in srgb, var(--surface-panel) 96%, transparent),
      transparent
    );
  }

  .document-tabs-shell::after {
    right: 0;
    background: linear-gradient(
      270deg,
      color-mix(in srgb, var(--surface-panel) 96%, transparent),
      transparent
    );
  }

  .document-tabs-shell.can-scroll-left::before,
  .document-tabs-shell.can-scroll-right::after {
    opacity: 1;
  }

  .document-tabs {
    width: 100%;
    height: 100%;
    overflow-x: auto;
    overflow-y: hidden;
    overscroll-behavior-x: contain;
    scrollbar-width: none;
  }

  .document-tabs::-webkit-scrollbar {
    display: none;
  }

  .document-tab {
    padding: 0;
    overflow: hidden;
  }

  .document-select,
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
  .layout-switcher button:hover:not(:disabled) {
    color: var(--wb-text-primary, var(--text));
    background: var(--control-hover);
  }

  .document-tab .document-select:hover:not(:disabled) {
    border-color: transparent;
    background: transparent;
  }

  .document-select:focus-visible,
  .document-close:focus-visible,
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
    max-width: 150px;
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
    align-self: center;
    margin-right: 3px;
    opacity: 0;
    transition:
      color 120ms ease,
      background 120ms ease,
      box-shadow 120ms ease,
      opacity 120ms ease,
      transform 80ms ease;
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

  .layout-switcher,
  .split-mode-label {
    display: flex;
    align-items: center;
    gap: 2px;
    flex: 0 0 auto;
    padding: 4px;
    border-left: 1px solid var(--wb-border-subtle, var(--border));
  }

  .surface-switcher {
    align-self: center;
    min-height: 32px;
    flex: 0 0 auto;
    margin: 2px 4px;
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

  .layout-switcher button.active {
    color: var(--wb-accent-strong, var(--brand-strong));
    background: var(--wb-accent-soft, var(--brand-soft));
  }

  .layout-switcher button:disabled {
    opacity: 0.36;
    cursor: not-allowed;
  }

  @media (max-width: 1180px) {
    .surface-switcher .ui-tab span,
    .split-mode-label span {
      display: none;
    }
  }
</style>
