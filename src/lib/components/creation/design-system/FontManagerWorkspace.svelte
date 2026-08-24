<script lang="ts">
  import { onDestroy } from "svelte";
  import { IconTypography } from "@tabler/icons-svelte";
  import type { FontManagerState } from "$lib/fonts/manager-state.svelte";
  import { t } from "$lib/i18n/runtime.svelte";
  import type { ProjectWorkspaceMutationService } from "$lib/session/workspace-mutation-service";
  import type { GlobalStatusState } from "$lib/status/state.svelte";
  import ResourceWorkspaceShell from "./ResourceWorkspaceShell.svelte";
  import FontDetail from "./font-manager/FontDetail.svelte";
  import FontInstaller from "./font-manager/FontInstaller.svelte";
  import { FontManagerController } from "./font-manager/controller.svelte";

  let {
    state,
    query,
    createRequest,
    busy = $bindable(false),
    globalStatus,
    workspaceMutations,
  }: {
    state: FontManagerState;
    query: string;
    createRequest: number;
    busy?: boolean;
    globalStatus: GlobalStatusState;
    workspaceMutations: ProjectWorkspaceMutationService;
  } = $props();

  function createController() {
    return new FontManagerController(state, workspaceMutations, globalStatus);
  }
  const controller = createController();
  let lastCreateRequest = 0;
  let createRequestReady = false;

  $effect(() => {
    controller.query = query;
    busy = controller.mutating;
  });

  $effect(() => {
    const request = createRequest;
    if (!createRequestReady) {
      createRequestReady = true;
      lastCreateRequest = request;
      return;
    }
    if (request === lastCreateRequest) return;
    lastCreateRequest = request;
    controller.beginCreate();
  });

  $effect(() => {
    const file = controller.selectedFontPreviewFile?.file;
    const workspaceRevision = workspaceMutations.snapshot?.revision;
    if (!file || workspaceRevision === undefined) {
      controller.clearFontPreview();
      return;
    }
    void controller.loadSelectedFontPreview(file, workspaceRevision);
  });

  onDestroy(() => controller.dispose());
</script>

{#snippet list()}
  {#if state.error}<div class="ui-empty-state error" role="alert">{state.error}</div>
  {:else if controller.graph}
    <section class="font-role-overview" aria-label={t("design-font-roles-label")}><header><strong>{t("design-semantic-use")}</strong><small>{t("design-authoritative-scss")}</small></header><div>{#each controller.roles as role (role.id)}<span class:missing={!role.installed} title={role.diagnostic ?? role.value ?? ""}><small>{role.label}</small><strong>{role.family ?? t("design-role-missing", { variable: role.variableName })}</strong></span>{/each}</div></section>
    {#each controller.visibleFonts as family (family.id)}
      <button type="button" class="font-row ui-entity-selectable" data-ui-selected={controller.selectedFont?.id === family.id ? "true" : undefined} aria-pressed={controller.selectedFont?.id === family.id} onclick={() => controller.selectFont(family.id)}>
        <span class="resource-icon"><IconTypography size={16} stroke={1.8} /></span><div><strong>{family.family}</strong><small>{family.directories.join(", ") || family.faces[0]?.url || "—"}</small></div><span>{t("design-files-count", { count: family.files.length })}</span><span class="font-registration" class:missing={!family.registration.registered} title={family.registration.registered ? t("design-font-registered-in", { stylesheets: family.registration.stylesheets.join(", ") }) : t("design-font-unregistered-help")}>{family.delivery === "system" ? t("design-delivery-system") : family.origin === "bundled" ? t("design-origin-bundled") : family.origin === "local" ? t("design-origin-local") : family.origin === "theme" ? t("design-origin-theme") : t("design-origin-external")} · {family.registration.registered ? family.registration.managed ? t("design-font-managed") : t("design-font-registered") : t("design-font-unregistered")}</span>
      </button>
    {:else}<div class="ui-empty-state">{t("design-empty-fonts")}</div>{/each}
  {:else}<div class="ui-empty-state">{t("design-loading-fonts")}</div>{/if}
{/snippet}

{#snippet detail()}
  {#if controller.detailMode === "create"}<FontInstaller {controller} />{:else}<FontDetail {controller} />{/if}
{/snippet}

<ResourceWorkspaceShell panelId="design-panel-fonts" tabId="design-tab-fonts" detailLabel={t("design-detail-label")} {list} {detail} />

<style>
  .font-row { display: grid; width: 100%; grid-template-columns: 34px minmax(0, 1fr) auto 140px; align-items: center; gap: 8px; min-height: 52px; padding: 7px 9px; border: 1px solid transparent; border-radius: 7px; color: var(--wb-text-primary); background: transparent; text-align: left; }
  .font-row > div { display: grid; min-width: 0; gap: 3px; }
  .font-row strong, .font-row small, .font-row span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .font-row strong { color: var(--text-strong); font-size: 12px; }
  .font-row small, .font-row > span { color: var(--wb-text-muted); font-size: 12px; }
  .resource-icon { display: grid; width: 29px; height: 29px; place-items: center; border-radius: 6px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .font-registration { color: var(--wb-accent-strong); font-weight: 700; text-align: right; } .font-registration.missing { color: var(--danger); }
  .font-role-overview { display: grid; gap: 7px; margin-bottom: 8px; padding: 9px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-chrome); }
  .font-role-overview > header { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  .font-role-overview > header strong { color: var(--text-strong); font-size: 12px; } .font-role-overview > header small { color: var(--wb-text-muted); font-size: 11px; }
  .font-role-overview > div { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 5px; }
  .font-role-overview > div > span { display: grid; min-width: 0; gap: 2px; padding: 6px 7px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; background: var(--wb-surface-document); }
  .font-role-overview small { color: var(--wb-accent-strong); font-size: 11px; font-weight: 800; text-transform: uppercase; }
  .font-role-overview strong { overflow: hidden; color: var(--text-strong); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .font-role-overview .missing strong { color: var(--danger); }
</style>
