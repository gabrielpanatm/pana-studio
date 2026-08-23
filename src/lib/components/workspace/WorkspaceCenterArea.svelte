<script lang="ts">
  import { onDestroy, type Component, type ComponentProps } from "svelte";
  import type { TerminalPaneProps } from "$lib/components/TerminalPane.svelte";
  import EditorShell from "$lib/components/EditorShell.svelte";
  import WorkbenchBottomPanel from "$lib/components/workbench/WorkbenchBottomPanel.svelte";
  import MotionTimelinePanel from "$lib/components/workspace/MotionTimelinePanel.svelte";
  import WorkspaceResizeHandle from "$lib/components/workspace/WorkspaceResizeHandle.svelte";
  import type { GlobalStatusState } from "$lib/status/state.svelte";
  import type { MotionWorkspaceState } from "$lib/motion/workspace.svelte";
  import type { TerminalWorkspaceState } from "$lib/terminal/workspace.svelte";
  import type { ProjectWorkspaceMutationService } from "$lib/session/workspace-mutation-service";
  import type { PreviewInsertDropRequest } from "$lib/state/preview-insert-controller";
  import type { EditorActionOutcome } from "$lib/editor-runtime/action-outcome";
  import type { CssMutationAuthorityReceipt } from "$lib/css/mutation-contract";
  import type { ScssVariable } from "$lib/css/contracts";
  import type { TeraDropRequest } from "$lib/tera/model";
  import type {
    PageFrontmatterField,
    PageFrontmatterMutationValue,
  } from "$lib/markdown/frontmatter";
  import type { ApplicationPreferencesState } from "$lib/application/preferences.svelte";
  import type { PublishWorkspaceState } from "$lib/deploy/publish-state.svelte";
  import type { WorkspaceLayoutState } from "$lib/ui/workspace-layout.svelte";
  import { startPointerSession } from "$lib/ui/pointer-session";
  import type { WorkbenchDocumentNavigationService } from "$lib/workbench/document-navigation";
  import type {
    ApplicationSurface,
    CenterView,
  } from "$lib/application/contracts";
  import type {
    AuditFinding,
    AuditRunMode,
  } from "$lib/audit/contracts";
  import type { CoordinatedElementSelection } from "$lib/canvas/contracts";
  import type {
    FileExplorerOperationPlan,
    FileExplorerOperationRequest,
    FileExplorerSnapshot,
  } from "$lib/project/file-explorer-contract";
  import type {
    ProjectFile,
    ProjectScan,
  } from "$lib/project/lifecycle-contract";
  import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";
  import type { SourceGraph } from "$lib/source-graph/graph-contract";
  import type { SourceRange } from "$lib/source-graph/contracts";
  import type { VersionPreviewReceipt } from "$lib/versioning/contracts";
  import type {
    WorkbenchActivity,
    WorkbenchDocumentActivationSnapshot,
    WorkbenchSnapshot,
    WorkspaceSourceOpenOptions,
  } from "$lib/workbench/contracts";
  import { t } from "$lib/i18n/runtime.svelte";
  import { errorMessage } from "$lib/util";
  import type { DesignClassInventorySnapshot } from "$lib/css/design-system-contract";
  import type { MotionPreviewMode } from "$lib/motion/workspace.svelte";

  type ForwardedEditorShellProps = Omit<
    ComponentProps<typeof EditorShell>,
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

  let {
    session,
    creation,
    auxiliary,
    workspaceCommands,
    creationCommands,
    contentCommands,
    auditCommands,
    applicationPreferences,
    globalStatus,
    motionWorkspace,
    terminalWorkspace,
    workspaceMutations,
    publishWorkspace,
    workspaceLayout,
    workbenchDocuments,
    TerminalPaneComponent = null,
    breakpointValue,
    openWorkspaceSource,
    editorProps,
    documentActivation,
    previewFrame = $bindable(),
    codeEditorHost = $bindable(),
  }: {
    session: {
      applicationSurface: ApplicationSurface;
      workbenchSnapshot: WorkbenchSnapshot | null;
      centerView: CenterView;
      sessionId: string;
      projectRoot: string;
      project: ProjectScan | null;
      workspace: ProjectWorkspaceSnapshot | null;
      interactionLocked: boolean;
      activeRenderedTemplatePath: string;
      activeScannedPath: string | null;
      jsRefreshToken: number;
      interactivePreviewEnabled: boolean;
    };
    creation: {
      sourceGraph: SourceGraph | null;
      coordinatedElementSelection: CoordinatedElementSelection | null;
      activeCanvasPreviewRevision: string;
      assetPreviewRevision: string | null;
      templateWorkbenchPreferredPagePath: string | null;
      designClassInventory: DesignClassInventorySnapshot | null;
      designClassInventoryLoading: boolean;
      designClassInventoryError: string;
      scssVariables: ScssVariable[];
      fileExplorerSnapshot: FileExplorerSnapshot | null;
      scannedPages: ProjectFile[];
      scannedTemplates: ProjectFile[];
    };
    auxiliary: {
      aiContextStatus: import("$lib/ai/contracts").AiContextStatus | null;
      currentAudit: import("$lib/audit/contracts").AuditRunReceipt | null;
      projectStatus: string;
      refreshToken: number;
      activeVersionPreview: VersionPreviewReceipt | null;
      externalDiskWatchRevision: number;
      projectAuditLoading: boolean;
      projectAuditError: string;
      validationRunning: boolean;
      validationMessage: string;
      currentProjectPath: string;
      dirtyAreas: string[];
      canSave: boolean;
      diskBlockedReason: string | null;
      auditView: "overview" | "runtime";
      auditFocusSerial: number;
      motionSelectionSummary: import("$lib/editor/contracts").InspectorSelectionSummarySnapshot | null;
      motionDataAnim: string | null;
    };
    workspaceCommands: {
      setInspectorJsPending: (pending: boolean) => void;
      setPreviewExecutionMode: (mode: MotionPreviewMode) => void;
      setWorkbenchActivity: (activity: WorkbenchActivity) => Promise<unknown>;
      openInBrowser: (route?: string | null) => Promise<unknown>;
      saveActiveFile: () => Promise<unknown>;
      openAudit: (focusObservability?: boolean) => Promise<unknown>;
      revealSourceRange: (file: string, range: SourceRange) => void;
      showVersionPreview: (receipt: VersionPreviewReceipt) => Promise<void>;
      returnToLivePreview: () => Promise<void>;
    };
    creationCommands: {
      updateTemplateWorkbenchContext: (
        project: ProjectScan,
        template: ProjectFile,
        pageFile: string,
        options: { preferredRoute: string; strict: true },
      ) => Promise<unknown>;
      insertTeraPaletteItemAtTarget: (request: TeraDropRequest) => Promise<unknown>;
      insertPaletteElementAtTarget: (request: PreviewInsertDropRequest) => Promise<unknown>;
      refreshClassInventory: () => Promise<unknown>;
      createVariable: (path: string, name: string, value: string) => Promise<boolean>;
      createClass: (name: string, path: string) => Promise<boolean>;
      updateVariable: (variable: ScssVariable, value: string) => Promise<boolean>;
      renameClass: (oldName: string, newName: string) => Promise<boolean>;
      refreshFileExplorer: () => Promise<unknown>;
      planFileExplorer: (request: FileExplorerOperationRequest) => Promise<FileExplorerOperationPlan>;
      commitFileExplorer: (plan: FileExplorerOperationPlan) => Promise<unknown>;
      injectRawCss: (id: string, css: string) => void;
      projectCommittedCssMutation: (
        authority: CssMutationAuthorityReceipt,
        liveEpoch: number | null,
      ) => Promise<unknown>;
      applyImageSource: (source: string) => Promise<EditorActionOutcome>;
    };
    contentCommands: {
      createPage: (input: { title: string; slug: string; section: string }) => Promise<string | null>;
      openPageEditor: (relativePath: string) => Promise<unknown>;
      updateFrontmatterSource: (relativePath: string, source: string) => void;
      updateFrontmatterField: (
        relativePath: string,
        field: PageFrontmatterField,
        value: PageFrontmatterMutationValue,
      ) => Promise<string>;
      readPageSettings: (relativePath: string) => Promise<string>;
    };
    auditCommands: {
      applySafeFix: (finding: AuditFinding, fixId: string) => Promise<unknown>;
      refresh: (force?: boolean, mode?: AuditRunMode) => Promise<import("$lib/deploy/contracts").AuditRefreshResult>;
      runValidation: () => Promise<boolean>;
      setView: (view: "overview" | "runtime") => void;
    };
    applicationPreferences: ApplicationPreferencesState;
    globalStatus: GlobalStatusState;
    motionWorkspace: MotionWorkspaceState;
    terminalWorkspace: TerminalWorkspaceState;
    workspaceMutations: ProjectWorkspaceMutationService;
    publishWorkspace: PublishWorkspaceState;
    workspaceLayout: WorkspaceLayoutState;
    workbenchDocuments: WorkbenchDocumentNavigationService;
    TerminalPaneComponent?: Component<TerminalPaneProps> | null;
    breakpointValue: (name: string, fallback: string) => string;
    openWorkspaceSource: (
      path: string,
      options?: WorkspaceSourceOpenOptions,
    ) => void | Promise<void>;
    editorProps: ForwardedEditorShellProps;
    documentActivation: WorkbenchDocumentActivationSnapshot;
    previewFrame?: HTMLIFrameElement;
    codeEditorHost?: HTMLDivElement;
  } = $props();

  const bottomPanelOpen = $derived(
    session.applicationSurface === "workbench"
      && Boolean(session.workbenchSnapshot?.bottomPanel.open)
      && session.workbenchSnapshot?.bottomPanel.activeView === "terminal",
  );
  const activeWorkbenchActivity = $derived(
    session.workbenchSnapshot?.activeActivity ?? "editor",
  );
  const editorSurfaceActive = $derived(
    session.applicationSurface === "workbench"
      && activeWorkbenchActivity === "editor"
      && session.centerView !== "kernel",
  );
  type RetainedAuxiliarySurface =
    | Exclude<WorkbenchActivity, "editor">
    | "settings"
    | "kernel";
  type AuxiliaryWorkspaceComponent = Component<any>;
  let AuditWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let ContentWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let ContentModelsWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let DataWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let AssetsWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let BlocksWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let ComponentsWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let DesignSystemWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let KernelWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let PublishWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let SettingsWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let TaxonomiesWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let TemplatesWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let VersionControlWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  const auxiliaryWorkspaceLoads = new Map<
    RetainedAuxiliarySurface,
    Promise<void>
  >();
  const auxiliaryWorkspaceLoaders: Record<
    RetainedAuxiliarySurface,
    () => Promise<void>
  > = {
    settings: async () => {
      SettingsWorkspace = (await import(
        "$lib/components/settings/SettingsWorkspace.svelte"
      )).default;
    },
    templates: async () => {
      TemplatesWorkspace = (await import(
        "$lib/components/templates/TemplatesWorkspace.svelte"
      )).default;
    },
    components: async () => {
      ComponentsWorkspace = (await import(
        "$lib/components/creation/ComponentsWorkspace.svelte"
      )).default;
    },
    blocks: async () => {
      BlocksWorkspace = (await import(
        "$lib/components/creation/BlocksWorkspace.svelte"
      )).default;
    },
    design_system: async () => {
      DesignSystemWorkspace = (await import(
        "$lib/components/creation/DesignSystemWorkspace.svelte"
      )).default;
    },
    assets: async () => {
      AssetsWorkspace = (await import(
        "$lib/components/creation/AssetsWorkspace.svelte"
      )).default;
    },
    content: async () => {
      ContentWorkspace = (await import(
        "$lib/components/content/ContentWorkspace.svelte"
      )).default;
    },
    content_models: async () => {
      ContentModelsWorkspace = (await import(
        "$lib/components/content-models/ContentModelsWorkspace.svelte"
      )).default;
    },
    taxonomies: async () => {
      TaxonomiesWorkspace = (await import(
        "$lib/components/taxonomies/TaxonomiesWorkspace.svelte"
      )).default;
    },
    data: async () => {
      DataWorkspace = (await import(
        "$lib/components/data/DataWorkspace.svelte"
      )).default;
    },
    versioning: async () => {
      VersionControlWorkspace = (await import(
        "$lib/components/versioning/VersionControlWorkspace.svelte"
      )).default;
    },
    publish: async () => {
      PublishWorkspace = (await import(
        "$lib/components/publish/PublishWorkspace.svelte"
      )).default;
    },
    audit: async () => {
      AuditWorkspace = (await import(
        "$lib/components/audit/AuditWorkspace.svelte"
      )).default;
    },
    kernel: async () => {
      KernelWorkspace = (await import(
        "$lib/components/kernel/KernelWorkspace.svelte"
      )).default;
    },
  };
  let retainedAuxiliarySurface = $state<RetainedAuxiliarySurface | null>(null);
  let retainedAuxiliarySessionId = $state("");

  function ensureAuxiliaryWorkspaceLoaded(surface: RetainedAuxiliarySurface) {
    if (auxiliaryWorkspaceLoads.has(surface)) return;
    const load = auxiliaryWorkspaceLoaders[surface]()
      .then(() => {
        globalStatus.clear("workbench.activity.lazy-load");
      })
      .catch((error) => {
        auxiliaryWorkspaceLoads.delete(surface);
        globalStatus.escalate({
          id: "workbench.activity.lazy-load",
          level: "error",
          title: t("workbench-activity-open-failed"),
          message: errorMessage(error),
        });
      });
    auxiliaryWorkspaceLoads.set(surface, load);
  }

  $effect(() => {
    const sessionId = session.sessionId;
    if (retainedAuxiliarySessionId !== sessionId) {
      retainedAuxiliarySessionId = sessionId;
      retainedAuxiliarySurface = null;
    }
    if (session.applicationSurface === "settings") {
      retainedAuxiliarySurface = "settings";
    } else if (activeWorkbenchActivity !== "editor") {
      retainedAuxiliarySurface = activeWorkbenchActivity;
    } else if (session.centerView === "kernel") {
      retainedAuxiliarySurface = "kernel";
    }
    if (retainedAuxiliarySurface) {
      ensureAuxiliaryWorkspaceLoaded(retainedAuxiliarySurface);
    }
  });
  const motionTimelineAvailable = $derived(
    session.applicationSurface === "workbench"
      && activeWorkbenchActivity === "editor"
      && session.centerView === "preview"
      && Boolean(session.activeRenderedTemplatePath)
      && Boolean(session.project),
  );
  const motionTimelineOpen = $derived(
    motionTimelineAvailable && motionWorkspace.timelineOpen,
  );
  const motionTimelineHeight = $derived(
    motionWorkspace.timelineCollapsed ? 36 : motionWorkspace.timelineHeight,
  );
  let motionTimelineResizing = $state(false);
  let cancelMotionTimelineResize: (() => void) | null = null;
  const dirtyWorkbenchPaths = $derived(
    session.workspace?.documents.files
      .filter((file) => file.dirty)
      .map((file) => file.relativePath)
      ?? [],
  );
  const responsiveBreakpoints = $derived([
    {
      id: "mobile",
      label: t("workbench-responsive-mobile"),
      widthPx: Number.parseFloat(breakpointValue("bp-mobil", "768px")) || 768,
    },
    {
      id: "tablet",
      label: t("workbench-responsive-tablet"),
      widthPx: Number.parseFloat(breakpointValue("bp-tableta", "1024px")) || 1_024,
    },
  ]);
  const activateWorkbenchDocument: ComponentProps<typeof EditorShell>["activateWorkbenchDocument"] = (
    groupId,
    document,
  ) => workbenchDocuments.activate(groupId, document);
  const closeWorkbenchDocument: ComponentProps<typeof EditorShell>["closeWorkbenchDocument"] = (
    groupId,
    document,
  ) => workbenchDocuments.close(groupId, document);
  const setWorkbenchSurface: ComponentProps<typeof EditorShell>["setWorkbenchSurface"] = (
    surface,
  ) => workbenchDocuments.setSurface(surface);
  const onMotionPreviewStatus: ComponentProps<typeof EditorShell>["onMotionPreviewStatus"] = (
    status,
  ) => motionWorkspace.acceptPreviewStatus(status);

  $effect(() => {
    motionWorkspace.bind(
      session.activeRenderedTemplatePath,
      session.projectRoot,
      session.sessionId,
      session.jsRefreshToken,
    );
  });

  $effect(() => {
    workspaceCommands.setInspectorJsPending(motionWorkspace.pendingCount > 0);
  });

  $effect(() => {
    const mode = motionWorkspace.previewMode;
    const shouldExecute = mode !== "design";
    if (session.interactivePreviewEnabled !== shouldExecute) {
      workspaceCommands.setPreviewExecutionMode(mode);
    }
  });

  function resizeMotionTimeline(event: PointerEvent) {
    if (event.button !== 0) return;
    cancelMotionTimelineResize?.();
    event.preventDefault();

    const startY = event.clientY;
    const startHeight = motionWorkspace.timelineHeight;
    let latestY = startY;

    motionTimelineResizing = true;
    document.body.classList.add("is-resizing", "is-row-resizing");

    const heightAt = (clientY: number) => Math.max(
      190,
      Math.min(560, startHeight - (clientY - startY)),
    );

    const publishLiveResize = () => {
      motionWorkspace.timelineHeight = heightAt(latestY);
    };

    const finish = () => {
      document.body.classList.remove("is-resizing", "is-row-resizing");
      cancelMotionTimelineResize = null;
      motionTimelineResizing = false;
    };

    const session = startPointerSession({
      pointerId: event.pointerId,
      captureTarget: event.currentTarget instanceof HTMLElement ? event.currentTarget : null,
      safetyTimeoutMs: 8_000,
      onMove: (moveEvent, currentSession) => {
        moveEvent.preventDefault();
        latestY = moveEvent.clientY;
        currentSession.requestFrame(publishLiveResize);
      },
      onCommit: (upEvent, currentSession) => {
        if (upEvent) latestY = upEvent.clientY;
        currentSession.flushFrame();
        motionWorkspace.timelineHeight = heightAt(latestY);
        finish();
      },
      onCancel: () => {
        motionWorkspace.timelineHeight = startHeight;
        finish();
      },
    });
    cancelMotionTimelineResize = () => session.cancel("programmatic");
  }

  onDestroy(() => cancelMotionTimelineResize?.());
</script>

<section
  class:bottom-panel-open={bottomPanelOpen}
  class:motion-timeline-open={motionTimelineOpen}
  class="center-stack"
  style={`--terminal-pane-height: ${workspaceLayout.terminalPaneHeight}px; --motion-timeline-height: ${motionTimelineHeight}px;`}
  aria-label={t("workbench-center-area")}
>
  <div
    class="editor-shell-shell"
    inert={session.interactionLocked ? true : undefined}
    aria-busy={session.interactionLocked}
  >
    {#if session.project && session.sessionId}
      {#key session.sessionId}
        <div
          class="stable-editor-surface"
          class:surface-inactive={!editorSurfaceActive}
          inert={!editorSurfaceActive ? true : undefined}
          aria-hidden={!editorSurfaceActive}
        >
          <EditorShell
            {...editorProps}
            {documentActivation}
            bind:previewFrame
            bind:codeEditorHost
            surfaceActive={editorSurfaceActive}
            {responsiveBreakpoints}
            motionPreviewMode={motionWorkspace.previewMode}
            motionPreviewRequest={motionWorkspace.previewRequest}
            dirtyWorkbenchPaths={dirtyWorkbenchPaths}
            {activateWorkbenchDocument}
            {closeWorkbenchDocument}
            {setWorkbenchSurface}
            {onMotionPreviewStatus}
          />
        </div>
      {/key}
    {/if}

  </div>

  {#if motionTimelineOpen}
    {#if !motionWorkspace.timelineCollapsed}
      <WorkspaceResizeHandle
        kind="timeline"
        active={motionTimelineResizing}
        withBottomPanel={bottomPanelOpen}
        ariaLabel={t("workbench-resize-motion-timeline")}
        onDrag={resizeMotionTimeline}
        onReset={() => { motionWorkspace.timelineHeight = 300; }}
      />
    {/if}
    <MotionTimelinePanel
      workspace={motionWorkspace}
      selectionSummary={auxiliary.motionSelectionSummary}
      dataAnim={auxiliary.motionDataAnim}
    />
  {/if}

  {#if bottomPanelOpen}
    <WorkspaceResizeHandle
      kind="terminal"
      active={workspaceLayout.activeResizeKind === "terminal"}
      ariaLabel={t("workbench-resize-bottom-panel")}
      onDrag={(event) => workspaceLayout.startResizeDrag("terminal", event)}
      onReset={() => workspaceLayout.resetResize("terminal")}
    />

    <WorkbenchBottomPanel
      workspace={terminalWorkspace}
      {TerminalPaneComponent}
    />
  {/if}
</section>

{#if retainedAuxiliarySurface}
  <div
    class="workspace-auxiliary-overlay"
    class:surface-inactive={editorSurfaceActive}
    inert={editorSurfaceActive ? true : undefined}
    aria-hidden={editorSurfaceActive}
  >
    {#if retainedAuxiliarySurface === "settings" && SettingsWorkspace}
      <SettingsWorkspace
        aiContextStatus={auxiliary.aiContextStatus}
        {applicationPreferences}
        {globalStatus}
        {workspaceLayout}
      />
    {:else if retainedAuxiliarySurface === "templates" && TemplatesWorkspace}
      <TemplatesWorkspace
        {globalStatus}
        {workspaceMutations}
        sourceGraph={creation.sourceGraph}
        activeScannedPath={session.activeScannedPath}
        openEditor={() => workspaceCommands.setWorkbenchActivity("editor")}
        {openWorkspaceSource}
      />
    {:else if retainedAuxiliarySurface === "components" && ComponentsWorkspace}
      <ComponentsWorkspace
        {globalStatus}
        {workspaceMutations}
        sourceGraph={creation.sourceGraph}
        {openWorkspaceSource}
      />
    {:else if retainedAuxiliarySurface === "blocks" && BlocksWorkspace}
      <BlocksWorkspace
        sourceGraph={creation.sourceGraph}
        coordinatedElementSelection={creation.coordinatedElementSelection}
        activeScannedPath={session.activeScannedPath}
        activeCanvasPreviewRevision={creation.activeCanvasPreviewRevision}
        {workspaceMutations}
        scannedProject={session.project}
        templateWorkbenchPreferredPagePath={creation.templateWorkbenchPreferredPagePath}
        updateTemplateWorkbenchContext={(
          project: ProjectScan,
          template: ProjectFile,
          pageFile: string,
          options: { preferredRoute: string; strict: true },
        ) => (
          creationCommands.updateTemplateWorkbenchContext(project, template, pageFile, options)
        )}
        insertTeraPaletteItemAtTarget={creationCommands.insertTeraPaletteItemAtTarget}
        insertPaletteElementAtTarget={creationCommands.insertPaletteElementAtTarget}
        openEditor={() => workspaceCommands.setWorkbenchActivity("editor")}
      />
    {:else if retainedAuxiliarySurface === "design_system" && DesignSystemWorkspace}
      <DesignSystemWorkspace
        sourceGraph={creation.sourceGraph}
        designClassInventory={creation.designClassInventory}
        designClassInventoryLoading={creation.designClassInventoryLoading}
        designClassInventoryError={creation.designClassInventoryError}
        scssVariables={creation.scssVariables}
        fileExplorerSnapshot={creation.fileExplorerSnapshot}
        commands={{
          refreshClassInventory: creationCommands.refreshClassInventory,
          createVariable: creationCommands.createVariable,
          createClass: creationCommands.createClass,
          updateVariable: creationCommands.updateVariable,
          renameClass: creationCommands.renameClass,
          refreshFileExplorer: creationCommands.refreshFileExplorer,
          planFileExplorer: creationCommands.planFileExplorer,
          commitFileExplorer: creationCommands.commitFileExplorer,
          injectRawCss: creationCommands.injectRawCss,
          projectCommittedCssMutation: creationCommands.projectCommittedCssMutation,
        }}
        {globalStatus}
        {workspaceMutations}
        {openWorkspaceSource}
      />
    {:else if retainedAuxiliarySurface === "assets" && AssetsWorkspace}
      <AssetsWorkspace
        sourceGraph={creation.sourceGraph}
        previewRevision={creation.assetPreviewRevision}
        coordinatedElementSelection={creation.coordinatedElementSelection}
        previewBaseUrl={session.project?.previewBaseUrl ?? null}
        fileExplorerSnapshot={creation.fileExplorerSnapshot}
        commands={{
          refreshFileExplorer: creationCommands.refreshFileExplorer,
          planFileExplorer: creationCommands.planFileExplorer,
          commitFileExplorer: creationCommands.commitFileExplorer,
          applyImageSource: creationCommands.applyImageSource,
          openEditor: () => workspaceCommands.setWorkbenchActivity("editor"),
          openInBrowser: (route: string) => workspaceCommands.openInBrowser(route),
        }}
        {globalStatus}
        {workspaceMutations}
      />
    {:else if retainedAuxiliarySurface === "content" && ContentWorkspace}
      <ContentWorkspace
        sourceGraph={creation.sourceGraph}
        contentWorkspace={session.workbenchSnapshot?.contentWorkspace ?? null}
        currentAudit={auxiliary.currentAudit}
        projectStatus={auxiliary.projectStatus}
        refreshToken={auxiliary.refreshToken}
        scannedPages={creation.scannedPages}
        scannedTemplates={creation.scannedTemplates}
        activeTheme={session.project?.activeTheme ?? null}
        commands={{
          ...contentCommands,
          openTaxonomies: () => workspaceCommands.setWorkbenchActivity("taxonomies"),
          openContentModels: () => workspaceCommands.setWorkbenchActivity("content_models"),
          openInBrowser: (route: string) => workspaceCommands.openInBrowser(route),
        }}
        {globalStatus}
        {workspaceMutations}
        {openWorkspaceSource}
      />
    {:else if retainedAuxiliarySurface === "content_models" && ContentModelsWorkspace}
      <ContentModelsWorkspace
        {globalStatus}
        {workspaceMutations}
        sourceGraph={creation.sourceGraph}
        {openWorkspaceSource}
      />
    {:else if retainedAuxiliarySurface === "taxonomies" && TaxonomiesWorkspace}
      <TaxonomiesWorkspace
        {globalStatus}
        {workspaceMutations}
        openTemplates={() => workspaceCommands.setWorkbenchActivity("templates")}
        openInBrowser={(route: string) => workspaceCommands.openInBrowser(route)}
        {openWorkspaceSource}
      />
    {:else if retainedAuxiliarySurface === "data" && DataWorkspace}
      <DataWorkspace
        {globalStatus}
        {workspaceMutations}
        sourceGraph={creation.sourceGraph}
        uiLocale={applicationPreferences.locale}
        openEditor={() => workspaceCommands.setWorkbenchActivity("editor")}
        {openWorkspaceSource}
      />
    {:else if retainedAuxiliarySurface === "versioning" && VersionControlWorkspace}
      <VersionControlWorkspace
        {globalStatus}
        {workspaceMutations}
        activeScannedPath={session.activeScannedPath}
        activeVersionPreview={auxiliary.activeVersionPreview}
        showVersionPreview={workspaceCommands.showVersionPreview}
        returnToLivePreview={workspaceCommands.returnToLivePreview}
      />
    {:else if retainedAuxiliarySurface === "publish" && PublishWorkspace}
      <PublishWorkspace
        {publishWorkspace}
        {workspaceMutations}
        {globalStatus}
        scannedProject={Boolean(session.project)}
        externalDiskWatchRevision={auxiliary.externalDiskWatchRevision}
        saveActiveFile={workspaceCommands.saveActiveFile}
        openAudit={workspaceCommands.openAudit}
        revealSourceRange={workspaceCommands.revealSourceRange}
        {openWorkspaceSource}
      />
    {:else if retainedAuxiliarySurface === "audit" && AuditWorkspace}
      <AuditWorkspace
        snapshot={auxiliary.currentAudit}
        {workspaceMutations}
        projectAuditLoading={auxiliary.projectAuditLoading}
        projectAuditError={auxiliary.projectAuditError}
        validationRunningState={{
          running: auxiliary.validationRunning,
          message: auxiliary.validationMessage,
        }}
        projectHealth={{
          currentProjectPath: auxiliary.currentProjectPath,
          projectFileCount: session.project?.files.length ?? 0,
          sourceNodeCount: creation.sourceGraph?.nodes.length ?? 0,
          dirtyAreas: auxiliary.dirtyAreas,
          canSave: auxiliary.canSave,
          diskBlockedReason: auxiliary.diskBlockedReason,
          projectStatus: auxiliary.projectStatus,
        }}
        applySafeAuditFix={auditCommands.applySafeFix}
        refreshProjectAudit={auditCommands.refresh}
        runZolaValidation={auditCommands.runValidation}
        revealSourceRange={workspaceCommands.revealSourceRange}
        {globalStatus}
        {openWorkspaceSource}
        requestedView={auxiliary.auditView}
        observabilityFocusSerial={auxiliary.auditFocusSerial}
        onViewChange={auditCommands.setView}
      />
    {:else if retainedAuxiliarySurface === "kernel" && KernelWorkspace}
      <KernelWorkspace
        currentProjectPath={auxiliary.currentProjectPath}
        projectFileCount={session.project?.files.length ?? 0}
        sourceNodeCount={creation.sourceGraph?.nodes.length ?? 0}
        dirtyAreas={auxiliary.dirtyAreas}
        canSave={auxiliary.canSave}
        diskBlockedReason={auxiliary.diskBlockedReason}
        projectStatus={auxiliary.projectStatus}
        onStatusUpdate={(
          text: string,
          kind: "restored" | "saving" | "error",
        ) => globalStatus.set(text, kind)}
      />
    {:else}
      <div class="workspace-lazy-loading" role="status" aria-live="polite">
        {t("common-loading")}
      </div>
    {/if}
  </div>
{/if}
