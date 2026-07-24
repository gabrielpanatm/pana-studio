<script lang="ts">
  import type { EditorActionOutcome } from "$lib/editor-runtime/action-outcome";
  import { tick, untrack } from "svelte";
  import HtmlPane from "$lib/components/inspector/HtmlPane.svelte";
  import JsPane  from "$lib/components/inspector/JsPane.svelte";
  import BlockPropertiesPane from "$lib/components/inspector/BlockPropertiesPane.svelte";
  import CssPane from "$lib/components/inspector/panes/CssPane.svelte";
  import TeraSourceCard from "$lib/components/inspector/TeraSourceCard.svelte";
  import type {
    CssMutationAuthorityReceipt,
    CssProperty,
    CssRuleContext,
    EditableAttributes,
    InspectorTab,
    InspectorPendingArea,
    PageCssTarget,
    ProjectFile,
    ProjectZolaImageIntent,
    PreviewSelectionState,
    SaveState,
    ScssVariable,
    SelectionInfo,
    SourceGraph,
    SourceGraphNode,
  } from "$lib/types";
  import {
    createCssRequestIdentity,
    cssRequestIdentityMatches,
    findClassInScss,
    getCssRuleContext,
    resolvePageCssTarget,
    setCssRuleAtViewport,
    setPageCssRuleAtViewport,
    type CssRequestIdentity,
    type CssViewport,
  } from "$lib/project/io";
  import { formatSourceEditLocation } from "$lib/source-graph/location";
  import { pageJsRelativePath } from "$lib/js/page-path";
  import { projectRelativeZolaPath } from "$lib/project/files";
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
      onStatusUpdate?.(`${label} este în sesiunea proiectului — Ctrl+S persistă pe disc`, "unsaved");
      if (!onCssWorkspaceMutationCommitted) return;
      try {
        await onCssWorkspaceMutationCommitted(receipt.authority, liveEpoch);
      } catch (error) {
        if (!isCurrentCssIdentity(identity)) return;
        const message = error instanceof Error ? error.message : String(error);
        onStatusUpdate?.(
          `${label} este în sesiunea proiectului, dar proiecția CSS live nu a fost finalizată: ${message}`,
          "error",
        );
      }
    });
    cssWorkspaceMutationTail = task
      .catch((error) => {
        if (!isCurrentCssIdentity(identity)) return;
        if (liveEpoch !== null) onInspectorLivePropertiesRejected?.(liveEpoch);
        cssWorkspaceMutationFailure = error instanceof Error ? error.message : String(error);
        onStatusUpdate?.(`${label} nu a putut fi aplicat în sesiunea proiectului: ${cssWorkspaceMutationFailure}`, "error");
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
    selectedElement = null,
    projectRoot = "",
    runtimeSessionId = "",
    previewSelection = { kind: "none" } as PreviewSelectionState,
    sourceGraph = null,
    selectedTemplateSourceNode = null,
    saveState = "idle",
    targetCssFile = "",
    codeSelectedCssTarget = null,
    cssSourceRevision = 0,
    activeRenderedTemplatePath = null,
    previewDevice = "desktop" as CssViewport,
    refreshToken = 0,
    jsRefreshToken = 0,
    workspaceRevision = 0,
    previewRevision = "",
    blockPropertiesHeight = 220,
    blockPropertiesCollapsed = false,
    cachebustAssets = false,
    cssFiles = [],
    projectFiles = [],
    setTargetCssFile = () => {},
    scssVariables = [],
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
    deleteHtmlElement,
    editSelectedTeraLayer,
    deleteSelectedTeraNode,
    openSelectedTeraSource,
    pendingTag = null,
    tagStatus = "",
    changeElementTag,
    openSourceLocation,
    onLivePropertyChange,
    onLivePropertiesChange,
    onCssWorkspaceMutationCommitted,
    onInspectorLivePropertiesRejected,
    injectPreviewCss,
    onStatusUpdate,
    onPendingChange,
    onSourceContextChange,
    onInspectorTabChange,
    beforeInspectorTabChange,
    onCssCodeTargetChange,
    getOpenCssRuleContext,
    applyNativeBlockOption,
    persistBlockPropertiesLayout,
  }: {
    selectedElement?: SelectionInfo | null;
    projectRoot?: string;
    runtimeSessionId?: string;
    previewSelection?: PreviewSelectionState;
    sourceGraph?: SourceGraph | null;
    selectedTemplateSourceNode?: SourceGraphNode | null;
    saveState?: SaveState;
    targetCssFile?: string;
    codeSelectedCssTarget?: { selector: string; file: string; revision: number } | null;
    cssSourceRevision?: number;
    activeRenderedTemplatePath?: string | null;
    previewDevice?: CssViewport;
    refreshToken?: number;
    jsRefreshToken?: number;
    workspaceRevision?: number;
    previewRevision?: string;
    blockPropertiesHeight?: number;
    blockPropertiesCollapsed?: boolean;
    cachebustAssets?: boolean;
    cssFiles?: string[];
    projectFiles?: ProjectFile[];
    setTargetCssFile?: (path: string) => void;
    scssVariables?: ScssVariable[];
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
    deleteHtmlElement: () => void | Promise<void>;
    editSelectedTeraLayer: () => void | Promise<void>;
    deleteSelectedTeraNode: () => void | Promise<void>;
    openSelectedTeraSource: () => void | Promise<void>;
    pendingTag?: string | null;
    tagStatus?: string;
    changeElementTag: (tag: string) => void;
    openSourceLocation: (source: string) => void;
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
    onSourceContextChange?: (context: { label: string; value: string; openable?: boolean } | null) => void;
    onInspectorTabChange?: (tab: InspectorTab) => void;
    beforeInspectorTabChange?: (from: InspectorTab, to: InspectorTab) => void | Promise<void>;
    onCssCodeTargetChange?: (target: { selector: string; file: string }) => void;
    getOpenCssRuleContext?: (file: string, selector: string, viewport: CssViewport) => CssRuleContext | null;
    applyNativeBlockOption: (request: ApplyNativeBlockOptionRequest) => Promise<EditorActionOutcome>;
    persistBlockPropertiesLayout?: (height: number, collapsed: boolean) => void;
  } = $props();

  // AppState resolves the canonical ProjectWorkspace target and remains the
  // authority. Do not loosen that decision from preview-only metadata here.
  const canEditHtmlEffective = $derived(canEditHtml);
  const hasTeraSelection = $derived(previewSelection.kind === "tera" && !selectedElement);

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
      onStatusUpdate?.(`Schimbarea tabului Inspector a fost blocată: ${message}`, "error");
      return false;
    }
    if (serial !== inspectorTabChangeSerial || inspectorTab !== previousTab) return false;
    inspectorTab = nextTab;
    return true;
  }

  let selectedClass = $state<string | null>(null);
  let selectorSuffix = $state(""); // active pseudo: "" | ":hover" | ":focus" | ":active"
  let customSuffix = $state("");
  let usingCustom = $state(false);

  const activeSuffix = $derived(usingCustom ? customSuffix : selectorSuffix);
  const effectiveSelector = $derived(selectedClass ? "." + selectedClass + activeSuffix : null);
  const viewportLabel = $derived(
    previewDevice === "tablet" ? "Tablet"
      : previewDevice === "mobile" ? "Mobil"
        : "Desktop",
  );

  let classRules = $state<CssProperty[]>([]);
  let cssRuleContext = $state<CssRuleContext | null>(null);
  let loadingClassRules = $state(false);
  let pendingValues = $state<Record<string, string>>({});
  let currentCssSelector = $derived(selectedElement?.cssSelector ?? null);

  let searchingClass = $state(false);
  let cssWorkspaceMutationTail: Promise<void> = Promise.resolve();
  let cssWorkspaceMutationFailure = "";

  // Call ID guard — ensures only the latest loadRulesForClass call updates state
  let loadCallId = 0;
  let sourceDetectionCallId = 0;
  let searchCallId = 0;
  let lastCssRuntimeKey = "";
  let lastCssSelectionKey = "";
  let lastHandledRefreshToken: number | null = null;
  let lastCodeCssTargetRevision = 0;
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
    sourceDetectionCallId++;
    searchCallId++;
    loadingClassRules = false;
    searchingClass = false;
    classRules = [];
    cssRuleContext = null;
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

  function selectedTemplatePath() {
    return selectedElement?.sourceLocation?.file ?? activeRenderedTemplatePath ?? null;
  }

  function selectedSourceLocationDisplay() {
    const source = selectedElement?.sourceLocation ?? null;
    return source
      ? formatSourceEditLocation({ ...source, file: projectRelativeZolaPath(source.file) })
      : "";
  }

  function selectedPageJsPath() {
    const templatePath = selectedTemplatePath();
    return templatePath ? pageJsRelativePath(templatePath) : null;
  }

  function syntheticCssRuleContext(file: string, selector: string, viewport: CssViewport): CssRuleContext {
    return {
      file,
      selector,
      viewport,
      resolvedBreakpoint: viewport === "tablet" ? "$bp-tableta" : viewport === "mobile" ? "$bp-mobil" : null,
      baseRules: [],
      viewportRules: [],
      hasBaseRule: false,
      hasViewportRule: false,
    };
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
    const target = codeSelectedCssTarget;
    if (!target || target.revision === lastCodeCssTargetRevision) return;
    lastCodeCssTargetRevision = target.revision;
    const state = inspectorStateForCssSelector(target.selector);
    if (!state) return;

    inspectorTab = "css";
    selectedClass = state.selectedClass;
    if (target.file && target.file !== targetCssFile) {
      setTargetCssFile(target.file);
    }
    void tick().then(() => {
      selectorSuffix = state.selectorSuffix;
      customSuffix = state.customSuffix;
      usingCustom = state.usingCustom;
    });
  });

  $effect(() => {
    const revision = cssSourceRevision;
    const selector = effectiveSelector;
    const file = targetCssFile;
    const viewport = previewDevice;
    void revision;
    if (!selector || !file) return;
    const context = getOpenCssRuleContext?.(file, selector, viewport);
    if (!context) return;
    applyLiveCssRuleContext(context);
    pendingValues = untrack(() => pendingValuesForCurrentSelector());
  });

  $effect(() => {
    const nextRefreshToken = refreshToken;
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
      selectedElement?.classes.includes(classToRefresh) &&
      selectorToRefresh &&
      fileToRefresh,
    );

    if (!keepClassSelected) {
      selectedClass = null;
      selectorSuffix = "";
      customSuffix = "";
      usingCustom = false;
    }
    loadingClassRules = false;
    classRules = [];
    cssRuleContext = null;
    cssTargetInfo = null;
    pendingValues = untrack(() => pendingValuesForCurrentSelector());
    searchingClass = false;
    loadCallId++;
    sourceDetectionCallId++;
    untrack(() => {
      onPendingChange?.("css", false);
      onPendingChange?.("js", false);
    });

    if (keepClassSelected && selectorToRefresh && fileToRefresh) {
      untrack(() => {
        void loadRulesForClass(selectorToRefresh, fileToRefresh, viewportToRefresh);
      });
    }
  });

  $effect(() => {
    let context: { label: string; value: string; openable?: boolean } | null = null;
    if (hasTeraSelection && selectedTemplateSourceNode) {
      context = {
        label: "Tera",
        value: selectedTemplateSourceNode.range
          ? formatSourceEditLocation({
              file: projectRelativeZolaPath(selectedTemplateSourceNode.file),
              line: selectedTemplateSourceNode.range.line,
              column: selectedTemplateSourceNode.range.column,
            })
          : projectRelativeZolaPath(selectedTemplateSourceNode.file),
        openable: true,
      };
    } else if (inspectorTab === "html" && selectedElement?.sourceLocation) {
      context = { label: "HTML", value: selectedSourceLocationDisplay(), openable: true };
    } else if (inspectorTab === "css" && targetCssFile) {
      context = { label: `SCSS · ${viewportLabel}`, value: targetCssFile, openable: true };
    } else if (inspectorTab === "js") {
      const jsPath = selectedPageJsPath();
      if (jsPath) {
        context = {
          label: "JS",
          value: jsPath,
          openable: projectFiles.some((file) => file.relativePath === jsPath),
        };
      }
    }
    onSourceContextChange?.(context);
  });


  $effect(() => {
    const nextSelectionKey = selectedElement?.domPath ?? "";
    const activeClass = untrack(() => selectedClass);

    if (nextSelectionKey === lastCssSelectionKey) {
      if (activeClass && selectedElement && !selectedElement.classes.includes(activeClass)) {
        selectedClass = null;
      }
      return;
    }

    lastCssSelectionKey = nextSelectionKey;
    if (!activeClass || !selectedElement?.classes.includes(activeClass)) {
      selectedClass = null;
    }
    loadingClassRules = false;
    classRules = [];
    cssRuleContext = null;
    cssTargetInfo = null;
    pendingValues = untrack(() => pendingValuesForCurrentSelector());
    loadCallId++;
  });

  // Reset pseudo context whenever selected class changes (element change or chip click)
  $effect(() => {
    void selectedClass;
    selectorSuffix = "";
    customSuffix = "";
    usingCustom = false;
  });

  $effect(() => {
    const sel = effectiveSelector;
    const file = targetCssFile;
    if (inspectorTab === "css" && sel && file) {
      onCssCodeTargetChange?.({ selector: sel, file });
    }
  });

  $effect(() => {
    const sel = effectiveSelector;
    const file = targetCssFile;
    const viewport = previewDevice;
    // ProjectSession face parte din cheia proiecției. O redeschidere la același
    // path trebuie să invalideze explicit citirea Inspectorului din runtime A.
    const sessionRoot = projectRoot;
    const sessionId = runtimeSessionId;
    if (!sel || !file || !sessionRoot || !sessionId) {
      loadingClassRules = false;
      classRules = [];
      cssRuleContext = null;
      cssTargetInfo = null;
      pendingValues = {};
      return;
    }
    pendingValues = untrack(() => pendingValuesForCurrentSelector());

    const target = untrack(() => cssTargetInfo);
    if (target?.pageOwned && !target.exists && target.file === file && target.selector === sel) {
      loadCallId++;
      loadingClassRules = false;
      cssRuleContext = syntheticCssRuleContext(file, sel, viewport);
      classRules = [];
      return;
    }

    untrack(() => {
      void loadRulesForClass(sel, file, viewport);
    });
  });

  async function loadRulesForClass(selector: string, file: string, viewport: CssViewport) {
    const identity = captureCssIdentity();
    const myCallId = ++loadCallId;
    loadingClassRules = true;
    classRules = [];
    cssRuleContext = null;
    pendingValues = untrack(() => pendingValuesForCurrentSelector());
    const openSourceContext = getOpenCssRuleContext?.(file, selector, viewport);
    if (openSourceContext) {
      applyLiveCssRuleContext(openSourceContext);
      if (myCallId === loadCallId) loadingClassRules = false;
      return;
    }
    try {
      const currentTarget = untrack(() => cssTargetInfo);
      if (currentTarget?.file === file && currentTarget.pageOwned && !currentTarget.exists) {
        cssRuleContext = syntheticCssRuleContext(file, selector, viewport);
        classRules = [];
        return;
      }

      const context = await getCssRuleContext(file, selector, viewport, identity);

      if (myCallId !== loadCallId || !isCurrentCssIdentity(identity)) return;
      cssRuleContext = context;
      classRules = context.hasViewportRule ? context.viewportRules : context.baseRules;

      if (!context.hasBaseRule) {
        const templatePath = selectedTemplatePath();
        const target = await resolvePageCssTarget({
          templatePath,
          selector,
          scssFiles: cssFiles,
          fallbackFile: file,
        }, identity);
        if (myCallId !== loadCallId || !isCurrentCssIdentity(identity)) return;
        cssTargetInfo = target;
        if (target.file && target.file !== file) {
          setTargetCssFile(target.file);
          return;
        }
        if (target.pageOwned && !target.exists) {
          cssRuleContext = syntheticCssRuleContext(target.file, selector, viewport);
          classRules = [];
          return;
        }
      }

      if (!context.hasBaseRule && cssFiles.length > 1) {
        const result = await findClassInScss(selector, cssFiles, identity);
        if (myCallId !== loadCallId || !isCurrentCssIdentity(identity)) return;
        if (result && result.file !== file) {
          setTargetCssFile(result.file);
          classRules = result.rules;
        } else if (result) {
          cssRuleContext = {
            file,
            selector,
            viewport,
            resolvedBreakpoint: null,
            baseRules: result.rules,
            viewportRules: result.rules,
            hasBaseRule: result.rules.length > 0,
            hasViewportRule: result.rules.length > 0,
          };
          classRules = result.rules;
        }
      }
    } catch (error) {
      if (!isCurrentCssIdentity(identity)) return;
      if (myCallId === loadCallId) {
        classRules = [];
        cssRuleContext = null;
        const message = error instanceof Error ? error.message : String(error);
        onStatusUpdate?.(`Regulile CSS nu au putut fi citite din ${file}: ${message}`, "error");
      }
    } finally {
      if (myCallId === loadCallId) {
        loadingClassRules = false;
      }
    }
  }

  async function selectClassForCss(className: string) {
    if (!await changeInspectorTab("css")) return;
    selectedClass = className;
    classRules = [];
    cssRuleContext = null;
    pendingValues = pendingValuesForCurrentSelector();

    const selector = `.${className}`;
    const identity = captureCssIdentity();
    const myDetectionId = ++sourceDetectionCallId;
    let result: PageCssTarget | null = null;
    try {
      result = await resolvePageCssTarget({
        templatePath: selectedTemplatePath(),
        selector,
        scssFiles: cssFiles,
        fallbackFile: targetCssFile || null,
      }, identity);
    } catch (error) {
      if (isCurrentCssIdentity(identity)) {
        const message = error instanceof Error ? error.message : String(error);
        onStatusUpdate?.(`Ținta CSS pentru ${selector} nu a putut fi rezolvată: ${message}`, "error");
      }
      return;
    }

    if (
      myDetectionId !== sourceDetectionCallId
      || selectedClass !== className
      || !isCurrentCssIdentity(identity)
      || !result
    ) {
      return;
    }

    cssTargetInfo = result;
    if (result.pageOwned && !result.exists) {
      classRules = [];
      cssRuleContext = syntheticCssRuleContext(result.file, selector, previewDevice);
      loadingClassRules = false;
    }
    if (result.file !== targetCssFile) {
      setTargetCssFile(result.file);
    }
  }

  function captureCurrentCssMutationTarget() {
    if (!effectiveSelector || !targetCssFile) return;
    const identity = captureCssIdentity();
    const file = targetCssFile;
    const selector = effectiveSelector;
    const viewport = previewDevice;
    const pageTarget = cssTargetInfo;
    const targetKey = [
      identity.expectedProjectRoot,
      identity.expectedSessionId,
      file,
      selector,
      viewport,
      pageTarget?.pageOwned ? pageTarget.templatePath ?? "page" : "existing",
    ].join("\u0000");
    return { identity, file, selector, viewport, pageTarget, targetKey };
  }

  function draftCssProperty(property: string, value: string) {
    const target = captureCurrentCssMutationTarget();
    if (!target) return;
    const baseline = captureCssPendingValueBaseline(pendingValues, property);
    const nextPendingValues = { ...pendingValues, [property]: value };
    pendingValues = nextPendingValues;
    const appliedLiveEpoch = onLivePropertiesChange?.(
      target.selector,
      nextPendingValues,
      target.viewport,
    );
    const liveEpoch = typeof appliedLiveEpoch === "number" ? appliedLiveEpoch : null;
    const { identity, file, selector, viewport, pageTarget, targetKey } = target;
    if (pageTarget?.pageOwned && pageTarget.templatePath) {
      stageCssRuleMutation({
        key: targetKey,
        identity,
        label: `CSS ${selector}`,
        liveEpoch,
        run: (properties) => setPageCssRuleAtViewport({
          templatePath: pageTarget.templatePath ?? "",
          relativePath: file,
          selector,
          properties,
          viewport,
          cachebustAssets,
        }, identity),
      }, property, value, baseline);
    } else {
      stageCssRuleMutation({
        key: targetKey,
        identity,
        label: `CSS ${selector}`,
        liveEpoch,
        run: (properties) => setCssRuleAtViewport({
          relativePath: file,
          selector,
          properties,
          viewport,
        }, identity),
      }, property, value, baseline);
    }
    if (!onLivePropertiesChange) onLivePropertyChange?.(selector, property, value);
    onStatusUpdate?.(`Previzualizare CSS modificată: ${property} — commit la încheierea editării`, "unsaved");
  }

  function commitCssProperty(property: string, value?: string) {
    if (value !== undefined && pendingValues[property] !== value) {
      draftCssProperty(property, value);
    }
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
    onStatusUpdate?.(`Editarea CSS pentru ${property} a fost anulată`, "idle");
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
    commit: commitCssProperty,
    cancel: cancelCssProperty,
    continuous: continuousCssProperty,
  };

  async function searchClassInAllFiles() {
    const sel = effectiveSelector;
    if (!sel || !cssFiles.length) return;
    const identity = captureCssIdentity();
    const myCallId = ++searchCallId;
    searchingClass = true;
    try {
      const result = await findClassInScss(sel, cssFiles, identity);
      if (myCallId !== searchCallId || !isCurrentCssIdentity(identity)) return;
      if (result) {
        cssTargetInfo = {
          file: result.file,
          selector: sel,
          targetKind: "existing",
          exists: true,
          linked: false,
          href: null,
          templatePath: selectedTemplatePath(),
          pageOwned: false,
          reason: "Regula există deja în acest fișier.",
        };
        setTargetCssFile(result.file);
        classRules = result.rules;
        pendingValues = pendingValuesForCurrentSelector();
      }
    } finally {
      if (myCallId === searchCallId) searchingClass = false;
    }
  }

</script>

<aside
  class="inspector-pane"
  aria-label="Inspector"
>
  {#if hasTeraSelection}
    <div class="inspector-main tera-main">
      <div class="inspector-scroll">
        <TeraSourceCard
          node={selectedTemplateSourceNode}
          graph={sourceGraph}
          previewSelector={previewSelection.kind === "tera" ? previewSelection.selector : null}
          {editSelectedTeraLayer}
          {openSelectedTeraSource}
          {deleteSelectedTeraNode}
        />
      </div>
    </div>
  {:else}
    <div class="inspector-context">
      <section class="selection-card">
        <div class="selection-heading">
          <p class="selector">{selectedElement?.cssSelector ?? "Niciun element selectat"}</p>
          {#if selectedElement?.blockContext}
            <span class="block-chip">
              {selectedElement.blockContext.providerId} › &lt;{selectedElement.tag}&gt;
            </span>
          {/if}
        </div>
        <div class="selection-meta">
          {#if selectedElement?.classes.length}
            {#each selectedElement.classes as className}
              <button
                class="class-chip"
                class:active={selectedClass === className}
                type="button"
                title="Editează .{className}"
                onclick={() => { void selectClassForCss(className); }}

              >{className}</button>
            {/each}
          {:else}
            <span class="subtle-chip">fără clase</span>
          {/if}
        </div>
      </section>
    </div>

    <div class="inspector-main">
      <nav class="inspector-tabs" aria-label="Secțiuni inspector">
        <button class:active={inspectorTab === "html"} type="button" onclick={() => { void changeInspectorTab("html"); }}>
          HTML
        </button>
        <button class:active={inspectorTab === "css"} type="button" onclick={() => { void changeInspectorTab("css"); }}>
          CSS
        </button>
        <button class:active={inspectorTab === "js"} type="button" onclick={() => { void changeInspectorTab("js"); }}>
          JS
        </button>
      </nav>
      <div class="inspector-scroll">

        {#if inspectorTab === "css"}
        <CssPane
          {selectedElement}
          {selectedClass}
          {effectiveSelector}
          {activeSuffix}
          {viewportLabel}
          {previewDevice}
          {targetCssFile}
          pageCssTarget={cssTargetInfo}
          cssFileCount={cssFiles.length}
          {cssRuleContext}
          {classRules}
          {pendingValues}
          {scssVariables}
          {scannedAssets}
          {loadingClassRules}
          {searchingClass}
          {selectorSuffix}
          {customSuffix}
          {usingCustom}
          {cssPropertyEdit}
          {searchClassInAllFiles}
          setSelectorSuffix={(value) => { selectorSuffix = value; }}
          setCustomSuffix={(value) => { customSuffix = value; }}
          setUsingCustom={(value) => { usingCustom = value; }}
        />

        {:else if inspectorTab === "js"}
        <JsPane
          {selectedElement}
          {projectRoot}
          {runtimeSessionId}
          refreshToken={jsRefreshToken}
          onSwitchToHtml={() => { void changeInspectorTab("html"); }}
        />

        {:else if inspectorTab === "html"}
        <HtmlPane
          {selectedElement}
          canEditHtml={canEditHtmlEffective}
          {attributeValues}
          {textContentValue}
          {imageSourceValue}
          classEditorValue={classEditorValue}
          {pendingTag}
          {scannedAssets}
          {isActivePreviewHtmlSource}
          {attributeStatus}
          {textStatus}
          {classStatus}
          {imageStatus}
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
          deleteHtmlElement={deleteHtmlElement}
          changeElementTag={changeElementTag}
          {tagStatus}
        />

        {/if}
      </div>
    </div>
    <BlockPropertiesPane
      {selectedElement}
      {projectRoot}
      {runtimeSessionId}
      {workspaceRevision}
      {previewRevision}
      height={blockPropertiesHeight}
      collapsed={blockPropertiesCollapsed}
      onLayoutCommit={persistBlockPropertiesLayout}
      onApply={applyNativeBlockOption}
    />
  {/if}
</aside>

<style>
  .inspector-pane {
    position: relative;
    display: flex;
    flex-direction: column;
    min-height: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-panel);
    overflow: hidden;
    overscroll-behavior: contain;
    background: var(--surface);
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

  .selection-card {
    padding: 10px;
    border: 1px solid var(--border-2);
    border-radius: var(--radius-control);
    background: var(--surface-2);
  }

  .selection-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    min-width: 0;
  }

  .selector {
    display: inline-flex;
    max-width: 100%;
    margin: 0;
    padding: 5px 7px;
    border-radius: var(--radius-control);
    color: #ffffff;
    font-family: "JetBrains Mono", "SFMono-Regular", Consolas, monospace;
    font-size: 12px;
    background: var(--selector-bg);
  }

  .block-chip {
    overflow: hidden;
    max-width: 48%;
    padding: 3px 6px;
    border-radius: var(--radius-control);
    color: var(--brand-strong);
    background: var(--brand-soft);
    font-size: 11px;
    font-weight: 700;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .selection-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 8px;
  }

  .class-chip,
  .subtle-chip {
    display: inline-flex;
    align-items: center;
    min-height: 24px;
    padding: 0 8px;
    border: 1px solid var(--chip-border);
    border-radius: var(--radius-control);
    font-size: 12px;
    font-weight: 600;
    background: var(--chip-bg);
    cursor: pointer;
    color: var(--text);
    transition: border-color 120ms ease, background 120ms ease;
  }

  .class-chip:hover {
    border-color: var(--border-strong);
    background: var(--control-hover);
  }

  .class-chip.active {
    border-color: var(--brand);
    color: var(--brand-strong);
    background: var(--brand-soft);
  }

  .subtle-chip {
    border-color: var(--border-3);
    color: var(--text-muted);
    background: var(--surface-9);
    cursor: default;
  }

  .inspector-tabs {
    display: grid;
    flex: 0 0 auto;
    grid-template-columns: repeat(3, 1fr);
    gap: 0;
    margin-top: 10px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .inspector-tabs button {
    min-height: 32px;
    padding: 0 9px;
    border: 0;
    border-bottom: 2px solid transparent;
    border-radius: 0;
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 600;
    background: transparent;
  }

  .inspector-tabs .active {
    border-bottom-color: var(--brand);
    color: var(--brand-strong);
    background: transparent;
  }

</style>
