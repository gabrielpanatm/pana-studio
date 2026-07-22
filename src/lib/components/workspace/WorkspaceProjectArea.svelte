<script lang="ts">
  import ProjectPane from "$lib/components/ProjectPane.svelte";
  import WorkspaceResizeHandle from "$lib/components/workspace/WorkspaceResizeHandle.svelte";
  import type { AppState } from "$lib/state/app.svelte";

  let { app }: { app: AppState } = $props();

  const activityUsesFullWorkspace = $derived(
    (app.workbenchSnapshot?.activeActivity ?? "editor") !== "editor",
  );
</script>

{#if !app.leftPaneCollapsed && !activityUsesFullWorkspace}
  <div class="project-pane-shell">
    <ProjectPane
      scannedProject={!!app.scannedProject}
      projectRoot={app.scannedProject?.root ?? ""}
      runtimeSessionId={app.kernelProjectSessionId}
      allProjectFiles={app.scannedProject?.files ?? []}
      scannedPages={app.scannedPages}
      scannedStyles={app.scannedStyles}
      scannedTemplates={app.scannedTemplates}
      scannedScripts={app.scannedScripts}
      scannedAssets={app.scannedAssets}
      activeScannedPath={app.activeScannedPath}
      pageSections={app.pageSections}
      selectedElement={app.selectedElement}
      previewSelection={app.previewSelection}
      sourceGraph={app.sourceGraph}
      loopPaletteItems={app.loopPaletteItems()}
      activeRenderedTemplatePath={app.activeRenderedTemplatePath}
      templateHtmlEditSourceId={app.templateHtmlEditSourceId}
      templateWorkbenchPlan={app.templateWorkbenchPlan}
      fileMoveBlockedReason={app.immediateDiskOperationBlockedReason}
      pageSource={app.pageSettingsSource()}
      openScannedFile={(file) => app.loadScannedProjectFile(file)}
      createProjectFile={(path, content) => app.createProjectFile(path, content)}
      moveProjectFile={(request) => app.moveProjectFile(request)}
      renameProjectFile={(request) => app.renameProjectFile(request)}
      deleteProjectFile={(request) => app.deleteProjectFile(request)}
      selectPageSection={(section) => app.selectLayerSection(section)}
      selectTeraSource={(section, sourceId) => app.selectTeraLayerSource(section, sourceId)}
      hoverPageSection={(section) => app.hoverLayerSection(section)}
      hoverTeraSource={(section, sourceId) => app.hoverTeraLayerSource(section, sourceId)}
      startElementPaletteDrag={(element, event) => app.startElementPaletteDrag(element, event)}
      startTeraPaletteDrag={(item, event) => app.startTeraPaletteDrag(item, event)}
      moveLayerElement={(request) => app.moveLayerElement(request)}
      moveTeraNode={async (request) => {
        await app.moveTeraNodeAtTarget(request);
      }}
      deleteLayerElement={async (selector) => {
        await app.deleteHtmlElement(selector);
      }}
      openLayerContextMenu={(request) => app.openLayerContextMenu(request)}
      editSelectedTeraLayer={() => app.editSelectedTeraLayer()}
      deleteSelectedTeraNode={async () => {
        await app.deleteSelectedTeraNode();
      }}
      openSelectedTeraSource={() => app.openSelectedTeraSource()}
      openTemplateWorkbenchSource={(file) => {
        const target = app.scannedProject?.files.find((candidate) => candidate.relativePath === file);
        if (target) void app.loadScannedProjectFile(target);
      }}
      updatePageFrontmatterSource={(path, source) => app.updatePageFrontmatterSource(path, source)}
    />
  </div>
{/if}

{#if !app.leftPaneCollapsed && !activityUsesFullWorkspace}
  <WorkspaceResizeHandle
    kind="left"
    active={app.activeResizeKind === "left"}
    ariaLabel="Redimensioneaza panoul din stanga"
    onDrag={(event) => app.startResizeDrag("left", event)}
    onReset={() => app.resetResize("left")}
  />
{/if}
