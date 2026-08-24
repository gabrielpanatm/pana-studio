import type {
  CssInspectorContextResolution,
  CssRuleContext,
  CssViewport,
  PageCssTarget,
} from "$lib/css/contracts";
import type { CssProperty } from "$lib/css/property-contract";
import type { CssPropertyEditController } from "$lib/inspector/css-property-edit";
import type { SelectionMutationIdentity } from "$lib/preview/contracts";
import {
  cssInspectorSubjectKey,
  sameCssSemanticSelection,
} from "$lib/inspector/css-selection-stability";

export type CssInspectorSelectorState = Readonly<{
  selectedClass: string;
  selectorSuffix: string;
  customSuffix: string;
  usingCustom: boolean;
}>;

export function cssInspectorSelectorState(
  selector: string,
): CssInspectorSelectorState | null {
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

/** Reactive projection owned by the CSS Inspector domain, independent of Svelte UI. */
export class CssInspectorState {
  classRules = $state<CssProperty[]>([]);
  ruleContext = $state<CssRuleContext | null>(null);
  resolution = $state<CssInspectorContextResolution | null>(null);
  selectionIdentity = $state<SelectionMutationIdentity | null>(null);
  target = $state<PageCssTarget | null>(null);
  loading = $state(false);
  pendingValues = $state<Record<string, string>>({});

  private propertyEditController: CssPropertyEditController | null = null;
  private coordinatedSelector = $state<CssInspectorSelectorState | null>(null);
  private currentViewport = $state<CssViewport>("desktop");
  private currentTargetFile = $state("");

  syncPresentation(
    focusSelector: string | null,
    viewport: CssViewport,
    targetFile: string,
  ) {
    this.coordinatedSelector = focusSelector
      ? cssInspectorSelectorState(focusSelector)
      : null;
    this.currentViewport = viewport;
    this.currentTargetFile = targetFile;
  }

  get propertyEdit() {
    if (!this.propertyEditController) {
      throw new Error("CSS Inspector property edit controller is not attached.");
    }
    return this.propertyEditController;
  }

  attachPropertyEdit(controller: CssPropertyEditController) {
    if (this.propertyEditController && this.propertyEditController !== controller) {
      throw new Error("CSS Inspector property edit controller already has an owner.");
    }
    this.propertyEditController = controller;
  }

  get selectedClass() {
    return this.coordinatedSelector?.selectedClass ?? null;
  }

  get selectorSuffix() {
    return this.coordinatedSelector?.selectorSuffix ?? "";
  }

  get customSuffix() {
    return this.coordinatedSelector?.customSuffix ?? "";
  }

  get usingCustom() {
    return this.coordinatedSelector?.usingCustom ?? false;
  }

  get effectiveSelector() {
    if (!this.selectedClass) return null;
    const suffix = this.usingCustom ? this.customSuffix : this.selectorSuffix;
    return `.${this.selectedClass}${suffix}`;
  }

  get presentedSelectorState() {
    return cssInspectorSelectorState(this.resolution?.selector ?? "")
      ?? this.coordinatedSelector;
  }

  get presentedSelector() {
    return this.resolution?.selector ?? this.effectiveSelector;
  }

  get presentedViewport() {
    return this.resolution?.viewport ?? this.currentViewport;
  }

  get projectionTransitioning() {
    const resolution = this.resolution;
    const selector = this.effectiveSelector;
    return Boolean(
      resolution
      && selector
      && (
        resolution.selector !== selector
        || resolution.viewport !== this.currentViewport
        || (resolution.target && resolution.target.file !== this.currentTargetFile)
      ),
    );
  }

  get hasEditableTarget() {
    const resolution = this.resolution;
    const selector = this.effectiveSelector;
    return Boolean(
      !this.loading
      && resolution
      && resolution.state !== "ambiguous"
      && resolution.target
      && selector
      && resolution.selector === selector
      && resolution.viewport === this.currentViewport
      && resolution.target.file === this.currentTargetFile
    );
  }

  resetProjection(clearPending = true) {
    this.loading = false;
    this.classRules = [];
    this.ruleContext = null;
    this.resolution = null;
    this.selectionIdentity = null;
    this.target = null;
    if (clearPending) this.pendingValues = {};
  }

  resetSession() {
    this.resetProjection(true);
  }

  beginRead(retainProjection: boolean) {
    if (!retainProjection) {
      this.resetProjection(false);
      this.pendingValues = {};
    }
    this.loading = !retainProjection;
  }

  finishRead() {
    this.loading = false;
  }

  hasStableProjection(expectedSelection: SelectionMutationIdentity) {
    return Boolean(
      this.resolution
      && this.resolution.state !== "ambiguous"
      && this.resolution.target
      && sameCssSemanticSelection(this.selectionIdentity, expectedSelection),
    );
  }

  rebaseSelection(expectedSelection: SelectionMutationIdentity) {
    if (!this.selectionIdentity) return;
    const currentSubject = cssInspectorSubjectKey(this.selectionIdentity);
    if (
      !currentSubject
      || currentSubject !== cssInspectorSubjectKey(expectedSelection)
    ) return;
    this.selectionIdentity = expectedSelection;
  }

  applyResolution(
    resolution: CssInspectorContextResolution,
    expectedSelection: SelectionMutationIdentity,
    openContext: CssRuleContext | null = null,
  ) {
    this.resolution = resolution;
    this.selectionIdentity = expectedSelection;
    this.target = resolution.target;
    const resolvedContext = resolution.ruleContext;
    if (!resolvedContext) {
      this.ruleContext = null;
      this.classRules = [];
      this.loading = false;
      this.settlePendingValues();
      return;
    }
    if (!openContext) {
      this.applyLiveContext(resolvedContext);
      this.settlePendingValues();
      return;
    }
    const resolvedRules = resolvedContext.hasViewportRule
      ? resolvedContext.viewportRules
      : resolvedContext.baseRules;
    const liveRules = openContext.hasViewportRule
      ? openContext.viewportRules
      : openContext.baseRules;
    const sameRules = JSON.stringify(liveRules) === JSON.stringify(resolvedRules);
    this.applyLiveContext({
      ...openContext,
      background: sameRules ? resolvedContext.background : openContext.background,
      grid: sameRules ? resolvedContext.grid : openContext.grid,
    });
    this.settlePendingValues();
  }

  applyLiveContext(context: CssRuleContext) {
    this.ruleContext = context;
    this.classRules = context.hasViewportRule ? context.viewportRules : context.baseRules;
    this.loading = false;
  }

  replacePendingValues(values: Readonly<Record<string, string>>) {
    this.pendingValues = { ...values };
  }

  clearPendingValues() {
    this.pendingValues = {};
  }

  settlePendingValues() {
    const entries = Object.entries(this.pendingValues);
    if (!entries.length) return;
    const canonical = new Map(
      this.classRules.map((rule) => [rule.property, rule.value] as const),
    );
    const next = { ...this.pendingValues };
    let changed = false;
    for (const [property, pendingValue] of entries) {
      const canonicalValue = canonical.get(property) ?? "";
      if (canonicalValue.trim() !== pendingValue.trim()) continue;
      delete next[property];
      changed = true;
    }
    if (changed) this.pendingValues = next;
  }
}
