import type {
  CssInspectorContextResolution,
  CssRuleContext,
  CssViewport,
} from "$lib/css/contracts";
import {
  createCssRequestIdentity,
  cssRequestIdentityMatches,
  resolveCssInspectorContext,
  type CssRequestIdentity,
} from "$lib/css/io";
import type { InspectorSelectionSummarySnapshot, SelectionSnapshot } from "$lib/editor/contracts";
import { primarySelectionEntry, selectionResolution } from "$lib/kernel/selection-read-model";
import type { SelectionMutationIdentity } from "$lib/preview/contracts";
import type { CssInspectorState } from "$lib/inspector/css-inspector-state.svelte";
import {
  cssInspectorReadIsCurrent,
  cssInspectorSubjectKey,
  sameCssSemanticSelection,
} from "$lib/inspector/css-selection-stability";

export type CssInspectorCodeTarget = Readonly<{
  selector: string;
  file: string;
  property?: string | null;
  expectedSelectionRevision?: number | null;
}>;

export type CssInspectorReaderStatus =
  | Readonly<{ kind: "readFailed"; file: string; error: string }>
  | Readonly<{ kind: "targetFailed"; selector: string; error: string }>;

export type CssInspectorReaderInput = Readonly<{
  projectRoot: string;
  runtimeSessionId: string;
  targetCssFile: string;
  cssSourceRevision: number;
  activeRenderedTemplatePath: string | null;
  previewDevice: CssViewport;
  refreshToken: number;
  historyProjectionQuiesced: boolean;
  workspaceRevision: number;
  htmlProjectionPending: boolean;
  selectionSnapshot: SelectionSnapshot | null;
  selectionSummary: InspectorSelectionSummarySnapshot | null;
  presentedFocusSelector: string | null;
}>;

type ResolveCssInspector = typeof resolveCssInspectorContext;

export type CssInspectorReaderDependencies = Readonly<{
  resolve?: ResolveCssInspector;
  createIdentity?: typeof createCssRequestIdentity;
  identityMatches?: typeof cssRequestIdentityMatches;
  getOpenContext?: (
    file: string,
    selector: string,
    viewport: CssViewport,
  ) => CssRuleContext | null;
  changeCodeTarget?: (target: CssInspectorCodeTarget) => boolean | Promise<boolean>;
  reportStatus?: (status: CssInspectorReaderStatus) => void;
  resetPendingAreas?: () => void;
}>;

export function captureCssInspectorSelectionIdentity(
  snapshot: SelectionSnapshot | null,
  projectRoot: string,
  runtimeSessionId: string,
): SelectionMutationIdentity | null {
  const anchor = primarySelectionEntry(snapshot)?.anchor;
  if (
    !snapshot
    || selectionResolution(snapshot) !== "resolved"
    || snapshot.projectRoot !== projectRoot
    || snapshot.runtimeSessionId !== runtimeSessionId
    || !anchor
    || !Number.isSafeInteger(snapshot.selectionRevision)
    || snapshot.selectionRevision <= 0
  ) return null;
  const members = snapshot.members.map((member) => ({
    memberId: member.memberId,
    editorNodeId: member.anchor.editorNodeId?.trim() || null,
    sourceNodeId: member.anchor.sourceNodeId?.trim() || null,
    renderInstanceId: member.anchor.renderInstanceId?.trim() || null,
  }));
  if (
    !snapshot.primaryMemberId
    || members.length === 0
    || !members.some((member) => member.memberId === snapshot.primaryMemberId)
  ) return null;
  return Object.freeze({
    selectionRevision: snapshot.selectionRevision,
    workspaceRevision: snapshot.canvasIdentity.workspaceRevision,
    primaryMemberId: snapshot.primaryMemberId,
    members,
  });
}

export function selectedCssInspectorTemplatePath(
  snapshot: SelectionSnapshot | null,
  activeRenderedTemplatePath: string | null,
) {
  const primary = primarySelectionEntry(snapshot);
  if (primary?.subject.kind === "cssRule") return activeRenderedTemplatePath;
  return primary?.provenance.definition?.file
    ?? primary?.provenance.composition?.file
    ?? activeRenderedTemplatePath;
}

/** Latest-wins reader for the Rust-owned CSS Inspector projection. */
export class CssInspectorReader {
  private readonly state: CssInspectorState;
  private readonly dependencies: Required<Pick<
    CssInspectorReaderDependencies,
    "resolve" | "createIdentity" | "identityMatches"
  >> & CssInspectorReaderDependencies;
  private serial = 0;
  private runtimeKey = "";
  private selectionKey = "";
  private projectionKey = "";
  private openSourceKey = "";
  private lastRefreshToken: number | null = null;
  private current: CssInspectorReaderInput | null = null;
  private disposed = false;

  constructor(state: CssInspectorState, dependencies: CssInspectorReaderDependencies = {}) {
    this.state = state;
    this.dependencies = {
      ...dependencies,
      resolve: dependencies.resolve ?? resolveCssInspectorContext,
      createIdentity: dependencies.createIdentity ?? createCssRequestIdentity,
      identityMatches: dependencies.identityMatches ?? cssRequestIdentityMatches,
    };
  }

  reconcile(input: CssInspectorReaderInput): Promise<void> | null {
    if (this.disposed) return null;
    this.current = input;
    this.state.syncPresentation(
      input.presentedFocusSelector,
      input.previewDevice,
      input.targetCssFile,
    );

    const runtimeKey = `${input.projectRoot}\u0000${input.runtimeSessionId}`;
    if (runtimeKey !== this.runtimeKey) {
      this.runtimeKey = runtimeKey;
      this.invalidate();
      this.selectionKey = "";
      this.projectionKey = "";
      this.openSourceKey = "";
      this.lastRefreshToken = input.refreshToken;
      this.state.resetSession();
    }

    const expectedSelection = this.captureSelection(input);
    if (input.htmlProjectionPending && !expectedSelection) return null;
    const selectionKey = input.selectionSnapshot
      ? `${input.selectionSnapshot.runtimeSessionId}:${cssInspectorSubjectKey(expectedSelection)}`
      : "";
    if (selectionKey !== this.selectionKey) {
      this.selectionKey = selectionKey;
      this.invalidate();
      this.projectionKey = "";
      this.state.resetProjection(true);
    } else if (
      expectedSelection
      && this.state.selectionIdentity
      && (
        expectedSelection.selectionRevision !== this.state.selectionIdentity.selectionRevision
        || !sameCssSemanticSelection(expectedSelection, this.state.selectionIdentity)
      )
    ) {
      this.state.rebaseSelection(expectedSelection);
    }

    if (this.lastRefreshToken === null) {
      this.lastRefreshToken = input.refreshToken;
    } else if (
      input.refreshToken !== this.lastRefreshToken
      && !input.historyProjectionQuiesced
    ) {
      this.lastRefreshToken = input.refreshToken;
      this.state.clearPendingValues();
      this.dependencies.resetPendingAreas?.();
      this.projectionKey = "";
      const selectedClass = this.state.selectedClass;
      const keepProjection = Boolean(
        selectedClass
        && input.selectionSummary?.classes.includes(selectedClass)
        && this.state.effectiveSelector
        && input.targetCssFile,
      );
      if (!keepProjection) {
        this.invalidate();
        this.state.resetProjection(true);
        return null;
      }
    }

    this.reconcileOpenSource(input);
    if (input.historyProjectionQuiesced) {
      this.projectionKey = "";
      return null;
    }
    if (
      !this.state.effectiveSelector
      || !input.targetCssFile
      || !input.projectRoot
      || !input.runtimeSessionId
      || !expectedSelection
    ) {
      this.invalidate();
      this.projectionKey = "";
      this.state.resetProjection(true);
      return null;
    }

    const projectionKey = [
      input.projectRoot,
      input.runtimeSessionId,
      expectedSelection.selectionRevision,
      this.state.effectiveSelector,
      input.targetCssFile,
      input.previewDevice,
      input.refreshToken,
    ].join("\u0000");
    if (projectionKey === this.projectionKey) return null;
    this.projectionKey = projectionKey;
    return this.load(
      this.state.effectiveSelector,
      input.targetCssFile,
      input.previewDevice,
      expectedSelection,
      true,
    );
  }

  async selectClass(className: string): Promise<"allowed" | "blocked"> {
    const input = this.current;
    if (!input || input.historyProjectionQuiesced || this.disposed) return "blocked";
    const expectedSelection = this.captureSelection(input);
    if (!expectedSelection) return "blocked";
    this.state.clearPendingValues();
    const selector = `.${className}`;
    const identity = this.dependencies.createIdentity(input.projectRoot, input.runtimeSessionId);
    const callId = ++this.serial;
    this.projectionKey = "";
    const retainProjection = this.state.hasStableProjection(expectedSelection);
    this.state.beginRead(retainProjection);
    try {
      const resolution = await this.resolve(
        input,
        selector,
        expectedSelection,
        identity,
        input.previewDevice,
      );
      if (
        callId !== this.serial
        || !this.identityIsCurrent(identity)
        || !this.subjectIsCurrent(expectedSelection)
      ) return "blocked";
      if (resolution.state === "ambiguous" || !resolution.target) {
        this.applyResolution(resolution, expectedSelection);
        return "blocked";
      }
      const allowed = await this.dependencies.changeCodeTarget?.({
        selector,
        file: resolution.target.file,
        expectedSelectionRevision: expectedSelection.selectionRevision,
      });
      if (callId !== this.serial || !this.subjectIsCurrent(expectedSelection)) return "blocked";
      return allowed ? "allowed" : "blocked";
    } catch (cause) {
      if (this.identityIsCurrent(identity)) {
        this.dependencies.reportStatus?.({
          kind: "targetFailed",
          selector,
          error: cause instanceof Error ? cause.message : String(cause),
        });
      }
      return "blocked";
    } finally {
      if (callId === this.serial) this.state.finishRead();
    }
  }

  captureCurrentSelection() {
    return this.current ? this.captureSelection(this.current) : null;
  }

  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.current = null;
    this.invalidate();
    this.state.resetSession();
  }

  private async load(
    selector: string,
    file: string,
    viewport: CssViewport,
    expectedSelection: SelectionMutationIdentity,
    retainIfStable: boolean,
  ) {
    const input = this.current;
    if (!input || input.historyProjectionQuiesced) return;
    const identity = this.dependencies.createIdentity(input.projectRoot, input.runtimeSessionId);
    const expectedRefreshToken = input.refreshToken;
    const callId = ++this.serial;
    const retainProjection = retainIfStable && this.state.hasStableProjection(expectedSelection);
    this.state.beginRead(retainProjection);
    try {
      const resolution = await this.resolve(
        input,
        selector,
        expectedSelection,
        identity,
        viewport,
      );
      if (!this.readIsCurrent(callId, identity, expectedRefreshToken, expectedSelection)) return;
      if (
        resolution.state !== "ambiguous"
        && resolution.target
        && resolution.target.file !== file
      ) {
        const allowed = await this.dependencies.changeCodeTarget?.({
          selector,
          file: resolution.target?.file ?? file,
          expectedSelectionRevision: expectedSelection.selectionRevision,
        });
        if (
          !allowed
          && callId === this.serial
          && !retainProjection
        ) this.state.resetProjection(true);
        return;
      }
      this.applyResolution(resolution, expectedSelection);
    } catch (cause) {
      if (!this.readIsCurrent(callId, identity, expectedRefreshToken, expectedSelection)) return;
      if (!retainProjection) this.state.resetProjection(true);
      this.dependencies.reportStatus?.({
        kind: "readFailed",
        file,
        error: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      if (callId === this.serial) this.state.finishRead();
    }
  }

  private resolve(
    input: CssInspectorReaderInput,
    selector: string,
    expectedSelection: SelectionMutationIdentity,
    identity: CssRequestIdentity,
    viewport: CssViewport,
  ) {
    return this.dependencies.resolve({
      templatePath: selectedCssInspectorTemplatePath(
        input.selectionSnapshot,
        input.activeRenderedTemplatePath,
      ),
      selector,
      viewport,
      fallbackFile: input.targetCssFile || null,
      expectedWorkspaceRevision: input.workspaceRevision,
      expectedSelection,
    }, identity);
  }

  private applyResolution(
    resolution: CssInspectorContextResolution,
    expectedSelection: SelectionMutationIdentity,
  ) {
    const context = resolution.ruleContext;
    const openContext = context
      ? this.dependencies.getOpenContext?.(
        context.file,
        context.selector,
        context.viewport,
      ) ?? null
      : null;
    this.state.applyResolution(resolution, expectedSelection, openContext);
  }

  private reconcileOpenSource(input: CssInspectorReaderInput) {
    const selector = this.state.effectiveSelector;
    const sourceKey = [
      input.cssSourceRevision,
      selector ?? "",
      input.targetCssFile,
      input.previewDevice,
    ].join("\u0000");
    if (sourceKey === this.openSourceKey) return;
    this.openSourceKey = sourceKey;
    const resolution = this.state.resolution;
    if (
      !selector
      || !input.targetCssFile
      || resolution?.state === "ambiguous"
      || resolution?.target?.file !== input.targetCssFile
      || resolution.selector !== selector
      || resolution.viewport !== input.previewDevice
    ) return;
    const context = this.dependencies.getOpenContext?.(
      input.targetCssFile,
      selector,
      input.previewDevice,
    );
    if (context) this.state.applyLiveContext(context);
    this.state.clearPendingValues();
  }

  private captureSelection(input: CssInspectorReaderInput) {
    return captureCssInspectorSelectionIdentity(
      input.selectionSnapshot,
      input.projectRoot,
      input.runtimeSessionId,
    );
  }

  private identityIsCurrent(identity: CssRequestIdentity) {
    const current = this.current;
    return Boolean(
      current
      && this.dependencies.identityMatches(
        identity,
        current.projectRoot,
        current.runtimeSessionId,
      ),
    );
  }

  private subjectIsCurrent(expected: SelectionMutationIdentity) {
    const expectedSubject = cssInspectorSubjectKey(expected);
    return Boolean(
      expectedSubject
      && expectedSubject === cssInspectorSubjectKey(this.captureCurrentSelection()),
    );
  }

  private readIsCurrent(
    callId: number,
    identity: CssRequestIdentity,
    expectedRefreshToken: number,
    expectedSelection: SelectionMutationIdentity,
  ) {
    const current = this.current;
    return Boolean(
      current
      && callId === this.serial
      && this.identityIsCurrent(identity)
      && !current.historyProjectionQuiesced
      && current.refreshToken === expectedRefreshToken
      && cssInspectorReadIsCurrent(expectedSelection, this.captureSelection(current)),
    );
  }

  private invalidate() {
    this.serial += 1;
    this.state.finishRead();
  }
}
