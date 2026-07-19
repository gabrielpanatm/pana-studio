<script lang="ts">
  import InspectorPane from "$lib/components/InspectorPane.svelte";
  import WorkspaceResizeHandle from "$lib/components/workspace/WorkspaceResizeHandle.svelte";
  import type { AppState } from "$lib/state/app.svelte";
  import type { SaveState } from "$lib/types";

  let {
    app,
    setStatusSourceContext = () => {},
  }: {
    app: AppState;
    setStatusSourceContext?: (
      context: { label: string; value: string; openable?: boolean } | null,
    ) => void;
  } = $props();

  let cssFiles = $derived(app.scannedProject
    ? app.scannedProject.files
        .filter((file) => (file.kind === "CSS" || file.kind === "SCSS") && file.role === "style")
        .map((file) => file.relativePath)
    : []);
  const centerUsesFullWorkspace = $derived(app.centerView === "site" || app.centerView === "kernel");
</script>

{#if !app.rightPaneCollapsed && !centerUsesFullWorkspace}
  <WorkspaceResizeHandle
    kind="right"
    active={app.activeResizeKind === "right"}
    ariaLabel="Redimensioneaza panoul din dreapta"
    onDrag={(event) => app.startResizeDrag("right", event)}
    onReset={() => app.resetResize("right")}
  />
{/if}

{#if !app.rightPaneCollapsed && !centerUsesFullWorkspace}
  <div
    class="inspector-pane-shell"
    inert={app.aiEditLeaseFrontendLockActive ? true : undefined}
    aria-busy={app.aiEditLeaseFrontendLockActive}
  >
    <InspectorPane
      selectedElement={app.selectedElement}
      projectRoot={app.sessionProjectRoot}
      runtimeSessionId={app.kernelProjectSessionId}
      previewSelection={app.previewSelection}
      sourceGraph={app.sourceGraph}
      selectedTemplateSourceNode={app.selectedTemplateSourceNode}
      saveState={app.saveState}
      targetCssFile={app.targetCssFile}
      codeSelectedCssTarget={app.codeSelectedCssTarget}
      cssSourceRevision={app.cssSourceRevision}
      activeRenderedTemplatePath={app.activeRenderedTemplatePath}
      previewDevice={app.previewDevice}
      refreshToken={app.refreshToken}
      jsRefreshToken={app.jsRefreshToken}
      cachebustAssets={app.cachebustAssets}
      {cssFiles}
      projectFiles={app.scannedProject?.files ?? []}
      setTargetCssFile={(path) => (app.targetCssFile = path)}
      scssVariables={app.scssVariables}
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
      cancelHtmlAttributeDraft={(expectedContextKey) => app.cancelHtmlAttributeDraft(expectedContextKey)}
      deleteHtmlElement={async () => {
        await app.deleteHtmlElement();
      }}
      editSelectedTeraLayer={() => app.editSelectedTeraLayer()}
      deleteSelectedTeraNode={async () => {
        await app.deleteSelectedTeraNode();
      }}
      openSelectedTeraSource={() => app.openSelectedTeraSource()}
      pendingTag={app.pendingTag}
      tagStatus={app.tagStatus}
      changeElementTag={(tag) => app.changeElementTag(tag)}
      openSourceLocation={(source) => app.openSourceLocation(source)}
      onLivePropertiesChange={(sel, properties, viewport) => app.applyInspectorLiveProperties(sel, properties, viewport)}
      onCssWorkspaceMutationCommitted={(authority, liveEpoch) =>
        app.projectCommittedInspectorCssMutation(authority, liveEpoch)}
      onScssVariableCommitted={(variable, value) => {
        app.scssVariables = app.scssVariables.map((candidate) => (
          candidate.file === variable.file && candidate.name === variable.name
            ? { ...candidate, value }
            : candidate
        ));
      }}
      onInspectorLivePropertiesRejected={(liveEpoch) => app.clearInspectorLiveProperties(liveEpoch)}
      injectPreviewCss={(css) => app.injectRawCss("pana-animation-preview", css)}
      onStatusUpdate={(text, kind) => app.setGlobalStatus(text, kind as SaveState)}
      onPendingChange={(area, pending) => app.setInspectorPending(area, pending, "inspector-pane")}
      onSourceContextChange={setStatusSourceContext}
      beforeInspectorTabChange={async (from, to) => {
        if (from === "js" && to !== "js") {
          await app.flushInteractiveEditorDrafts("template-switch");
        }
      }}
      onInspectorTabChange={(tab) => { app.activeInspectorTab = tab; }}
      onCssCodeTargetChange={(target) => app.setCssCodeRevealTarget(target)}
      getOpenCssRuleContext={(file, selector, viewport) =>
        app.cssRuleContextFromOpenSource(file, selector, viewport)}
    />
  </div>
{/if}
