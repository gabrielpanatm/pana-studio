<script lang="ts">
  import type { EditorActionOutcome } from "$lib/editor-runtime/action-outcome";
  import { t } from "$lib/i18n/runtime.svelte";
  import {
    IconHierarchy3,
    IconPalette,
    IconPointerBolt,
  } from "@tabler/icons-svelte";
  import { tick, untrack } from "svelte";
  import { primarySelectionEntry, selectionResolution } from "$lib/kernel/selection-read-model";
  import HtmlPane from "$lib/components/inspector/HtmlPane.svelte";
  import HtmlInspectorCoordinator from "$lib/components/inspector/HtmlInspectorCoordinator.svelte";
  import type JsPane from "$lib/components/inspector/JsPane.svelte";
  import type BlockPropertiesPane from "$lib/components/inspector/BlockPropertiesPane.svelte";
  import type CssInspectorCoordinator from "$lib/components/inspector/CssInspectorCoordinator.svelte";
  import SelectionSummaryCard from "$lib/components/inspector/SelectionSummaryCard.svelte";
  import TeraSourceCard from "$lib/components/inspector/TeraSourceCard.svelte";
  import MarkdownSourceCard from "$lib/components/inspector/MarkdownSourceCard.svelte";
  import type {
    CssRuleContext,
    CssViewport,
    ScssVariable,
  } from "$lib/css/contracts";
  import type { CssMutationAuthorityReceipt } from "$lib/css/mutation-contract";
  import type { InspectorTab } from "$lib/application/contracts";
  import type {
    BlockSelectionContext,
    EditableAttributes,
    InspectorHtmlPhysicalFacts,
    InspectorPendingArea,
  } from "$lib/canvas/contracts";
  import type {
    DynamicWidgetProperties,
    DynamicWidgetSelectionContext,
    DynamicWidgetSnapshot,
  } from "$lib/content-models/contracts";
  import type { EditorNavigationNode } from "$lib/editor/contracts";
  import type {
    InspectorSelectionSummarySnapshot,
    SelectionSnapshot,
  } from "$lib/editor/contracts";
  import type { InstalledFontVariationAxis } from "$lib/fonts/contracts";
  import type { ProjectZolaImageIntent } from "$lib/preview/contracts";
  import type { ProjectFile } from "$lib/project/lifecycle-contract";
  import type { SourceGraphNode } from "$lib/source-graph/contracts";
  import type {
    ApplyNativeBlockOptionRequest,
    ApplyNativeIconRequest,
  } from "$lib/editor/html-actions/media";
  import type { MotionWorkspaceState } from "$lib/motion/workspace.svelte";
  import {
    projectHtmlInspectorClassSummary,
    type StableHtmlInspectorProjection,
  } from "$lib/inspector/html-projection-stability";

  let {
    inspectorSelectionSummary = null,
    selectionInitializing = false,
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
    motionWorkspace,
    workspaceRevision = 0,
    previewRevision = "",
    blockPropertiesHeight = 220,
    blockPropertiesCollapsed = false,
    scssVariables = [],
    fontFamilies = [],
    installedFontAxes = [],
    attributeValues,
    attributeStatus = "",
    attributePending = false,
    textContentValue = "",
    textStatus = "",
    classEditorValue = "",
    classPending = false,
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
    enterBoundary,
    deleteSelectedTeraNode,
    openSelectedTeraSource,
    openSelectedMarkdownContent,
    pendingTag = null,
    tagStatus = "",
    changeElementTag,
    onLivePropertiesChange,
    onCssWorkspaceMutationCommitted,
    onInspectorLivePropertiesRejected,
    onStatusUpdate,
    onPendingChange,
    onInspectorTabChange,
    beforeInspectorTabChange,
    onCssCodeTargetChange,
    getOpenCssRuleContext,
    applyNativeBlockOption,
    applyNativeIcon,
    applyNativeBlockSlotMutation,
    updateDynamicWidget,
    deleteDynamicWidget,
    persistBlockPropertiesLayout,
    gridOverlayEnabled = false,
    onGridOverlayChange,
  }: {
    inspectorSelectionSummary?: InspectorSelectionSummarySnapshot | null;
    selectionInitializing?: boolean;
    inspectorHtmlPhysicalFacts?: InspectorHtmlPhysicalFacts | null;
    inspectorBlockSelectionContext?: BlockSelectionContext | null;
    inspectorDynamicWidgetSelectionContext?: DynamicWidgetSelectionContext | null;
    sourceGraph?: import("$lib/source-graph/graph-contract").SourceGraph | null;
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
    motionWorkspace: MotionWorkspaceState;
    workspaceRevision?: number;
    previewRevision?: string;
    blockPropertiesHeight?: number;
    blockPropertiesCollapsed?: boolean;
    scssVariables?: ScssVariable[];
    fontFamilies?: string[];
    installedFontAxes?: InstalledFontVariationAxis[];
    attributeValues: EditableAttributes;
    attributeStatus?: string;
    attributePending?: boolean;
    textContentValue?: string;
    textStatus?: string;
    classEditorValue?: string;
    classPending?: boolean;
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
    enterBoundary: (scopeId: string) => void | Promise<void>;
    deleteSelectedTeraNode: () => void | Promise<void>;
    openSelectedTeraSource: () => void | Promise<void>;
    openSelectedMarkdownContent: () => void | Promise<void>;
    pendingTag?: string | null;
    tagStatus?: string;
    changeElementTag: (tag: string) => void;
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
    applyNativeIcon: (request: ApplyNativeIconRequest) => Promise<EditorActionOutcome>;
    applyNativeBlockSlotMutation: (
      request: import("$lib/blocks/contracts").NativeBlockSlotMutationRequest,
    ) => Promise<EditorActionOutcome>;
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

  const baseInspectorSelectionSummary = $derived(
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
  const presentedInspectorSelectionSummary = $derived(
    projectHtmlInspectorClassSummary(
      baseInspectorSelectionSummary,
      presentedClassEditorValue,
    ),
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

  // The editor domain resolves the canonical ProjectWorkspace target and remains the
  // authority. During a resolving frame the last complete projection stays
  // visually identical; the panel itself is inert until the atomic swap.
  const canEditHtmlEffective = $derived(
    presentedCanEditHtml
      && (
        (presentedSelectionSnapshot?.aggregateCapabilities.memberCount ?? 0) === 1
        || presentedSelectionSnapshot?.aggregateCapabilities.canBatchAttributes === true
      ),
  );
  const hasTeraSelection = $derived(
    selectionResolution(selectionSnapshot) === "resolved"
      && primarySelectionEntry(selectionSnapshot)?.subject.kind === "boundary"
      && primarySelectionEntry(selectionSnapshot)?.subject.boundaryKind !== "markdown",
  );
  const hasMarkdownSelection = $derived(
    selectionResolution(selectionSnapshot) === "resolved"
      && primarySelectionEntry(selectionSnapshot)?.subject.kind === "boundary"
      && primarySelectionEntry(selectionSnapshot)?.subject.boundaryKind === "markdown",
  );
  const directAuthoringDocumentPath = $derived(
    hasTeraSelection
      && selectedEditorNavigationNode?.kind === "boundary"
      && selectedEditorNavigationNode.boundary?.kind === "template"
      && selectedEditorNavigationNode.sourceKind === "block"
      && selectedEditorNavigationNode.origin === "project"
      && selectedEditorNavigationNode.capabilities.requiresEditScopeId === null
      && selectedEditorNavigationNode.file === selectionSnapshot?.activeDocumentPath
        ? selectedEditorNavigationNode.file
        : null,
  );

  let inspectorTab = $state<InspectorTab>("html");
  let inspectorTabChangeSerial = 0;
  let CssCoordinatorComponent = $state<typeof CssInspectorCoordinator | null>(null);
  let JsPaneComponent = $state<typeof JsPane | null>(null);
  let BlockPropertiesPaneComponent = $state<typeof BlockPropertiesPane | null>(null);

  async function loadInspectorTab(tab: InspectorTab) {
    if (tab === "css" && !CssCoordinatorComponent) {
      CssCoordinatorComponent = (await import("$lib/components/inspector/CssInspectorCoordinator.svelte")).default;
    } else if (tab === "js" && !JsPaneComponent) {
      JsPaneComponent = (await import("$lib/components/inspector/JsPane.svelte")).default;
    }
  }

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
    try {
      await loadInspectorTab(nextTab);
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

  let cssCoordinator = $state<{ selectClass: (className: string) => Promise<"allowed" | "blocked"> } | null>(null);

  async function selectClassForCss(className: string): Promise<"allowed" | "blocked"> {
    if (!await changeInspectorTab("css")) return "blocked";
    await tick();
    return await cssCoordinator?.selectClass(className) ?? "blocked";
  }

  $effect(() => {
    onInspectorTabChange?.(inspectorTab);
  });

  $effect(() => {
    if (
      selectionSnapshot?.focus.kind === "cssRule"
      || selectionSnapshot?.focus.kind === "cssProperty"
    ) untrack(() => { void changeInspectorTab("css"); });
  });

  const blockSelectionContext = $derived(
    presentedSelectionSnapshot?.aggregateCapabilities.primaryOnlyEditsAllowed
      ? inspectorBlockSelectionContext
      : null,
  );
  const dynamicBlockSelectionContext = $derived(
    presentedSelectionSnapshot?.aggregateCapabilities.primaryOnlyEditsAllowed
      ? inspectorDynamicWidgetSelectionContext
      : null,
  );

  $effect(() => {
    if (
      (blockSelectionContext || dynamicBlockSelectionContext)
      && !BlockPropertiesPaneComponent
    ) {
      void import("$lib/components/inspector/BlockPropertiesPane.svelte")
        .then((module) => { BlockPropertiesPaneComponent = module.default; })
        .catch((error) => {
          const message = error instanceof Error ? error.message : String(error);
          onStatusUpdate?.(t("inspector-tab-change-blocked", { error: message }), "error");
        });
    }
  });

</script>

<HtmlInspectorCoordinator
  summary={inspectorSelectionSummary}
  selection={selectionSnapshot}
  physicalFacts={inspectorHtmlPhysicalFacts}
  {attributeValues}
  {textContentValue}
  {classEditorValue}
  {imageSourceValue}
  {pendingTag}
  {attributeStatus}
  {textStatus}
  {classStatus}
  {imageStatus}
  {tagStatus}
  {canEditHtml}
  {isActivePreviewHtmlSource}
  bind:stableProjection={stableHtmlProjection}
  bind:pending={htmlProjectionPending}
/>

<aside
  class="inspector-pane"
  class:html-projection-pending={htmlProjectionPending}
  aria-label={t("inspector-pane-label")}
  aria-busy={htmlProjectionPending}
>
  <div class="inspector-context">
    <SelectionSummaryCard
      summary={presentedInspectorSelectionSummary}
      selection={presentedSelectionSnapshot}
      initializing={selectionInitializing}
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
          {enterBoundary}
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

        {#if CssCoordinatorComponent}
          <div class="inspector-route" hidden={inspectorTab !== "css"} inert={inspectorTab !== "css"}>
            <CssCoordinatorComponent
              bind:this={cssCoordinator}
              selectionSummary={presentedInspectorSelectionSummary}
              {presentedSelectionSnapshot}
              {selectionSnapshot}
              {htmlProjectionPending}
              {projectRoot}
              {runtimeSessionId}
              {targetCssFile}
              {cssSourceRevision}
              {activeRenderedTemplatePath}
              {previewDevice}
              {refreshToken}
              {historyProjectionQuiesced}
              {workspaceRevision}
              {scssVariables}
              {fontFamilies}
              {installedFontAxes}
              {scannedAssets}
              {onLivePropertiesChange}
              {onCssWorkspaceMutationCommitted}
              {onInspectorLivePropertiesRejected}
              {onStatusUpdate}
              {onPendingChange}
              {onCssCodeTargetChange}
              {getOpenCssRuleContext}
              {gridOverlayEnabled}
              {onGridOverlayChange}
            />
          </div>
        {/if}

        {#if JsPaneComponent}
          <div class="inspector-route" hidden={inspectorTab !== "js"} inert={inspectorTab !== "js"}>
            <JsPaneComponent
              selectionSummary={presentedInspectorSelectionSummary}
              dataAnim={presentedAttributeValues["data-anim"] ?? null}
              workspace={motionWorkspace}
              onSwitchToHtml={() => { void changeInspectorTab("html"); }}
            />
          </div>
        {/if}

        <div class="inspector-route" hidden={inspectorTab !== "html"} inert={inspectorTab !== "html"}>
          <HtmlPane
            selectionSummary={presentedInspectorSelectionSummary}
            selectionSnapshot={presentedSelectionSnapshot}
            physicalFacts={presentedHtmlPhysicalFacts}
            canEditHtml={canEditHtmlEffective}
            attributeValues={presentedAttributeValues}
            {attributePending}
            textContentValue={presentedTextContentValue}
            imageSourceValue={presentedImageSourceValue}
            classEditorValue={presentedClassEditorValue}
            {classPending}
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
        </div>
      </div>
    </div>
    {#if BlockPropertiesPaneComponent && (blockSelectionContext || dynamicBlockSelectionContext)}
    <BlockPropertiesPaneComponent
      selectionContext={blockSelectionContext}
      dynamicSelectionContext={dynamicBlockSelectionContext}
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
      onIconUpdate={(intent, context, source) => applyNativeIcon({
        intent,
        rootTag: context.rootTag,
        rootSourceId: source.rootSourceNodeId,
        rootLocation: source.rootLocation,
        rootSessionId: context.rootSessionId,
      })}
      onSlotMutation={applyNativeBlockSlotMutation}
      onDynamicUpdate={updateDynamicWidget}
      onDynamicDelete={deleteDynamicWidget}
    />
    {/if}
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

  .inspector-route {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    width: 100%;
    min-width: 0;
    min-height: 0;
  }

  .inspector-route[hidden] {
    display: none;
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
