import type { CssViewport, PageCssTarget } from "$lib/css/contracts";
import {
  createCssRequestIdentity,
  cssRequestIdentityMatches,
  setCssRuleAtViewport,
  setPageCssRuleAtViewport,
  setReusableCssRuleAtViewport,
  type CssRequestIdentity,
} from "$lib/css/io";
import type { CssMutationAuthorityReceipt } from "$lib/css/mutation-contract";
import type {
  CssContinuousEditHandlers,
  CssPendingValueBaseline,
  CssPropertyEditController,
} from "$lib/inspector/css-property-edit";
import {
  captureCssPendingValueBaseline,
  restoreCssPendingValueBaseline,
} from "$lib/inspector/css-property-edit";
import type { CssInspectorState } from "$lib/inspector/css-inspector-state.svelte";
import type { CssInspectorCodeTarget } from "$lib/inspector/css-inspector-reader";
import {
  cssSemanticSelectionKey,
  sameCssSemanticSelection,
} from "$lib/inspector/css-selection-stability";
import type { SelectionMutationIdentity } from "$lib/preview/contracts";
import { flushFileBufferDraftSync } from "$lib/session/file-buffer-draft-sync";

export type CssInspectorMutationStatus =
  | Readonly<{ kind: "saved"; label: string }>
  | Readonly<{ kind: "liveFailed"; label: string; error: string }>
  | Readonly<{ kind: "mutationFailed"; label: string; error: string }>
  | Readonly<{ kind: "previewChanged"; property: string }>
  | Readonly<{ kind: "editCancelled"; property: string }>;

export type CssInspectorMutationContext = Readonly<{
  projectRoot: string;
  runtimeSessionId: string;
  targetCssFile: string;
  previewDevice: CssViewport;
}>;

type CssMutationReceipt = Promise<{ authority: CssMutationAuthorityReceipt }>;

export type CssInspectorMutationQueueDependencies = Readonly<{
  state: CssInspectorState;
  context: () => CssInspectorMutationContext;
  captureSelection: () => SelectionMutationIdentity | null;
  flushDraftSync?: typeof flushFileBufferDraftSync;
  createIdentity?: typeof createCssRequestIdentity;
  identityMatches?: typeof cssRequestIdentityMatches;
  mutateExisting?: typeof setCssRuleAtViewport;
  mutatePage?: typeof setPageCssRuleAtViewport;
  mutateReusable?: typeof setReusableCssRuleAtViewport;
  changeCodeTarget?: (target: CssInspectorCodeTarget) => boolean | Promise<boolean>;
  applyLiveProperties?: (
    selector: string | null,
    properties: Record<string, string>,
    viewport?: CssViewport,
  ) => number | void;
  projectCommittedMutation?: (
    authority: CssMutationAuthorityReceipt,
    liveEpoch: number | null,
  ) => void | Promise<void>;
  rejectLiveProperties?: (liveEpoch: number) => void;
  reportStatus?: (status: CssInspectorMutationStatus) => void;
  setPending?: (pending: boolean) => void;
}>;

type CssMutationTarget = Readonly<{
  identity: CssRequestIdentity;
  expectedSelection: SelectionMutationIdentity;
  file: string;
  selector: string;
  viewport: CssViewport;
  pageTarget: PageCssTarget | null;
  targetKey: string;
}>;

type StagedCssRuleMutation = {
  key: string;
  identity: CssRequestIdentity;
  label: string;
  liveEpoch: number | null;
  properties: Record<string, string>;
  baselines: Record<string, CssPendingValueBaseline>;
  run: (properties: Record<string, string>) => CssMutationReceipt;
};

/** Owns Inspector CSS drafts, microtask batching and serial durable writes. */
export class CssInspectorMutationQueue {
  readonly edit: CssPropertyEditController;

  private readonly state: CssInspectorState;
  private readonly dependencies: CssInspectorMutationQueueDependencies & Required<Pick<
    CssInspectorMutationQueueDependencies,
    | "flushDraftSync"
    | "createIdentity"
    | "identityMatches"
    | "mutateExisting"
    | "mutatePage"
    | "mutateReusable"
  >>;
  private readonly staged = new Map<string, StagedCssRuleMutation>();
  private readonly continuousBindings = new Map<string, CssContinuousEditHandlers>();
  private flushPromise: Promise<void> | null = null;
  private flushScheduled = false;
  private mutationTail: Promise<void> = Promise.resolve();
  private mutationFailure = "";
  private queued = 0;
  private generation = 0;
  private sessionKey = "";
  private disposed = false;

  constructor(dependencies: CssInspectorMutationQueueDependencies) {
    this.state = dependencies.state;
    this.dependencies = {
      ...dependencies,
      flushDraftSync: dependencies.flushDraftSync ?? flushFileBufferDraftSync,
      createIdentity: dependencies.createIdentity ?? createCssRequestIdentity,
      identityMatches: dependencies.identityMatches ?? cssRequestIdentityMatches,
      mutateExisting: dependencies.mutateExisting ?? setCssRuleAtViewport,
      mutatePage: dependencies.mutatePage ?? setPageCssRuleAtViewport,
      mutateReusable: dependencies.mutateReusable ?? setReusableCssRuleAtViewport,
    };
    this.edit = Object.freeze({
      draft: (property, value) => this.draftProperty(property, value),
      draftMany: (properties) => this.draftProperties(properties),
      commit: (property, value) => this.commitProperty(property, value),
      commitMany: (properties) => this.commitProperties(properties),
      cancel: (property) => this.cancelProperty(property),
      cancelMany: (properties) => this.cancelProperties(properties),
      continuous: (property) => this.continuousProperty(property),
    });
    this.state.attachPropertyEdit(this.edit);
  }

  get stagedCount() {
    return this.staged.size;
  }

  get queuedCount() {
    return this.queued;
  }

  get failure() {
    return this.mutationFailure;
  }

  get pendingForRegistry() {
    return this.staged.size > 0
      || this.queued > 0
      || this.flushPromise !== null
      || Boolean(this.mutationFailure);
  }

  syncSession(projectRoot: string, runtimeSessionId: string) {
    if (this.disposed) return;
    const sessionKey = `${projectRoot}\u0000${runtimeSessionId}`;
    if (sessionKey === this.sessionKey) return;
    this.sessionKey = sessionKey;
    this.resetQueue();
  }

  async flush() {
    if (this.flushPromise) return await this.flushPromise;
    const generation = this.generation;
    const work = async () => {
      while (this.isGenerationCurrent(generation) && this.staged.size > 0) {
        const entries = Array.from(this.staged.values());
        this.staged.clear();
        this.updatePending();
        for (const entry of entries) this.enqueue(entry, generation);
        await this.mutationTail;
      }
    };
    const promise = work().finally(() => {
      if (this.flushPromise === promise) this.flushPromise = null;
      this.updatePending();
      if (this.isGenerationCurrent(generation) && this.staged.size > 0) {
        this.scheduleFlush();
      }
    });
    this.flushPromise = promise;
    await promise;
  }

  async flushForRegistry() {
    await this.flush();
    await this.mutationTail;
    if (this.mutationFailure) throw new Error(this.mutationFailure);
  }

  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.sessionKey = "";
    this.resetQueue();
  }

  private draftProperties(properties: Readonly<Record<string, string>>) {
    const target = this.captureTarget();
    if (!target) return;
    const entries = Object.entries(properties);
    if (!entries.length) return;
    const focusedProperty = entries.length === 1 ? entries[0][0] : "background-image";
    void this.dependencies.changeCodeTarget?.({
      selector: target.selector,
      file: target.file,
      property: focusedProperty,
      expectedSelectionRevision: target.expectedSelection.selectionRevision,
    });
    const baselines = Object.fromEntries(entries.map(([property]) => [
      property,
      captureCssPendingValueBaseline(this.state.pendingValues, property),
    ]));
    const nextPendingValues = { ...this.state.pendingValues, ...properties };
    this.state.replacePendingValues(nextPendingValues);
    const appliedLiveEpoch = this.dependencies.applyLiveProperties?.(
      target.selector,
      nextPendingValues,
      target.viewport,
    );
    const liveEpoch = typeof appliedLiveEpoch === "number" ? appliedLiveEpoch : null;
    const mutation = this.createStagedMutation(target, liveEpoch);
    for (const [property, value] of entries) {
      this.stage(mutation, property, value, baselines[property]);
    }
    this.dependencies.reportStatus?.({ kind: "previewChanged", property: focusedProperty });
  }

  private draftProperty(property: string, value: string) {
    this.draftProperties({ [property]: value });
  }

  private commitProperty(property: string, value?: string) {
    if (value !== undefined && this.state.pendingValues[property] !== value) {
      this.draftProperty(property, value);
    }
    this.scheduleFlush();
  }

  private commitProperties(properties: Readonly<Record<string, string>> = {}) {
    if (Object.keys(properties).length) this.draftProperties(properties);
    this.scheduleFlush();
  }

  private cancelProperty(property: string) {
    const target = this.captureTarget();
    if (!target) return;
    const staged = this.staged.get(target.targetKey);
    const baseline = staged?.baselines[property];
    if (!staged || !baseline || !(property in staged.properties)) return;

    const nextProperties = { ...staged.properties };
    const nextBaselines = { ...staged.baselines };
    delete nextProperties[property];
    delete nextBaselines[property];
    const hasRemainingDrafts = Object.keys(nextProperties).length > 0;
    if (hasRemainingDrafts) {
      this.staged.set(target.targetKey, {
        ...staged,
        properties: nextProperties,
        baselines: nextBaselines,
      });
    } else {
      this.staged.delete(target.targetKey);
    }

    const nextPendingValues = restoreCssPendingValueBaseline(
      this.state.pendingValues,
      property,
      baseline,
    );
    this.state.replacePendingValues(nextPendingValues);
    const appliedLiveEpoch = this.dependencies.applyLiveProperties?.(
      target.selector,
      nextPendingValues,
      target.viewport,
    );
    const liveEpoch = typeof appliedLiveEpoch === "number" ? appliedLiveEpoch : null;
    if (hasRemainingDrafts) {
      const remaining = this.staged.get(target.targetKey);
      if (remaining) this.staged.set(target.targetKey, { ...remaining, liveEpoch });
    } else if (liveEpoch !== null) {
      const generation = this.generation;
      const tail = this.mutationTail;
      void tail.then(() => {
        if (this.isGenerationCurrent(generation)) {
          this.dependencies.rejectLiveProperties?.(liveEpoch);
        }
      });
    }
    this.updatePending();
    this.dependencies.reportStatus?.({ kind: "editCancelled", property });
  }

  private cancelProperties(properties: readonly string[]) {
    for (const property of properties) this.cancelProperty(property);
  }

  private continuousProperty(property: string): CssContinuousEditHandlers {
    const existing = this.continuousBindings.get(property);
    if (existing) return existing;
    const bindings: CssContinuousEditHandlers = Object.freeze({
      oninput: (value) => this.draftProperty(property, value),
      oncommit: () => this.commitProperty(property),
      oncancel: () => this.cancelProperty(property),
    });
    this.continuousBindings.set(property, bindings);
    return bindings;
  }

  private captureTarget(): CssMutationTarget | null {
    if (this.disposed) return null;
    const context = this.dependencies.context();
    const selector = this.state.effectiveSelector;
    const expectedSelection = this.dependencies.captureSelection();
    const resolution = this.state.resolution;
    if (
      !selector
      || !context.targetCssFile
      || !expectedSelection
      || !resolution
      || resolution.state === "ambiguous"
      || !sameCssSemanticSelection(this.state.selectionIdentity, expectedSelection)
      || resolution.selector !== selector
      || resolution.viewport !== context.previewDevice
      || resolution.target?.file !== context.targetCssFile
    ) return null;
    const identity = this.dependencies.createIdentity(
      context.projectRoot,
      context.runtimeSessionId,
    );
    const pageTarget = this.state.target;
    const targetKey = [
      identity.expectedProjectRoot,
      identity.expectedSessionId,
      cssSemanticSelectionKey(expectedSelection),
      context.targetCssFile,
      selector,
      context.previewDevice,
      pageTarget?.targetKind === "reusable"
        ? pageTarget.templatePath ?? "reusable"
        : pageTarget?.pageOwned ? pageTarget.templatePath ?? "page" : "existing",
    ].join("\u0000");
    return {
      identity,
      expectedSelection,
      file: context.targetCssFile,
      selector,
      viewport: context.previewDevice,
      pageTarget,
      targetKey,
    };
  }

  private createStagedMutation(
    target: CssMutationTarget,
    liveEpoch: number | null,
  ): Omit<StagedCssRuleMutation, "properties" | "baselines"> {
    const { identity, expectedSelection, file, selector, viewport, pageTarget } = target;
    if (pageTarget?.targetKind === "reusable" && pageTarget.templatePath) {
      const templatePath = pageTarget.templatePath;
      return {
        key: target.targetKey,
        identity,
        label: `CSS reutilizabil ${selector}`,
        liveEpoch,
        run: (properties) => this.dependencies.mutateReusable({
          templatePath,
          relativePath: file,
          selector,
          properties,
          viewport,
          expectedSelection,
        }, identity),
      };
    }
    if (pageTarget?.pageOwned && pageTarget.templatePath) {
      const templatePath = pageTarget.templatePath;
      return {
        key: target.targetKey,
        identity,
        label: `CSS ${selector}`,
        liveEpoch,
        run: (properties) => this.dependencies.mutatePage({
          templatePath,
          relativePath: file,
          selector,
          properties,
          viewport,
          expectedSelection,
        }, identity),
      };
    }
    return {
      key: target.targetKey,
      identity,
      label: `CSS ${selector}`,
      liveEpoch,
      run: (properties) => this.dependencies.mutateExisting({
        relativePath: file,
        selector,
        properties,
        viewport,
        expectedSelection,
      }, identity),
    };
  }

  private stage(
    mutation: Omit<StagedCssRuleMutation, "properties" | "baselines">,
    property: string,
    value: string,
    baseline: CssPendingValueBaseline,
  ) {
    const current = this.staged.get(mutation.key);
    this.staged.set(mutation.key, {
      ...mutation,
      label: current?.label ?? mutation.label,
      properties: { ...(current?.properties ?? {}), [property]: value },
      baselines: {
        ...(current?.baselines ?? {}),
        [property]: current?.baselines[property] ?? baseline,
      },
    });
    this.updatePending();
  }

  private scheduleFlush() {
    if (this.flushScheduled || this.staged.size === 0 || this.disposed) return;
    const generation = this.generation;
    this.flushScheduled = true;
    queueMicrotask(() => {
      if (!this.isGenerationCurrent(generation)) return;
      this.flushScheduled = false;
      void this.flush();
    });
  }

  private enqueue(entry: StagedCssRuleMutation, generation: number) {
    this.queued += 1;
    this.updatePending();
    const task = this.mutationTail.then(async () => {
      if (!this.entryIsCurrent(entry.identity, generation)) return;
      await this.dependencies.flushDraftSync({ throwOnFailure: true });
      if (!this.entryIsCurrent(entry.identity, generation)) return;
      const receipt = await entry.run(entry.properties);
      if (!this.entryIsCurrent(entry.identity, generation)) return;
      this.mutationFailure = "";
      this.dependencies.reportStatus?.({ kind: "saved", label: entry.label });
      if (!this.dependencies.projectCommittedMutation) return;
      try {
        await this.dependencies.projectCommittedMutation(receipt.authority, entry.liveEpoch);
      } catch (cause) {
        if (!this.entryIsCurrent(entry.identity, generation)) return;
        this.dependencies.reportStatus?.({
          kind: "liveFailed",
          label: entry.label,
          error: cause instanceof Error ? cause.message : String(cause),
        });
      }
    });
    this.mutationTail = task
      .catch((cause) => {
        if (!this.entryIsCurrent(entry.identity, generation)) return;
        if (entry.liveEpoch !== null) {
          this.dependencies.rejectLiveProperties?.(entry.liveEpoch);
        }
        this.mutationFailure = cause instanceof Error ? cause.message : String(cause);
        this.dependencies.reportStatus?.({
          kind: "mutationFailed",
          label: entry.label,
          error: this.mutationFailure,
        });
      })
      .finally(() => {
        if (!this.isGenerationCurrent(generation)) return;
        this.queued = Math.max(0, this.queued - 1);
        this.updatePending();
      });
  }

  private entryIsCurrent(identity: CssRequestIdentity, generation: number) {
    if (!this.isGenerationCurrent(generation)) return false;
    const context = this.dependencies.context();
    return this.dependencies.identityMatches(
      identity,
      context.projectRoot,
      context.runtimeSessionId,
    );
  }

  private isGenerationCurrent(generation: number) {
    return !this.disposed && generation === this.generation;
  }

  private resetQueue() {
    this.generation += 1;
    this.staged.clear();
    this.flushPromise = null;
    this.flushScheduled = false;
    this.mutationTail = Promise.resolve();
    this.mutationFailure = "";
    this.queued = 0;
    this.continuousBindings.clear();
    this.updatePending();
  }

  private updatePending() {
    this.dependencies.setPending?.(this.staged.size > 0 || this.queued > 0);
  }
}
