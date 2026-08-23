<script lang="ts">
  import { untrack } from "svelte";
  import { t } from "$lib/i18n/runtime.svelte";
  import CssPane from "$lib/components/inspector/panes/CssPane.svelte";
  import type { CssRuleContext, CssViewport, ScssVariable } from "$lib/css/contracts";
  import type { CssMutationAuthorityReceipt } from "$lib/css/mutation-contract";
  import type { InspectorPendingArea } from "$lib/canvas/contracts";
  import type {
    InspectorSelectionSummarySnapshot,
    SelectionSnapshot,
  } from "$lib/editor/contracts";
  import type { InstalledFontVariationAxis } from "$lib/fonts/contracts";
  import type { ProjectFile } from "$lib/project/lifecycle-contract";
  import {
    CssInspectorReader,
    type CssInspectorReaderStatus,
  } from "$lib/inspector/css-inspector-reader";
  import {
    CssInspectorMutationQueue,
    type CssInspectorMutationStatus,
  } from "$lib/inspector/css-inspector-mutation-queue";
  import { CssInspectorState } from "$lib/inspector/css-inspector-state.svelte";
  import { registerEditFlushHandler } from "$lib/session/edit-flush-registry";

  let {
    selectionSummary = null,
    presentedSelectionSnapshot = null,
    selectionSnapshot = null,
    htmlProjectionPending = false,
    projectRoot = "",
    runtimeSessionId = "",
    targetCssFile = "",
    cssSourceRevision = 0,
    activeRenderedTemplatePath = null,
    previewDevice = "desktop" as CssViewport,
    refreshToken = 0,
    historyProjectionQuiesced = false,
    workspaceRevision = 0,
    scssVariables = [],
    fontFamilies = [],
    installedFontAxes = [],
    scannedAssets = [],
    onLivePropertiesChange,
    onCssWorkspaceMutationCommitted,
    onInspectorLivePropertiesRejected,
    onStatusUpdate,
    onPendingChange,
    onCssCodeTargetChange,
    getOpenCssRuleContext,
    gridOverlayEnabled = false,
    onGridOverlayChange,
  }: {
    selectionSummary?: InspectorSelectionSummarySnapshot | null;
    presentedSelectionSnapshot?: SelectionSnapshot | null;
    selectionSnapshot?: SelectionSnapshot | null;
    htmlProjectionPending?: boolean;
    projectRoot?: string;
    runtimeSessionId?: string;
    targetCssFile?: string;
    cssSourceRevision?: number;
    activeRenderedTemplatePath?: string | null;
    previewDevice?: CssViewport;
    refreshToken?: number;
    historyProjectionQuiesced?: boolean;
    workspaceRevision?: number;
    scssVariables?: ScssVariable[];
    fontFamilies?: string[];
    installedFontAxes?: InstalledFontVariationAxis[];
    scannedAssets?: ProjectFile[];
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
    onStatusUpdate?: (text: string, kind: string) => void;
    onPendingChange?: (area: InspectorPendingArea, pending: boolean) => void;
    onCssCodeTargetChange?: (target: {
      selector: string;
      file: string;
      property?: string | null;
      expectedSelectionRevision?: number | null;
    }) => boolean | Promise<boolean>;
    getOpenCssRuleContext?: (
      file: string,
      selector: string,
      viewport: CssViewport,
    ) => CssRuleContext | null;
    gridOverlayEnabled?: boolean;
    onGridOverlayChange?: (enabled: boolean) => void;
  } = $props();

  function reportReaderStatus(status: CssInspectorReaderStatus) {
    if (status.kind === "readFailed") {
      onStatusUpdate?.(t("inspector-css-read-failed", status), "error");
    } else {
      onStatusUpdate?.(t("inspector-css-target-failed", status), "error");
    }
  }

  function reportMutationStatus(status: CssInspectorMutationStatus) {
    if (status.kind === "saved") {
      onStatusUpdate?.(t("inspector-css-session-saved", status), "unsaved");
    } else if (status.kind === "liveFailed") {
      onStatusUpdate?.(t("inspector-css-live-failed", status), "error");
    } else if (status.kind === "mutationFailed") {
      onStatusUpdate?.(t("inspector-css-mutation-failed", status), "error");
    } else if (status.kind === "previewChanged") {
      onStatusUpdate?.(t("inspector-css-preview-changed", status), "unsaved");
    } else {
      onStatusUpdate?.(t("inspector-css-edit-cancelled", status), "idle");
    }
  }

  const inspectorState = new CssInspectorState();
  const reader = new CssInspectorReader(inspectorState, {
    getOpenContext: (file, selector, viewport) =>
      getOpenCssRuleContext?.(file, selector, viewport) ?? null,
    changeCodeTarget: (target) => onCssCodeTargetChange?.(target) ?? false,
    reportStatus: reportReaderStatus,
    resetPendingAreas: () => {
      onPendingChange?.("css", false);
      onPendingChange?.("js", false);
    },
  });
  const mutationQueue = new CssInspectorMutationQueue({
    state: inspectorState,
    context: () => ({ projectRoot, runtimeSessionId, targetCssFile, previewDevice }),
    captureSelection: () => reader.captureCurrentSelection(),
    changeCodeTarget: (target) => onCssCodeTargetChange?.(target) ?? false,
    applyLiveProperties: (selector, properties, viewport) =>
      onLivePropertiesChange?.(selector, properties, viewport),
    projectCommittedMutation: (authority, liveEpoch) =>
      onCssWorkspaceMutationCommitted?.(authority, liveEpoch),
    rejectLiveProperties: (liveEpoch) =>
      onInspectorLivePropertiesRejected?.(liveEpoch),
    reportStatus: reportMutationStatus,
    setPending: (pending) => onPendingChange?.("css", pending),
  });

  const viewportLabel = $derived(
    inspectorState.presentedViewport === "tablet" ? t("inspector-viewport-tablet")
      : inspectorState.presentedViewport === "mobile" ? t("inspector-viewport-mobile")
        : t("inspector-viewport-desktop"),
  );

  $effect(() => registerEditFlushHandler(
    "inspector-css-workspace",
    () => mutationQueue.flushForRegistry(),
    () => mutationQueue.pendingForRegistry,
  ));

  $effect(() => {
    const focus = presentedSelectionSnapshot?.focus;
    const presentedFocusSelector = focus?.kind === "cssRule" || focus?.kind === "cssProperty"
      ? focus.selector
      : null;
    const input = {
      projectRoot,
      runtimeSessionId,
      targetCssFile,
      cssSourceRevision,
      activeRenderedTemplatePath,
      previewDevice,
      refreshToken,
      historyProjectionQuiesced,
      workspaceRevision,
      htmlProjectionPending,
      selectionSnapshot,
      selectionSummary,
      presentedFocusSelector,
    };
    mutationQueue.syncSession(projectRoot, runtimeSessionId);
    untrack(() => { void reader.reconcile(input); });
  });

  $effect(() => () => {
    reader.dispose();
    mutationQueue.dispose();
  });

  export function selectClass(className: string) {
    return reader.selectClass(className);
  }

  function selectCssVariant(suffix: string) {
    if (!inspectorState.selectedClass || !targetCssFile) return;
    void onCssCodeTargetChange?.({
      selector: `.${inspectorState.selectedClass}${suffix}`,
      file: targetCssFile,
    });
  }
</script>

<CssPane
  selectionSummary={selectionSummary}
  selectedClass={inspectorState.presentedSelectorState?.selectedClass ?? inspectorState.selectedClass}
  effectiveSelector={inspectorState.presentedSelector}
  {viewportLabel}
  previewDevice={inspectorState.presentedViewport}
  pageCssTarget={inspectorState.target}
  resolution={inspectorState.resolution}
  cssRuleContext={inspectorState.ruleContext}
  classRules={inspectorState.classRules}
  pendingValues={inspectorState.pendingValues}
  {scssVariables}
  {fontFamilies}
  {installedFontAxes}
  {scannedAssets}
  loadingClassRules={inspectorState.loading}
  selectorSuffix={inspectorState.presentedSelectorState?.selectorSuffix ?? inspectorState.selectorSuffix}
  customSuffix={inspectorState.presentedSelectorState?.customSuffix ?? inspectorState.customSuffix}
  usingCustom={inspectorState.presentedSelectorState?.usingCustom ?? inspectorState.usingCustom}
  projectionTransitioning={inspectorState.projectionTransitioning}
  cssPropertyEdit={inspectorState.propertyEdit}
  {gridOverlayEnabled}
  {onGridOverlayChange}
  {selectCssVariant}
/>
