  import { tick } from "svelte";
  import { contextMenu } from "$lib/context-menu/store.svelte";
  import { dispatchExternalReconcileInteractionBarrier } from "$lib/session/external-reconcile-barrier";
  import { WorkspaceAuthorityService } from "$lib/session/workspace-authority-service";
  import { ExternalDiskState } from "$lib/session/external-disk-state.svelte";
  import { reconcilePageAssetContracts } from "$lib/page-assets/contract";
  import { flushWorkspaceMutationInputs } from "$lib/session/workspace-mutation-coordinator";
  import { AppSessionService } from "$lib/state/app-session-controller";
  import { ApplicationPreferencesState } from "$lib/application/preferences.svelte";
  import { PublishWorkspaceState } from "$lib/deploy/publish-state.svelte";
  import { ProjectAuditWorkspaceState } from "$lib/audit/workspace-state.svelte";
  import { AiContextState } from "$lib/ai/context-state.svelte";
  import { AiCoordinationState } from "$lib/ai/coordination-state.svelte";
  import { CanvasInteractionWorkspace } from "$lib/canvas/interaction-workspace";
  import { MotionWorkspaceState } from "$lib/motion/workspace.svelte";
  import { GlobalStatusState } from "$lib/status/state.svelte";
  import { TerminalWorkspaceState } from "$lib/terminal/workspace.svelte";
  import { FileExplorerWorkspaceState } from "$lib/workbench/file-explorer-state.svelte";
  import { WorkbenchWorkspaceState } from "$lib/workbench/workspace-state.svelte";
  import { WorkbenchNavigationService } from "$lib/workbench/navigation-service";
  import { SourceWorkspaceState } from "$lib/editor/source-workspace.svelte";
  import { SourceNavigationService } from "$lib/editor/source-navigation-service";
  import { HtmlEditingService } from "$lib/editor/html-editing-service";
  import { DynamicWidgetService } from "$lib/editor/dynamic-widget-service";
  import { InsertCatalogDragService } from "$lib/creation/insert-catalog-drag-service";
  import { EditorNavigationService } from "$lib/editor/navigation-service";
  import { EditorSelectionService } from "$lib/editor/selection-service";
  import { TeraEditingService } from "$lib/editor/tera-editing-service";
  import { EditorInteractionRuntime } from "$lib/editor/interaction-runtime.svelte";
  import { resetEditorAfterExternalReconcile } from "$lib/editor/external-reconcile-reset";
  import { HtmlAuthoringState } from "$lib/editor/html-authoring-state.svelte";
  import { SelectionWorkspaceState } from "$lib/editor/selection-workspace.svelte";
  import { PageSectionsState } from "$lib/preview/page-sections.svelte";
  import { PreviewSurfaceState } from "$lib/preview/surface-state.svelte";
  import { PreviewWorkspaceState } from "$lib/preview/workspace-state.svelte";
  import { PreviewInsertService } from "$lib/preview/insert-service";
  import { PreviewRuntimeService } from "$lib/preview/runtime-service";
  import { DesignClassInventoryState } from "$lib/css/class-inventory-state.svelte";
  import { CssAuthoringState } from "$lib/css/authoring-state.svelte";
  import { CssWorkspaceService } from "$lib/css/workspace-service";
  import { ControlledPreviewWorkspaceState } from "$lib/preview/controlled-state.svelte";
  import { ApplicationShellState } from "$lib/application/shell-state.svelte";
  import { ProjectSessionState } from "$lib/project/session-state.svelte";
  import { ProjectSaveService } from "$lib/project/save-service";
  import { ProjectDocumentService } from "$lib/project/document-service";
  import { ProjectDerivedStateService } from "$lib/project/derived-state-service";
  import { ProjectResetService } from "$lib/project/reset-service";
  import { TemplateWorkbenchService } from "$lib/project/template-workbench-service";
  import { ProjectAttachmentService } from "$lib/project/attachment-service";
  import { ProjectPreviewBootstrapService } from "$lib/project/preview-bootstrap-service";
  import { ProjectTransitionService } from "$lib/project/transition-service";
  import { PageSettingsService } from "$lib/markdown/page-settings-service";
  import { ProjectDocumentWorkspaceState } from "$lib/project/document-workspace.svelte";
  import { ProjectAnalysisState } from "$lib/project/analysis-state.svelte";
  import { ProjectStartupState } from "$lib/project/startup-state.svelte";
  import { ProjectTransitionLeaseState } from "$lib/project/transition-lease-state.svelte";
  import { reportProjectCapabilityDegraded } from "$lib/project/io/lifecycle";
  import { HistoryOperationState } from "$lib/versioning/history-operation-state.svelte";
  import { VersionPreviewState } from "$lib/versioning/preview-state.svelte";
  import { VersionPreviewService } from "$lib/versioning/preview-service";
  import { AcceptedDiskState } from "$lib/session/accepted-disk-state.svelte";
  import { EditorReadModelState } from "$lib/editor/read-model.svelte";
  import { ProjectBrowserPreviewService } from "$lib/state/project-browser-preview-controller";
  import {
    beginPreviewRefreshLease,
    invalidatePreviewDomTreeProjection,
    invalidatePreviewRefreshLease,
    previewRefreshLeaseMatches,
  } from "$lib/state/preview-controller";

export type ApplicationCompositionGlobals = Readonly<{
  motionWorkspace: MotionWorkspaceState;
  globalStatus: GlobalStatusState;
  applicationPreferences: ApplicationPreferencesState;
}>;

export function createApplicationComposition({
  motionWorkspace,
  globalStatus,
  applicationPreferences,
}: ApplicationCompositionGlobals) {
    let publish!: PublishWorkspaceState;
    let browserPreview!: ProjectBrowserPreviewService;
    let saveWorkspace!: ProjectSaveService;
    let aiCoordination!: AiCoordinationState;
    let readModel!: EditorReadModelState;
    let externalDisk!: ExternalDiskState;
    let transitionLease!: ProjectTransitionLeaseState;
    let sourceWorkspace!: SourceWorkspaceState;
    let editorInteraction!: EditorInteractionRuntime;
    let htmlEditing!: HtmlEditingService;
    let editorNavigation!: EditorNavigationService;
    let editorSelection!: EditorSelectionService;
    let teraEditing!: TeraEditingService;
    let workspaceAuthority!: WorkspaceAuthorityService;
    let previewRuntime!: PreviewRuntimeService;
    let projectDocuments!: ProjectDocumentService;
    let projectDerived!: ProjectDerivedStateService;
    let projectTransitions!: ProjectTransitionService;
    const shell = new ApplicationShellState();
    const projectSession = new ProjectSessionState();
    const startup = new ProjectStartupState({
      openProjectRoot: (root, options) => projectTransitions.open(root, options),
      escalateStatus: (notification) => globalStatus.escalate(notification),
      clearStatus: (id) => globalStatus.clear(id),
    });
    externalDisk = new ExternalDiskState(() => ({
      session: {
        get runtimeSessionId() { return projectSession.runtimeSessionId; },
        get epoch() { return projectSession.epoch; },
        get project() { return projectSession.project; },
        get transitionLocked() { return transitionLease.isActive; },
        get historyLocked() { return historyOperation.leaseActive; },
        get aiLocked() { return aiCoordination.frontendLockActive; },
      },
      editor: {
        get activeScannedPath() { return documents.activeScannedPath; },
        get sourceCache() { return sourceWorkspace.sourceCache; },
        get mutationEpoch() { return projectSession.editorMutationEpoch; },
        get selectionEpoch() { return selectionWorkspace.selectionEpoch; },
        get dirty() { return readModel.globalDirtyState.dirty; },
      },
      projections: {
        invalidateProjectSession: () => projectSession.invalidateLeases(),
        acceptProject: (project) => { projectSession.project = project; },
        acceptWorkspace: (workspace) => { projectSession.workspace = workspace; },
        setProjectStatus: (status) => { projectSession.status = status; },
        acceptSources: (sourceCache, activeSource) => {
          sourceWorkspace.sourceCache = sourceCache;
          if (activeSource !== null) sourceWorkspace.source = activeSource;
        },
        acceptScssVariables: (variables) => { analysis.scssVariables = variables; },
        invalidateDerived: () => { projectSession.refreshToken += 1; },
        invalidatePageJs: () => { projectSession.jsRefreshToken += 1; },
      },
      commands: {
        setStatus: (text, kind) => globalStatus.set(text, kind),
        escalateStatus: (notification) => globalStatus.escalate(notification),
        clearStatus: (id) => globalStatus.clear(id),
        refreshSourceGraph: async (options) => { await previewRuntime.refreshSourceGraph(options); },
        quiesceInteractions: () => dispatchExternalReconcileInteractionBarrier(),
        waitForInteractionLock: () => tick(),
        resetHistory: () => resetEditorAfterExternalReconcile({
          editor: editorInteraction,
          html: htmlAuthoring,
          css: cssAuthoring,
          selection: selectionWorkspace,
          preview: previewWorkspace,
        }),
        projectLatestPreview: (options) => (
          workspaceAuthority.projectLatest(options)
        ),
      },
    }));
    transitionLease = new ProjectTransitionLeaseState({
      guards: () => ({
        aiEditLocked: aiCoordination.frontendLockActive,
        aiRecoveryReloadAuthorized: aiCoordination.recoveryReloadAuthorized,
        historyLocked: historyOperation.leaseActive,
      }),
      cancelEditorDrafts: () => editorInteraction.htmlDraft.cancel(),
      invalidatePreview: () => {
        const commands = previewWorkspace.commands();
        invalidatePreviewRefreshLease(commands.session);
        invalidatePreviewDomTreeProjection(commands);
      },
      invalidateSourceGraph: () => { analysis.sourceGraphLoadSerial += 1; },
      quiesceInteractions: () => dispatchExternalReconcileInteractionBarrier(),
      drainActiveSave: () => saveWorkspace.drain(),
      suspendExternalDisk: () => externalDisk.suspendAndDrain(),
      recoverExternalDiskAfterFailure: () => {
        if (
          externalDisk.snapshot.reconciling
          && !externalDisk.snapshot.workspaceProjectionRecoveryRequired
        ) externalDisk.rollbackFailedProjectTransition();
      },
      resumeExternalDisk: () => externalDisk.resumeAfterTransition(),
    });
    const historyOperation = new HistoryOperationState();
    const versionPreview = new VersionPreviewState();
    const acceptedDisk = new AcceptedDiskState();
    const projectAudit = new ProjectAuditWorkspaceState({
      authority: () => ({
        projectRoot: projectSession.root,
        runtimeSessionId: projectSession.runtimeSessionId,
        workspace: projectSession.workspace,
        activeRelativePath: documents.activeScannedPath,
      }),
      runStructural: (operation) => workspaceAuthority.runStructural(operation),
      requireStructuralLease: (lease) => workspaceAuthority.requireLease(lease),
      settleMutation: (receipt, options) => workspaceAuthority.settle(receipt, options),
      invalidatePublish: () => publish.invalidate(),
      setStatus: (text, kind) => globalStatus.set(text, kind),
    });
    let workbench!: WorkbenchWorkspaceState;
    const fileExplorer = new FileExplorerWorkspaceState({
      authority: () => ({
        projectRoot: projectSession.root,
        runtimeSessionId: projectSession.runtimeSessionId,
        workspace: projectSession.workspace,
        workbenchRevision: workbench.snapshot?.revision ?? null,
        activeRelativePath: documents.activeScannedPath,
      }),
      refreshWorkbench: () => workbench.refresh(),
      acceptWorkbench: (snapshot) => { workbench.snapshot = snapshot; },
      loadProjectFile: (file, options) => projectDocuments.load(file, options),
      settleMutation: (mutation, options) => workspaceAuthority.settle(mutation, options),
      setStatus: (text, kind) => globalStatus.set(text, kind),
    });
    publish = new PublishWorkspaceState({
      authority: () => ({
        projectRoot: projectSession.root,
        runtimeSessionId: projectSession.runtimeSessionId,
        workspace: projectSession.workspace,
        workspaceDirty: readModel.globalDirtyState.dirty,
      }),
      acceptAudit: (receipt, clearError) => projectAudit.accept(receipt, clearError),
    });
    const terminal = new TerminalWorkspaceState({
      setPaneOpen: (open) => workbench.setBottomPanel(open, "terminal"),
      currentProjectPath: () => documents.currentProjectPath,
      runZolaValidation: (reason) => controlledPreview.runValidation(reason),
      openCurrentProjectInBrowser: () => browserPreview.open(),
      setGlobalStatus: (text, kind) => globalStatus.set(text, kind),
    });
    workbench = new WorkbenchWorkspaceState({
      authority: () => ({
        projectRoot: projectSession.root,
        runtimeSessionId: projectSession.runtimeSessionId,
        project: projectSession.project,
        activeRelativePath: documents.activeScannedPath,
        centerView: shell.centerView,
        canvasSurfaceResumeRequired: previewSurface.resumeRequired,
        canvasSurfaceMounted: previewWorkspace.hasMountedSurface(),
      }),
      flushDrafts: (reason) => flushWorkspaceMutationInputs(reason),
      loadProjectFile: (file, options) => projectDocuments.load(file, options),
      setCenterView: (view) => { shell.centerView = view; },
      synchronizeTerminalPane: (open) => terminal.synchronizePaneOpen(open),
      clearStatus: (id) => globalStatus.clear(id),
      escalateStatus: (request) => globalStatus.escalate(request),
    });
    sourceWorkspace = new SourceWorkspaceState({
      context: () => ({
        activeScannedPath: documents.activeScannedPath,
        activePreviewPath: documents.activePreviewPath,
        projectTransitionLocked: transitionLease.isActive,
        historyLocked: historyOperation.quiesceActive || historyOperation.leaseActive,
        aiLocked: aiCoordination.frontendLockActive,
        selection: selectionWorkspace.session,
        saveHasPending: readModel.saveHasPending,
      }),
      setStatus: (text, kind) => globalStatus.set(text, kind),
      syncHtmlCodeToPreview: (sourceText, cursorPosition) => (
        previewRuntime.syncHtmlCode(sourceText, cursorPosition)
      ),
      selectSourcePosition: (file, offset) => editorSelection.selectSourcePosition(file, offset),
      getPreviewDocument: () => previewWorkspace.getDocument(),
      postPreviewMessage: (payload) => previewWorkspace.postMessage(payload),
      selectPreviewElement: (element, options) => editorSelection.selectPreviewElement(element, options),
      save: () => saveWorkspace.saveActiveFile(),
    }, applicationPreferences);
    const htmlAuthoring = new HtmlAuthoringState(() => { projectSession.editorMutationEpoch += 1; });
    const cssAuthoring = new CssAuthoringState();
    const analysis = new ProjectAnalysisState();
    const documents = new ProjectDocumentWorkspaceState({
      session: projectSession,
      sourceGraph: () => analysis.sourceGraph,
    });
    const selectionWorkspace: SelectionWorkspaceState = new SelectionWorkspaceState({
      context: () => ({
        activeCanvasIdentity: previewWorkspace.activeIdentity,
        activeCanvasUrl: previewWorkspace.activeUrl,
        activeScannedPath: documents.activeScannedPath,
        browserPreviewRoute: documents.browserPreviewRoute,
        previewSrc: previewWorkspace.src,
        workspace: projectSession.workspace,
        targetCssFile: cssAuthoring.targetFile,
        sourceGraph: analysis.sourceGraph,
      }),
      applySelectionState: (observation) => editorSelection.apply(observation),
      projectSelectionOnCanvas: (selection) => editorSelection.projectOnCanvas(selection),
      resolveSourceEditTarget: (sourceId) => previewRuntime.resolveSourceEditTarget(sourceId),
    });
    const aiContext = new AiContextState(() => ({
      project: projectSession.project,
      workspace: projectSession.workspace,
      activeScannedPath: documents.activeScannedPath,
      activePreviewPath: documents.activePreviewPath,
      centerView: shell.centerView,
      previewDevice: workbench.previewDevice,
      sourceLanguage: sourceWorkspace.sourceLanguage,
      coordinatedElementSelection: selectionWorkspace.coordinatedElement,
      editorSelection: selectionWorkspace.session,
      activeCssSelector: selectionWorkspace.activeCssSelector,
      targetCssFile: cssAuthoring.targetFile,
      scssVariables: analysis.scssVariables,
      dirtyState: readModel.globalDirtyState,
      externalDisk: externalDisk.snapshot,
    }));
    aiCoordination = new AiCoordinationState({
      context: aiContext,
      activeScannedPath: () => documents.activeScannedPath,
      workspace: () => projectSession.workspace,
      externalDisk,
      quiesceInteractions: () => dispatchExternalReconcileInteractionBarrier(),
      discardAndReload: (preferredPath) => projectTransitions.discardAndReload(preferredPath),
      setStatus: (text, kind, options) => globalStatus.set(text, kind, options),
      escalateStatus: (notification) => globalStatus.escalate(notification),
      clearStatus: (id) => globalStatus.clear(id),
    });
    const pageSections = new PageSectionsState(() => analysis.sourceGraph);
    const previewSurface = new PreviewSurfaceState();
    let controlledPreview!: ControlledPreviewWorkspaceState;
    const previewWorkspace: PreviewWorkspaceState = new PreviewWorkspaceState({
      session: projectSession,
      surface: previewSurface,
      css: cssAuthoring,
      sections: pageSections,
      selection: selectionWorkspace.session,
      controlled: () => controlledPreview,
      motion: motionWorkspace,
      context: () => ({
        activePage: documents.activeRenderedPreviewPageFile,
        isActivePage: documents.isActiveRenderedPreviewPage,
        templateWorkbenchActive: documents.templateActive,
        project: projectSession.project,
        activeScannedPath: documents.activeScannedPath,
        activeVersionPreview: versionPreview.active,
      }),
      setStatus: (text, kind) => globalStatus.set(text, kind),
      clearStatus: (id) => globalStatus.clear(id),
      reportCanvasDegraded: async (projectRoot, runtimeSessionId, diagnostic) => {
        const lifecycle = await reportProjectCapabilityDegraded(
          projectRoot,
          runtimeSessionId,
          "canvas",
          diagnostic,
        );
        if (
          projectSession.root === projectRoot
          && projectSession.runtimeSessionId === runtimeSessionId
        ) projectSession.lifecycle = lifecycle;
      },
      projectLatest: (options) => workspaceAuthority.projectLatest(options),
      loadProjectFile: (file, options) => projectDocuments.load(file, options),
      invalidateSourceGraph: () => { analysis.sourceGraphLoadSerial += 1; },
    });
    const designClassInventory = new DesignClassInventoryState(() => ({
      projectRoot: projectSession.root,
      runtimeSessionId: projectSession.runtimeSessionId,
      workspace: projectSession.workspace,
    }));
    controlledPreview = new ControlledPreviewWorkspaceState({
      context: () => ({
        projectPresent: Boolean(projectSession.project),
        projectStatus: projectSession.status,
      }),
      beginRefreshLease: () => beginPreviewRefreshLease(previewWorkspace.commands().session),
      refreshLeaseCurrent: (lease) => previewRefreshLeaseMatches(
        previewWorkspace.commands().session,
        lease,
      ),
      reloadPreview: (lease) => previewWorkspace.reload(lease),
      setProjectStatus: (status) => { projectSession.status = status; },
        setGlobalStatus: (text, kind, options) => globalStatus.set(text, kind, options),
    });
    readModel = new EditorReadModelState({
      project: projectSession,
      documents,
      source: sourceWorkspace,
      html: htmlAuthoring,
      selection: selectionWorkspace,
      ai: aiCoordination,
      externalDisk,
    });
    const templateWorkbench = new TemplateWorkbenchService({
      project: projectSession,
      documents,
      preview: previewWorkspace,
      selection: selectionWorkspace,
      status: globalStatus,
    });
    workspaceAuthority = new WorkspaceAuthorityService({
      session: {
        project: projectSession,
        documents,
        source: sourceWorkspace,
        analysis,
      },
      preview: { surface: previewSurface, workspace: previewWorkspace },
      selection: selectionWorkspace,
      locks: {
        transition: transitionLease,
        history: historyOperation,
        ai: aiCoordination,
      },
      disk: externalDisk,
      status: globalStatus,
      reconcileDerived: (options) => projectDerived.reconcile(options),
      reprojectTemplate: (minimumRevision) => templateWorkbench.reproject(minimumRevision),
    });
    projectDocuments = new ProjectDocumentService({
      project: projectSession,
      documents,
      source: sourceWorkspace,
      preview: previewWorkspace,
      shell,
      template: templateWorkbench,
      authority: workspaceAuthority,
      workbench,
      selection: selectionWorkspace,
      status: globalStatus,
    });
    const canvasInteraction = new CanvasInteractionWorkspace({
      preview: previewWorkspace,
      surface: previewSurface,
      documents,
      shell,
      project: projectSession,
      workbench,
      selection: selectionWorkspace,
      analysis,
      editorRuntime: () => editorInteraction.commands,
      commands: {
        closeContextMenu: () => contextMenu.close(),
        moveEditorNavigationNode: (sourceNodeId, targetNodeId, position, preplanned, inputEmittedAtMs) => (
          editorNavigation.move(
            sourceNodeId,
            targetNodeId,
            position,
            preplanned,
            inputEmittedAtMs,
          )
        ),
        postPreviewMessage: (payload) => previewWorkspace.postMessage(payload),
        previewEditorNavigationMove: (sourceNodeId, targetNodeId, position) => (
          editorNavigation.previewMove(sourceNodeId, targetNodeId, position)
        ),
        recordCanvasProjectionRuntimeEvent: (kind, identity, durationMs, diagnostic, metrics) => (
          previewWorkspace.recordRuntimeEvent(kind, identity, durationMs, diagnostic, metrics)
        ),
        setGlobalStatus: (text, kind) => globalStatus.set(text, kind),
        syncCodeSelectionHighlight: (reveal) => sourceWorkspace.syncSelectionHighlight(reveal),
      },
    });
    editorNavigation = new EditorNavigationService({
      project: projectSession,
      selection: selectionWorkspace,
      analysis,
      canvas: canvasInteraction,
      html: () => htmlEditing,
      editor: () => editorInteraction,
      status: globalStatus,
      setPreviewTeraSelection: (target, options) => (
        editorSelection.setTera(target, options)
      ),
      flushDrafts: (reason) => flushWorkspaceMutationInputs(reason),
      projectCommittedMove: (context, receipt) => (
        workspaceAuthority.projectCommittedEditorMove(context, receipt)
      ),
    });
    teraEditing = new TeraEditingService({
      analysis,
      documents,
      selection: selectionWorkspace,
      source: sourceWorkspace,
      navigation: editorNavigation,
      status: globalStatus,
      runStructural: (operation) => workspaceAuthority.runStructural(operation),
      projectCommitted: (lease, receipt, patch, projectLocalState) => (
        workspaceAuthority.projectCommittedStructural(lease, receipt, patch, projectLocalState)
      ),
    });
    const workbenchNavigation = new WorkbenchNavigationService({
      shell,
      workbench,
      project: projectSession,
      documents,
      source: sourceWorkspace,
      status: globalStatus,
      flushDrafts: (reason) => flushWorkspaceMutationInputs(reason),
      projectLatestPreview: () => workspaceAuthority.projectLatest({
        reason: "manual",
      }),
    });
    const sourceNavigation = new SourceNavigationService({
      project: projectSession,
      documents,
      source: sourceWorkspace,
      css: cssAuthoring,
      selection: selectionWorkspace,
      preview: previewWorkspace,
      workbench: workbenchNavigation,
      workbenchState: workbench,
      shell,
      history: historyOperation,
      status: globalStatus,
      projectLatestPreview: (options) => (
        workspaceAuthority.projectLatest(options)
      ),
      loadFile: (file) => projectDocuments.load(file),
    });
    const versionPreviewService = new VersionPreviewService({
      project: projectSession,
      documents,
      preview: previewWorkspace,
      motion: motionWorkspace,
      shell,
      state: versionPreview,
      flushDrafts: () => flushWorkspaceMutationInputs("template-switch"),
      projectLatestPreview: () => workspaceAuthority.projectLatest({
        reason: "manual",
        force: true,
      }),
    });
    browserPreview = new ProjectBrowserPreviewService({
      project: projectSession,
      transition: transitionLease,
      status: globalStatus,
      route: () => documents.browserPreviewRoute,
    });
    const appSession = new AppSessionService({
      ai: aiCoordination,
      status: globalStatus,
      project: projectSession,
      documents,
      source: sourceWorkspace,
      save: () => saveWorkspace.saveActiveFile(),
      flushDrafts: (reason) => flushWorkspaceMutationInputs(reason),
      requestPreviewRefresh: (reason) => previewWorkspace.requestRefresh(reason),
    });
    const cssWorkspace = new CssWorkspaceService({
      project: projectSession,
      documents,
      analysis,
      authoring: cssAuthoring,
      source: sourceWorkspace,
      workbench,
      preview: previewWorkspace,
      inventory: designClassInventory,
      status: globalStatus,
      structural: {
        run: (operation) => workspaceAuthority.runStructural(operation),
        require: (lease) => workspaceAuthority.requireLease(lease),
        settle: (receipt, options) => workspaceAuthority.settle(receipt, options),
      },
    });
    const pageSettings = new PageSettingsService({
      project: projectSession,
      documents,
      source: sourceWorkspace,
      status: globalStatus,
      settleMutation: (receipt, options) => (
        workspaceAuthority.settle(receipt, options)
      ),
    });
    editorInteraction = new EditorInteractionRuntime({
      editor: {
        get centerView() { return shell.centerView; },
        setCenterView: (view) => workbenchNavigation.setCenterView(view),
        deleteHtmlTarget: (target) => htmlEditing.deleteTarget(target),
        duplicateHtmlTarget: (target) => htmlEditing.duplicateTarget(target),
        selectHtmlTarget: (target, options) => editorSelection.selectHtmlTarget(target, options),
        setPreviewTeraSelection: (target, options) => (
          editorSelection.setTera(target, options)
        ),
        enterEditorNavigationScope: (scopeId) => editorNavigation.enterScope(scopeId),
        openSelectedTeraSource: () => editorSelection.openSelectedTeraSource(),
        deleteSelectedTeraNode: (target) => teraEditing.delete(target),
        setGlobalStatus: (text, kind) => globalStatus.set(text, kind),
      },
      htmlDraft: () => ({
        context: () => ({
          projectRoot: projectSession.root,
          runtimeSessionId: projectSession.runtimeSessionId,
          projectSessionEpoch: projectSession.epoch,
          htmlPending: htmlAuthoring.htmlPending,
          workspace: projectSession.workspace,
          coordinatedSelection: selectionWorkspace.coordinatedElement,
        }),
        previewRuntime: previewWorkspace.runtime,
        setHtmlPending: (area, pending) => htmlAuthoring.setHtmlPending(area, pending),
        setGlobalStatus: (text, kind) => globalStatus.set(text, kind),
        postPreviewMessage: (payload) => previewWorkspace.postMessage(payload),
        applyTextToTarget: (target, text, options) => (
          htmlEditing.applyTextToTarget(target, text, options)
        ),
        applyAttributesToTarget: (target, attributes) => (
          htmlEditing.applyAttributesToTarget(target, attributes)
        ),
        applyCurrentAttributes: (attributes) => (
          htmlEditing.applyAttributes(attributes)
        ),
        projectLatestPreview: (options) => (
          workspaceAuthority.projectLatest(options)
        ),
        reconcileWorkspaceDerivedState: (options) => (
          projectDerived.reconcile(options)
        ),
      }),
    });
    htmlEditing = new HtmlEditingService({
      project: projectSession,
      documents,
      readModel,
      html: htmlAuthoring,
      source: sourceWorkspace,
      selection: selectionWorkspace,
      editor: editorInteraction,
      status: globalStatus,
      structural: {
        run: (operation) => workspaceAuthority.runStructural(operation),
        leaseMatches: (lease) => workspaceAuthority.leaseMatches(lease),
        projectCommitted: (lease, receipt, patch, projectLocalState) => (
          workspaceAuthority.projectCommittedStructural(lease, receipt, patch, projectLocalState)
        ),
        projectCommittedBatch: (lease, receipt) => (
          workspaceAuthority.projectCommittedSelectionBatch(lease, receipt)
        ),
        settleMutation: (receipt, options) => (
          workspaceAuthority.settle(receipt, options)
        ),
      },
      editStructural: {
        run: (operation) => workspaceAuthority.runStructural(operation),
        projectCommitted: (lease, receipt, patch, projectLocalState) => (
          workspaceAuthority.projectCommittedStructural(lease, receipt, patch, projectLocalState)
        ),
      },
      commands: {
        loadProjectFile: (file) => projectDocuments.load(file),
        reconcilePageAssets: (tpl) => reconcilePageAssetContracts({
          get sourceCache() { return sourceWorkspace.sourceCache; },
          set sourceCache(cache) { sourceWorkspace.sourceCache = cache; },
          get activeScannedPath() { return documents.activeScannedPath; },
          get source() { return sourceWorkspace.source; },
          set source(source) { sourceWorkspace.source = source; },
          get sessionProjectRoot() { return projectSession.root; },
          get kernelProjectSessionId() { return projectSession.runtimeSessionId; },
          get projectSessionEpoch() { return projectSession.epoch; },
          setGlobalStatus: (text, kind) => globalStatus.set(text, kind),
        }, tpl),
      },
    });
    editorSelection = new EditorSelectionService({
      selection: selectionWorkspace,
      canvas: canvasInteraction,
      readModel,
      source: sourceWorkspace,
      html: htmlAuthoring,
      css: cssAuthoring,
      editor: () => editorInteraction,
      htmlEditing: () => htmlEditing,
      status: globalStatus,
      viewport: () => workbench.previewDevice,
      setCenterView: (view) => workbenchNavigation.setCenterView(view),
      openContentPage: (relativePath) => workbench.openContentPage(relativePath),
    });
    const previewInserts = new PreviewInsertService({
      html: htmlEditing,
      tera: teraEditing,
      navigation: editorNavigation,
      status: globalStatus,
    });
    previewRuntime = new PreviewRuntimeService({
      project: projectSession,
      documents,
      source: sourceWorkspace,
      analysis,
      css: cssAuthoring,
      selection: selectionWorkspace,
      selectionService: editorSelection,
      sections: pageSections,
      preview: previewWorkspace,
      canvas: canvasInteraction,
      authority: workspaceAuthority,
      inserts: previewInserts,
      preferences: applicationPreferences,
      status: globalStatus,
      restoreLiveCssLayers: () => cssWorkspace.restoreLiveLayers(),
    });
    projectDerived = new ProjectDerivedStateService({
      project: projectSession,
      documents,
      analysis,
      disk: acceptedDisk,
      externalDisk,
      transition: transitionLease,
      preview: previewWorkspace,
      previewRuntime,
      status: globalStatus,
      loadFile: (file, options) => projectDocuments.load(file, options),
    });
    const setProjectRoot = (root = "") => {
      if (projectSession.root !== root) {
        projectAudit.reset();
        publish.invalidate();
        designClassInventory.reset();
      }
      projectSession.root = root;
    };
    const projectReset = new ProjectResetService({
      sources: { documents, analysis, source: sourceWorkspace },
      preview: {
        workspace: previewWorkspace,
        sections: pageSections,
        version: versionPreview,
        selection: editorSelection,
      },
      editor: {
        html: htmlAuthoring,
        css: cssAuthoring,
        selection: selectionWorkspace,
        runtime: editorInteraction,
      },
      workspace: {
        project: projectSession,
        workbench,
        explorer: fileExplorer,
        publish,
        acceptedDisk,
        externalDisk,
        status: globalStatus,
      },
      setProjectRoot,
    });
    const projectAttachment = new ProjectAttachmentService({
      shell,
      project: projectSession,
      source: sourceWorkspace,
      analysis,
      css: cssAuthoring,
      publish,
      workbench,
      reset: projectReset,
      documents: projectDocuments,
      externalDisk,
      acceptedDisk,
      transition: transitionLease,
      startup,
      status: globalStatus,
      setProjectRoot,
    });
    const projectPreviewBootstrap = new ProjectPreviewBootstrapService({
      project: projectSession,
      documents,
      preview: previewWorkspace,
      selection: selectionWorkspace,
      template: templateWorkbench,
      transition: transitionLease,
      controlled: controlledPreview,
      runtime: previewRuntime,
      status: globalStatus,
    });
    projectTransitions = new ProjectTransitionService({
      project: projectSession,
      startup,
      documents,
      lease: transitionLease,
      attachment: projectAttachment,
      preview: projectPreviewBootstrap,
      reset: projectReset,
      externalDisk,
      acceptedDisk,
      ai: aiCoordination,
      status: globalStatus,
      shell,
      terminal,
      audit: projectAudit,
    });
    const dynamicWidgets = new DynamicWidgetService({
      project: projectSession,
      selection: selectionWorkspace,
      status: globalStatus,
      settleMutation: (receipt, options) => (
        workspaceAuthority.settle(receipt, options)
      ),
    });
    const insertCatalogDrag = new InsertCatalogDragService({
      project: projectSession,
      documents,
      selection: selectionWorkspace,
      surface: previewSurface,
      preview: previewWorkspace,
      shell,
      workbench,
      status: globalStatus,
    });
    saveWorkspace = new ProjectSaveService({
      project: projectSession,
      documents,
      disk: acceptedDisk,
      externalDisk,
      transition: transitionLease,
      history: historyOperation,
      ai: aiCoordination,
      html: htmlAuthoring,
      editor: editorInteraction,
      status: globalStatus,
      commands: {
        applyTagChange: () => htmlEditing.applyTag(),
        applyClasses: () => htmlEditing.applyClasses(),
        applyImageSource: (src) => htmlEditing.applyImage(src),
        reconcileWorkspaceDerivedState: (options) => (
          projectDerived.reconcile(options)
        ),
        projectLatestPreview: (options) => (
          workspaceAuthority.projectLatest(options)
        ),
        markPreviewSavedToDisk: (message) => previewWorkspace.markSavedToDisk(message),
        scheduleZolaValidation: (reason) => controlledPreview.scheduleValidation(reason ?? "save"),
      },
    });
    return {
      publishWorkspace: publish,
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
      projectReset,
      templateWorkbench,
      projectTransitions,
      saveWorkspace,
      pageSettings,
      terminalWorkspace: terminal,
    };
  }
