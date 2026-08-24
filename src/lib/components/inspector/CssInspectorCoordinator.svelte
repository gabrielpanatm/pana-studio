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
  import { CssEditSessionCoordinator } from "$lib/inspector/css-edit-session-coordinator";
  import { registerEditFlushHandler } from "$lib/session/edit-flush-registry";
  import type { GlobalStatusPublishOptions } from "$lib/status/global-status";

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
    onStatusUpdate?: (
      text: string,
      kind: string,
      options?: GlobalStatusPublishOptions,
    ) => void;
    onPendingChange?: (area: InspectorPendingArea, pending: boolean) => void;
    onCssCodeTargetChange?: (target: {
      selector: string;
      file: string;
      property?: string | null;
      expectedSelectionRevision?: number | null;
      expectedSelection?: import("$lib/preview/contracts").SelectionMutationIdentity | null;
    }) => boolean | Promise<boolean>;
    getOpenCssRuleContext?: (
      file: string,
      selector: string,
      viewport: CssViewport,
    ) => CssRuleContext | null;
    gridOverlayEnabled?: boolean;
    onGridOverlayChange?: (enabled: boolean) => void;
  } = $props();

  const presentedCssFocus = $derived.by(() => {
    const focus = presentedSelectionSnapshot?.focus;
    return focus?.kind === "cssRule" || focus?.kind === "cssProperty"
      ? focus
      : null;
  });
  const activeCssTargetFile = $derived(presentedCssFocus?.file ?? targetCssFile);
  const presentedFocusSelector = $derived(presentedCssFocus?.selector ?? null);

  function reportReaderStatus(status: CssInspectorReaderStatus) {
    if (status.kind === "readFailed") {
      onStatusUpdate?.(t("inspector-css-read-failed", status), "error", {
        source: "css-inspector",
        code: "css-inspector.read-failed",
        dedupeKey: "css-inspector:read",
        resolutionKey: "css-inspector:read",
      });
    } else {
      onStatusUpdate?.(t("inspector-css-target-failed", status), "error", {
        source: "css-inspector",
        code: "css-inspector.target-failed",
        dedupeKey: "css-inspector:target",
        resolutionKey: "css-inspector:target",
      });
    }
  }

  function reportMutationStatus(status: CssInspectorMutationStatus) {
    const options = status.interactionId
      ? editSession.statusOptions(
        status.interactionId,
        status.kind === "previewChanged" ? "preview"
          : status.kind === "saved" || status.kind === "editCancelled" ? "saved"
            : "error",
      )
      : {
        source: "css-inspector",
        code: "css-inspector.target-unavailable",
        dedupeKey: "css-inspector:target",
        resolutionKey: "css-inspector:target",
      } satisfies GlobalStatusPublishOptions;
    if (status.kind === "saved") {
      onStatusUpdate?.(t("inspector-css-session-saved", status), "unsaved", options);
    } else if (status.kind === "liveFailed") {
      onStatusUpdate?.(t("inspector-css-live-failed", status), "error", options);
    } else if (status.kind === "mutationFailed") {
      onStatusUpdate?.(t("inspector-css-mutation-failed", status), "error", options);
    } else if (status.kind === "previewChanged") {
      onStatusUpdate?.(t("inspector-css-preview-changed", status), "unsaved", options);
    } else if (status.kind === "targetUnavailable") {
      onStatusUpdate?.(
        t("inspector-css-edit-unavailable", { property: status.property }),
        "error",
        options,
      );
    } else {
      onStatusUpdate?.(
        t("inspector-css-edit-cancelled", { property: status.property }),
        "idle",
        options,
      );
    }
  }

  const inspectorState = new CssInspectorState();
  const editSession = new CssEditSessionCoordinator();
  const reader = new CssInspectorReader(inspectorState, {
    editSession,
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
    editSession,
    state: inspectorState,
    context: () => ({
      projectRoot,
      runtimeSessionId,
      targetCssFile: activeCssTargetFile,
      previewDevice,
    }),
    captureSelection: () => reader.captureCurrentSelection(),
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
    const input = {
      projectRoot,
      runtimeSessionId,
      targetCssFile: activeCssTargetFile,
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
    if (!inspectorState.selectedClass || !activeCssTargetFile) return;
    void onCssCodeTargetChange?.({
      selector: `.${inspectorState.selectedClass}${suffix}`,
      file: activeCssTargetFile,
      expectedSelection: reader.captureCurrentSelection(),
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
  editingReady={inspectorState.hasEditableTarget}
  cssPropertyEdit={inspectorState.propertyEdit}
  {gridOverlayEnabled}
  {onGridOverlayChange}
  {selectCssVariant}
/>
