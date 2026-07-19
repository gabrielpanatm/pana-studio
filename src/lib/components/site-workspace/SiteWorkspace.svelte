<script lang="ts">
  import {
    IconCode,
    IconPlus,
    IconRefresh,
    IconSitemap,
    IconWorld,
    IconX,
  } from "@tabler/icons-svelte";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import LoopBuilderPanel from "$lib/components/site-workspace/loops/LoopBuilderPanel.svelte";
  import SiteDesignPanel from "./SiteDesignPanel.svelte";
  import SiteOverviewPanel from "./SiteOverviewPanel.svelte";
  import SitePagesPanel from "./SitePagesPanel.svelte";
  import SiteSourcesPanel from "./SiteSourcesPanel.svelte";
  import SiteStructurePanel from "./SiteStructurePanel.svelte";
  import SiteWorkspaceSidebar from "./SiteWorkspaceSidebar.svelte";
  import type { SiteWorkspaceSection } from "./workspace-model";
  import { buildManagedFontFaceBlock, upsertManagedFontFaceCssBlock } from "$lib/fonts/font-face";
  import {
    buildFontRoleRows,
    fontFaceTargetForVariable,
    fontStackForFamily,
    type FontRoleRow,
  } from "$lib/fonts/model";
  import {
    activeFontPreloadFiles,
    fontPreloadTargetCandidates,
    upsertFontPreloadBlock,
  } from "$lib/fonts/preload";
  import {
    createCssRequestIdentity,
    cssRequestIdentityMatches,
    downloadGoogleFontFamily,
    getFontInventory,
    getScssVariables,
    readProjectFile,
    readSourceGraph,
    searchGoogleFonts,
    setScssVariable,
  } from "$lib/project/io";
  import { buildSiteArchitecture } from "$lib/source-graph/architecture";
  import { buildTeraEditorContext } from "$lib/source-graph/context";
  import {
    firstSourceNodeInOutline,
    semanticOutlineTree,
    templateCompositionNodes,
    templateCompositionTree,
    type SemanticOutlineItem,
  } from "$lib/source-graph/outline";
  import {
    requireCurrentPreviewStructuralSession,
    type PreviewStructuralSessionLease,
  } from "$lib/kernel/preview-structural-lane";
  import { captureSiteActionUiLease, siteActionUiLeaseMatches } from "$lib/source-graph/action-ui-lease";
  import { partialTemplateName, type SiteStructureSessionHost } from "$lib/source-graph/template-actions";
  import { scannedCacheKey } from "$lib/project/files";
  import {
    targetTemplateForWorkspace,
    visitorPreviewUrlForSourcePage,
  } from "$lib/source-graph/workspace-selection";
  import {
    createArchiveWorkspaceAction,
    createPageWorkspaceAction,
    createPartialWorkspaceAction,
    createSingleWorkspaceAction,
    includePartialWorkspaceAction,
    type PartialPreset,
    type WorkspaceTemplateActionResult,
  } from "$lib/source-graph/workspace-actions";
  import {
    initialSourceNodeIdForPath,
    sourceNodeById,
    sourcePageByNodeId,
    sourceRelationsFrom,
    sourceRelationsTo,
    sourceStyleByNodeId,
    sourceStylesForPage,
    sourceTemplateByNodeId,
    sourceTemplateChainForPage,
  } from "$lib/source-graph/view";
  import { queueFileBufferDraftTextTransitionForPath } from "$lib/session/file-buffer-draft-sync";
  import type { LoopDefinition } from "$lib/loops/model";
  import type {
    FontInventory,
    LocalFontFamily,
    ScssVariable,
    SourceGraph,
    SourceGraphPage,
  } from "$lib/types";
  import { errorMessage } from "$lib/util";

  type Props = {
    currentProjectPath?: string;
    runtimeSessionId?: string;
    projectSessionEpoch?: number;
    projectTransitionFrontendLeaseActive?: boolean;
    kernelUndoRedoFrontendLeaseActive?: boolean;
    activePath?: string | null;
    selectedSourceId?: string | null;
    refreshToken?: number;
    previewSrc?: string;
    previewDocumentMarkup?: string | null;
    previewDevice?: "desktop" | "tablet" | "mobile";
    previewZoom?: number;
    tabletPreviewWidth?: string;
    mobilePreviewWidth?: string;
    sourceCache?: Record<string, string>;
    openFile?: (path: string) => void | Promise<void>;
    beginPreviewStructuralWriteBoundary?: () => Promise<void>;
    endPreviewStructuralWriteBoundary?: () => void;
    projectCommittedSiteStructure?: (
      lease: PreviewStructuralSessionLease,
      touchedFiles: string[],
      workspaceRevision: number,
      preferredRelativePath?: string | null,
    ) => Promise<SourceGraph | null>;
    openExternalRun?: (route?: string) => void | Promise<void>;
    onActiveRouteChange?: (route: string) => void;
    onStatusUpdate?: (text: string, kind: "idle" | "error") => void;
    loopDefinitions?: LoopDefinition[];
    onRegisterLoop?: (definition: LoopDefinition) => void;
    onRemoveLoop?: (id: string) => void;
  };

  type SiteActionPanel = "page" | "archive" | "single" | "component" | "loop" | null;

  let {
    currentProjectPath = "",
    runtimeSessionId = "",
    projectSessionEpoch = 0,
    projectTransitionFrontendLeaseActive = false,
    kernelUndoRedoFrontendLeaseActive = false,
    activePath = null,
    selectedSourceId = null,
    refreshToken = 0,
    previewSrc = "",
    previewDocumentMarkup: _previewDocumentMarkup = null,
    previewDevice: _previewDevice = "desktop",
    previewZoom: _previewZoom = 100,
    tabletPreviewWidth: _tabletPreviewWidth = "1024px",
    mobilePreviewWidth: _mobilePreviewWidth = "768px",
    sourceCache = {},
    openFile = () => {},
    beginPreviewStructuralWriteBoundary = async () => {
      throw new Error("Site Workspace nu are callback pentru bariera structurală.");
    },
    endPreviewStructuralWriteBoundary = () => {},
    projectCommittedSiteStructure = async () => {
      throw new Error("Site Workspace nu are callback pentru rescan-ul autoritar.");
    },
    openExternalRun = () => {},
    onActiveRouteChange = () => {},
    onStatusUpdate = () => {},
    loopDefinitions = [],
    onRegisterLoop = () => {},
    onRemoveLoop = () => {},
  }: Props = $props();

  const siteStructureHost: SiteStructureSessionHost = {
    get sessionProjectRoot() { return currentProjectPath; },
    get kernelProjectSessionId() { return runtimeSessionId; },
    get projectSessionEpoch() { return projectSessionEpoch; },
    get projectTransitionFrontendLeaseActive() { return projectTransitionFrontendLeaseActive; },
    get kernelUndoRedoFrontendLeaseActive() { return kernelUndoRedoFrontendLeaseActive; },
    beginPreviewStructuralWriteBoundary: () => beginPreviewStructuralWriteBoundary(),
    endPreviewStructuralWriteBoundary: () => endPreviewStructuralWriteBoundary(),
    async projectCommittedSiteStructure(lease, touchedFiles, workspaceRevision, preferredRelativePath) {
      const projectedGraph = await projectCommittedSiteStructure(
        lease,
        touchedFiles,
        workspaceRevision,
        preferredRelativePath,
      );
      requireCurrentPreviewStructuralSession(siteStructureHost, lease);
      if (!projectedGraph) throw new Error("Rescan-ul Site Workspace nu a proiectat Source Graph-ul autoritar.");
      graph = projectedGraph;
    },
  };

  let graph = $state<SourceGraph | null>(null);
  let scssVariables = $state<ScssVariable[]>([]);
  let scssVariablesError = $state("");
  let fontInventory = $state<FontInventory | null>(null);
  let fontInventoryError = $state("");
  let loading = $state(false);
  let loadError = $state("");
  let activeSection = $state<SiteWorkspaceSection>("overview");
  let activePageId = $state<string | null>(null);
  let selectedNodeId = $state<string | null>(null);
  let targetTemplateNodeId = $state<string | null>(null);
  let activeAction = $state<SiteActionPanel>(null);
  let lastLoadKey = "";
  let lastActivePath = "";
  let loadSerial = 0;
  let actionCallId = 0;
  let actionBusy = $state(false);
  let actionStatus = $state("");

  let pageTitle = $state("Despre noi");
  let pageSlug = $state("despre");
  let pageTemplateName = $state("page.html");
  let pageDraft = $state(true);
  let archiveTitle = $state("Blog");
  let archiveSlug = $state("blog");
  let archiveTemplateName = $state("blog.html");
  let singleTitle = $state("Primul articol");
  let singleSlug = $state("primul-articol");
  let singleSectionSlug = $state("blog");
  let singleTemplateName = $state("articol.html");
  let partialName = $state("cta");
  let partialPreset = $state<PartialPreset>("cta");
  const partialPresetOptions = [
    { value: "cta", label: "Îndemn la acțiune (CTA)" },
    { value: "header", label: "Antet" },
    { value: "footer", label: "Subsol" },
    { value: "generic", label: "Componentă generică" },
  ];

  const nodesById = $derived(sourceNodeById(graph));
  const selectedNode = $derived(selectedNodeId ? (nodesById.get(selectedNodeId) ?? null) : null);
  const selectedTechnicalPage = $derived(sourcePageByNodeId(graph, selectedNodeId));
  const activePage = $derived.by(() => {
    const pages = graph?.pages ?? [];
    return pages.find((page) => page.id === activePageId)
      ?? selectedTechnicalPage
      ?? pages.find((page) => page.pageKind === "home")
      ?? pages[0]
      ?? null;
  });
  const activePageTemplate = $derived(sourceTemplateByNodeId(graph, activePage?.templateNodeId ?? null));
  const selectedTemplate = $derived(sourceTemplateByNodeId(graph, selectedNodeId));
  const selectedStyle = $derived(sourceStyleByNodeId(graph, selectedNodeId));
  const activeTemplateChain = $derived(sourceTemplateChainForPage(graph, activePage));
  const activePageStyles = $derived(sourceStylesForPage(graph, activePage));
  const outgoingRelations = $derived(sourceRelationsFrom(graph, selectedNodeId));
  const incomingRelations = $derived(sourceRelationsTo(graph, selectedNodeId));
  const teraContext = $derived(buildTeraEditorContext(graph, selectedNodeId));
  const siteArchitecture = $derived(buildSiteArchitecture(graph));
  const compositionTemplate = $derived(targetTemplateForWorkspace(
    graph,
    targetTemplateNodeId,
    activePageTemplate ?? selectedTemplate,
  ));
  const compositionNodes = $derived(templateCompositionNodes(graph, compositionTemplate));
  const compositionTree = $derived(templateCompositionTree(compositionNodes));
  const semanticCompositionTree = $derived(semanticOutlineTree(compositionTree));
  const bodyStructureZones = $derived(
    semanticCompositionTree.find((item) => item.id === "semantic:body")?.children ?? semanticCompositionTree,
  );
  const fontRoleRows = $derived(buildFontRoleRows(scssVariables));
  const visitorPreviewUrl = $derived(visitorPreviewUrlForSourcePage(previewSrc, activePage));

  $effect(() => {
    onActiveRouteChange(activePage?.url ?? "/");
  });

  function initialPageForGraph(sourceGraph: SourceGraph, nodeId: string | null) {
    return sourcePageByNodeId(sourceGraph, nodeId)
      ?? sourceGraph.pages.find((page) => page.pageKind === "home")
      ?? sourceGraph.pages[0]
      ?? null;
  }

  async function loadGraph() {
    if (!currentProjectPath || !runtimeSessionId) {
      loadSerial += 1;
      graph = null;
      scssVariables = [];
      scssVariablesError = "";
      fontInventory = null;
      fontInventoryError = "";
      selectedNodeId = null;
      activePageId = null;
      loadError = "";
      return;
    }
    if (projectTransitionFrontendLeaseActive || kernelUndoRedoFrontendLeaseActive) {
      loadSerial += 1;
      loading = false;
      return;
    }

    const serial = ++loadSerial;
    const expectedRoot = currentProjectPath;
    const expectedSessionId = runtimeSessionId;
    const expectedEpoch = projectSessionEpoch;
    const cssIdentity = createCssRequestIdentity(expectedRoot, expectedSessionId);
    loading = true;
    loadError = "";

    try {
      const [nextGraph, variablesResult, inventoryResult] = await Promise.all([
        readSourceGraph({ expectedProjectRoot: expectedRoot, expectedSessionId }),
        getScssVariables(cssIdentity).then(
          (value) => ({ value, error: "" }),
          (error) => ({ value: [] as ScssVariable[], error: errorMessage(error) }),
        ),
        getFontInventory().then(
          (value) => ({ value, error: "" }),
          (error) => ({ value: null as FontInventory | null, error: errorMessage(error) }),
        ),
      ]);
      if (!siteWorkspaceLoadLeaseMatches(serial, expectedRoot, expectedSessionId, expectedEpoch)) return;
      const initialNodeId = initialSourceNodeIdForPath(nextGraph, activePath);
      const preservedPage = nextGraph.pages.find((page) => page.id === activePageId) ?? initialPageForGraph(nextGraph, initialNodeId);
      graph = nextGraph;
      scssVariables = variablesResult.value;
      scssVariablesError = variablesResult.error;
      fontInventory = inventoryResult.value;
      fontInventoryError = inventoryResult.error;
      selectedNodeId = initialNodeId ?? preservedPage?.id ?? null;
      activePageId = preservedPage?.id ?? null;
    } catch (error) {
      if (!siteWorkspaceLoadLeaseMatches(serial, expectedRoot, expectedSessionId, expectedEpoch)) return;
      loadError = errorMessage(error);
      graph = null;
      selectedNodeId = null;
      activePageId = null;
      onStatusUpdate(`Harta website-ului nu a putut fi citită: ${loadError}`, "error");
    } finally {
      if (siteWorkspaceLoadLeaseMatches(serial, expectedRoot, expectedSessionId, expectedEpoch)) loading = false;
    }
  }

  function siteWorkspaceLoadLeaseMatches(serial: number, root: string, sessionId: string, epoch: number) {
    return serial === loadSerial
      && !projectTransitionFrontendLeaseActive
      && !kernelUndoRedoFrontendLeaseActive
      && cssRequestIdentityMatches(
        { expectedProjectRoot: root, expectedSessionId: sessionId },
        currentProjectPath,
        runtimeSessionId,
      )
      && projectSessionEpoch === epoch;
  }

  function selectPage(page: SourceGraphPage) {
    activePageId = page.id;
    selectedNodeId = page.id;
    targetTemplateNodeId = page.templateNodeId;
  }

  function selectNode(nodeId: string | null) {
    if (!nodeId) return;
    selectedNodeId = nodeId;
  }

  function selectZone(zone: SemanticOutlineItem) {
    const node = firstSourceNodeInOutline(zone) ?? zone.node;
    if (node) selectedNodeId = node.id;
  }

  function openSection(section: SiteWorkspaceSection) {
    activeSection = section;
    activeAction = null;
  }

  function openAction(panel: Exclude<SiteActionPanel, null>) {
    activeAction = activeAction === panel ? null : panel;
  }

  async function openSource(path: string | null | undefined) {
    if (path) await openFile(path);
  }

  function openActivePageContent(page: SourceGraphPage) {
    return openSource(page.file);
  }

  function registerLoopDefinition(definition: LoopDefinition) {
    onRegisterLoop(definition);
    onStatusUpdate(`Lista dinamică „${definition.label}” este pregătită în panoul Adaugă.`, "idle");
  }

  function removeLoopDefinition(id: string) {
    onRemoveLoop(id);
    onStatusUpdate("Lista dinamică a fost eliminată din panoul Adaugă.", "idle");
  }

  function sourceVariableForRole(role: FontRoleRow) {
    if (!role.variable) return null;
    return scssVariables.find((variable) => variable.file === role.variable?.file && variable.name === role.variable?.name) ?? role.variable;
  }

  function currentProjectSource(relativePath: string, diskSource: string) {
    const cachedSource = sourceCache[scannedCacheKey({ relativePath })];
    return typeof cachedSource === "string" ? cachedSource : diskSource;
  }

  async function updateScssVariableValue(variable: ScssVariable, value: string) {
    const nextValue = value.trim();
    if (!nextValue || nextValue === variable.value) return;
    const identity = createCssRequestIdentity(currentProjectPath, runtimeSessionId);
    try {
      await setScssVariable(variable.file, variable.name, nextValue, identity);
      if (!cssRequestIdentityMatches(identity, currentProjectPath, runtimeSessionId)) return;
      scssVariables = scssVariables.map((entry) => (
        entry.file === variable.file && entry.name === variable.name ? { ...entry, value: nextValue } : entry
      ));
      onStatusUpdate(`Culoarea ${variable.name} a fost actualizată în sesiune. Apasă Save pentru persistență.`, "idle");
    } catch (error) {
      onStatusUpdate(`Culoarea nu a putut fi actualizată: ${errorMessage(error)}`, "error");
    }
  }

  async function updateFontRole(role: FontRoleRow, family: LocalFontFamily) {
    const variable = sourceVariableForRole(role);
    if (!variable) {
      onStatusUpdate(`Rolul ${role.label} nu are încă variabilă SCSS detectată.`, "error");
      return;
    }
    const targetFile = fontFaceTargetForVariable(variable);
    if (!targetFile) {
      onStatusUpdate(`Nu am găsit fișierul SCSS de bază pentru rolul ${role.label}.`, "error");
      return;
    }
    await draftFontFaceCss(family.family, buildManagedFontFaceBlock(family), targetFile);
    const nextValue = fontStackForFamily(role.id, family.family, role.variable?.value ?? variable.value);
    const identity = createCssRequestIdentity(currentProjectPath, runtimeSessionId);
    await setScssVariable(variable.file, variable.name, nextValue, identity);
    if (!cssRequestIdentityMatches(identity, currentProjectPath, runtimeSessionId)) return;
    scssVariables = scssVariables.map((entry) => (
      entry.file === variable.file && entry.name === variable.name ? { ...entry, value: nextValue } : entry
    ));
    onStatusUpdate(`Fontul ${family.family} a fost pregătit pentru ${role.label}. Apasă Save pentru persistență.`, "idle");
  }

  function fontFaceTargetFile() {
    const targetVariable = fontRoleRows.find((role) => role.variable)?.variable ?? null;
    return fontFaceTargetForVariable(targetVariable);
  }

  async function draftFontFaceCss(familyName: string, cssBlock: string, targetOverride: string | null = null) {
    const targetFile = targetOverride ?? fontFaceTargetFile();
    if (!targetFile) {
      onStatusUpdate("Nu am găsit fișierul SCSS de bază pentru @font-face.", "error");
      return false;
    }
    const diskSource = await readProjectFile(targetFile).catch(() => "");
    const currentSource = currentProjectSource(targetFile, diskSource);
    const nextSource = upsertManagedFontFaceCssBlock(currentSource, familyName, cssBlock);
    if (nextSource === currentSource) {
      onStatusUpdate(`@font-face pentru ${familyName} există deja.`, "idle");
      return false;
    }
    queueFileBufferDraftTextTransitionForPath(targetFile, currentSource, nextSource, "site_workspace.font_face");
    onStatusUpdate(`@font-face pentru ${familyName} este pregătit în ${targetFile}. Apasă Save.`, "idle");
    return true;
  }

  async function generateFontFaceDraft(family: LocalFontFamily) {
    await draftFontFaceCss(family.family, buildManagedFontFaceBlock(family));
  }

  async function generateFontPreloadDraft() {
    const files = activeFontPreloadFiles(fontRoleRows, fontInventory);
    if (!files.length) {
      onStatusUpdate("Nu am găsit fișiere locale pentru fonturile active.", "error");
      return;
    }
    const target = await fontPreloadDraftTarget();
    if (!target) {
      onStatusUpdate("Nu am găsit un template cu <head> pentru preload-uri.", "error");
      return;
    }
    const nextSource = upsertFontPreloadBlock(target.currentSource, files);
    if (nextSource === target.currentSource) {
      onStatusUpdate("Preload-urile fonturilor sunt deja pregătite.", "idle");
      return;
    }
    queueFileBufferDraftTextTransitionForPath(target.file, target.currentSource, nextSource, "site_workspace.font_preload");
    onStatusUpdate(`${files.length} preload-uri de font sunt pregătite în ${target.file}. Apasă Save.`, "idle");
  }

  async function fontPreloadDraftTarget() {
    for (const template of fontPreloadTargetCandidates(graph)) {
      const diskSource = await readProjectFile(template.file).catch(() => "");
      const currentSource = currentProjectSource(template.file, diskSource);
      if (/<\/head>/i.test(currentSource)) return { file: template.file, currentSource };
    }
    return null;
  }

  async function downloadGoogleFontAction(family: string, weights: number[], variable: boolean) {
    if (!fontFaceTargetFile()) {
      onStatusUpdate("Nu am găsit fișierul SCSS de bază pentru @font-face.", "error");
      return;
    }
    onStatusUpdate(`Descarc fontul ${family} din Google Fonts...`, "idle");
    try {
      const result = await downloadGoogleFontFamily(family, weights, variable);
      await draftFontFaceCss(result.family.family, result.fontFaceCss);
      fontInventory = await getFontInventory().catch(() => fontInventory);
      onStatusUpdate(`Fontul ${result.family.family} a fost descărcat local și așteaptă Save.`, "idle");
    } catch (error) {
      onStatusUpdate(`Fontul nu a putut fi descărcat: ${errorMessage(error)}`, "error");
    }
  }

  function searchGoogleFontsAction(query: string, limit = 40, offset = 0) {
    return searchGoogleFonts(query, limit, offset);
  }

  async function runSiteStructureAction(
    pendingMessage: string,
    operation: () => Promise<WorkspaceTemplateActionResult | undefined>,
    projectResult?: (result: WorkspaceTemplateActionResult) => void,
  ) {
    const callId = ++actionCallId;
    const uiLease = captureSiteActionUiLease(siteStructureHost, callId);
    actionBusy = true;
    actionStatus = pendingMessage;
    try {
      const result = await operation();
      if (!result || !siteActionUiLeaseMatches(siteStructureHost, uiLease, actionCallId)) return;
      projectResult?.(result);
      if (!siteActionUiLeaseMatches(siteStructureHost, uiLease, actionCallId)) return;
      actionStatus = result.message;
      activeAction = null;
      onStatusUpdate(result.message, "idle");
    } catch (error) {
      if (!siteActionUiLeaseMatches(siteStructureHost, uiLease, actionCallId)) return;
      actionStatus = errorMessage(error);
      onStatusUpdate(`Website: ${actionStatus}`, "error");
    } finally {
      if (siteActionUiLeaseMatches(siteStructureHost, uiLease, actionCallId)) actionBusy = false;
    }
  }

  async function createPageAction() {
    await runSiteStructureAction("Se creează pagina...", () => createPageWorkspaceAction(siteStructureHost, {
      pageTitle,
      pageSlug,
      pageTemplateName,
      pageDraft,
      context: { targetTemplate: compositionTemplate, activeTheme: graph?.activeTheme },
    }));
  }

  async function createArchiveAction() {
    await runSiteStructureAction("Se creează secțiunea...", () => createArchiveWorkspaceAction(siteStructureHost, {
      archiveTitle,
      archiveSlug,
      archiveTemplateName,
      context: { targetTemplate: compositionTemplate, activeTheme: graph?.activeTheme },
    }), (result) => { singleSectionSlug = result.singleSectionSlug ?? singleSectionSlug; });
  }

  async function createSingleAction() {
    await runSiteStructureAction("Se creează pagina din secțiune...", () => createSingleWorkspaceAction(siteStructureHost, {
      singleSectionSlug,
      singleTitle,
      singleSlug,
      singleTemplateName,
      context: { targetTemplate: compositionTemplate, activeTheme: graph?.activeTheme },
    }));
  }

  async function createPartialAction() {
    await runSiteStructureAction("Se creează componenta reutilizabilă...", () => createPartialWorkspaceAction(siteStructureHost, {
      partialName,
      partialPreset,
      context: { targetTemplate: compositionTemplate, activeTheme: graph?.activeTheme },
    }));
  }

  async function includePartialAction() {
    if (!compositionTemplate || !partialTemplateName(partialName)) {
      actionStatus = "Alege un template și un nume valid pentru componentă.";
      return;
    }
    await runSiteStructureAction("Se creează și se include componenta...", () => includePartialWorkspaceAction(siteStructureHost, {
      partialName,
      partialPreset,
      targetTemplate: compositionTemplate,
      activeTheme: graph?.activeTheme,
    }));
  }

  let lastActionSessionKey = "";
  $effect(() => {
    const key = `${currentProjectPath}\u0000${runtimeSessionId}\u0000${projectSessionEpoch}\u0000${projectTransitionFrontendLeaseActive}\u0000${kernelUndoRedoFrontendLeaseActive}`;
    if (key === lastActionSessionKey) return;
    lastActionSessionKey = key;
    actionCallId += 1;
    actionBusy = false;
    actionStatus = "";
  });

  $effect(() => {
    const key = `${currentProjectPath}|${runtimeSessionId}|${projectSessionEpoch}|${projectTransitionFrontendLeaseActive}|${kernelUndoRedoFrontendLeaseActive}|${refreshToken}`;
    if (key === lastLoadKey || actionBusy) return;
    lastLoadKey = key;
    void loadGraph();
  });

  $effect(() => {
    const key = activePath ?? "";
    if (!graph || key === lastActivePath) return;
    lastActivePath = key;
    const nextNodeId = initialSourceNodeIdForPath(graph, activePath);
    if (!nextNodeId) return;
    selectedNodeId = nextNodeId;
    const page = sourcePageByNodeId(graph, nextNodeId);
    if (page) activePageId = page.id;
  });

  $effect(() => {
    if (!graph || !selectedSourceId || !nodesById.has(selectedSourceId)) return;
    selectedNodeId = selectedSourceId;
    const page = sourcePageByNodeId(graph, selectedSourceId);
    if (page) activePageId = page.id;
  });

  $effect(() => {
    if (!graph) {
      targetTemplateNodeId = null;
      return;
    }
    const targetExists = targetTemplateNodeId
      ? graph.templates.some((template) => template.nodeId === targetTemplateNodeId && !template.isPartial)
      : false;
    if (!targetExists) targetTemplateNodeId = activePage?.templateNodeId ?? null;
  });
</script>

<section class="site-workspace" aria-label="Website Builder">
  <header class="workspace-header">
    <div class="workspace-title">
      <span class="workspace-mark"><IconSitemap size={18} stroke={1.9} /></span>
      <div><small>Website Builder</small><strong>{activePage?.title ?? "Site"}</strong></div>
    </div>
    <div class="current-route"><IconWorld size={15} stroke={1.9} /><span>{activePage?.url ?? "/"}</span></div>
    <div class="header-actions">
      <button type="button" onclick={() => openAction("page")}><IconPlus size={15} /> Pagină</button>
      <button type="button" onclick={() => openAction("component")}><IconPlus size={15} /> Componentă</button>
      <button class="icon-button" type="button" title="Reîncarcă harta site-ului" aria-label="Reîncarcă harta site-ului" onclick={loadGraph}><IconRefresh size={16} /></button>
    </div>
  </header>

  {#if loading}
    <div class="workspace-state">Se construiește harta website-ului…</div>
  {:else if loadError}
    <div class="workspace-state error"><strong>Website-ul nu a putut fi analizat</strong><span>{loadError}</span><button type="button" onclick={loadGraph}>Încearcă din nou</button></div>
  {:else if graph}
    <div class:drawer-open={activeAction} class="workspace-body">
      <SiteWorkspaceSidebar
        siteTitle={graph.pages.find((page) => page.pageKind === "home")?.title ?? activePage?.title ?? "Website"}
        themeName={graph.activeTheme}
        pages={graph.pages}
        {activeSection}
        {activePageId}
        onSectionChange={openSection}
        onPageSelect={(page) => { selectPage(page); activeSection = "pages"; }}
        onCreatePage={() => openAction("page")}
      />

      <main class="workspace-stage">
        {#if activeSection === "overview"}
          <SiteOverviewPanel
            page={activePage}
            pages={graph.pages}
            templateCount={graph.templates.filter((template) => !template.isPartial).length}
            styleCount={graph.styles.length}
            reusableParts={siteArchitecture.reusableParts}
            diagnostics={graph.diagnostics}
            previewUrl={visitorPreviewUrl}
            onOpenPageSource={openActivePageContent}
            onOpenExternal={() => openExternalRun(activePage?.url ?? "/")}
            onNavigate={openSection}
          />
        {:else if activeSection === "pages"}
          <SitePagesPanel
            page={activePage}
            templateChain={activeTemplateChain}
            styles={activePageStyles}
            previewUrl={visitorPreviewUrl}
            onCreatePage={() => openAction("page")}
            onOpenContent={openActivePageContent}
            onOpenSource={openSource}
            onOpenStructure={() => openSection("structure")}
            onOpenExternal={() => openExternalRun(activePage?.url ?? "/")}
          />
        {:else if activeSection === "structure"}
          <SiteStructurePanel
            page={activePage}
            zones={bodyStructureZones}
            {selectedNodeId}
            templateChain={activeTemplateChain}
            styles={activePageStyles}
            reusableParts={siteArchitecture.reusableParts}
            previewUrl={visitorPreviewUrl}
            onSelectZone={selectZone}
            onSelectNode={selectNode}
            onOpenSource={openSource}
            onCreateComponent={() => openAction("component")}
            onCreateLoop={() => openAction("loop")}
            onOpenExternal={() => openExternalRun(activePage?.url ?? "/")}
          />
        {:else if activeSection === "design"}
          <SiteDesignPanel
            variables={scssVariables}
            variablesError={scssVariablesError}
            styles={graph.styles}
            fontRoles={fontRoleRows}
            {fontInventory}
            {fontInventoryError}
            onOpenSource={openSource}
            onUpdateVariable={updateScssVariableValue}
            onRoleFamilyChange={updateFontRole}
            onGenerateFontFace={generateFontFaceDraft}
            onGenerateFontPreloads={generateFontPreloadDraft}
            onDownloadGoogleFont={downloadGoogleFontAction}
            onSearchGoogleFonts={searchGoogleFontsAction}
          />
        {:else}
          <SiteSourcesPanel
            {graph}
            page={activePage}
            {selectedNode}
            {outgoingRelations}
            {incomingRelations}
            impactLabel={teraContext.impactLabel}
            editabilityLabel={teraContext.editabilityLabel}
            structureLabel={teraContext.structureLabel}
            onSelectNode={selectNode}
            onOpenSource={openSource}
          />
        {/if}
      </main>

      {#if activeAction}
        <aside class="action-drawer" aria-label="Creează în website">
          <header>
            <div><small>Adaugă în website</small><h2>{activeAction === "page" ? "Pagină nouă" : activeAction === "archive" ? "Secțiune de conținut" : activeAction === "single" ? "Pagină din secțiune" : activeAction === "loop" ? "Listă dinamică" : "Componentă reutilizabilă"}</h2></div>
            <button class="icon-button" type="button" aria-label="Închide" onclick={() => (activeAction = null)}><IconX size={17} /></button>
          </header>

          {#if activeAction === "page"}
            <div class="action-panel">
              <p>Creează o pagină individuală cu adresă, conținut și template propriu.</p>
              <label><span>Titlul paginii</span><input bind:value={pageTitle} placeholder="Despre noi" disabled={actionBusy} /></label>
              <label><span>Adresă (slug)</span><input bind:value={pageSlug} placeholder="despre" disabled={actionBusy} /></label>
              <label><span>Aspect inițial</span><input bind:value={pageTemplateName} placeholder="page.html" disabled={actionBusy} /></label>
              <label class="checkbox-line"><input type="checkbox" bind:checked={pageDraft} disabled={actionBusy} /><span>Păstrează pagina ca draft</span></label>
              <button class="primary-action" type="button" disabled={actionBusy} onclick={createPageAction}>Creează pagina</button>
              <div class="secondary-actions"><button type="button" onclick={() => (activeAction = "archive")}>Creează o secțiune</button><button type="button" onclick={() => (activeAction = "single")}>Creează o pagină din secțiune</button></div>
            </div>
          {:else if activeAction === "archive"}
            <div class="action-panel">
              <p>O secțiune grupează articole sau alte pagini, de exemplu Blog, Servicii ori Proiecte.</p>
              <label><span>Numele secțiunii</span><input bind:value={archiveTitle} placeholder="Blog" disabled={actionBusy} /></label>
              <label><span>Adresa secțiunii</span><input bind:value={archiveSlug} placeholder="blog" disabled={actionBusy} /></label>
              <label><span>Template pentru listă</span><input bind:value={archiveTemplateName} placeholder="blog.html" disabled={actionBusy} /></label>
              <button class="primary-action" type="button" disabled={actionBusy} onclick={createArchiveAction}>Creează secțiunea</button>
              <button type="button" onclick={() => (activeAction = "page")}>Înapoi la pagină simplă</button>
            </div>
          {:else if activeAction === "single"}
            <div class="action-panel">
              <p>Creează un exemplu de pagină individuală într-o secțiune existentă.</p>
              <label><span>Secțiune</span><input bind:value={singleSectionSlug} placeholder="blog" disabled={actionBusy} /></label>
              <label><span>Titlu exemplu</span><input bind:value={singleTitle} placeholder="Primul articol" disabled={actionBusy} /></label>
              <label><span>Adresă exemplu</span><input bind:value={singleSlug} placeholder="primul-articol" disabled={actionBusy} /></label>
              <label><span>Template pagină</span><input bind:value={singleTemplateName} placeholder="articol.html" disabled={actionBusy} /></label>
              <button class="primary-action" type="button" disabled={actionBusy} onclick={createSingleAction}>Creează pagina din secțiune</button>
              <button type="button" onclick={() => (activeAction = "page")}>Înapoi la pagină simplă</button>
            </div>
          {:else if activeAction === "component"}
            <div class="action-panel">
              <p>Componentele reutilizabile pot fi folosite în mai multe pagini: antet, subsol, navigație sau CTA.</p>
              <label><span>Numele componentei</span><input bind:value={partialName} placeholder="cta" disabled={actionBusy} /></label>
              <label><span>Tip inițial</span><SelectControl value={partialPreset} options={partialPresetOptions} disabled={actionBusy} ariaLabel="Tip componentă" onchange={(value) => (partialPreset = value as PartialPreset)} /></label>
              <div class="target-template"><span>Pagina primește componenta în</span><strong>{compositionTemplate?.name ?? "Niciun template selectat"}</strong></div>
              <button class="primary-action" type="button" disabled={actionBusy} onclick={createPartialAction}>Creează componenta</button>
              <button type="button" disabled={actionBusy || !compositionTemplate} onclick={includePartialAction}>Creează și include la finalul template-ului</button>
              <small class="placement-note">Poziționarea fină se face apoi vizual în Canvas sau în cod. Workspace-ul nu mută componente prin drag-and-drop.</small>
            </div>
          {:else if activeAction === "loop"}
            <LoopBuilderPanel definitions={loopDefinitions} onRegisterLoop={registerLoopDefinition} onRemoveLoop={removeLoopDefinition} />
          {/if}
          {#if actionStatus}<p class="action-status">{actionStatus}</p>{/if}
        </aside>
      {/if}
    </div>
  {:else}
    <div class="workspace-state"><strong>Site fără structură detectată</strong><span>Inițializează sau deschide un proiect Zola pentru a continua.</span></div>
  {/if}
</section>

<style>
  .site-workspace {
    position: relative;
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface);
    box-shadow: var(--shadow);
  }

  .workspace-header {
    display: grid;
    grid-template-columns: minmax(220px, 310px) minmax(180px, 1fr) auto;
    gap: 12px;
    align-items: center;
    flex: 0 0 auto;
    min-height: 58px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border-2);
    background: color-mix(in srgb, var(--surface) 88%, var(--surface-3));
  }

  .workspace-title {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 9px;
    align-items: center;
    min-width: 0;
  }

  .workspace-mark {
    display: grid;
    width: 34px;
    height: 34px;
    place-items: center;
    border-radius: 10px;
    color: var(--brand);
    background: color-mix(in srgb, var(--brand) 11%, var(--surface));
  }

  .workspace-title > div {
    display: grid;
    gap: 1px;
    min-width: 0;
  }

  .workspace-title small,
  .action-drawer header small,
  .target-template span {
    color: var(--brand);
    font-size: 9px;
    font-weight: 900;
    letter-spacing: .08em;
    text-transform: uppercase;
  }

  .workspace-title strong {
    overflow: hidden;
    color: var(--text-strong);
    font-size: 14px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .current-route {
    display: flex;
    align-items: center;
    justify-self: center;
    gap: 7px;
    width: min(520px, 100%);
    height: 33px;
    padding: 0 10px;
    border: 1px solid var(--border-2);
    border-radius: 8px;
    color: var(--text-muted);
    background: var(--surface-2);
    font-family: var(--font-mono, monospace);
    font-size: 11px;
    font-weight: 700;
  }

  .header-actions,
  .secondary-actions {
    display: flex;
    gap: 6px;
  }

  button {
    border: 1px solid var(--border);
    border-radius: 8px;
    color: var(--text-strong);
    background: var(--surface);
    font: inherit;
    cursor: pointer;
  }

  button:hover:not(:disabled) {
    border-color: color-mix(in srgb, var(--brand) 55%, var(--border));
  }

  button:disabled {
    cursor: not-allowed;
    opacity: .52;
  }

  .header-actions button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    min-height: 33px;
    padding: 0 9px;
    font-size: 10px;
    font-weight: 850;
  }

  .header-actions .icon-button,
  .icon-button {
    display: grid;
    width: 33px;
    min-width: 33px;
    height: 33px;
    place-items: center;
    padding: 0;
  }

  .workspace-body {
    display: grid;
    grid-template-columns: minmax(245px, 280px) minmax(0, 1fr);
    flex: 1 1 auto;
    min-width: 0;
    min-height: 0;
  }

  .workspace-body.drawer-open {
    grid-template-columns: minmax(235px, 260px) minmax(0, 1fr) minmax(300px, 360px);
  }

  .workspace-stage {
    min-width: 0;
    min-height: 0;
    overflow: auto;
    background: var(--surface);
  }

  .workspace-state {
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 7px;
    flex: 1 1 auto;
    min-height: 0;
    padding: 24px;
    color: var(--text-muted);
    font-size: 12px;
    text-align: center;
  }

  .workspace-state strong {
    color: var(--text-strong);
    font-size: 15px;
  }

  .workspace-state.error strong {
    color: #b91c1c;
  }

  .workspace-state button {
    min-height: 34px;
    padding: 0 11px;
    font-size: 11px;
    font-weight: 850;
  }

  .action-drawer {
    min-width: 0;
    min-height: 0;
    overflow: auto;
    border-left: 1px solid var(--border-2);
    background: var(--surface-2);
  }

  .action-drawer > header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    min-height: 64px;
    padding: 10px 12px;
    border-bottom: 1px solid var(--border-2);
  }

  .action-drawer h2 {
    margin: 2px 0 0;
    color: var(--text-strong);
    font-size: 16px;
  }

  .action-panel {
    display: grid;
    gap: 10px;
    padding: 13px;
  }

  .action-panel > p,
  .placement-note {
    color: var(--text-muted);
    font-size: 11px;
    line-height: 1.5;
  }

  .action-panel label {
    display: grid;
    gap: 5px;
    color: var(--text-muted);
    font-size: 10px;
    font-weight: 850;
  }

  .action-panel input:not([type="checkbox"]) {
    width: 100%;
    min-width: 0;
    height: 35px;
    padding: 0 9px;
    border: 1px solid var(--border);
    border-radius: 7px;
    outline: 0;
    color: var(--text);
    background: var(--surface);
    font: inherit;
    font-size: 11px;
  }

  .action-panel input:focus {
    border-color: var(--brand);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--brand) 14%, transparent);
  }

  .checkbox-line {
    display: flex !important;
    align-items: center;
    grid-template-columns: auto 1fr;
  }

  .action-panel > button,
  .secondary-actions button {
    min-height: 37px;
    padding: 0 10px;
    font-size: 10px;
    font-weight: 850;
  }

  .primary-action {
    border-color: var(--brand);
    color: #fff;
    background: var(--brand);
  }

  .secondary-actions button {
    flex: 1 1 0;
  }

  .target-template {
    display: grid;
    gap: 3px;
    padding: 10px;
    border: 1px solid var(--border-2);
    border-radius: 8px;
    background: var(--surface);
  }

  .target-template strong {
    overflow: hidden;
    color: var(--text-strong);
    font-family: var(--font-mono, monospace);
    font-size: 10px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .action-status {
    margin: 0 13px 13px;
    padding: 9px;
    border: 1px solid var(--border-2);
    border-radius: 8px;
    color: var(--text-muted);
    background: var(--surface);
    font-size: 10px;
    line-height: 1.45;
  }

  @media (max-width: 1180px) {
    .workspace-body,
    .workspace-body.drawer-open {
      grid-template-columns: 230px minmax(0, 1fr);
    }

    .action-drawer {
      position: absolute;
      z-index: 5;
      top: 58px;
      right: 0;
      bottom: 0;
      width: min(360px, 92vw);
      box-shadow: -18px 0 42px color-mix(in srgb, var(--text-strong) 14%, transparent);
    }
  }
</style>
