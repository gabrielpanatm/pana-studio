import type { EditorActionOutcome } from "$lib/editor-runtime/action-outcome";
import { committedAction, noopAction } from "$lib/editor-runtime/action-outcome";
import type { PreviewRuntime } from "$lib/editor-runtime/preview-runtime";
import {
  isLatestHtmlAttributeDraftSettlement,
  liveProjectableHtmlAttributeDraft,
} from "$lib/html/live-attribute-draft";
import {
  type ProjectWorkspacePreviewProjectionOptions,
  type ProjectWorkspacePreviewProjectionOutcome,
} from "$lib/kernel/project-workspace-preview-coordinator";
import {
  createLatestWinsAsyncQueue,
  type LatestWinsAsyncQueue,
} from "$lib/session/latest-wins-async-queue";
import { registerEditFlushHandler } from "$lib/session/edit-flush-registry";
import type { WorkspaceDerivedReconciliationOutcome } from "$lib/session/workspace-mutation-coordinator";
import {
  captureHtmlActionTarget,
  type HtmlActionTarget,
} from "$lib/editor/html-actions/target";
import type {
  CoordinatedElementSelection,
  EditableAttributes,
  HtmlPendingArea,
} from "$lib/canvas/contracts";
import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";
import { errorMessage } from "$lib/util";
import { t } from "$lib/i18n/runtime.svelte";

const HTML_TEXT_RECOVERY_INTERVAL_MS = 200;
const HTML_TEXT_CANONICAL_IDLE_MS = 650;
const HTML_TEXT_HISTORY_IDLE_MS = 1_800;

type HtmlTextDraftCommitTask = Readonly<{
  key: string;
  projectRoot: string;
  runtimeSessionId: string;
  projectSessionEpoch: number;
  target: HtmlActionTarget;
  text: string;
  editSessionId: string;
}>;

type ActiveHtmlTextEditSession = {
  id: string;
  key: string;
  projectRoot: string;
  runtimeSessionId: string;
  projectSessionEpoch: number;
  target: HtmlActionTarget;
  text: string;
  projectedText: string | null;
  finishPromise: Promise<boolean> | null;
};

type ActiveHtmlAttributeEditSession = {
  id: string;
  key: string;
  projectRoot: string;
  runtimeSessionId: string;
  projectSessionEpoch: number;
  target: HtmlActionTarget;
  attributes: EditableAttributes;
  baselineAttributes: EditableAttributes;
  baselineNames: string[];
  latestLiveEpoch: number;
  latestLiveProjection: Promise<void> | null;
  finishPromise: Promise<EditorActionOutcome | null> | null;
};

export type HtmlDraftState = {
  attributeValues: EditableAttributes;
  attributeStatus: string;
  textContentValue: string;
  textStatus: string;
  activeTextEditKey: string | null;
  activeTextEditValue: string | null;
  textEditOriginalKey: string | null;
  textEditOriginalText: string | null;
};

export type HtmlDraftSessionControllerHost = {
    context: () => Readonly<{
      projectRoot: string;
      runtimeSessionId: string;
      projectSessionEpoch: number;
      htmlPending: Record<HtmlPendingArea, boolean>;
      workspace: ProjectWorkspaceSnapshot | null;
      coordinatedSelection: CoordinatedElementSelection | null;
    }>;
    previewRuntime: Pick<PreviewRuntime, "sendAndWait">;
    setHtmlPending: (area: HtmlPendingArea, pending: boolean) => void;
    setGlobalStatus: (text: string, kind: "idle" | "unsaved" | "saving" | "saved" | "restored" | "error") => void;
    postPreviewMessage: (payload: Record<string, unknown>) => void;
    applyTextToTarget: (
      target: HtmlActionTarget,
      text: string,
      options: { deferCanonicalProjection: boolean; editSessionId: string },
    ) => Promise<EditorActionOutcome>;
    applyAttributesToTarget: (
      target: HtmlActionTarget,
      attributes: EditableAttributes,
    ) => Promise<EditorActionOutcome>;
    applyCurrentAttributes: (
      attributes: EditableAttributes | null,
    ) => Promise<EditorActionOutcome>;
    projectLatestPreview: (
      options: ProjectWorkspacePreviewProjectionOptions<"workspace-mutation">,
    ) => Promise<ProjectWorkspacePreviewProjectionOutcome>;
    reconcileWorkspaceDerivedState: (options: {
      expectedProjectRoot: string;
      expectedSessionId: string;
      expectedWorkspaceRevision: number;
      topologyChanged: boolean;
      preferredRelativePath?: string | null;
      refreshSourceGraph?: boolean;
      refreshScss?: boolean;
    }) => Promise<WorkspaceDerivedReconciliationOutcome>;
};

export function htmlTextSelectionKey(selection: CoordinatedElementSelection) {
  if (!selection.sourceNodeId) return null;
  return [
    selection.snapshot.runtimeSessionId,
    selection.snapshot.selectionRevision,
    selection.sourceNodeId,
    selection.renderInstanceId,
  ].join("::");
}

/**
 * Owns the complete lifetime of speculative HTML Inspector drafts.
 * Rust ProjectWorkspace mutations remain the only durable edit authority.
 */
export class HtmlDraftSessionController implements HtmlDraftState {
  attributeValues = $state<EditableAttributes>({});
  attributeStatus = $state("");
  textContentValue = $state("");
  textStatus = $state("");
  activeTextEditKey = $state<string | null>(null);
  activeTextEditValue = $state<string | null>(null);
  textEditOriginalKey = $state<string | null>(null);
  textEditOriginalText = $state<string | null>(null);

  private activeTextSession: ActiveHtmlTextEditSession | null = null;
  private activeAttributeSession: ActiveHtmlAttributeEditSession | null = null;
  private attributeSessionSerial = 0;
  private textSessionSerial = 0;
  private textCanonicalTimer: ReturnType<typeof setTimeout> | null = null;
  private textHistoryTimer: ReturnType<typeof setTimeout> | null = null;
  private textProjectionTail: Promise<void> = Promise.resolve();
  private readonly textDraftCommitQueue: LatestWinsAsyncQueue<HtmlTextDraftCommitTask>;
  private unregisterFlush: () => void;
  private readonly resolveHost: () => HtmlDraftSessionControllerHost;

  constructor(resolveHost: () => HtmlDraftSessionControllerHost) {
    this.resolveHost = resolveHost;
    this.textDraftCommitQueue = createLatestWinsAsyncQueue<HtmlTextDraftCommitTask>({
      key: (task) => task.key,
      delayMs: HTML_TEXT_RECOVERY_INTERVAL_MS,
      delayMode: "throttle",
      run: async (task) => {
        const host = this.resolveHost();
        if (
          task.projectRoot !== host.context().projectRoot
          || task.runtimeSessionId !== host.context().runtimeSessionId
          || task.projectSessionEpoch !== host.context().projectSessionEpoch
        ) return;
        const result = await host.applyTextToTarget(
          task.target,
          task.text,
          {
            deferCanonicalProjection: true,
            editSessionId: task.editSessionId,
          },
        );
        if (result.status !== "committed" && result.status !== "noop") {
          throw new Error(result.reason ?? t("workbench-text-draft-status", {
            status: result.status,
          }));
        }
      },
      onError: (error, task) => {
        const host = this.resolveHost();
        if (
          task.projectRoot !== host.context().projectRoot
          || task.runtimeSessionId !== host.context().runtimeSessionId
          || task.projectSessionEpoch !== host.context().projectSessionEpoch
        ) return;
        host.setGlobalStatus(
          t("workbench-text-draft-kernel-failed", {
            message: error instanceof Error ? error.message : String(error),
          }),
          "error",
        );
      },
    });
    this.unregisterFlush = registerEditFlushHandler(
      "html-draft-project-workspace",
      async () => {
        await this.finishActiveAttributeSession();
        await this.finishActiveTextSession();
      },
      () => this.hasPendingDrafts(),
    );
  }

  private hasPendingDrafts() {
    const textQueue = this.textDraftCommitQueue.snapshot();
    return this.activeAttributeSession !== null
      || this.activeTextSession !== null
      || textQueue.pendingCount > 0
      || textQueue.inFlight
      || textQueue.failureCount > 0;
  }

  updateAttribute(property: string, value: string) {
    const host = this.resolveHost();
    this.attributeValues = { ...this.attributeValues, [property]: value };
    host.setHtmlPending("attributes", true);
    host.setGlobalStatus(t("html-draft-attribute-changed", { property }), "unsaved");
    this.attributeStatus = t("html-draft-attribute-pending");
    const session = this.captureActiveAttributeSession();
    if (!session) return;
    session.attributes = { ...this.attributeValues };
    this.projectLiveAttributeDraft(session);
  }

  removeAttribute(name: string) {
    const host = this.resolveHost();
    const { [name]: _removed, ...rest } = this.attributeValues;
    this.attributeValues = rest;
    host.setHtmlPending("attributes", true);
    host.setGlobalStatus(t("html-draft-attribute-removed", { name }), "unsaved");
    this.attributeStatus = t("html-draft-attribute-removal-pending");
    const session = this.captureActiveAttributeSession();
    if (!session) return;
    session.attributes = { ...this.attributeValues };
    this.projectLiveAttributeDraft(session);
  }

  updateText(value: string, composing = false) {
    const host = this.resolveHost();
    this.textContentValue = value;
    const selection = host.context().coordinatedSelection;
    if (!selection || selection.observation.hasChildElements) {
      this.textStatus = t("html-draft-text-simple-only");
      return;
    }
    const key = htmlTextSelectionKey(selection);
    if (!key) {
      this.textStatus = t("html-actions-identity-missing", {
        action: t("html-actions-text-noun"),
      });
      host.setGlobalStatus(this.textStatus, "error");
      return;
    }
    if (this.textEditOriginalKey !== key) {
      this.textEditOriginalKey = key;
      this.textEditOriginalText = selection.observation.rawText ?? "";
    }
    host.setHtmlPending("text", true);
    host.setGlobalStatus(t("html-draft-text-changed"), "unsaved");
    this.textStatus = t("html-draft-text-pending");

    const session = this.captureActiveTextSession(value);
    if (!session) return;
    session.text = value;
    this.activeTextEditValue = value;
    host.postPreviewMessage({
      type: "apply-live-text-draft",
      editSessionId: session.id,
      target: {
        sourceId: session.target.sourceId ?? null,
        renderInstanceId: session.target.renderInstanceId ?? null,
        expectedTag: session.target.tag,
      },
      text: value,
    });
    if (composing) return;
    this.enqueueTextDraftCommit(session);
    this.scheduleTextCanonicalProjection(session.id);
    this.scheduleTextHistoryBoundary(session.id);
  }

  cancelAttributes(expectedContextKey?: string) {
    const session = this.activeAttributeSession;
    if (!session) return;
    const sessionContextKey = this.assetEditContextKey(session.target);
    if (expectedContextKey && sessionContextKey !== expectedContextKey) return;
    const host = this.resolveHost();
    const currentTarget = captureHtmlActionTarget(host.context().coordinatedSelection);
    if (currentTarget && this.assetEditContextKey(currentTarget) === sessionContextKey) {
      this.attributeValues = { ...session.baselineAttributes };
    }
    this.cancelActiveAttributeSession();
    host.setHtmlPending("attributes", false);
    this.attributeStatus = t("workbench-attribute-edit-cancelled");
  }

  async applyAttributes(
    attributes?: EditableAttributes,
  ): Promise<EditorActionOutcome> {
    const activeResult = await this.finishActiveAttributeSession(attributes);
    if (activeResult) return activeResult;
    const host = this.resolveHost();
    if (!host.context().htmlPending.attributes) {
      return noopAction(t("workbench-attributes-already-confirmed"));
    }
    return await host.applyCurrentAttributes(attributes ?? null);
  }

  async applyText(): Promise<EditorActionOutcome> {
    const committed = await this.finishActiveTextSession();
    if (!committed) return noopAction(t("workbench-text-already-confirmed"));
    return committedAction();
  }

  async flush() {
    await this.finishActiveAttributeSession();
    await this.finishActiveTextSession();
  }

  cancel() {
    this.cancelActiveAttributeSession();
    this.cancelActiveTextSession();
    this.textDraftCommitQueue.reset();
  }

  destroy() {
    this.unregisterFlush();
    this.cancel();
  }

  private draftTargetIdentity(target: HtmlActionTarget) {
    return target.sourceId ?? null;
  }

  private assetEditContextKey(target: HtmlActionTarget) {
    return [
      target.sessionId ?? "",
      target.selectionRevision ?? "",
      target.sourceId ?? "",
      target.renderInstanceId ?? "",
      target.tag,
    ].join("::");
  }

  private captureActiveAttributeSession(): ActiveHtmlAttributeEditSession | null {
    const host = this.resolveHost();
    const selection = host.context().coordinatedSelection;
    const target = captureHtmlActionTarget(selection);
    const projectRoot = host.context().projectRoot;
    const runtimeSessionId = host.context().runtimeSessionId;
    const targetIdentity = target ? this.draftTargetIdentity(target) : null;
    if (!selection || !target || !targetIdentity || !projectRoot || !runtimeSessionId) return null;
    const key = `${projectRoot}\u0000${runtimeSessionId}\u0000attributes\u0000${targetIdentity}`;
    const current = this.activeAttributeSession;
    if (current && current.key === key && current.projectSessionEpoch === host.context().projectSessionEpoch) {
      return current;
    }

    if (current) this.cancelActiveAttributeSession();
    const id = `attr_${Date.now().toString(36)}_${(++this.attributeSessionSerial).toString(36)}`;
    const baselineAttributes = Object.fromEntries(
      Object.entries(target.attributes ?? {})
        .filter(([name]) => !name.toLowerCase().startsWith("data-pana-")),
    );
    const session: ActiveHtmlAttributeEditSession = {
      id,
      key,
      projectRoot,
      runtimeSessionId,
      projectSessionEpoch: host.context().projectSessionEpoch,
      target,
      attributes: { ...this.attributeValues },
      baselineAttributes,
      baselineNames: Object.keys(baselineAttributes),
      latestLiveEpoch: 0,
      latestLiveProjection: null,
      finishPromise: null,
    };
    this.activeAttributeSession = session;
    return session;
  }

  private projectLiveAttributeDraft(session: ActiveHtmlAttributeEditSession) {
    const host = this.resolveHost();
    const draftEpoch = ++session.latestLiveEpoch;
    const projection = liveProjectableHtmlAttributeDraft(
      session.target.tag,
      session.attributes,
      session.baselineNames,
    );
    const settlement = host.previewRuntime.sendAndWait({
      type: "apply-live-attribute-draft",
      editSessionId: session.id,
      draftEpoch,
      target: {
        sourceId: session.target.sourceId ?? null,
        renderInstanceId: session.target.renderInstanceId ?? null,
        expectedTag: session.target.tag,
      },
      attributes: projection.attributes,
      baselineNames: projection.baselineNames,
    }).then((ack) => {
      if (!ack.ok) throw new Error(ack.error || t("workbench-attribute-live-rejected"));
      if (!isLatestHtmlAttributeDraftSettlement(
        this.activeAttributeSession?.id ?? null,
        this.activeAttributeSession?.latestLiveEpoch ?? -1,
        session.id,
        draftEpoch,
      )) return;
      this.attributeStatus = t("workbench-attribute-draft-confirmed");
    }).catch((error) => {
      if (isLatestHtmlAttributeDraftSettlement(
        this.activeAttributeSession?.id ?? null,
        this.activeAttributeSession?.latestLiveEpoch ?? -1,
        session.id,
        draftEpoch,
      )) {
        this.attributeStatus = t("workbench-attribute-live-failed", {
          message: error instanceof Error ? error.message : String(error),
        });
      }
      throw error;
    });
    session.latestLiveProjection = settlement;
    void settlement.catch(() => {});
  }

  private cancelActiveAttributeSession() {
    const session = this.activeAttributeSession;
    if (session) {
      const clear = this.resolveHost().previewRuntime.sendAndWait({
        type: "clear-live-attribute-draft",
        editSessionId: session.id,
        draftEpoch: session.latestLiveEpoch,
      });
      void clear.catch(() => {});
    }
    this.activeAttributeSession = null;
  }

  private async finishActiveAttributeSession(
    attributeOverride?: EditableAttributes,
  ): Promise<EditorActionOutcome | null> {
    const session = this.activeAttributeSession;
    if (!session) return null;
    if (attributeOverride) session.attributes = { ...attributeOverride };
    const host = this.resolveHost();
    if (
      session.projectRoot !== host.context().projectRoot
      || session.runtimeSessionId !== host.context().runtimeSessionId
      || session.projectSessionEpoch !== host.context().projectSessionEpoch
    ) {
      this.cancelActiveAttributeSession();
      return null;
    }
    if (session.finishPromise) return await session.finishPromise;
    const operation = this.finishCapturedAttributeSession(session);
    session.finishPromise = operation;
    try {
      return await operation;
    } finally {
      if (session.finishPromise === operation) session.finishPromise = null;
    }
  }

  private async finishCapturedAttributeSession(
    session: ActiveHtmlAttributeEditSession,
  ): Promise<EditorActionOutcome | null> {
    while (this.activeAttributeSession?.id === session.id) {
      const liveProjection = session.latestLiveProjection;
      if (liveProjection) {
        try {
          await liveProjection;
        } catch {
          // Speculative Canvas failure cannot override Rust authority.
        }
      }
      if (this.activeAttributeSession?.id !== session.id) return null;

      const submittedLiveEpoch = session.latestLiveEpoch;
      const submittedAttributes = { ...session.attributes };
      const result = await this.resolveHost().applyAttributesToTarget(
        session.target,
        submittedAttributes,
      );
      if (result.status !== "committed" && result.status !== "noop") return result;
      if (this.activeAttributeSession?.id !== session.id) return result;
      if (session.latestLiveEpoch !== submittedLiveEpoch) continue;

      try {
        const ack = await this.resolveHost().previewRuntime.sendAndWait({
          type: "clear-live-attribute-draft",
          editSessionId: session.id,
          draftEpoch: submittedLiveEpoch,
        });
        if (!ack.ok) throw new Error(ack.error || t("workbench-canvas-draft-close-unconfirmed"));
      } catch (error) {
        this.attributeStatus = t("workbench-attribute-source-confirmed-canvas-failed", {
          message: error instanceof Error ? error.message : String(error),
        });
      }
      if (this.activeAttributeSession?.id !== session.id) return result;
      if (session.latestLiveEpoch !== submittedLiveEpoch) continue;

      this.activeAttributeSession = null;
      this.resolveHost().setHtmlPending("attributes", false);
      this.attributeStatus = result.status === "noop"
        ? t("workbench-attributes-no-changes")
        : t("workbench-attributes-confirmed");
      return result;
    }
    return null;
  }

  private captureActiveTextSession(value: string): ActiveHtmlTextEditSession | null {
    const host = this.resolveHost();
    const selection = host.context().coordinatedSelection;
    const target = captureHtmlActionTarget(selection);
    const projectRoot = host.context().projectRoot;
    const runtimeSessionId = host.context().runtimeSessionId;
    if (
      !selection
      || selection.observation.hasChildElements
      || !target
      || !projectRoot
      || !runtimeSessionId
    ) return null;
    const key = htmlTextSelectionKey(selection);
    if (!key || !target.sourceId) return null;
    const current = this.activeTextSession;
    if (
      current
      && current.key === key
      && current.projectRoot === projectRoot
      && current.runtimeSessionId === runtimeSessionId
      && current.projectSessionEpoch === host.context().projectSessionEpoch
    ) return current;

    this.clearTextTimers();
    const id = `text_${Date.now().toString(36)}_${(++this.textSessionSerial).toString(36)}`;
    const session: ActiveHtmlTextEditSession = {
      id,
      key,
      projectRoot,
      runtimeSessionId,
      projectSessionEpoch: host.context().projectSessionEpoch,
      target,
      text: value,
      projectedText: null,
      finishPromise: null,
    };
    this.activeTextSession = session;
    this.activeTextEditKey = key;
    this.activeTextEditValue = value;
    return session;
  }

  private enqueueTextDraftCommit(session: ActiveHtmlTextEditSession) {
    this.textDraftCommitQueue.enqueue({
      key: `${session.projectRoot}\u0000${session.runtimeSessionId}\u0000text\u0000${session.id}`,
      projectRoot: session.projectRoot,
      runtimeSessionId: session.runtimeSessionId,
      projectSessionEpoch: session.projectSessionEpoch,
      target: session.target,
      text: session.text,
      editSessionId: session.id,
    });
  }

  private clearTextTimers() {
    if (this.textCanonicalTimer !== null) clearTimeout(this.textCanonicalTimer);
    if (this.textHistoryTimer !== null) clearTimeout(this.textHistoryTimer);
    this.textCanonicalTimer = null;
    this.textHistoryTimer = null;
  }

  private scheduleTextCanonicalProjection(editSessionId: string) {
    if (this.textCanonicalTimer !== null) clearTimeout(this.textCanonicalTimer);
    this.textCanonicalTimer = setTimeout(() => {
      this.textCanonicalTimer = null;
      void this.projectActiveTextSession(editSessionId).catch((error) => {
        if (this.activeTextSession?.id !== editSessionId) return;
        this.resolveHost().setGlobalStatus(
          t("workbench-text-projection-failed", { message: errorMessage(error) }),
          "error",
        );
      });
    }, HTML_TEXT_CANONICAL_IDLE_MS);
  }

  private scheduleTextHistoryBoundary(editSessionId: string) {
    if (this.textHistoryTimer !== null) clearTimeout(this.textHistoryTimer);
    this.textHistoryTimer = setTimeout(() => {
      this.textHistoryTimer = null;
      void this.finishActiveTextSession(editSessionId).catch((error) => {
        if (this.activeTextSession?.id !== editSessionId) return;
        this.resolveHost().setGlobalStatus(t("workbench-text-edit-close-failed", {
          message: errorMessage(error),
        }), "error");
      });
    }, HTML_TEXT_HISTORY_IDLE_MS);
  }

  private projectActiveTextSession(editSessionId: string): Promise<void> {
    const task = this.textProjectionTail
      .catch(() => undefined)
      .then(async () => {
        const session = this.activeTextSession;
        if (!session || session.id !== editSessionId) return;
        await this.textDraftCommitQueue.flush({ throwOnFailure: true });
        const projectedText = session.text;
        const host = this.resolveHost();
        if (
          this.activeTextSession?.id !== editSessionId
          || session.projectRoot !== host.context().projectRoot
          || session.runtimeSessionId !== host.context().runtimeSessionId
          || session.projectSessionEpoch !== host.context().projectSessionEpoch
        ) return;
        const workspaceRevision = host.context().workspace?.revision;
        if (workspaceRevision === undefined) {
          throw new Error(t("workbench-text-workspace-revision-missing"));
        }
        const derived = await host.reconcileWorkspaceDerivedState({
          expectedProjectRoot: session.projectRoot,
          expectedSessionId: session.runtimeSessionId,
          expectedWorkspaceRevision: workspaceRevision,
          topologyChanged: false,
          preferredRelativePath: session.target.sourceLocation?.file ?? null,
          refreshSourceGraph: true,
          refreshScss: false,
        });
        if (this.activeTextSession?.id !== editSessionId) return;
        const preview = await host.projectLatestPreview({
          reason: "workspace-mutation",
          minimumWorkspaceRevision: workspaceRevision,
          expectedWorkspaceRevision: workspaceRevision,
          requestedPaths: session.target.sourceLocation?.file
            ? [session.target.sourceLocation.file]
            : undefined,
        });
        if (this.activeTextSession?.id === editSessionId) {
          session.projectedText = projectedText;
          if (derived.warnings.length > 0 || preview.status === "deferred") {
            host.setGlobalStatus(t("workbench-text-resync"), "unsaved");
          }
        }
      });
    this.textProjectionTail = task.catch(() => undefined);
    return task;
  }

  private cancelActiveTextSession() {
    const session = this.activeTextSession;
    this.clearTextTimers();
    if (session) {
      this.resolveHost().postPreviewMessage({
        type: "clear-live-text-draft",
        editSessionId: session.id,
      });
    }
    this.activeTextSession = null;
    this.activeTextEditKey = null;
    this.activeTextEditValue = null;
    this.textEditOriginalKey = null;
    this.textEditOriginalText = null;
  }

  private async finishActiveTextSession(expectedEditSessionId?: string) {
    const session = this.activeTextSession;
    if (!session || (expectedEditSessionId && session.id !== expectedEditSessionId)) {
      await this.textDraftCommitQueue.flush({ throwOnFailure: true });
      return false;
    }
    if (session.finishPromise) return await session.finishPromise;
    const operation = this.finishCapturedTextSession(session);
    session.finishPromise = operation;
    try {
      return await operation;
    } finally {
      if (session.finishPromise === operation) session.finishPromise = null;
    }
  }

  private async finishCapturedTextSession(session: ActiveHtmlTextEditSession): Promise<boolean> {
    this.clearTextTimers();
    await this.textDraftCommitQueue.flush({ throwOnFailure: true });
    if (this.activeTextSession?.id !== session.id) return false;
    await this.textProjectionTail.catch(() => undefined);
    if (this.activeTextSession?.id !== session.id) return false;
    if (session.projectedText !== session.text) {
      await this.projectActiveTextSession(session.id);
    }
    if (this.activeTextSession?.id !== session.id) return false;
    this.resolveHost().postPreviewMessage({
      type: "clear-live-text-draft",
      editSessionId: session.id,
    });
    this.activeTextSession = null;
    this.activeTextEditKey = null;
    this.activeTextEditValue = null;
    this.textEditOriginalKey = null;
    this.textEditOriginalText = null;
    this.resolveHost().setHtmlPending("text", false);
    this.textStatus = t("workbench-text-confirmed");
    return true;
  }
}
