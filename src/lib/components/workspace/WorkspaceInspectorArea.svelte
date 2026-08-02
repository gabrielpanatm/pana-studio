<script lang="ts">
  import InspectorPane from "$lib/components/InspectorPane.svelte";
  import WorkspaceResizeHandle from "$lib/components/workspace/WorkspaceResizeHandle.svelte";
  import { t } from "$lib/i18n/runtime.svelte";
  import { getFontManager } from "$lib/project/io";
  import type { AppState } from "$lib/state/app.svelte";
  import type { GlobalStatusKind } from "$lib/status/global-status";
  import type { InstalledFontVariationAxis } from "$lib/types";

  let { app }: { app: AppState } = $props();

  const editorSidebarActive = $derived(
    app.applicationSurface === "workbench"
      && (app.workbenchSnapshot?.activeActivity ?? "editor") === "editor",
  );
  let installedFontFamilies = $state<string[]>([]);
  let installedFontAxes = $state<InstalledFontVariationAxis[]>([]);
  let fontLoadSequence = 0;

  $effect(() => {
    const snapshot = app.projectWorkspaceSnapshot;
    if (!snapshot) {
      installedFontFamilies = [];
      installedFontAxes = [];
      return;
    }
    const requestId = ++fontLoadSequence;
    const expectedRevision = snapshot.revision;
    void getFontManager({
      expectedProjectRoot: snapshot.projectRoot,
      expectedSessionId: snapshot.runtimeSessionId,
      expectedRevision,
    }).then((manager) => {
      if (
        requestId !== fontLoadSequence
        || app.projectWorkspaceSnapshot?.revision !== expectedRevision
      ) return;
      installedFontFamilies = manager.inventory.families
        .filter((family) => family.registration.registered)
        .map((family) => family.family);
      const axes = manager.inventory.families
        .filter((family) => family.registration.registered)
        .flatMap((family) => family.files.flatMap((file) => (
          file.axes.map((axis) => ({ family: family.family, ...axis }))
        )));
      installedFontAxes = axes.filter((axis, index) => axes.findIndex((candidate) => (
        candidate.family === axis.family
        && candidate.tag === axis.tag
        && candidate.min === axis.min
        && candidate.default === axis.default
        && candidate.max === axis.max
      )) === index);
    }).catch(() => {
      if (requestId === fontLoadSequence) {
        installedFontFamilies = [];
        installedFontAxes = [];
      }
    });
  });
</script>

{#if !app.rightPaneCollapsed && editorSidebarActive}
  <WorkspaceResizeHandle
    kind="right"
    active={app.activeResizeKind === "right"}
    ariaLabel={t("workbench-resize-right-panel")}
    onDrag={(event) => app.startResizeDrag("right", event)}
    onReset={() => app.resetResize("right")}
  />
{/if}

{#if app.scannedProject && app.kernelProjectSessionId}
  {#key app.kernelProjectSessionId}
    <div
      class="inspector-pane-shell"
      hidden={app.rightPaneCollapsed}
      inert={!editorSidebarActive
        || app.rightPaneCollapsed
        || app.aiEditLeaseFrontendLockActive
        || app.kernelUndoRedoFrontendQuiesceActive
        || app.kernelUndoRedoFrontendLeaseActive
        ? true
        : undefined}
      aria-hidden={!editorSidebarActive || app.rightPaneCollapsed}
      aria-busy={app.aiEditLeaseFrontendLockActive
        || app.kernelUndoRedoFrontendQuiesceActive
        || app.kernelUndoRedoFrontendLeaseActive}
    >
      <InspectorPane
      inspectorSelectionSummary={app.inspectorSelectionSummary}
      inspectorHtmlPhysicalFacts={app.inspectorHtmlPhysicalFacts}
      inspectorBlockSelectionContext={app.inspectorBlockSelectionContext}
      projectRoot={app.sessionProjectRoot}
      runtimeSessionId={app.kernelProjectSessionId}
      selectedTemplateSourceNode={app.selectedTemplateSourceNode}
      selectedEditorNavigationNode={app.selectedEditorNavigationNode}
      targetCssFile={app.targetCssFile}
      selectionSnapshot={app.selectionSnapshot}
      cssSourceRevision={app.cssSourceRevision}
      activeRenderedTemplatePath={app.activeRenderedTemplatePath}
      previewDevice={app.previewDevice}
      refreshToken={app.refreshToken}
      historyProjectionQuiesced={app.kernelUndoRedoFrontendQuiesceActive
        || app.kernelUndoRedoFrontendLeaseActive}
      jsRefreshToken={app.jsRefreshToken}
      motionWorkspace={app.motionWorkspace}
      workspaceRevision={app.projectWorkspaceSnapshot?.revision ?? 0}
      previewRevision={app.activeCanvasIdentity?.previewRevision ?? ""}
      blockPropertiesHeight={app.applicationSettings?.blockPropertiesHeight ?? 220}
      blockPropertiesCollapsed={app.applicationSettings?.blockPropertiesCollapsed ?? false}
      cachebustAssets={app.cachebustAssets}
      projectFiles={app.scannedProject?.files ?? []}
      scssVariables={app.scssVariables}
      fontFamilies={installedFontFamilies}
      {installedFontAxes}
      attributeValues={app.attributeValues}
      attributeStatus={app.attributeStatus}
      textContentValue={app.textContentValue}
      textStatus={app.textStatus}
      classEditorValue={app.classEditorValue}
      classStatus={app.classStatus}
      imageSourceValue={app.imageSourceValue}
      imageStatus={app.imageStatus}
      scannedAssets={app.scannedAssets}
      updateAttributeValue={(prop, val) => app.updateAttributeValue(prop, val)}
      removeAttribute={(name) => app.removeAttribute(name)}
      isActivePreviewHtmlSource={app.isActivePreviewHtmlSource}
      canEditHtml={app.canEditHtml}
      applyAttributesToHtml={(attributes) => app.applyAttributesToHtml(attributes)}
      updateTextContentValue={(val, composing) => app.updateTextContentValue(val, composing)}
      applyTextContentToHtml={() => app.applyTextContentToHtml()}
      setClassEditorValue={(value) => (app.classEditorValue = value)}
      applyClassesToHtml={() => app.applyClassesToHtml()}
      generateClassForSelectedHtml={() => app.generateClassForSelectedHtml()}
      generateDataAnimForSelectedHtml={() => app.generateDataAnimForSelectedHtml()}
      setImageSourceValue={(value) => (app.imageSourceValue = value)}
      applyZolaImageProcessingToHtml={(intent) => app.applyZolaImageProcessingToHtml(intent)}
      cancelHtmlAttributeDraft={(expectedContextKey) => app.cancelHtmlAttributeDraft(expectedContextKey)}
      enterTeraBoundary={async (scopeId) => {
        await app.enterEditorNavigationScope(scopeId);
      }}
      deleteSelectedTeraNode={async () => {
        await app.deleteSelectedTeraNode();
      }}
      openSelectedTeraSource={() => app.openSelectedTeraSource()}
      openSelectedMarkdownContent={() => app.openSelectedMarkdownContent()}
      pendingTag={app.pendingTag}
      tagStatus={app.tagStatus}
      changeElementTag={(tag) => app.changeElementTag(tag)}
      onLivePropertiesChange={(sel, properties, viewport) => app.applyInspectorLiveProperties(sel, properties, viewport)}
      onCssWorkspaceMutationCommitted={(authority, liveEpoch) =>
        app.projectCommittedInspectorCssMutation(authority, liveEpoch)}
      onInspectorLivePropertiesRejected={(liveEpoch) => app.clearInspectorLiveProperties(liveEpoch)}
      gridOverlayEnabled={app.gridOverlayEnabled}
      onGridOverlayChange={(enabled) => app.setGridOverlayEnabled(enabled)}
      injectPreviewCss={(css) => app.injectRawCss("pana-animation-preview", css)}
      onStatusUpdate={(text, kind) => app.setGlobalStatus(text, kind as GlobalStatusKind)}
      onPendingChange={(area, pending) => app.setInspectorPending(area, pending, "inspector-pane")}
      beforeInspectorTabChange={async (from, to) => {
        if (from === "js" && to !== "js") {
          await app.flushInteractiveEditorDrafts("template-switch");
        }
      }}
      onInspectorTabChange={(tab) => app.selectInspectorTab(tab)}
      onCssCodeTargetChange={(target) => app.selectCssFocusFromInspector(target)}
      getOpenCssRuleContext={(file, selector, viewport) =>
        app.cssRuleContextFromOpenSource(file, selector, viewport)}
      applyNativeBlockOption={(request) => app.applyNativeBlockOption(request)}
      persistBlockPropertiesLayout={(height, collapsed) => {
        void app.persistBlockPropertiesLayout(height, collapsed);
      }}
      />
    </div>
  {/key}
{/if}
