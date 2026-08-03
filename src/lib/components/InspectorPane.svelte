<script lang="ts">
  import type { EditorActionOutcome } from "$lib/editor-runtime/action-outcome";
  import { t } from "$lib/i18n/runtime.svelte";
  import {
    IconHierarchy3,
    IconPalette,
    IconPointerBolt,
  } from "@tabler/icons-svelte";
  import { tick, untrack } from "svelte";
  import HtmlPane from "$lib/components/inspector/HtmlPane.svelte";
  import JsPane  from "$lib/components/inspector/JsPane.svelte";
  import BlockPropertiesPane from "$lib/components/inspector/BlockPropertiesPane.svelte";
  import CssPane from "$lib/components/inspector/panes/CssPane.svelte";
  import SelectionSummaryCard from "$lib/components/inspector/SelectionSummaryCard.svelte";
  import TeraSourceCard from "$lib/components/inspector/TeraSourceCard.svelte";
  import MarkdownSourceCard from "$lib/components/inspector/MarkdownSourceCard.svelte";
  import type {
    CssMutationAuthorityReceipt,
    CssInspectorContextResolution,
    CssProperty,
    CssRuleContext,
    BlockSelectionContext,
    DynamicWidgetProperties,
    DynamicWidgetSelectionContext,
    DynamicWidgetSnapshot,
    EditableAttributes,
    EditorNavigationNode,
    InspectorHtmlPhysicalFacts,
    InspectorTab,
    InspectorPendingArea,
    InspectorSelectionSummarySnapshot,
    PageCssTarget,
    ProjectFile,
    ProjectZolaImageIntent,
    ScssVariable,
    SelectionMutationIdentity,
    SelectionSnapshot,
    SourceGraphNode,
    InstalledFontVariationAxis,
  } from "$lib/types";
  import {
    createCssRequestIdentity,
    cssRequestIdentityMatches,
    resolveCssInspectorContext,
    setCssRuleAtViewport,
    setPageCssRuleAtViewport,
    setReusableCssRuleAtViewport,
    type CssRequestIdentity,
    type CssViewport,
  } from "$lib/project/io";
  import { registerEditFlushHandler } from "$lib/session/edit-flush-registry";
  import { flushFileBufferDraftSync } from "$lib/session/file-buffer-draft-sync";
  import type {
    CssContinuousEditHandlers,
    CssPendingValueBaseline,
    CssPropertyEditController,
  } from "$lib/inspector/css-property-edit";
  import {
    captureCssPendingValueBaseline,
    restoreCssPendingValueBaseline,
  } from "$lib/inspector/css-property-edit";
  import type { ApplyNativeBlockOptionRequest } from "$lib/state/html-actions-controller";
  import type { MotionWorkspaceState } from "$lib/state/motion-workspace.svelte";
  import {
    advanceStableHtmlInspectorProjection,
    type StableHtmlInspectorProjection,
  } from "$lib/inspector/html-projection-stability";
  import {
    cssInspectorSubjectKey,
    cssSemanticSelectionKey,
    sameCssSemanticSelection,
  } from "$lib/inspector/css-selection-stability";

  function captureCssIdentity(): CssRequestIdentity {
    return createCssRequestIdentity(projectRoot, runtimeSessionId);
  }

  function isCurrentCssIdentity(identity: CssRequestIdentity): boolean {
    return cssRequestIdentityMatches(identity, projectRoot, runtimeSessionId);
  }

  let queuedCssRuleMutationCount = 0;

  function enqueueCssWorkspaceMutation(
    identity: CssRequestIdentity,
    label: string,
    liveEpoch: number | null,
    mutation: () => Promise<{ authority: CssMutationAuthorityReceipt }>,
  ) {
    queuedCssRuleMutationCount += 1;
    updatePendingIndicators();
    const task = cssWorkspaceMutationTail.then(async () => {
      if (!isCurrentCssIdentity(identity)) return;
      await flushFileBufferDraftSync({ throwOnFailure: true });
      if (!isCurrentCssIdentity(identity)) return;
      const receipt = await mutation();
      if (!isCurrentCssIdentity(identity)) return;
      cssWorkspaceMutationFailure = "";
      onStatusUpdate?.(t("inspector-css-session-saved", { label }), "unsaved");
      if (!onCssWorkspaceMutationCommitted) return;
      try {
        await onCssWorkspaceMutationCommitted(receipt.authority, liveEpoch);
      } catch (error) {
        if (!isCurrentCssIdentity(identity)) return;
        const message = error instanceof Error ? error.message : String(error);
        onStatusUpdate?.(
          t("inspector-css-live-failed", { label, error: message }),
          "error",
        );
      }
    });
    cssWorkspaceMutationTail = task
      .catch((error) => {
        if (!isCurrentCssIdentity(identity)) return;
        if (liveEpoch !== null) onInspectorLivePropertiesRejected?.(liveEpoch);
        cssWorkspaceMutationFailure = error instanceof Error ? error.message : String(error);
        onStatusUpdate?.(t("inspector-css-mutation-failed", { label, error: cssWorkspaceMutationFailure }), "error");
      })
      .finally(() => {
        queuedCssRuleMutationCount = Math.max(0, queuedCssRuleMutationCount - 1);
        updatePendingIndicators();
      });
  }

  type StagedCssRuleMutation = {
    key: string;
    identity: CssRequestIdentity;
    label: string;
    liveEpoch: number | null;
    properties: Record<string, string>;
    baselines: Record<string, CssPendingValueBaseline>;
    run: (properties: Record<string, string>) => Promise<{ authority: CssMutationAuthorityReceipt }>;
  };

  const stagedCssRuleMutations = new Map<string, StagedCssRuleMutation>();
  let stagedCssFlushPromise: Promise<void> | null = null;
  let stagedCssFlushScheduled = false;

  function updatePendingIndicators() {
    onPendingChange?.("css", stagedCssRuleMutations.size > 0 || queuedCssRuleMutationCount > 0);
  }

  function stageCssRuleMutation(
    mutation: Omit<StagedCssRuleMutation, "properties" | "baselines">,
    property: string,
    value: string,
    baseline: CssPendingValueBaseline,
  ) {
    const current = stagedCssRuleMutations.get(mutation.key);
    stagedCssRuleMutations.set(mutation.key, {
      ...mutation,
      label: current?.label ?? mutation.label,
      properties: { ...(current?.properties ?? {}), [property]: value },
      baselines: {
        ...(current?.baselines ?? {}),
        [property]: current?.baselines[property] ?? baseline,
      },
    });
    updatePendingIndicators();
  }

  async function flushStagedCssPanelMutations() {
    if (stagedCssFlushPromise) return stagedCssFlushPromise;
    stagedCssFlushPromise = (async () => {
      while (stagedCssRuleMutations.size > 0) {
        const cssMutations = Array.from(stagedCssRuleMutations.values());
        stagedCssRuleMutations.clear();
        updatePendingIndicators();

        for (const entry of cssMutations) {
          enqueueCssWorkspaceMutation(entry.identity, entry.label, entry.liveEpoch, () =>
            entry.run(entry.properties));
        }
        await cssWorkspaceMutationTail;
      }
    })().finally(() => {
      stagedCssFlushPromise = null;
      updatePendingIndicators();
    });
    return stagedCssFlushPromise;
  }

  function scheduleStagedCssPanelFlush() {
    if (stagedCssFlushScheduled || stagedCssFlushPromise) return;
    if (stagedCssRuleMutations.size === 0) return;
    stagedCssFlushScheduled = true;
    queueMicrotask(() => {
      stagedCssFlushScheduled = false;
      void flushStagedCssPanelMutations();
    });
  }

  let {
    inspectorSelectionSummary = null,
    inspectorHtmlPhysicalFacts = null,
    inspectorBlockSelectionContext = null,
    inspectorDynamicWidgetSelectionContext = null,
    sourceGraph = null,
    projectRoot = "",
    runtimeSessionId = "",
    selectedTemplateSourceNode = null,
    selectedEditorNavigationNode = null,
    targetCssFile = "",
    selectionSnapshot = null,
    cssSourceRevision = 0,
    activeRenderedTemplatePath = null,
    previewDevice = "desktop" as CssViewport,
    refreshToken = 0,
    historyProjectionQuiesced = false,
    jsRefreshToken = 0,
    motionWorkspace,
    workspaceRevision = 0,
    previewRevision = "",
    blockPropertiesHeight = 220,
    blockPropertiesCollapsed = false,
    cachebustAssets = false,
    projectFiles = [],
    scssVariables = [],
    fontFamilies = [],
    installedFontAxes = [],
    attributeValues,
    attributeStatus = "",
    textContentValue = "",
    textStatus = "",
    classEditorValue = "",
    classStatus = "",
    imageSourceValue = "",
    imageStatus = "",
    scannedAssets = [],
    isActivePreviewHtmlSource,
    canEditHtml = false,
    updateAttributeValue,
    removeAttribute,
    applyAttributesToHtml,
    updateTextContentValue,
    applyTextContentToHtml,
    setClassEditorValue,
    applyClassesToHtml,
    generateClassForSelectedHtml,
    generateDataAnimForSelectedHtml,
    setImageSourceValue,
    applyZolaImageProcessingToHtml,
    cancelHtmlAttributeDraft,
    enterTeraBoundary,
    deleteSelectedTeraNode,
    openSelectedTeraSource,
    openSelectedMarkdownContent,
    pendingTag = null,
    tagStatus = "",
    changeElementTag,
    onLivePropertyChange,
    onLivePropertiesChange,
    onCssWorkspaceMutationCommitted,
    onInspectorLivePropertiesRejected,
    injectPreviewCss,
    onStatusUpdate,
    onPendingChange,
    onInspectorTabChange,
    beforeInspectorTabChange,
    onCssCodeTargetChange,
    getOpenCssRuleContext,
    applyNativeBlockOption,
    updateDynamicWidget,
    deleteDynamicWidget,
    persistBlockPropertiesLayout,
    gridOverlayEnabled = false,
    onGridOverlayChange,
  }: {
    inspectorSelectionSummary?: InspectorSelectionSummarySnapshot | null;
    inspectorHtmlPhysicalFacts?: InspectorHtmlPhysicalFacts | null;
    inspectorBlockSelectionContext?: BlockSelectionContext | null;
    inspectorDynamicWidgetSelectionContext?: DynamicWidgetSelectionContext | null;
    sourceGraph?: import("$lib/types").SourceGraph | null;
    projectRoot?: string;
    runtimeSessionId?: string;
    selectedTemplateSourceNode?: SourceGraphNode | null;
    selectedEditorNavigationNode?: EditorNavigationNode | null;
    targetCssFile?: string;
    selectionSnapshot?: SelectionSnapshot | null;
    cssSourceRevision?: number;
    activeRenderedTemplatePath?: string | null;
    previewDevice?: CssViewport;
    refreshToken?: number;
    historyProjectionQuiesced?: boolean;
    jsRefreshToken?: number;
    motionWorkspace: MotionWorkspaceState;
    workspaceRevision?: number;
    previewRevision?: string;
    blockPropertiesHeight?: number;
    blockPropertiesCollapsed?: boolean;
    cachebustAssets?: boolean;
    projectFiles?: ProjectFile[];
    scssVariables?: ScssVariable[];
    fontFamilies?: string[];
    installedFontAxes?: InstalledFontVariationAxis[];
    attributeValues: EditableAttributes;
    attributeStatus?: string;
    textContentValue?: string;
    textStatus?: string;
    classEditorValue?: string;
    classStatus?: string;
    imageSourceValue?: string;
    imageStatus?: string;
    scannedAssets?: ProjectFile[];
    isActivePreviewHtmlSource: boolean;
    canEditHtml?: boolean;
    updateAttributeValue: (property: string, value: string) => void;
    removeAttribute: (name: string) => void;
    applyAttributesToHtml: (attributes?: EditableAttributes) => void | Promise<EditorActionOutcome>;
    updateTextContentValue: (value: string, composing?: boolean) => void;
    applyTextContentToHtml: () => void | Promise<EditorActionOutcome>;
    setClassEditorValue: (value: string) => void;
    applyClassesToHtml: () => void | Promise<EditorActionOutcome>;
    generateClassForSelectedHtml: () => void | Promise<EditorActionOutcome>;
    generateDataAnimForSelectedHtml: () => void | Promise<EditorActionOutcome>;
    setImageSourceValue: (value: string) => void;
    applyZolaImageProcessingToHtml: (intent: ProjectZolaImageIntent) => void | Promise<EditorActionOutcome>;
    cancelHtmlAttributeDraft: (expectedContextKey?: string) => void;
    enterTeraBoundary: (scopeId: string) => void | Promise<void>;
    deleteSelectedTeraNode: () => void | Promise<void>;
    openSelectedTeraSource: () => void | Promise<void>;
    openSelectedMarkdownContent: () => void | Promise<void>;
    pendingTag?: string | null;
    tagStatus?: string;
    changeElementTag: (tag: string) => void;
    onLivePropertyChange?: (selector: string, property: string, value: string) => void;
    onLivePropertiesChange?: (
      selector: string | null,
      properties: Record<string, string>,
      viewport?: CssViewport,
    ) => number | void;
    onCssWorkspaceMutationCommitted?: (
      authority: CssMutationAuthorityReceipt,
      liveEpoch: number | null,
    ) => void | Promise<void>;
    onInspectorLivePropertiesRejected?: (liveEpoch: number) => void;
    injectPreviewCss?: (css: string) => void;
    onStatusUpdate?: (text: string, kind: string) => void;
    onPendingChange?: (area: InspectorPendingArea, pending: boolean) => void;
    onInspectorTabChange?: (tab: InspectorTab) => void;
    beforeInspectorTabChange?: (from: InspectorTab, to: InspectorTab) => void | Promise<void>;
    onCssCodeTargetChange?: (target: {
      selector: string;
      file: string;
      property?: string | null;
      expectedSelectionRevision?: number | null;
    }) => boolean | Promise<boolean>;
    getOpenCssRuleContext?: (file: string, selector: string, viewport: CssViewport) => CssRuleContext | null;
    applyNativeBlockOption: (request: ApplyNativeBlockOptionRequest) => Promise<EditorActionOutcome>;
    updateDynamicWidget: (
      snapshot: DynamicWidgetSnapshot,
      properties: DynamicWidgetProperties,
    ) => Promise<EditorActionOutcome>;
    deleteDynamicWidget: (snapshot: DynamicWidgetSnapshot) => Promise<EditorActionOutcome>;
    persistBlockPropertiesLayout?: (height: number, collapsed: boolean) => void;
    gridOverlayEnabled?: boolean;
    onGridOverlayChange?: (enabled: boolean) => void;
  } = $props();

  let stableHtmlProjection = $state<StableHtmlInspectorProjection | null>(null);
  let htmlProjectionPending = $state(false);

  $effect.pre(() => {
    const transition = advanceStableHtmlInspectorProjection(
      untrack(() => stableHtmlProjection),
      {
        summary: inspectorSelectionSummary,
        selection: selectionSnapshot,
        physicalFacts: inspectorHtmlPhysicalFacts,
        attributeValues,
        textContentValue,
        classEditorValue,
        imageSourceValue,
        pendingTag,
        attributeStatus,
        textStatus,
        classStatus,
        imageStatus,
        tagStatus,
        canEditHtml,
        isActivePreviewHtmlSource,
      },
    );
    stableHtmlProjection = transition.projection;
    htmlProjectionPending = transition.pending;
  });

  const presentedInspectorSelectionSummary = $derived(
    htmlProjectionPending && stableHtmlProjection
      ? stableHtmlProjection.summary
      : inspectorSelectionSummary,
  );
  const presentedSelectionSnapshot = $derived(
    htmlProjectionPending && stableHtmlProjection
      ? stableHtmlProjection.selection
      : selectionSnapshot,
  );
  const presentedHtmlPhysicalFacts = $derived(
    htmlProjectionPending && stableHtmlProjection
      ? stableHtmlProjection.physicalFacts
      : inspectorHtmlPhysicalFacts,
  );
  const presentedAttributeValues = $derived(
    htmlProjectionPending && stableHtmlProjection
      ? stableHtmlProjection.attributeValues
      : attributeValues,
  );
  const presentedTextContentValue = $derived(
    htmlProjectionPending && stableHtmlProjection
      ? stableHtmlProjection.textContentValue
      : textContentValue,
  );
  const presentedClassEditorValue = $derived(
    htmlProjectionPending && stableHtmlProjection
      ? stableHtmlProjection.classEditorValue
      : classEditorValue,
  );
  const presentedImageSourceValue = $derived(
    htmlProjectionPending && stableHtmlProjection
      ? stableHtmlProjection.imageSourceValue
      : imageSourceValue,
  );
  const presentedPendingTag = $derived(
    htmlProjectionPending && stableHtmlProjection
      ? stableHtmlProjection.pendingTag
      : pendingTag,
  );
  const presentedAttributeStatus = $derived(
    htmlProjectionPending && stableHtmlProjection
      ? stableHtmlProjection.attributeStatus
      : attributeStatus,
  );
  const presentedTextStatus = $derived(
    htmlProjectionPending && stableHtmlProjection
      ? stableHtmlProjection.textStatus
      : textStatus,
  );
  const presentedClassStatus = $derived(
    htmlProjectionPending && stableHtmlProjection
      ? stableHtmlProjection.classStatus
      : classStatus,
  );
  const presentedImageStatus = $derived(
    htmlProjectionPending && stableHtmlProjection
      ? stableHtmlProjection.imageStatus
      : imageStatus,
  );
  const presentedTagStatus = $derived(
    htmlProjectionPending && stableHtmlProjection
      ? stableHtmlProjection.tagStatus
      : tagStatus,
  );
  const presentedCanEditHtml = $derived(
    htmlProjectionPending && stableHtmlProjection
      ? stableHtmlProjection.canEditHtml
      : canEditHtml,
  );
  const presentedIsActivePreviewHtmlSource = $derived(
    htmlProjectionPending && stableHtmlProjection
      ? stableHtmlProjection.isActivePreviewHtmlSource
      : isActivePreviewHtmlSource,
  );

  // AppState resolves the canonical ProjectWorkspace target and remains the
  // authority. During a resolving frame the last complete projection stays
  // visually identical; the panel itself is inert until the atomic swap.
  const canEditHtmlEffective = $derived(presentedCanEditHtml);
  const hasTeraSelection = $derived(
    selectionSnapshot?.resolution === "resolved"
      && selectionSnapshot.subject?.kind === "teraBoundary",
  );
  const hasMarkdownSelection = $derived(
    selectionSnapshot?.resolution === "resolved"
      && selectionSnapshot.subject?.kind === "markdownBoundary",
  );
  const directAuthoringDocumentPath = $derived(
    hasTeraSelection
      && selectedEditorNavigationNode?.kind === "teraBoundary"
      && selectedEditorNavigationNode.sourceKind === "block"
      && selectedEditorNavigationNode.origin === "project"
      && selectedEditorNavigationNode.capabilities.requiresEditScopeId === null
      && selectedEditorNavigationNode.file === selectionSnapshot?.activeDocumentPath
        ? selectedEditorNavigationNode.file
        : null,
  );

  let inspectorTab = $state<InspectorTab>("html");
  let inspectorTabChangeSerial = 0;

  async function changeInspectorTab(nextTab: InspectorTab) {
    const previousTab = inspectorTab;
    if (nextTab === previousTab) return true;
    const serial = ++inspectorTabChangeSerial;
    try {
      await beforeInspectorTabChange?.(previousTab, nextTab);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      onStatusUpdate?.(t("inspector-tab-change-blocked", { error: message }), "error");
      return false;
    }
    if (serial !== inspectorTabChangeSerial || inspectorTab !== previousTab) return false;
    inspectorTab = nextTab;
    return true;
  }

  const inspectorTabs: InspectorTab[] = ["html", "css", "js"];

  async function focusInspectorTab(nextTab: InspectorTab) {
    if (!await changeInspectorTab(nextTab)) return;
    await tick();
    document.getElementById(`inspector-tab-${nextTab}`)?.focus();
  }

  function handleInspectorTabKeydown(event: KeyboardEvent, tab: InspectorTab) {
    const index = inspectorTabs.indexOf(tab);
    let nextIndex = index;
    if (event.key === "ArrowRight") nextIndex = (index + 1) % inspectorTabs.length;
    else if (event.key === "ArrowLeft") {
      nextIndex = (index - 1 + inspectorTabs.length) % inspectorTabs.length;
    } else if (event.key === "Home") nextIndex = 0;
    else if (event.key === "End") nextIndex = inspectorTabs.length - 1;
    else return;
    event.preventDefault();
    void focusInspectorTab(inspectorTabs[nextIndex]);
  }

  const coordinatedCssState = $derived(
    presentedSelectionSnapshot?.focus.kind === "cssRule"
      || presentedSelectionSnapshot?.focus.kind === "cssProperty"
      ? inspectorStateForCssSelector(presentedSelectionSnapshot.focus.selector)
      : null,
  );
  const selectedClass = $derived(coordinatedCssState?.selectedClass ?? null);
  const selectorSuffix = $derived(coordinatedCssState?.selectorSuffix ?? "");
  const customSuffix = $derived(coordinatedCssState?.customSuffix ?? "");
  const usingCustom = $derived(coordinatedCssState?.usingCustom ?? false);
  const activeSuffix = $derived(usingCustom ? customSuffix : selectorSuffix);
  const effectiveSelector = $derived(selectedClass ? "." + selectedClass + activeSuffix : null);
  const viewportLabel = $derived(
    previewDevice === "tablet" ? t("inspector-viewport-tablet")
      : previewDevice === "mobile" ? t("inspector-viewport-mobile")
        : t("inspector-viewport-desktop"),
  );

  let classRules = $state<CssProperty[]>([]);
  let cssRuleContext = $state<CssRuleContext | null>(null);
  let cssInspectorResolution = $state<CssInspectorContextResolution | null>(null);
  let cssInspectorSelectionIdentity = $state<SelectionMutationIdentity | null>(null);
  let loadingClassRules = $state(false);
  let pendingValues = $state<Record<string, string>>({});
  let cssWorkspaceMutationTail: Promise<void> = Promise.resolve();
  let cssWorkspaceMutationFailure = "";

  // Call ID guard — ensures only the latest loadRulesForClass call updates state
  let loadCallId = 0;
  let lastCssRuntimeKey = "";
  let lastCssSelectionKey = "";
  let lastHandledRefreshToken: number | null = null;
  let cssTargetInfo = $state<PageCssTarget | null>(null);

  $effect(() => registerEditFlushHandler("inspector-css-workspace", async () => {
    await flushStagedCssPanelMutations();
    await cssWorkspaceMutationTail;
    if (cssWorkspaceMutationFailure) {
      throw new Error(cssWorkspaceMutationFailure);
    }
  }));

  $effect(() => {
    const runtimeKey = `${projectRoot}\u0000${runtimeSessionId}`;
    if (runtimeKey === lastCssRuntimeKey) return;
    lastCssRuntimeKey = runtimeKey;
    loadCallId++;
    loadingClassRules = false;
    classRules = [];
    cssRuleContext = null;
    cssInspectorResolution = null;
    cssInspectorSelectionIdentity = null;
    cssTargetInfo = null;
    cssWorkspaceMutationFailure = "";
    queuedCssRuleMutationCount = 0;
    stagedCssRuleMutations.clear();
    updatePendingIndicators();
  });

  function applyLiveCssRuleContext(context: CssRuleContext) {
    cssRuleContext = context;
    classRules = context.hasViewportRule ? context.viewportRules : context.baseRules;
    loadingClassRules = false;
  }

  function applyCssInspectorResolution(
    resolution: CssInspectorContextResolution,
    expectedSelection: SelectionMutationIdentity,
  ) {
    cssInspectorResolution = resolution;
    cssInspectorSelectionIdentity = expectedSelection;
    cssTargetInfo = resolution.target;
    const resolvedContext = resolution.ruleContext;
    if (!resolvedContext) {
      cssRuleContext = null;
      classRules = [];
      loadingClassRules = false;
      return;
    }
    const liveContext = getOpenCssRuleContext?.(
      resolvedContext.file,
      resolvedContext.selector,
      resolvedContext.viewport,
    );
    if (!liveContext) {
      applyLiveCssRuleContext(resolvedContext);
      return;
    }
    const resolvedRules = resolvedContext.hasViewportRule
      ? resolvedContext.viewportRules
      : resolvedContext.baseRules;
    const liveRules = liveContext.hasViewportRule
      ? liveContext.viewportRules
      : liveContext.baseRules;
    const sameRules = JSON.stringify(liveRules) === JSON.stringify(resolvedRules);
    applyLiveCssRuleContext({
      ...liveContext,
      background: sameRules ? resolvedContext.background : liveContext.background,
      grid: sameRules ? resolvedContext.grid : liveContext.grid,
    });
  }

  function selectedTemplatePath() {
    return selectionSnapshot?.provenance?.definition?.file
      ?? selectionSnapshot?.provenance?.composition?.file
      ?? activeRenderedTemplatePath
      ?? null;
  }

  function captureCssSelectionIdentity(): SelectionMutationIdentity | null {
    const snapshot = selectionSnapshot;
    const anchor = snapshot?.anchor;
    if (
      !snapshot
      || snapshot.resolution !== "resolved"
      || snapshot.projectRoot !== projectRoot
      || snapshot.runtimeSessionId !== runtimeSessionId
      || !anchor
      || !Number.isSafeInteger(snapshot.selectionRevision)
      || snapshot.selectionRevision <= 0
    ) return null;
    const identity: SelectionMutationIdentity = Object.freeze({
      selectionRevision: snapshot.selectionRevision,
      editorNodeId: anchor.editorNodeId?.trim() || null,
      sourceNodeId: anchor.sourceNodeId?.trim() || null,
      renderInstanceId: anchor.renderInstanceId?.trim() || null,
    });
    if (!identity.editorNodeId && !identity.sourceNodeId && !identity.renderInstanceId) {
      return null;
    }
    return identity;
  }

  function isCurrentCssInspectorSubject(expected: SelectionMutationIdentity): boolean {
    const expectedKey = cssInspectorSubjectKey(expected);
    return Boolean(
      expectedKey
      && expectedKey === cssInspectorSubjectKey(captureCssSelectionIdentity()),
    );
  }

  function inspectorStateForCssSelector(selector: string) {
    const normalized = selector.trim();
    if (!normalized.startsWith(".")) return null;
    const withoutDot = normalized.slice(1);
    const simple = withoutDot.match(/^([A-Za-z_-][A-Za-z0-9_-]*)(.*)$/);
    if (!simple) {
      return {
        selectedClass: withoutDot,
        selectorSuffix: "",
        customSuffix: "",
        usingCustom: false,
      };
    }
    const suffix = simple[2] ?? "";
    if (!suffix) {
      return {
        selectedClass: simple[1],
        selectorSuffix: "",
        customSuffix: "",
        usingCustom: false,
      };
    }
    if (/[\s>+~.#\[]/.test(suffix)) {
      return {
        selectedClass: withoutDot,
        selectorSuffix: "",
        customSuffix: "",
        usingCustom: false,
      };
    }
    if ([":hover", ":focus", ":active"].includes(suffix)) {
      return {
        selectedClass: simple[1],
        selectorSuffix: suffix,
        customSuffix: "",
        usingCustom: false,
      };
    }
    return {
      selectedClass: simple[1],
      selectorSuffix: "",
      customSuffix: suffix,
      usingCustom: true,
    };
  }

  function pendingValuesForCurrentSelector() {
    return {};
  }

  $effect(() => {
    onInspectorTabChange?.(inspectorTab);
  });

  $effect(() => {
    if (
      selectionSnapshot?.focus.kind === "cssRule"
      || selectionSnapshot?.focus.kind === "cssProperty"
    ) inspectorTab = "css";
  });

  $effect(() => {
    const revision = cssSourceRevision;
    const selector = effectiveSelector;
    const file = targetCssFile;
    const viewport = previewDevice;
    const resolution = cssInspectorResolution;
    void revision;
    if (
      !selector
      || !file
      || resolution?.state === "ambiguous"
      || resolution?.target?.file !== file
      || resolution.selector !== selector
      || resolution.viewport !== viewport
    ) return;
    const context = getOpenCssRuleContext?.(file, selector, viewport);
    if (!context) return;
    applyLiveCssRuleContext(context);
    pendingValues = untrack(() => pendingValuesForCurrentSelector());
  });

  $effect(() => {
    const nextRefreshToken = refreshToken;
    if (historyProjectionQuiesced) return;
    if (lastHandledRefreshToken === null) {
      lastHandledRefreshToken = nextRefreshToken;
      return;
    }
    if (nextRefreshToken === lastHandledRefreshToken) return;
    lastHandledRefreshToken = nextRefreshToken;

    const classToRefresh = untrack(() => selectedClass);
    const suffixToRefresh = untrack(() => (usingCustom ? customSuffix : selectorSuffix));
    const selectorToRefresh = classToRefresh ? `.${classToRefresh}${suffixToRefresh}` : null;
    const fileToRefresh = untrack(() => targetCssFile);
    const viewportToRefresh = untrack(() => previewDevice);
    const keepClassSelected = Boolean(
      classToRefresh &&
      presentedInspectorSelectionSummary?.classes.includes(classToRefresh) &&
      selectorToRefresh &&
      fileToRefresh,
    );

    pendingValues = untrack(() => pendingValuesForCurrentSelector());
    untrack(() => {
      onPendingChange?.("css", false);
      onPendingChange?.("js", false);
    });

    if (keepClassSelected && selectorToRefresh && fileToRefresh) {
      untrack(() => {
        void loadRulesForClass(
          selectorToRefresh,
          fileToRefresh,
          viewportToRefresh,
          { preserveProjection: true },
        );
      });
      return;
    }

    loadingClassRules = false;
    classRules = [];
    cssRuleContext = null;
    cssInspectorResolution = null;
    cssInspectorSelectionIdentity = null;
    cssTargetInfo = null;
    loadCallId++;
  });

  $effect(() => {
    const projectionPending = htmlProjectionPending;
    const nextSelectionIdentity = captureCssSelectionIdentity();
    if (projectionPending && !nextSelectionIdentity) return;
    const nextSelectionKey = selectionSnapshot
      ? [
        selectionSnapshot.runtimeSessionId,
        cssInspectorSubjectKey(nextSelectionIdentity),
      ].join(":")
      : "";
    if (nextSelectionKey === lastCssSelectionKey) {
      if (
        nextSelectionIdentity
        && cssInspectorSelectionIdentity
        && (
          nextSelectionIdentity.selectionRevision
            !== cssInspectorSelectionIdentity.selectionRevision
          || !sameCssSemanticSelection(
            nextSelectionIdentity,
            cssInspectorSelectionIdentity,
          )
        )
      ) {
        cssInspectorSelectionIdentity = nextSelectionIdentity;
      }
      return;
    }

    lastCssSelectionKey = nextSelectionKey;
    loadingClassRules = false;
    classRules = [];
    cssRuleContext = null;
    cssInspectorResolution = null;
    cssInspectorSelectionIdentity = null;
    cssTargetInfo = null;
    pendingValues = untrack(() => pendingValuesForCurrentSelector());
    loadCallId++;
  });

  $effect(() => {
    const projectionQuiesced = historyProjectionQuiesced;
    const sel = effectiveSelector;
    const file = targetCssFile;
    const viewport = previewDevice;
    // ProjectSession face parte din cheia proiecției. O redeschidere la același
    // path trebuie să invalideze explicit citirea Inspectorului din runtime A.
    const sessionRoot = projectRoot;
    const sessionId = runtimeSessionId;
    if (projectionQuiesced) return;
    if (!sel || !file || !sessionRoot || !sessionId) {
      loadingClassRules = false;
      classRules = [];
      cssRuleContext = null;
      cssInspectorResolution = null;
      cssInspectorSelectionIdentity = null;
      cssTargetInfo = null;
      pendingValues = {};
      return;
    }
    pendingValues = untrack(() => pendingValuesForCurrentSelector());

    untrack(() => {
      void loadRulesForClass(sel, file, viewport);
    });
  });

  async function loadRulesForClass(
    selector: string,
    file: string,
    viewport: CssViewport,
    options: { preserveProjection?: boolean } = {},
  ) {
    if (historyProjectionQuiesced) return;
    const expectedSelection = captureCssSelectionIdentity();
    if (!expectedSelection) return;
    const identity = captureCssIdentity();
    const expectedRefreshToken = refreshToken;
    const myCallId = ++loadCallId;
    const preserveProjection = Boolean(
      options.preserveProjection
      && cssInspectorResolution
      && cssInspectorResolution.state !== "ambiguous"
      && cssInspectorResolution.selector === selector
      && cssInspectorResolution.viewport === viewport
      && cssInspectorResolution.target?.file === file,
    );
    loadingClassRules = !preserveProjection;
    if (!preserveProjection) {
      classRules = [];
      cssRuleContext = null;
      cssInspectorResolution = null;
      cssInspectorSelectionIdentity = null;
      cssTargetInfo = null;
    }
    pendingValues = untrack(() => pendingValuesForCurrentSelector());
    try {
      const resolution = await resolveCssInspectorContext({
        templatePath: selectedTemplatePath(),
        selector,
        viewport,
        fallbackFile: file || null,
        expectedWorkspaceRevision: workspaceRevision,
        expectedSelection,
      }, identity);
      if (
        myCallId !== loadCallId
        || !isCurrentCssIdentity(identity)
        || historyProjectionQuiesced
        || refreshToken !== expectedRefreshToken
        || !isCurrentCssInspectorSubject(expectedSelection)
      ) return;
      if (
        resolution.state !== "ambiguous"
        && resolution.target
        && resolution.target.file !== file
      ) {
        const allowed = await onCssCodeTargetChange?.({
          selector,
          file: resolution.target.file,
          expectedSelectionRevision: expectedSelection.selectionRevision,
        });
        if (!allowed && myCallId === loadCallId) {
          classRules = [];
          cssRuleContext = null;
          cssInspectorResolution = null;
          cssInspectorSelectionIdentity = null;
          cssTargetInfo = null;
        }
        return;
      }
      applyCssInspectorResolution(resolution, expectedSelection);
    } catch (error) {
      if (
        !isCurrentCssIdentity(identity)
        || historyProjectionQuiesced
        || refreshToken !== expectedRefreshToken
      ) return;
      if (myCallId === loadCallId) {
        if (!preserveProjection) {
          classRules = [];
          cssRuleContext = null;
          cssInspectorResolution = null;
          cssInspectorSelectionIdentity = null;
          cssTargetInfo = null;
        }
        const message = error instanceof Error ? error.message : String(error);
        onStatusUpdate?.(t("inspector-css-read-failed", { file, error: message }), "error");
      }
    } finally {
      if (myCallId === loadCallId) {
        loadingClassRules = false;
      }
    }
  }

  async function selectClassForCss(
    className: string,
  ): Promise<"allowed" | "blocked"> {
    const expectedSelection = captureCssSelectionIdentity();
    if (!expectedSelection || !await changeInspectorTab("css")) return "blocked";
    pendingValues = pendingValuesForCurrentSelector();

    const selector = `.${className}`;
    const identity = captureCssIdentity();
    const myCallId = ++loadCallId;
    loadingClassRules = true;
    try {
      const resolution = await resolveCssInspectorContext({
        templatePath: selectedTemplatePath(),
        selector,
        viewport: previewDevice,
        fallbackFile: targetCssFile || null,
        expectedWorkspaceRevision: workspaceRevision,
        expectedSelection,
      }, identity);
      if (
        myCallId !== loadCallId
        || !isCurrentCssIdentity(identity)
        || !isCurrentCssInspectorSubject(expectedSelection)
      ) return "blocked";
      applyCssInspectorResolution(resolution, expectedSelection);
      if (resolution.state === "ambiguous" || !resolution.target) return "blocked";
      const allowed = await onCssCodeTargetChange?.({
        selector,
        file: resolution.target.file,
        expectedSelectionRevision: expectedSelection.selectionRevision,
      });
      return allowed ? "allowed" : "blocked";
    } catch (error) {
      if (isCurrentCssIdentity(identity)) {
        const message = error instanceof Error ? error.message : String(error);
        onStatusUpdate?.(t("inspector-css-target-failed", { selector, error: message }), "error");
      }
      return "blocked";
    } finally {
      if (myCallId === loadCallId) loadingClassRules = false;
    }
  }

  function selectCssVariant(suffix: string) {
    if (!selectedClass || !targetCssFile) return;
    onCssCodeTargetChange?.({
      selector: `.${selectedClass}${suffix}`,
      file: targetCssFile,
    });
  }

  function captureCurrentCssMutationTarget() {
    if (!effectiveSelector || !targetCssFile) return;
    const expectedSelection = captureCssSelectionIdentity();
    const resolution = cssInspectorResolution;
    if (
      !expectedSelection
      || !resolution
      || resolution.state === "ambiguous"
      || !sameCssSemanticSelection(cssInspectorSelectionIdentity, expectedSelection)
      || resolution.selector !== effectiveSelector
      || resolution.viewport !== previewDevice
      || resolution.target?.file !== targetCssFile
    ) return;
    const identity = captureCssIdentity();
    const file = targetCssFile;
    const selector = effectiveSelector;
    const viewport = previewDevice;
    const pageTarget = cssTargetInfo;
    const targetKey = [
      identity.expectedProjectRoot,
      identity.expectedSessionId,
      cssSemanticSelectionKey(expectedSelection),
      file,
      selector,
      viewport,
      pageTarget?.targetKind === "reusable"
        ? pageTarget.templatePath ?? "reusable"
        : pageTarget?.pageOwned ? pageTarget.templatePath ?? "page" : "existing",
    ].join("\u0000");
    return {
      identity,
      expectedSelection,
      file,
      selector,
      viewport,
      pageTarget,
      targetKey,
    };
  }

  function draftCssProperties(properties: Readonly<Record<string, string>>) {
    const target = captureCurrentCssMutationTarget();
    if (!target) return;
    const entries = Object.entries(properties);
    if (!entries.length) return;
    const focusedProperty = entries.length === 1 ? entries[0][0] : "background-image";
    onCssCodeTargetChange?.({
      selector: target.selector,
      file: target.file,
      property: focusedProperty,
      expectedSelectionRevision: target.expectedSelection.selectionRevision,
    });
    const baselines = Object.fromEntries(entries.map(([property]) => [
      property,
      captureCssPendingValueBaseline(pendingValues, property),
    ]));
    const nextPendingValues = { ...pendingValues, ...properties };
    pendingValues = nextPendingValues;
    const appliedLiveEpoch = onLivePropertiesChange?.(
      target.selector,
      nextPendingValues,
      target.viewport,
    );
    const liveEpoch = typeof appliedLiveEpoch === "number" ? appliedLiveEpoch : null;
    const {
      identity,
      expectedSelection,
      file,
      selector,
      viewport,
      pageTarget,
      targetKey,
    } = target;
    if (pageTarget?.targetKind === "reusable" && pageTarget.templatePath) {
      const mutation = {
        key: targetKey,
        identity,
        label: `CSS reutilizabil ${selector}`,
        liveEpoch,
        run: (properties: Record<string, string>) => setReusableCssRuleAtViewport({
          templatePath: pageTarget.templatePath ?? "",
          relativePath: file,
          selector,
          properties,
          viewport,
          cachebustAssets,
          expectedSelection,
        }, identity),
      };
      for (const [property, value] of entries) {
        stageCssRuleMutation(mutation, property, value, baselines[property]);
      }
    } else if (pageTarget?.pageOwned && pageTarget.templatePath) {
      const mutation = {
        key: targetKey,
        identity,
        label: `CSS ${selector}`,
        liveEpoch,
        run: (properties: Record<string, string>) => setPageCssRuleAtViewport({
          templatePath: pageTarget.templatePath ?? "",
          relativePath: file,
          selector,
          properties,
          viewport,
          cachebustAssets,
          expectedSelection,
        }, identity),
      };
      for (const [property, value] of entries) {
        stageCssRuleMutation(mutation, property, value, baselines[property]);
      }
    } else {
      const mutation = {
        key: targetKey,
        identity,
        label: `CSS ${selector}`,
        liveEpoch,
        run: (properties: Record<string, string>) => setCssRuleAtViewport({
          relativePath: file,
          selector,
          properties,
          viewport,
          expectedSelection,
        }, identity),
      };
      for (const [property, value] of entries) {
        stageCssRuleMutation(mutation, property, value, baselines[property]);
      }
    }
    if (!onLivePropertiesChange) {
      for (const [property, value] of entries) onLivePropertyChange?.(selector, property, value);
    }
    onStatusUpdate?.(t("inspector-css-preview-changed", { property: focusedProperty }), "unsaved");
  }

  function draftCssProperty(property: string, value: string) {
    draftCssProperties({ [property]: value });
  }

  function commitCssProperty(property: string, value?: string) {
    if (value !== undefined && pendingValues[property] !== value) {
      draftCssProperty(property, value);
    }
    scheduleStagedCssPanelFlush();
  }

  function commitCssProperties(properties: Readonly<Record<string, string>> = {}) {
    if (Object.keys(properties).length) draftCssProperties(properties);
    scheduleStagedCssPanelFlush();
  }

  function cancelCssProperty(property: string) {
    const target = captureCurrentCssMutationTarget();
    if (!target) return;
    const staged = stagedCssRuleMutations.get(target.targetKey);
    const baseline = staged?.baselines[property];
    if (!staged || !baseline || !(property in staged.properties)) return;

    const nextProperties = { ...staged.properties };
    const nextBaselines = { ...staged.baselines };
    delete nextProperties[property];
    delete nextBaselines[property];
    const hasRemainingDrafts = Object.keys(nextProperties).length > 0;
    if (!hasRemainingDrafts) {
      stagedCssRuleMutations.delete(target.targetKey);
    } else {
      stagedCssRuleMutations.set(target.targetKey, {
        ...staged,
        properties: nextProperties,
        baselines: nextBaselines,
      });
    }

    const nextPendingValues = restoreCssPendingValueBaseline(pendingValues, property, baseline);
    pendingValues = nextPendingValues;
    const appliedLiveEpoch = onLivePropertiesChange?.(
      target.selector,
      nextPendingValues,
      target.viewport,
    );
    const liveEpoch = typeof appliedLiveEpoch === "number" ? appliedLiveEpoch : null;
    if (hasRemainingDrafts) {
      const remaining = stagedCssRuleMutations.get(target.targetKey);
      if (remaining) {
        stagedCssRuleMutations.set(target.targetKey, { ...remaining, liveEpoch });
      }
    } else if (liveEpoch !== null) {
      // Dacă draftul anulat restaurează o valoare aflată deja într-o mutație
      // anterioară din coadă, păstrăm overlay-ul până când acea proiecție s-a
      // terminat. Guard-ul pe epoch nu permite ștergerea unui draft ulterior.
      void cssWorkspaceMutationTail.then(() => {
        onInspectorLivePropertiesRejected?.(liveEpoch);
      });
    }
    updatePendingIndicators();
    onStatusUpdate?.(t("inspector-css-edit-cancelled", { property }), "idle");
  }

  function cancelCssProperties(properties: readonly string[]) {
    for (const property of properties) cancelCssProperty(property);
  }

  const continuousCssPropertyBindings = new Map<string, CssContinuousEditHandlers>();

  function continuousCssProperty(property: string): CssContinuousEditHandlers {
    const existing = continuousCssPropertyBindings.get(property);
    if (existing) return existing;
    const bindings: CssContinuousEditHandlers = {
      oninput: (value) => draftCssProperty(property, value),
      oncommit: () => commitCssProperty(property),
      oncancel: () => cancelCssProperty(property),
    };
    continuousCssPropertyBindings.set(property, bindings);
    return bindings;
  }

  const cssPropertyEdit: CssPropertyEditController = {
    draft: draftCssProperty,
    draftMany: draftCssProperties,
    commit: commitCssProperty,
    commitMany: commitCssProperties,
    cancel: cancelCssProperty,
    cancelMany: cancelCssProperties,
    continuous: continuousCssProperty,
  };

</script>

<aside
  class="inspector-pane"
  class:html-projection-pending={htmlProjectionPending}
  aria-label={t("inspector-pane-label")}
  aria-busy={htmlProjectionPending}
>
  <div class="inspector-context">
    <SelectionSummaryCard
      summary={presentedInspectorSelectionSummary}
      authoringDocumentPath={directAuthoringDocumentPath}
      selectClass={selectClassForCss}
    />
  </div>

  {#if hasTeraSelection}
    <div class="inspector-main tera-main">
      <div class="inspector-scroll">
        <TeraSourceCard
          node={selectedTemplateSourceNode}
          navigationNode={selectedEditorNavigationNode}
          {enterTeraBoundary}
          {openSelectedTeraSource}
          {deleteSelectedTeraNode}
        />
      </div>
    </div>
  {:else if hasMarkdownSelection}
    <div class="inspector-main tera-main">
      <div class="inspector-scroll">
        <MarkdownSourceCard
          navigationNode={selectedEditorNavigationNode}
          editSelectedContent={openSelectedMarkdownContent}
          openSelectedSource={openSelectedTeraSource}
        />
      </div>
    </div>
  {:else}
    <div class="inspector-main">
      <div class="ui-tabs inspector-tabs" role="tablist" aria-label={t("inspector-sections-label")}>
        <button
          id="inspector-tab-html"
          class="ui-tab"
          class:active={inspectorTab === "html"}
          type="button"
          role="tab"
          aria-selected={inspectorTab === "html"}
          aria-controls="inspector-tab-panel"
          tabindex={inspectorTab === "html" ? 0 : -1}
          onclick={() => { void changeInspectorTab("html"); }}
          onkeydown={(event) => handleInspectorTabKeydown(event, "html")}
        >
          <IconHierarchy3 size={15} stroke={1.8} aria-hidden="true" />
          HTML
        </button>
        <button
          id="inspector-tab-css"
          class="ui-tab"
          class:active={inspectorTab === "css"}
          type="button"
          role="tab"
          aria-selected={inspectorTab === "css"}
          aria-controls="inspector-tab-panel"
          tabindex={inspectorTab === "css" ? 0 : -1}
          onclick={() => { void changeInspectorTab("css"); }}
          onkeydown={(event) => handleInspectorTabKeydown(event, "css")}
        >
          <IconPalette size={15} stroke={1.8} aria-hidden="true" />
          CSS
        </button>
        <button
          id="inspector-tab-js"
          class="ui-tab"
          class:active={inspectorTab === "js"}
          type="button"
          role="tab"
          aria-selected={inspectorTab === "js"}
          aria-controls="inspector-tab-panel"
          tabindex={inspectorTab === "js" ? 0 : -1}
          onclick={() => { void changeInspectorTab("js"); }}
          onkeydown={(event) => handleInspectorTabKeydown(event, "js")}
        >
          <IconPointerBolt size={15} stroke={1.8} aria-hidden="true" />
          JS
        </button>
      </div>
      <div
        id="inspector-tab-panel"
        class="inspector-scroll inspector-editor-scroll"
        role="tabpanel"
        aria-labelledby={`inspector-tab-${inspectorTab}`}
        inert={htmlProjectionPending}
      >

        {#if inspectorTab === "css"}
        <CssPane
          selectionSummary={presentedInspectorSelectionSummary}
          {selectedClass}
          {effectiveSelector}
          {viewportLabel}
          {previewDevice}
          pageCssTarget={cssTargetInfo}
          resolution={cssInspectorResolution}
          {cssRuleContext}
          {classRules}
          {pendingValues}
          {scssVariables}
          {fontFamilies}
          {installedFontAxes}
          {scannedAssets}
          {loadingClassRules}
          {selectorSuffix}
          {customSuffix}
          {usingCustom}
          {cssPropertyEdit}
          {gridOverlayEnabled}
          {onGridOverlayChange}
          {selectCssVariant}
        />

        {:else if inspectorTab === "js"}
        <JsPane
          selectionSummary={inspectorSelectionSummary}
          dataAnim={attributeValues["data-anim"] ?? null}
          workspace={motionWorkspace}
          onSwitchToHtml={() => { void changeInspectorTab("html"); }}
        />

        {:else if inspectorTab === "html"}
        <HtmlPane
          selectionSummary={presentedInspectorSelectionSummary}
          selectionSnapshot={presentedSelectionSnapshot}
          physicalFacts={presentedHtmlPhysicalFacts}
          canEditHtml={canEditHtmlEffective}
          attributeValues={presentedAttributeValues}
          textContentValue={presentedTextContentValue}
          imageSourceValue={presentedImageSourceValue}
          classEditorValue={presentedClassEditorValue}
          pendingTag={presentedPendingTag}
          {scannedAssets}
          isActivePreviewHtmlSource={presentedIsActivePreviewHtmlSource}
          attributeStatus={presentedAttributeStatus}
          textStatus={presentedTextStatus}
          classStatus={presentedClassStatus}
          imageStatus={presentedImageStatus}
          updateAttributeValue={updateAttributeValue}
          removeAttribute={removeAttribute}
          applyAttributesToHtml={applyAttributesToHtml}
          updateTextContentValue={updateTextContentValue}
          applyTextContentToHtml={applyTextContentToHtml}
          setClassEditorValue={setClassEditorValue}
          applyClassesToHtml={applyClassesToHtml}
          generateClassForSelectedHtml={generateClassForSelectedHtml}
          generateDataAnimForSelectedHtml={generateDataAnimForSelectedHtml}
          setImageSourceValue={setImageSourceValue}
          applyZolaImageProcessingToHtml={applyZolaImageProcessingToHtml}
          cancelHtmlAttributeDraft={cancelHtmlAttributeDraft}
          changeElementTag={changeElementTag}
          tagStatus={presentedTagStatus}
        />

        {/if}
      </div>
    </div>
    <BlockPropertiesPane
      selectionContext={inspectorBlockSelectionContext}
      dynamicSelectionContext={inspectorDynamicWidgetSelectionContext}
      {sourceGraph}
      selectedTag={inspectorSelectionSummary?.tag ?? null}
      {projectRoot}
      {runtimeSessionId}
      {workspaceRevision}
      {previewRevision}
      height={blockPropertiesHeight}
      collapsed={blockPropertiesCollapsed}
      onLayoutCommit={persistBlockPropertiesLayout}
      onApply={applyNativeBlockOption}
      onDynamicUpdate={updateDynamicWidget}
      onDynamicDelete={deleteDynamicWidget}
    />
  {/if}
</aside>

<style>
  .inspector-pane {
    position: relative;
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    width: 100%;
    height: 100%;
    min-height: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-panel);
    overflow: hidden;
    overscroll-behavior: contain;
    background: var(--material-panel);
    box-shadow: var(--shadow-panel);
  }

  .inspector-pane.html-projection-pending .inspector-context,
  .inspector-pane.html-projection-pending .inspector-editor-scroll {
    pointer-events: none;
  }

  .inspector-context {
    flex: 0 0 auto;
    padding: 10px 10px 0;
  }

  .inspector-main {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    min-height: 0;
  }

  .inspector-scroll {
    min-height: 0;
    padding: 10px;
    overflow-x: hidden;
    overflow-y: auto;
    overscroll-behavior: contain;
  }

  .inspector-editor-scroll {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    padding: 0;
  }

  .inspector-tabs {
    display: grid;
    flex: 0 0 auto;
    grid-template-columns: repeat(3, 1fr);
    margin: 10px 10px 0;
  }

  .inspector-tabs .ui-tab {
    width: 100%;
  }

</style>
