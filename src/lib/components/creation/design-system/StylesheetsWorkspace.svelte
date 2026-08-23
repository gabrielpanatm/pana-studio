<script lang="ts">
  import { IconAlertTriangle, IconDeviceFloppy, IconEdit, IconExternalLink, IconFileTypeCss, IconX } from "@tabler/icons-svelte";
  import { createProjectTextFile } from "$lib/content/io";
  import { l10n, t } from "$lib/i18n/runtime.svelte";
  import type { FileExplorerSnapshot } from "$lib/project/file-explorer-contract";
  import type { FileBufferRequestIdentity } from "$lib/project/workspace-contract";
  import type { SourceGraph } from "$lib/source-graph/graph-contract";
  import type { GlobalStatusState } from "$lib/status/state.svelte";
  import type { ProjectWorkspaceMutationService } from "$lib/session/workspace-mutation-service";
  import { errorMessage } from "$lib/util";
  import type { DesignSystemCommands, DetailMode } from "./contracts";
  import ResourceWorkspaceShell from "./ResourceWorkspaceShell.svelte";

  let {
    sourceGraph,
    fileExplorerSnapshot,
    query,
    createRequest,
    busy = $bindable(false),
    commands,
    globalStatus,
    workspaceMutations,
    openWorkspaceSource,
  }: {
    sourceGraph: SourceGraph | null;
    fileExplorerSnapshot: FileExplorerSnapshot | null;
    query: string;
    createRequest: number;
    busy?: boolean;
    commands: Pick<DesignSystemCommands, "refreshFileExplorer" | "planFileExplorer" | "commitFileExplorer">;
    globalStatus: GlobalStatusState;
    workspaceMutations: ProjectWorkspaceMutationService;
    openWorkspaceSource: (path: string) => void | Promise<void>;
  } = $props();

  let selectedStyleId = $state("");
  let detailMode = $state<DetailMode>("info");
  let formName = $state("");
  let formPath = $state("");
  let formError = $state("");
  let mutating = $state(false);
  let lastCreateRequest = 0;
  let createRequestReady = false;

  const normalizedQuery = $derived(query.trim().toLocaleLowerCase(l10n.locale));
  const styles = $derived(
    (sourceGraph?.styles ?? []).filter((style) => (
      !normalizedQuery
      || `${style.file} ${style.scope}`.toLocaleLowerCase(l10n.locale).includes(normalizedQuery)
    )),
  );
  const selectedStyle = $derived(
    (sourceGraph?.styles ?? []).find((style) => style.id === selectedStyleId)
      ?? styles[0]
      ?? null,
  );
  const styleUsageCounts = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const relation of sourceGraph?.relations ?? []) {
      if (relation.kind !== "usesStyle") continue;
      counts.set(relation.to, (counts.get(relation.to) ?? 0) + 1);
    }
    return counts;
  });

  $effect(() => { busy = mutating; });
  $effect(() => {
    const request = createRequest;
    if (!createRequestReady) {
      createRequestReady = true;
      lastCreateRequest = request;
      return;
    }
    if (request === lastCreateRequest) return;
    lastCreateRequest = request;
    beginCreate();
  });

  function identity(): FileBufferRequestIdentity {
    return {
      expectedProjectRoot: workspaceMutations.identity?.expectedProjectRoot ?? "",
      expectedSessionId: workspaceMutations.identity?.expectedSessionId ?? "",
    };
  }

  function resetPanel() {
    detailMode = "info";
    formName = "";
    formPath = "";
    formError = "";
  }

  function selectStyle(id: string) {
    selectedStyleId = id;
    resetPanel();
  }

  function beginCreate() {
    if (mutating) return;
    resetPanel();
    detailMode = "create";
    formName = "stil-nou.scss";
    formPath = "sass/pagini/stil-nou.scss";
  }

  function beginEdit() {
    if (!selectedStyle || mutating) return;
    resetPanel();
    detailMode = "edit";
    formName = selectedStyle.file.split("/").at(-1) ?? selectedStyle.file;
    formPath = selectedStyle.file;
  }

  async function createStylesheet() {
    if (mutating) return;
    formError = "";
    mutating = true;
    try {
      const receipt = await createProjectTextFile(
        formPath,
        `/* ${t("design-new-stylesheet-comment")} */\n`,
        identity(),
      );
      const settlement = await workspaceMutations.settle(receipt, {
        preferredRelativePath: receipt.relativePath,
        warningLabel: t("design-operation-stylesheet-create"),
      });
      selectedStyleId = sourceGraph?.styles.find((style) => style.file === receipt.relativePath)?.id ?? "";
      globalStatus.set(
        settlement.warnings.length > 0
          ? t("design-stylesheet-created-warning", { path: formPath })
          : t("design-stylesheet-created-success", { path: formPath }),
        "unsaved",
      );
      resetPanel();
    } catch (cause) {
      formError = errorMessage(cause);
    } finally {
      mutating = false;
    }
  }

  async function renameStylesheet() {
    if (mutating || !selectedStyle) return;
    formError = "";
    mutating = true;
    try {
      let explorer = fileExplorerSnapshot;
      if (!explorer?.entries.some((entry) => entry.relativePath === selectedStyle.file)) {
        explorer = await commands.refreshFileExplorer();
      }
      const entry = explorer?.entries.find((candidate) => candidate.relativePath === selectedStyle.file);
      if (!entry) throw new Error(t("project-files-source-gone"));
      const plan = await commands.planFileExplorer({ kind: "rename", entryId: entry.id, newName: formName });
      if (!plan.allowed) throw new Error(plan.diagnostic ?? t("project-files-rename-invalid"));
      await commands.commitFileExplorer(plan);
      const renamedPath = plan.destinationPath ?? selectedStyle.file;
      selectedStyleId = sourceGraph?.styles.find((style) => style.file === renamedPath)?.id ?? "";
      globalStatus.set(t("design-stylesheet-renamed-success", { path: renamedPath }), "unsaved");
      resetPanel();
    } catch (cause) {
      formError = errorMessage(cause);
    } finally {
      mutating = false;
    }
  }
</script>

{#snippet list()}
  {#each styles as style (style.id)}
    <button type="button" class="style-row ui-entity-selectable" data-ui-selected={selectedStyle?.id === style.id ? "true" : undefined} aria-pressed={selectedStyle?.id === style.id} onclick={() => selectStyle(style.id)}>
      <span class="resource-icon"><IconFileTypeCss size={16} stroke={1.8} /></span>
      <span><strong>{style.file.split("/").at(-1)}</strong><small>{style.file}</small></span>
      <code>{style.scope}</code>
      <small>{t("design-usages-count", { count: styleUsageCounts.get(style.nodeId) ?? 0 })}</small>
    </button>
  {:else}<div class="workspace-state">{t("design-empty-stylesheets")}</div>{/each}
{/snippet}

{#snippet detail()}
  {#if detailMode === "create" || detailMode === "edit"}
    <form class="resource-form" onsubmit={(event) => { event.preventDefault(); void (detailMode === "create" ? createStylesheet() : renameStylesheet()); }}>
      <header class="detail-heading"><div><span class="detail-kicker">{detailMode === "create" ? t("design-new-resource") : t("design-controlled-change")}</span><h2>{detailMode === "create" ? t("design-add-resource", { resource: t("design-view-stylesheets").toLocaleLowerCase(l10n.locale) }) : selectedStyle?.file.split("/").at(-1)}</h2><p>{detailMode === "create" ? t("design-create-description") : t("design-change-description")}</p></div><button class="ui-icon-button ui-close-button" type="button" aria-label={t("design-cancel-edit")} disabled={mutating} onclick={resetPanel}><IconX size={14} /></button></header>
      {#if detailMode === "create"}<label><span>{t("design-project-path")}</span><input bind:value={formPath} disabled={mutating} placeholder="sass/pagini/stil-nou.scss" /></label>{:else}<label><span>{t("design-file-name")}</span><input bind:value={formName} disabled={mutating} /></label><div class="source-card"><span>{t("design-current-path")}</span><code>{formPath}</code></div>{/if}
      {#if formError}<p class="form-error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>{/if}
      <div class="form-actions"><button type="button" disabled={mutating} onclick={resetPanel}>{t("design-cancel")}</button><button class="primary" type="submit" disabled={mutating || (detailMode === "create" ? !formPath.trim() : !formName.trim())}><IconDeviceFloppy size={14} /> {detailMode === "create" ? t("design-create-session") : t("design-save-changes")}</button></div>
    </form>
  {:else if selectedStyle}
    <span class="detail-kicker">{t("design-stylesheet-kicker", { scope: selectedStyle.scope })}</span><h2>{selectedStyle.file.split("/").at(-1)}</h2><p>{t("design-stylesheet-summary", { count: styleUsageCounts.get(selectedStyle.nodeId) ?? 0 })}</p>
    <div class="source-card"><span>{t("design-path")}</span><code>{selectedStyle.file}</code></div>
    <div class="detail-actions"><button class="ui-button primary primary-action" type="button" onclick={beginEdit}><IconEdit size={14} /> {t("design-edit")}</button><button class="ui-button secondary-action" type="button" onclick={() => openWorkspaceSource(selectedStyle.file)}>{t("design-open-editor")} <IconExternalLink size={13} /></button></div>
  {:else}<div class="workspace-state">{t("design-empty-stylesheets")}</div>{/if}
{/snippet}

<ResourceWorkspaceShell panelId="design-panel-styles" tabId="design-tab-styles" detailLabel={t("design-detail-label")} {list} {detail} />

<style>
  .style-row { display: grid; width: 100%; grid-template-columns: 34px minmax(0, 1fr) auto 70px; align-items: center; gap: 9px; min-height: 52px; padding: 7px 9px; border: 1px solid transparent; border-radius: 7px; color: var(--wb-text-primary); background: transparent; text-align: left; }
  .style-row > span:nth-child(2) { display: grid; min-width: 0; gap: 3px; }
  .style-row strong, .style-row small, .style-row code { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .style-row strong { color: var(--text-strong); font-size: 12px; }
  .style-row small, .style-row code { color: var(--wb-text-muted); font-size: 12px; }
  .style-row code { text-align: right; }
  .resource-icon { display: grid; width: 29px; height: 29px; place-items: center; border-radius: 6px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .source-card { display: grid; gap: 4px; margin-top: 9px; padding: 9px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  .source-card span { color: var(--wb-text-muted); font-size: 12px; font-weight: 800; text-transform: uppercase; }
  .source-card code { overflow: hidden; color: var(--wb-text-primary); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
</style>
