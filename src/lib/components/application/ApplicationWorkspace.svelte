<script lang="ts">
  import "../../../routes/workspace-shell.css";
  import { type Component, type ComponentProps } from "svelte";
  import type InspectorPane from "$lib/components/InspectorPane.svelte";
  import type EditorShellComponent from "$lib/components/EditorShell.svelte";
  import type { TerminalPaneProps } from "$lib/components/TerminalPane.svelte";
  import type { AppNotification } from "$lib/notifications/center";
  import type { EditFlushReason } from "$lib/session/edit-flush-registry";
  import type { GlobalStatusKind } from "$lib/status/global-status";
  import AppChrome from "$lib/components/workspace/AppChrome.svelte";
  import StartupView from "$lib/components/startup/StartupView.svelte";
  import ProjectOpenRecoveryDialog from "$lib/components/project/ProjectOpenRecoveryDialog.svelte";
  import ProjectTransitionDecisionDialog from "$lib/components/project/ProjectTransitionDecisionDialog.svelte";
  import { primarySelectionEditorNodeId } from "$lib/kernel/selection-read-model";
  import { isMessageFromExactPreviewFrame } from "$lib/preview/frame-origin";
  import { isPreviewControlPlaneMessage } from "$lib/state/app-preview-runtime-controller";
  import { ProjectWorkspaceMutationService } from "$lib/session/workspace-mutation-service";
  import { flushWorkspaceMutationInputs } from "$lib/session/workspace-mutation-coordinator";
  import { ApplicationPreferencesState } from "$lib/application/preferences.svelte";
  import { stableProjection } from "$lib/application/stable-projection";
  import { CommandCenterService } from "$lib/application/command-center-service.svelte";
  import { createApplicationComposition } from "$lib/application/composition.svelte";
  import {
    loadWorkspaceSurfaces,
    type WorkspaceSurfaces,
  } from "$lib/application/workspace-surfaces";
  import { WorkspacePageLifecycle } from "$lib/application/workspace-page-lifecycle";
  import { AiContextLifecycle } from "$lib/ai/context-lifecycle.svelte";
  import { CanvasInteractionLifecycle } from "$lib/canvas/interaction-lifecycle.svelte";
  import { CodeEditorLifecycle } from "$lib/editor/lifecycle.svelte";
  import { ProjectWorkspaceLifecycle } from "$lib/kernel/project-workspace-lifecycle.svelte";
  import { LifecycleGroup } from "$lib/lifecycle/group";
  import { MotionWorkspaceState } from "$lib/motion/workspace.svelte";
  import { MotionWorkspaceLifecycle } from "$lib/motion/workspace-lifecycle";
  import { NotificationCenterState } from "$lib/notifications/store.svelte";
  import { PreviewAppearanceLifecycle } from "$lib/preview/appearance-lifecycle.svelte";
  import { NativeWindowCloseLifecycle } from "$lib/session/native-window-close-lifecycle.svelte";
  import { GlobalStatusState } from "$lib/status/state.svelte";
  import { TerminalLifecycle } from "$lib/terminal/lifecycle.svelte";
  import { registerApplicationCompositionRuntimeProbe } from "$lib/performance/application-composition-runtime-probe.svelte";
  import { WorkspaceLayoutPersistenceLifecycle } from "$lib/ui/workspace-layout-lifecycle.svelte";
  import { WorkspaceLayoutState } from "$lib/ui/workspace-layout.svelte";
  import { WorkbenchDocumentNavigationService } from "$lib/workbench/document-navigation";
  import { WorkspaceHistoryService } from "$lib/versioning/workspace-history-service.svelte";
  import { reloadAuthorizedAiReconciliationFromDisk } from "$lib/state/ai-coordination-controller";
  import { deleteShortcutIntent } from "$lib/ui/app-shortcuts";
  import { errorMessage } from "$lib/util";
  import type { CenterView } from "$lib/application/contracts";
  import type {
    InsertCatalogContext,
    InsertCatalogItem,
    InsertCatalogSnapshot,
  } from "$lib/blocks/contracts";
  import type { EditorNavigationNode } from "$lib/editor/contracts";
  import type { ProjectMovePosition } from "$lib/preview/contracts";
  import type {
    FileExplorerOperationPlan,
    FileExplorerOperationRequest,
  } from "$lib/project/file-explorer-contract";
  import type { ProjectFile } from "$lib/project/lifecycle-contract";
  import type { WorkspaceSourceOpenOptions } from "$lib/workbench/contracts";
  import { onMount } from "svelte";
  import { t } from "$lib/i18n/runtime.svelte";

  type ForwardedInspectorPaneProps = Omit<
    ComponentProps<typeof InspectorPane>,
    | "motionWorkspace"
    | "fontFamilies"
    | "installedFontAxes"
    | "blockPropertiesHeight"
    | "blockPropertiesCollapsed"
    | "persistBlockPropertiesLayout"
  >;
  type ForwardedEditorShellProps = Omit<
    ComponentProps<typeof EditorShellComponent>,
    | "surfaceActive"
    | "responsiveBreakpoints"
    | "motionPreviewMode"
    | "motionPreviewRequest"
    | "onMotionPreviewStatus"
    | "dirtyWorkbenchPaths"
    | "documentActivation"
    | "activateWorkbenchDocument"
    | "closeWorkbenchDocument"
    | "setWorkbenchSurface"
    | "previewFrame"
    | "codeEditorHost"
  >;
  type WorkspaceCenterAreaProps = ComponentProps<WorkspaceSurfaces["centerArea"]>;
  const motionWorkspace = new MotionWorkspaceState();
  const motionWorkspaceLifecycle = new MotionWorkspaceLifecycle(
    () => motionWorkspace.flush(),
    () => motionWorkspace.pendingCount > 0,
  );
  const notificationCenter = new NotificationCenterState();
  const globalStatus = new GlobalStatusState(notificationCenter);
  const applicationPreferences = new ApplicationPreferencesState(globalStatus);
  const workspaceLayout = new WorkspaceLayoutState();
  const workspaceLayoutPersistence = new WorkspaceLayoutPersistenceLifecycle(workspaceLayout);
  const performanceProbeEnabled = import.meta.env.DEV
    || import.meta.env.VITE_PANA_PERFORMANCE_PROBE === "1";
  const compositionConstructionStartedAt = performanceProbeEnabled ? performance.now() : 0;
  const {
    publishWorkspace,
    projectAudit,
    fileExplorer,
    workbench,
    sourceWorkspace,
    htmlAuthoring,
    cssAuthoring,
    selectionWorkspace,
    documents,
    analysis,
    previewSurface,
    previewWorkspace,
    designClassInventory,
    controlledPreview,
    aiContext,
    aiCoordination,
    externalDisk,
    shell,
    projectSession,
    startup,
    transitionLease,
    historyOperation,
    versionPreview,
    readModel,
    canvasInteraction,
    workbenchNavigation,
    sourceNavigation,
    versionPreviewService,
    browserPreview,
    appSession,
    cssWorkspace,
    editorInteraction,
    htmlEditing,
    dynamicWidgets,
    editorNavigation,
    editorSelection,
    teraEditing,
    insertCatalogDrag,
    workspaceAuthority,
    previewRuntime,
    projectDocuments,
    projectDerived,
    projectTransitions,
    saveWorkspace,
    pageSettings,
    terminalWorkspace,
  } = createApplicationComposition({
    motionWorkspace,
    globalStatus,
    applicationPreferences,
  });
  const workspaceHistory = new WorkspaceHistoryService({
    project: projectSession,
    documents,
    source: sourceWorkspace,
    css: cssWorkspace,
    workbench,
    preview: previewWorkspace,
    derived: projectDerived,
    authority: workspaceAuthority,
    status: globalStatus,
  });
  const commandCenter = new CommandCenterService({
    guards: { ai: aiCoordination, externalDisk },
    workspace: {
      layout: workspaceLayout,
      shell,
      state: workbench,
      navigation: workbenchNavigation,
      source: sourceWorkspace,
    },
    project: {
      session: projectSession,
      documents: projectDocuments,
      startup,
      transitions: projectTransitions,
    },
    actions: {
      save: saveWorkspace,
      history: workspaceHistory,
      controlledPreview,
      browserPreview,
      appSession,
      derived: projectDerived,
      terminal: terminalWorkspace,
      motion: motionWorkspace,
      preview: previewWorkspace,
      selectActivity: (activity) => selectWorkbenchActivity(activity),
      openAudit: (view, focusObservability) => openAuditWorkspace(view, focusObservability),
    },
    preferences: applicationPreferences,
  });
  const htmlDraft = editorInteraction.htmlDraft;
  const applicationRuntimeLifecycle = {
    status: globalStatus,
    project: {
      reattach: () => projectTransitions.reattach(),
      startup,
    },
    preview: previewWorkspace,
    terminal: terminalWorkspace,
    source: sourceWorkspace,
    selection: selectionWorkspace,
    ai: { context: aiContext, coordination: aiCoordination },
    externalDisk,
    editor: editorInteraction,
  };
  const workspaceMutations = new ProjectWorkspaceMutationService(workspaceAuthority.settlementHost());
  const workbenchDocuments = new WorkbenchDocumentNavigationService({
    currentSnapshot: () => workbench.snapshot,
    resolveProjectFile: (relativePath) => fileExplorer.resolveProjectFile(relativePath),
    loadProjectFile: (file, options) => projectDocuments.load(file, options),
    applyIntent: (intent) => workbench.apply(intent),
    setCenterView: (view) => workbenchNavigation.setCenterView(view),
    beginDocumentActivation: (serial, document) => workbench.beginDocumentActivation(serial, document),
    updateDocumentActivation: (serial, patch) => workbench.updateDocumentActivation(serial, patch),
    currentTemplateCacheOutcome: () => documents.templatePublicationStatus,
  }, globalStatus);
  const projectAreaInsertCatalogContext: InsertCatalogContext = stableProjection({
    activeDocumentPath: () => documents.activeScannedPath,
    activeTemplatePath: () => documents.activeRenderedTemplatePath,
    activePagePath: () => documents.templatePreferredPagePath,
    canvasPreviewRevision: () => previewWorkspace.activeIdentity?.previewRevision ?? null,
    canvasAvailable: () => shell.centerView === "preview" && Boolean(previewWorkspace.activeIdentity),
    targetSourceId: () => selectionWorkspace.coordinatedElement?.sourceNodeId ?? null,
    targetTag: () => selectionWorkspace.coordinatedElement?.observation.tag ?? null,
  });
  const projectAreaPane = stableProjection({
    projectRoot: () => projectSession.project?.root ?? "",
    workspaceRevision: () => projectSession.workspace?.revision ?? 0,
    allProjectFiles: () => projectSession.project?.files ?? [],
    activeScannedPath: () => documents.activeScannedPath,
    layersAvailable: () => workbench.activeDocumentPresentation === "html",
    fileExplorerSnapshot: () => fileExplorer.snapshot,
    fileExplorerLoading: () => fileExplorer.loading,
    fileExplorerError: () => fileExplorer.error,
    insertCatalogContext: () => projectAreaInsertCatalogContext,
    editorNavigationSnapshot: () => selectionWorkspace.session.navigationSnapshot,
    editorNavigationLoading: () => selectionWorkspace.session.navigationLoading,
    editorNavigationError: () => selectionWorkspace.session.navigationError,
    coordinatedSelectionNodeIds: () => selectionWorkspace.session.selectionSnapshot?.members
      .flatMap((member) => member.anchor.editorNodeId ? [member.anchor.editorNodeId] : []) ?? [],
    coordinatedPrimaryNodeId: () => primarySelectionEditorNodeId(
      selectionWorkspace.session.selectionSnapshot,
    ),
    hoveredEditorNavigationNodeId: () => selectionWorkspace.session.hoverSnapshot?.editorNodeId ?? null,
    editorEditScopeId: () => selectionWorkspace.session.editScopeId,
  });
  const projectAreaCommands = {
    selectFileExplorerEntry: (entryId: string) => fileExplorer.select(entryId),
    planFileExplorerOperation: (operation: FileExplorerOperationRequest) => fileExplorer.plan(operation),
    commitFileExplorerOperation: (plan: FileExplorerOperationPlan) => fileExplorer.commit(plan),
    openScannedFile: (file: ProjectFile) => projectDocuments.load(file),
    startInsertCatalogDrag: (item: InsertCatalogItem, snapshot: InsertCatalogSnapshot, event: PointerEvent) => (
      insertCatalogDrag.start(item, snapshot, event)
    ),
    selectEditorNavigationNode: (
      node: EditorNavigationNode,
      options?: { toggle?: boolean; extendRange?: boolean; setPrimary?: boolean },
    ) => editorNavigation.select(node, options),
    hoverEditorNavigationNode: (node: EditorNavigationNode | null) => editorNavigation.hover(node),
    enterEditorNavigationScope: (scopeId: string) => editorNavigation.enterScope(scopeId),
    exitEditorNavigationScope: () => editorNavigation.exitScope(),
    previewEditorNavigationMove: (sourceNodeId: string, targetNodeId: string, position: ProjectMovePosition) => (
      editorNavigation.previewMove(sourceNodeId, targetNodeId, position)
    ),
    moveEditorNavigationNode: (sourceNodeId: string, targetNodeId: string, position: ProjectMovePosition) => (
      editorNavigation.move(sourceNodeId, targetNodeId, position)
    ),
    deleteEditorNavigationNode: (node: EditorNavigationNode) => editorNavigation.deleteNode(node),
    openEditorNavigationContextMenu: (node: EditorNavigationNode, x: number, y: number) => (
      editorNavigation.openContextMenu(node, x, y)
    ),
  };
  const centerAreaSession = stableProjection({
    applicationSurface: () => shell.surface,
    applicationSettingsSection: () => shell.settingsSection,
    workbenchSnapshot: () => workbench.snapshot,
    centerView: () => shell.centerView,
    sessionId: () => projectSession.runtimeSessionId,
    projectRoot: () => projectSession.root,
    project: () => projectSession.project,
    workspace: () => projectSession.workspace,
    interactionLocked: () => aiCoordination.frontendLockActive
      || historyOperation.quiesceActive
      || historyOperation.leaseActive,
    activeRenderedTemplatePath: () => documents.activeRenderedTemplatePath ?? "",
    activeScannedPath: () => documents.activeScannedPath,
    jsRefreshToken: () => projectSession.jsRefreshToken,
    interactivePreviewEnabled: () => previewWorkspace.interactiveEnabled,
  });
  const centerAreaCreation = stableProjection({
    sourceGraph: () => analysis.sourceGraph,
    coordinatedElementSelection: () => selectionWorkspace.coordinatedElement,
    activeCanvasPreviewRevision: () => previewWorkspace.activeIdentity?.previewRevision ?? "",
    assetPreviewRevision: () => previewWorkspace.workspaceRevision
      ?? previewWorkspace.pendingProjection?.identity.previewRevision
      ?? previewWorkspace.activeIdentity?.previewRevision
      ?? null,
    templateWorkbenchPreferredPagePath: () => documents.templatePreferredPagePath,
    designClassInventory: () => designClassInventory.snapshot,
    designClassInventoryLoading: () => designClassInventory.loading,
    designClassInventoryError: () => designClassInventory.error,
    scssVariables: () => analysis.scssVariables,
    fileExplorerSnapshot: () => fileExplorer.snapshot,
    scannedPages: () => documents.scannedPages,
    scannedTemplates: () => documents.scannedTemplates,
  });
  const centerAreaAuxiliary = stableProjection({
    aiContextStatus: () => aiContext.status,
    currentAudit: () => projectAudit.current(),
    projectStatus: () => projectSession.status,
    refreshToken: () => projectSession.refreshToken,
    activeVersionPreview: () => versionPreview.active,
    externalDiskWatchRevision: () => externalDisk.watchRevision,
    projectAuditLoading: () => projectAudit.loading,
    projectAuditError: () => projectAudit.error,
    validationRunning: () => controlledPreview.snapshot.validation === "running",
    validationMessage: () => controlledPreview.snapshot.validationMessage,
    currentProjectPath: () => documents.currentProjectPath,
    dirtyAreas: () => readModel.globalDirtyState.areas,
    canSave: () => readModel.globalDirtyState.canSave,
    diskBlockedReason: () => readModel.immediateDiskOperationBlockedReason,
    auditView: () => projectAudit.view,
    auditFocusSerial: () => projectAudit.observabilityFocusSerial,
    motionSelectionSummary: () => selectionWorkspace.session.inspectorSummary,
    motionDataAnim: () => selectionWorkspace.coordinatedElement
      ?.observation.attributes["data-anim"] ?? null,
  });
  const centerAreaWorkspaceCommands: WorkspaceCenterAreaProps["workspaceCommands"] = {
    setInspectorJsPending: (pending) => htmlAuthoring.setInspectorPending(
      "js", pending, "motion-timeline",
    ),
    setPreviewExecutionMode: (mode) => previewWorkspace.setExecutionMode(mode),
    setWorkbenchActivity: (activity) => workbench.setActivity(activity),
    openInBrowser: (route = null) => browserPreview.open(route),
    saveActiveFile: () => saveWorkspace.saveActiveFile(),
    openAudit: (focusObservability = false) => openAuditWorkspace("runtime", focusObservability),
    revealSourceRange: (file, range) => sourceWorkspace.revealSourceRange(file, range),
    showVersionPreview: (receipt) => versionPreviewService.show(receipt),
    returnToLivePreview: () => versionPreviewService.returnToLive(),
  };
  const centerAreaCreationCommands: WorkspaceCenterAreaProps["creationCommands"] = {
    refreshClassInventory: () => designClassInventory.refresh(),
    createVariable: (path, name, value) => cssWorkspace.createVariable(path, name, value),
    createClass: (name, path) => cssWorkspace.createClass(name, path),
    updateVariable: (variable, value) => cssWorkspace.updateVariable(variable, value),
    renameClass: (oldName, newName) => cssWorkspace.renameClass(oldName, newName),
    refreshFileExplorer: () => fileExplorer.refresh(),
    planFileExplorer: (request) => fileExplorer.plan(request),
    commitFileExplorer: (plan) => fileExplorer.commit(plan),
    injectRawCss: (id, css) => cssWorkspace.injectRaw(id, css),
    projectCommittedCssMutation: (authority, liveEpoch) => cssWorkspace
      .projectCommittedMutation(authority, liveEpoch),
    applyImageSource: (source) => htmlEditing.applyImage(source),
  };
  const centerAreaContentCommands: WorkspaceCenterAreaProps["contentCommands"] = {
    createPage: (input) => projectDocuments.create(input),
    openPageEditor: (relativePath) => workbench.openContentPage(relativePath),
    updateFrontmatterSource: (relativePath, source) => pageSettings.updateSource(relativePath, source),
    updateFrontmatterField: (relativePath, field, value) => pageSettings.updateField(
      relativePath, field, value,
    ),
    readPageSettings: (relativePath) => pageSettings.readDocument(relativePath),
  };
  const centerAreaAuditCommands: WorkspaceCenterAreaProps["auditCommands"] = {
    applySafeFix: (finding, fixId) => projectAudit.applySafeFix(finding, fixId),
    refresh: (force, mode) => projectAudit.refresh(force, mode),
    runValidation: () => controlledPreview.runValidation("refresh"),
    setView: (view) => { projectAudit.view = view; },
  };
  const compositionConstructionEndedAt = performanceProbeEnabled ? performance.now() : 0;
  const applicationLifecycles = new LifecycleGroup([
    motionWorkspaceLifecycle,
    workspaceLayoutPersistence,
    new CanvasInteractionLifecycle(canvasInteraction),
    new ProjectWorkspaceLifecycle({
      project: projectSession,
      ai: aiCoordination,
      explorer: fileExplorer,
      workbench,
      preview: workspaceAuthority.previewHost(),
      escalateStatus: (request) => globalStatus.escalate(request),
    }),
    new NativeWindowCloseLifecycle({
      shell,
      project: projectSession,
      startup,
      transition: transitionLease,
      closeProject: (root, owner) => projectTransitions.close(root, owner),
      setStatus: (text, kind) => globalStatus.set(text, kind),
    }),
    new CodeEditorLifecycle({
      documents,
      shell,
      workbench,
      source: sourceWorkspace,
      selection: selectionWorkspace,
      css: cssAuthoring,
      mutationLocked: () => transitionLease.isActive
        || historyOperation.quiesceActive
        || historyOperation.leaseActive
        || aiCoordination.frontendLockActive,
    }, applicationPreferences),
    new PreviewAppearanceLifecycle({
      preview: previewWorkspace,
      surface: previewSurface,
    }, applicationPreferences),
    new TerminalLifecycle(terminalWorkspace, applicationPreferences),
    new AiContextLifecycle(aiContext),
  ]);
  const unregisterCompositionRuntimeProbe = performanceProbeEnabled
    ? registerApplicationCompositionRuntimeProbe({
        constructionStartedAt: compositionConstructionStartedAt,
        constructionEndedAt: compositionConstructionEndedAt,
        workspaceLayout,
      })
    : () => {};
  if (import.meta.env.DEV && typeof window !== "undefined") {
    Object.defineProperty(window, "__PANA_EDITOR_SELECTION_DIAGNOSTICS__", {
      configurable: true,
      value: () => selectionWorkspace.session.diagnosticSnapshot(),
    });
    Object.defineProperty(window, "__PANA_PREVIEW_DIAGNOSTICS__", {
      configurable: true,
      value: () => ({
        projectRoot: projectSession.root,
        runtimeSessionId: projectSession.runtimeSessionId,
        lifecycle: projectSession.lifecycle.activeSession?.readiness ?? null,
        src: previewWorkspace.src,
        pendingProjection: previewWorkspace.pendingProjection?.identity ?? null,
        deferredProjection: previewWorkspace.deferredProjection?.identity ?? null,
        confirmation: previewWorkspace.confirmation
          ? {
              transactionId: previewWorkspace.confirmation.transactionId,
              surfaceGeneration: previewWorkspace.confirmation.surfaceGeneration,
              lastPhase: previewWorkspace.confirmation.lastPhase,
            }
          : null,
        activeIdentity: previewWorkspace.activeIdentity,
        surface: {
          generation: previewSurface.generation,
          loadedGeneration: previewSurface.loadedGeneration,
          mounted: previewWorkspace.hasMountedSurface(),
          resumeRequired: previewSurface.resumeRequired,
          resumeScheduled: previewSurface.resumeScheduled,
          resumeInFlight: Boolean(previewSurface.resumePromise),
        },
        templateWorkbenchActive: documents.templateActive,
        templateTarget: documents.templateTarget,
        resumeEvents: previewWorkspace.resumeDiagnosticSnapshot(),
      }),
    });
  }
  const pageLifecycle = new WorkspacePageLifecycle({
    resources: {
      domains: applicationLifecycles,
      layout: workspaceLayout,
      preferences: applicationPreferences,
      status: globalStatus,
      unregisterRuntimeProbe: unregisterCompositionRuntimeProbe,
    },
    runtime: applicationRuntimeLifecycle,
    events: {
      message: handleWindowMessage,
      shortcut: handleAppShortcuts,
      deleteShortcut: handleDeleteShortcut,
      projectLifecycle: (snapshot) => { projectSession.lifecycle = snapshot; },
    },
  });
  let ActivityRail = $state<WorkspaceSurfaces["activityRail"] | null>(null);
  let WorkspaceCenterArea = $state<WorkspaceSurfaces["centerArea"] | null>(null);
  let WorkspaceInspectorArea = $state<WorkspaceSurfaces["inspectorArea"] | null>(null);
  let WorkspaceProjectArea = $state<WorkspaceSurfaces["projectArea"] | null>(null);
  let workspaceSurfaceError = $state("");
  let TerminalPaneComponent = $state<Component<TerminalPaneProps> | null>(null);
  let terminalPaneLoading = false;
  let externalRecoveryInFlight = $state(false);
  const editorSidebarsAvailable = $derived(
    shell.surface === "workbench"
      && (workbench.snapshot?.activeActivity ?? "editor") === "editor",
  );
  const activeLifecycleReadiness = $derived(projectSession.lifecycle.activeSession?.readiness ?? null);
  const selectionInitializing = $derived(Boolean(
    projectSession.project
      && !selectionWorkspace.session.inspectorSummary
      && (
        selectionWorkspace.session.navigationLoading
        || previewSurface.resumeRequired
        || previewWorkspace.pendingProjection
        || previewWorkspace.confirmation
        || (
          activeLifecycleReadiness !== null
          && activeLifecycleReadiness.state !== "ready"
          && activeLifecycleReadiness.state !== "degraded"
        )
      ),
  ));
  const inspectorPaneProps = $derived.by((): ForwardedInspectorPaneProps => ({
    inspectorSelectionSummary: selectionWorkspace.session.inspectorSummary,
    selectionInitializing,
    inspectorHtmlPhysicalFacts: selectionWorkspace.htmlPhysicalFacts,
    inspectorBlockSelectionContext: selectionWorkspace.blockContext,
    inspectorDynamicWidgetSelectionContext: selectionWorkspace.dynamicWidgetContext,
    sourceGraph: analysis.sourceGraph,
    projectRoot: projectSession.root,
    runtimeSessionId: projectSession.runtimeSessionId,
    selectedTemplateSourceNode: selectionWorkspace.selectedTemplateSourceNode,
    selectedEditorNavigationNode: selectionWorkspace.selectedEditorNavigationNode,
    targetCssFile: cssAuthoring.targetFile,
    selectionSnapshot: selectionWorkspace.session.selectionSnapshot,
    cssSourceRevision: sourceWorkspace.cssSourceRevision,
    activeRenderedTemplatePath: documents.activeRenderedTemplatePath,
    previewDevice: workbench.previewDevice,
    refreshToken: projectSession.refreshToken,
    historyProjectionQuiesced: historyOperation.quiesceActive
      || historyOperation.leaseActive,
    workspaceRevision: projectSession.workspace?.revision ?? 0,
    previewRevision: previewWorkspace.activeIdentity?.previewRevision ?? "",
    scssVariables: analysis.scssVariables,
    attributeValues: htmlDraft.attributeValues,
    attributeStatus: htmlDraft.attributeStatus,
    attributePending: htmlAuthoring.htmlPending.attributes,
    textContentValue: htmlDraft.textContentValue,
    textStatus: htmlDraft.textStatus,
    classEditorValue: htmlAuthoring.classEditorValue,
    classPending: htmlAuthoring.htmlPending.classes,
    classStatus: htmlAuthoring.classStatus,
    imageSourceValue: htmlAuthoring.imageSourceValue,
    imageStatus: htmlAuthoring.imageStatus,
    scannedAssets: documents.scannedAssets,
    updateAttributeValue: (property, value) => htmlDraft.updateAttribute(property, value),
    removeAttribute: (name) => htmlDraft.removeAttribute(name),
    isActivePreviewHtmlSource: sourceWorkspace.isActivePreviewHtmlSource,
    canEditHtml: readModel.canEditHtml,
    applyAttributesToHtml: (attributes) => htmlDraft.applyAttributes(attributes),
    updateTextContentValue: (value, composing) => htmlDraft.updateText(value, composing),
    applyTextContentToHtml: () => htmlDraft.applyText(),
    setClassEditorValue: (value) => { htmlAuthoring.classEditorValue = value; },
    applyClassesToHtml: () => htmlEditing.applyClasses(),
    generateClassForSelectedHtml: () => htmlEditing.generateClass(),
    generateDataAnimForSelectedHtml: () => htmlEditing.generateDataAnim(),
    setImageSourceValue: (value) => { htmlAuthoring.imageSourceValue = value; },
    applyZolaImageProcessingToHtml: (intent) => htmlEditing.applyZolaImage(intent),
    cancelHtmlAttributeDraft: (expectedContextKey) => htmlDraft.cancelAttributes(expectedContextKey),
    enterBoundary: async (scopeId) => { await editorNavigation.enterScope(scopeId); },
    deleteSelectedTeraNode: async () => { await teraEditing.delete(); },
    openSelectedTeraSource: () => editorSelection.openSelectedTeraSource(),
    openSelectedMarkdownContent: () => editorSelection.openSelectedMarkdownContent(),
    pendingTag: htmlAuthoring.pendingTag,
    tagStatus: htmlAuthoring.tagStatus,
    changeElementTag: (tag) => htmlEditing.changeTag(tag),
    onLivePropertiesChange: (selection, properties, viewport) => (
      cssWorkspace.applyLiveProperties(selection, properties, viewport)
    ),
    onCssWorkspaceMutationCommitted: (authority, liveEpoch) => (
      cssWorkspace.projectCommittedMutation(authority, liveEpoch)
    ),
    onInspectorLivePropertiesRejected: (liveEpoch) => cssWorkspace.clearLiveProperties(liveEpoch),
    gridOverlayEnabled: previewWorkspace.gridOverlayEnabled,
    onGridOverlayChange: (enabled) => previewWorkspace.setGridOverlay(enabled),
    onStatusUpdate: (text, kind, options) => globalStatus.set(text, kind as GlobalStatusKind, options),
    onPendingChange: (area, pending) => htmlAuthoring.setInspectorPending(area, pending, "inspector-pane"),
    beforeInspectorTabChange: async (from, to) => {
      if (from === "js" && to !== "js") {
        await flushWorkspaceMutationInputs("template-switch");
      }
    },
    onInspectorTabChange: (tab) => sourceNavigation.selectInspectorTab(tab),
    onCssCodeTargetChange: (target) => sourceNavigation.selectCssFocus(target),
    getOpenCssRuleContext: (file, selector, viewport) => (
      sourceWorkspace.cssRuleContext(file, selector, viewport)
    ),
    applyNativeBlockOption: (request) => htmlEditing.applyNativeBlockOption(request),
    applyNativeIcon: (request) => htmlEditing.applyNativeIcon(request),
    applyNativeBlockSlotMutation: (request) => editorNavigation.applyNativeBlockSlotMutation(request),
    updateDynamicWidget: (snapshot, properties) => dynamicWidgets.update(snapshot, properties),
    deleteDynamicWidget: (snapshot) => dynamicWidgets.delete(snapshot),
  }));
  const editorShellProps: ForwardedEditorShellProps = {
    get centerView() { return shell.centerView; },
    get previewZoom() { return workbench.previewZoom; },
    get previewCanvasMode() { return workbench.canvasMode; },
    get previewCanvasPreset() { return workbench.canvasPreset; },
    get previewWidthPx() { return workbench.previewWidthPx; },
    get previewRulers() { return workbench.rulers; },
    get previewDocumentMarkup() { return previewWorkspace.documentMarkup; },
    get previewSrc() { return previewWorkspace.src; },
    get previewNavigationGuardActive() { return previewWorkspace.navigationGuardActive; },
    get interactivePreviewEnabled() {
      return previewWorkspace.interactiveEnabled
        && !aiCoordination.frontendLockActive
        && !historyOperation.quiesceActive
        && !historyOperation.leaseActive;
    },
    get interactivePreviewUrl() { return previewWorkspace.interactiveUrl; },
    get workbenchSnapshot() { return workbench.snapshot; },
    setWorkbenchSplit: async (split) => { await workbench.setSplit(split); },
    setWorkbenchSplitRatio: async (ratioBasisPoints) => { await workbench.setSplitRatio(ratioBasisPoints); },
    setCanvasViewport: async (viewport) => { await workbench.setCanvasViewport(viewport); },
    setPreviewZoom: (value) => workbench.setPreviewZoom(value),
    commitPreviewZoom: async (value) => {
      workbench.setPreviewZoom(value);
      await workbench.setCanvasViewport({ zoomPercent: workbench.previewZoom });
    },
    resetPreviewZoom: () => {
      workbench.resetPreviewZoom();
      void workbench.setCanvasViewport({ zoomPercent: workbench.previewZoom });
    },
    attachPreviewInspector: () => previewRuntime.attachInspector(),
    mountPreviewSurface: (frame) => previewWorkspace.mountAndTrackSurface(frame),
    unmountPreviewSurface: (frame) => previewWorkspace.unmountAndTrackSurface(frame),
    previewSurfaceLoaded: (frame) => previewWorkspace.onSurfaceLoaded(frame),
    setPreviewExecutionMode: (mode) => previewWorkspace.setExecutionMode(mode),
    onInteractiveLifecycleError: (message) => globalStatus.set(
      t("workbench-interactive-error", { detail: message }),
      "error",
    ),
    onInteractiveDomSnapshot: (nodes) => previewWorkspace.acceptInteractiveDomSnapshot(nodes),
    onInteractiveRealmRestarted: (previewRevision, durationMs) => {
      void previewWorkspace.recordInteractiveRealmEvent(
        "interactive_js_restarted",
        previewRevision,
        durationMs,
      );
    },
    onInteractiveRealmFailed: (previewRevision, durationMs, diagnostic) => {
      void previewWorkspace.recordInteractiveRealmEvent(
        "interactive_js_failed",
        previewRevision,
        durationMs,
        diagnostic,
      );
    },
    get currentSourcePath() { return sourceWorkspace.currentSourcePath; },
    get source() { return sourceWorkspace.source; },
    get sourceLanguage() { return sourceWorkspace.sourceLanguage; },
  };
  const lifecycleBlocksEditing = $derived(
    transitionLease.isActive
      || (
        editorSidebarsAvailable
        &&
        activeLifecycleReadiness !== null
        && activeLifecycleReadiness.state !== "ready"
        && activeLifecycleReadiness.state !== "degraded"
      ),
  );
  const lifecycleStatus = $derived.by(() => {
    switch (activeLifecycleReadiness?.state) {
      case "initializing_frontend":
        return t("workbench-project-hydrating");
      case "preparing_preview":
        return t("workbench-project-preparing-preview");
      case "awaiting_canvas":
        return t("workbench-project-awaiting-canvas");
      case "finalizing_frontend":
        return t("workbench-project-finalizing-frontend");
      default:
        return t("workbench-project-initializing");
    }
  });
  function breakpointValue(name: string, fallback: string) {
    return analysis.scssVariables.find((variable) => variable.name === name)?.value || fallback;
  }
  async function openApplicationSettings(section: import("$lib/application/shell-state.svelte").ApplicationSettingsSection = "general") {
    if (projectSession.project) await flushWorkspaceMutationInputs("template-switch");
    shell.openSettings(section);
  }
  function handleAppShortcuts(event: KeyboardEvent) {
    commandCenter.handleShortcut(event, editorSidebarsAvailable);
  }
  async function selectWorkbenchActivity(activity: import("$lib/workbench/contracts").WorkbenchActivity) {
    try {
      await workbench.setActivity(activity);
      shell.openWorkbench();
      globalStatus.clear("workbench.activity");
    } catch (error) {
      globalStatus.escalate({
        id: "workbench.activity",
        level: "warning",
        title: t("workbench-activity-open-failed"),
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }
  async function openAuditWorkspace(view: "overview" | "runtime", focusObservability = false) {
    projectAudit.open(view, focusObservability);
    if (terminalWorkspace.terminalPaneOpen) {
      await workbench.setBottomPanel(false, "terminal");
    }
    await workbench.setActivity("audit");
  }
  function handleWindowMessage(event: MessageEvent) {
    const data = event.data;
    const userIntentLocked = aiCoordination.frontendLockActive
      || externalDisk.snapshot.reconciling
      || externalDisk.snapshot.workspaceProjectionRecoveryRequired;
    if (userIntentLocked && !isPreviewControlPlaneMessage(data)) return;
    if (
      data?.source === "pana-studio-preview"
      && data.type === "preview-shortcut"
      && isMessageFromExactPreviewFrame(previewSurface.frame, event)
    ) {
      if (!previewWorkspace.runtime.acceptIncomingMessage()) return;
      if (data.shortcut === "save") void saveWorkspace.saveActiveFile();
      else if (data.shortcut === "undo") void workspaceHistory.run("undo");
      else if (data.shortcut === "redo") void workspaceHistory.run("redo");
      return;
    }
    previewRuntime.handleMessage(event);
  }
  function handleDeleteShortcut(event: KeyboardEvent) {
    if (aiCoordination.frontendLockActive || externalDisk.snapshot.reconciling || externalDisk.snapshot.workspaceProjectionRecoveryRequired) {
      event.preventDefault();
      return;
    }
    const intent = deleteShortcutIntent(event, {
      activeWorkbenchActivity: workbench.snapshot?.activeActivity ?? "editor",
      applicationSurface: shell.surface,
      centerView: shell.centerView,
      selectionSnapshot: selectionWorkspace.session.selectionSnapshot,
    });
    if (intent === "none") return;
    event.preventDefault();
    if (intent === "deleteSelectedTera") {
      void teraEditing.delete();
      return;
    }
    void htmlEditing.deleteSelected();
  }
  async function recoverExternalProjectionFromDisk() {
    if (externalRecoveryInFlight) return;
    externalRecoveryInFlight = true;
    try {
      await reloadAuthorizedAiReconciliationFromDisk(aiCoordination.controllerHost());
    } finally {
      externalRecoveryInFlight = false;
    }
  }
  async function ensureTerminalPaneLoaded() {
    if (TerminalPaneComponent || terminalPaneLoading) return;
    terminalPaneLoading = true;
    try {
      TerminalPaneComponent = (await import("$lib/components/TerminalPane.svelte")).default;
    } finally {
      terminalPaneLoading = false;
    }
  }
  function workspaceSurfacesLoaded() {
    return Boolean(
      ActivityRail
      && WorkspaceCenterArea
      && WorkspaceInspectorArea
      && WorkspaceProjectArea,
    );
  }
  async function ensureWorkspaceSurfacesLoaded() {
    if (workspaceSurfacesLoaded()) return;
    workspaceSurfaceError = "";
    try {
      const surfaces = await loadWorkspaceSurfaces();
      ActivityRail = surfaces.activityRail;
      WorkspaceCenterArea = surfaces.centerArea;
      WorkspaceInspectorArea = surfaces.inspectorArea;
      WorkspaceProjectArea = surfaces.projectArea;
    } catch (error) {
      workspaceSurfaceError = errorMessage(error);
      globalStatus.escalate({
        id: "workbench.shell.lazy-load",
        level: "error",
        title: t("workbench-activity-open-failed"),
        message: workspaceSurfaceError,
      });
    }
  }
  async function openWorkspaceSource(
    path: string,
    options: WorkspaceSourceOpenOptions = {},
  ) {
    const candidatePaths = [path];
    const file = projectSession.project?.files.find((item) => candidatePaths.includes(item.relativePath));
    if (!file) {
      globalStatus.set(t("workbench-file-not-scanned", { path }), "error");
      return;
    }
    await projectDocuments.load(file, {
      preferredTemplatePagePath: options.templateContextPagePath,
      preferredTemplateRoute: options.templateContextUrl,
      preferredComponentName: options.componentName,
    });
    await workbenchNavigation.setCenterView(options.surface === "visual" ? "preview" : "code");
  }
  $effect(() => {
    workspaceHistory.synchronize();
  });
  onMount(() => {
    pageLifecycle.start();
    return () => { pageLifecycle.stop(); };
  });
  $effect(() => {
    if (terminalWorkspace.terminalPaneOpen) void ensureTerminalPaneLoaded();
  });

  $effect(() => {
    if (projectSession.project || shell.surface === "settings") {
      void ensureWorkspaceSurfacesLoaded();
    }
  });
</script>

<svelte:head>
  <title>Pană Studio</title>
</svelte:head>

<main
  class:dark-theme={applicationPreferences.theme === "dark"}
  class:light-theme={applicationPreferences.theme === "light"}
  class:external-reconcile-lock={externalDisk.snapshot.reconciling || externalDisk.snapshot.workspaceProjectionRecoveryRequired}
  class:startup-active={!projectSession.project && shell.surface !== "settings"}
  class:lifecycle-lock={lifecycleBlocksEditing}
  class="app-shell"
  inert={externalDisk.snapshot.reconciling || externalDisk.snapshot.workspaceProjectionRecoveryRequired || lifecycleBlocksEditing}
  aria-busy={externalDisk.snapshot.reconciling || externalDisk.snapshot.workspaceProjectionRecoveryRequired || lifecycleBlocksEditing}
>
  {#if (projectSession.lifecycle.activeSession && projectSession.project) || shell.surface === "settings"}
    <AppChrome
      project={{
        root: projectSession.root,
        sessionId: projectSession.runtimeSessionId,
        present: Boolean(projectSession.project),
        savePending: readModel.saveHasPending,
      }}
      surface={{
        application: shell.surface,
        activeActivity: workbench.snapshot?.activeActivity ?? "editor",
        sourceStatus: selectionWorkspace.workbenchSourceStatus,
      }}
      commands={{
        flushDrafts: (reason: EditFlushReason) => flushWorkspaceMutationInputs(reason),
        openProjectFolder: () => startup.openFolder(),
        openProjectInBrowser: () => browserPreview.open(),
        save: () => saveWorkspace.saveActiveFile(),
        openCssSource: (target: { selector: string; file: string }) => sourceNavigation.openCssSource(target),
        openSourceLocation: (location: string) => htmlEditing.openSource(location),
        setCenterView: (view: CenterView) => workbenchNavigation.setCenterView(view),
        requestCodeSelectionReveal: () => sourceWorkspace.requestSelectionReveal(),
        handleNotificationAction: (notification: AppNotification, actionId: string) => (
          appSession.handleNotification(notification, actionId)
        ),
      }}
      {applicationPreferences}
      {notificationCenter}
      {globalStatus}
      {terminalWorkspace}
      {workspaceLayout}
      topbarCanUndo={workspaceHistory.state.canUndo}
      topbarCanRedo={workspaceHistory.state.canRedo}
      undoAction={() => workspaceHistory.run("undo")}
      redoAction={() => workspaceHistory.run("redo")}
      commandCenterOpen={commandCenter.open}
      openCommandCenter={() => commandCenter.show()}
      closeCommandCenter={() => commandCenter.close()}
      executeCommandCenterAction={(action) => commandCenter.execute(action)}
    >

      {#if ActivityRail && WorkspaceCenterArea && WorkspaceInspectorArea && WorkspaceProjectArea}
        <div class="workbench-frame">
        <ActivityRail
          activeActivity={workbench.snapshot?.activeActivity ?? "editor"}
          disabled={!projectSession.project}
          applicationSettingsActive={shell.surface === "settings"}
          selectActivity={selectWorkbenchActivity}
        />
        <section
          class:left-pane-collapsed={workspaceLayout.leftPaneCollapsed}
          class:right-pane-collapsed={workspaceLayout.rightPaneCollapsed}
          class="workspace"
          style={`--left-pane-width: ${workspaceLayout.leftPaneWidth}px; --right-pane-width: ${workspaceLayout.rightPaneWidth}px;`}
          aria-label={t("workbench-aria-label")}
        >
          <WorkspaceProjectArea
            visible={shell.surface === "workbench"
              && (workbench.snapshot?.activeActivity ?? "editor") === "editor"}
            sessionId={projectSession.runtimeSessionId}
            interactionLocked={historyOperation.quiesceActive
              || historyOperation.leaseActive}
            pane={projectAreaPane}
            commands={projectAreaCommands}
            {workspaceLayout}
          />

          <WorkspaceCenterArea
            session={centerAreaSession}
            creation={centerAreaCreation}
            auxiliary={centerAreaAuxiliary}
            workspaceCommands={centerAreaWorkspaceCommands}
            creationCommands={centerAreaCreationCommands}
            contentCommands={centerAreaContentCommands}
            auditCommands={centerAreaAuditCommands}
            editorProps={editorShellProps}
            documentActivation={workbench.documentActivation}
            bind:previewFrame={previewSurface.frame}
            bind:codeEditorHost={sourceWorkspace.hostElement}
            {applicationPreferences}
            {globalStatus}
            {motionWorkspace}
            {terminalWorkspace}
            {workspaceMutations}
            {publishWorkspace}
            {workspaceLayout}
            {workbenchDocuments}
            {TerminalPaneComponent}
            {breakpointValue}
            {openWorkspaceSource}
          />

          <WorkspaceInspectorArea
            visible={shell.surface === "workbench"
              && (workbench.snapshot?.activeActivity ?? "editor") === "editor"}
            sessionId={projectSession.runtimeSessionId}
            interactionLocked={aiCoordination.frontendLockActive
              || historyOperation.quiesceActive
              || historyOperation.leaseActive}
            paneProps={inspectorPaneProps}
            {applicationPreferences}
            {motionWorkspace}
            {workspaceMutations}
            {workspaceLayout}
          />
        </section>
        </div>
      {:else}
        <div class="workspace-lazy-loading" role="status" aria-live="polite">
          {workspaceSurfaceError || t("common-loading")}
        </div>
      {/if}
    </AppChrome>
  {:else}
    <StartupView
      startupFlow={startup.flow}
      startupError={startup.error}
      startupPending={startup.pending}
      startupCreationPlan={startup.creationPlan}
      startupCreationCatalog={startup.creationCatalog}
      startupSelectedOptionId={startup.selectedOptionId}
      {globalStatus}
      openApplicationSettings={() => openApplicationSettings()}
      cancelStartupCreationPlan={() => startup.cancelCreationPlan()}
      applyStartupProject={() => startup.applyProject()}
      selectStartupCreationOption={(optionId) => startup.selectCreationOption(optionId)}
      openProjectFolder={() => startup.openFolder()}
      planStartupProject={() => startup.planProject()}
      retryStartupProjectOpen={() => startup.retryOpen()}
    />
  {/if}

  <ProjectTransitionDecisionDialog
    request={startup.transitionDecision}
    confirm={(requestId: string, diagnostic: string) => projectTransitions.confirmOperatorDecision(requestId, diagnostic)}
    cancel={(requestId: string) => projectTransitions.cancelOperatorDecision(requestId)}
  />

  <ProjectOpenRecoveryDialog
    request={startup.openRecoveryDecision}
    abandon={(requestId: string) => projectTransitions.confirmOpenRecoveryAbandonment(requestId)}
    cancel={(requestId: string) => projectTransitions.cancelOpenRecovery(requestId)}
  />
</main>

{#if lifecycleBlocksEditing}
  <div class="project-lifecycle-overlay" role="status" aria-live="polite">
    <span class="project-lifecycle-spinner" aria-hidden="true"></span>
    <span>{lifecycleStatus}</span>
  </div>
{/if}

{#if externalDisk.snapshot.workspaceProjectionRecoveryRequired}
  <dialog open class="external-reconcile-recovery" aria-labelledby="external-reconcile-recovery-title">
    <strong id="external-reconcile-recovery-title">{t("workbench-external-recovery-title")}</strong>
    <p>{t("workbench-external-recovery-description")}</p>
    <button type="button" disabled={externalRecoveryInFlight} onclick={recoverExternalProjectionFromDisk}>
      {externalRecoveryInFlight ? t("workbench-external-recovery-loading") : t("workbench-external-recovery-action")}
    </button>
  </dialog>
{/if}
