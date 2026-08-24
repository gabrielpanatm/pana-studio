import { t } from "$lib/i18n/runtime.svelte";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import type { CenterView } from "$lib/application/contracts";
import type { ProjectFile } from "$lib/project/lifecycle-contract";
import type {
  WorkbenchCommandReceipt,
  WorkbenchDocumentActivationCacheOutcome,
  WorkbenchDocumentActivationPhase,
  WorkbenchDocumentActivationSnapshot,
  WorkbenchDocumentSnapshot,
  WorkbenchGroupId,
  WorkbenchIntent,
  WorkbenchSnapshot,
  WorkbenchSurface,
} from "$lib/workbench/contracts";
import { errorMessage } from "$lib/util";
import { activeWorkbenchDocument } from "$lib/workbench/document-presentation";

export type WorkbenchDocumentNavigationCommands = {
  currentSnapshot: () => WorkbenchSnapshot | null;
  resolveProjectFile: (relativePath: string) => Promise<ProjectFile | null>;
  loadProjectFile: (
    file: ProjectFile,
    options: { syncWorkbench: false },
  ) => Promise<unknown>;
  applyIntent: (intent: WorkbenchIntent) => Promise<WorkbenchCommandReceipt>;
  setCenterView: (view: CenterView) => Promise<unknown>;
  beginDocumentActivation?: (
    serial: number,
    document: WorkbenchDocumentSnapshot,
  ) => void;
  updateDocumentActivation?: (
    serial: number,
    patch: {
      phase?: WorkbenchDocumentActivationPhase;
      cacheOutcome?: WorkbenchDocumentActivationCacheOutcome;
      diagnostic?: string | null;
      metrics?: Partial<WorkbenchDocumentActivationSnapshot["metrics"]>;
    },
  ) => void;
  currentTemplateCacheOutcome?: () => "reused" | "materialized" | null;
};

type PendingDocumentActivation = {
  serial: number;
  groupId: WorkbenchGroupId;
  document: WorkbenchDocumentSnapshot;
  startedAt: number;
  resolve: () => void;
};

type DocumentActivationTimings = {
  intentMs: number;
  resolveMs: number;
  loadMs: number;
  surfaceMs: number;
};

/** Coordinates document tabs with the file projection shown by the editor. */
export class WorkbenchDocumentNavigationService {
  private readonly commands: WorkbenchDocumentNavigationCommands;
  private readonly status: Pick<GlobalStatusState, "set">;
  private activationSerial = 0;
  private pendingActivation: PendingDocumentActivation | null = null;
  private activationPumpActive = false;
  private activationPumpScheduled = false;

  constructor(
    commands: WorkbenchDocumentNavigationCommands,
    status: Pick<GlobalStatusState, "set">,
  ) {
    this.commands = commands;
    this.status = status;
  }

  async show(document: WorkbenchDocumentSnapshot) {
    const file = await this.commands.resolveProjectFile(document.relativePath);
    if (!file) {
      this.status.set(t("workbench-document-missing", { path: document.relativePath }), "error");
      return;
    }
    await this.commands.loadProjectFile(file, { syncWorkbench: false });
    await this.setSurface(document.surface);
  }

  async activate(groupId: WorkbenchGroupId, document: WorkbenchDocumentSnapshot): Promise<void> {
    const snapshot = this.commands.currentSnapshot();
    const alreadyActive = snapshot?.activeGroupId === groupId
      && snapshot.groups.find((group) => group.groupId === groupId)?.activeDocumentId
        === document.documentId;
    if (alreadyActive) return Promise.resolve();

    const serial = ++this.activationSerial;
    this.commands.beginDocumentActivation?.(serial, document);
    this.pendingActivation?.resolve();
    const promise = new Promise<void>((resolve) => {
      this.pendingActivation = {
        serial,
        groupId,
        document,
        startedAt: monotonicNow(),
        resolve,
      };
    });
    this.scheduleActivationPump();
    return promise;
  }

  private scheduleActivationPump() {
    if (this.activationPumpActive || this.activationPumpScheduled) return;
    this.activationPumpScheduled = true;
    queueMicrotask(() => {
      this.activationPumpScheduled = false;
      void this.pumpLatestActivation();
    });
  }

  private async pumpLatestActivation() {
    if (this.activationPumpActive) return;
    this.activationPumpActive = true;
    try {
      while (this.pendingActivation) {
        const request = this.pendingActivation;
        this.pendingActivation = null;
        const intentStartedAt = monotonicNow();
        try {
          const receipt = await this.commands.applyIntent({
            kind: "activate_document",
            documentId: request.document.documentId,
            groupId: request.groupId,
          });
          const activated = activeWorkbenchDocument(receipt.snapshot);
          if (!activated) {
            throw new Error("Workbench nu a publicat documentul activ confirmat de Rust.");
          }
          request.groupId = receipt.snapshot.activeGroupId;
          request.document = activated;
        } catch (error) {
          this.failActivation(request, error, monotonicNow() - intentStartedAt);
          continue;
        }
        const intentMs = monotonicNow() - intentStartedAt;
        if (!this.activationIsCurrent(request.serial)) {
          recordDocumentActivationPerformance(request, "stale", {
            intentMs,
            resolveMs: 0,
            loadMs: 0,
            surfaceMs: 0,
          }, "unknown");
          request.resolve();
          continue;
        }
        void this.finishActivation(request, intentMs);
      }
    } finally {
      this.activationPumpActive = false;
      if (this.pendingActivation) this.scheduleActivationPump();
    }
  }

  private async finishActivation(
    request: PendingDocumentActivation,
    intentMs: number,
  ) {
    const timings: DocumentActivationTimings = {
      intentMs,
      resolveMs: 0,
      loadMs: 0,
      surfaceMs: 0,
    };
    try {
      const resolveStartedAt = monotonicNow();
      const file = await this.commands.resolveProjectFile(request.document.relativePath);
      timings.resolveMs = monotonicNow() - resolveStartedAt;
      if (!this.activationIsCurrent(request.serial)) {
        this.settleStaleActivation(request, timings);
        return;
      }
      if (!file) {
        const diagnostic = t("workbench-document-missing", {
          path: request.document.relativePath,
        });
        this.status.set(diagnostic, "error");
        this.settleActivation(request, "failed", timings, "unknown", diagnostic);
        return;
      }

      this.commands.updateDocumentActivation?.(request.serial, {
        phase: "loading",
        metrics: { intentMs, resolveMs: timings.resolveMs },
      });
      const loadStartedAt = monotonicNow();
      await this.commands.loadProjectFile(file, { syncWorkbench: false });
      timings.loadMs = monotonicNow() - loadStartedAt;
      if (!this.activationIsCurrent(request.serial)) {
        this.settleStaleActivation(request, timings);
        return;
      }

      const surfaceStartedAt = monotonicNow();
      await this.setSurface(request.document.surface);
      timings.surfaceMs = monotonicNow() - surfaceStartedAt;
      if (!this.activationIsCurrent(request.serial)) {
        this.settleStaleActivation(request, timings);
        return;
      }
      const cacheOutcome = file.role === "template"
        ? this.commands.currentTemplateCacheOutcome?.() ?? "unknown"
        : "not_applicable";
      this.settleActivation(request, "ready", timings, cacheOutcome, null);
    } catch (error) {
      if (!this.activationIsCurrent(request.serial)) {
        this.settleStaleActivation(request, timings);
        return;
      }
      this.failActivation(request, error, timings.intentMs, timings);
    }
  }

  private activationIsCurrent(serial: number) {
    return serial === this.activationSerial;
  }

  private settleStaleActivation(
    request: PendingDocumentActivation,
    timings: DocumentActivationTimings,
  ) {
    recordDocumentActivationPerformance(request, "stale", timings, "unknown");
    request.resolve();
  }

  private failActivation(
    request: PendingDocumentActivation,
    error: unknown,
    intentMs: number,
    timings: DocumentActivationTimings = {
      intentMs,
      resolveMs: 0,
      loadMs: 0,
      surfaceMs: 0,
    },
  ) {
    const diagnostic = errorMessage(error);
    if (this.activationIsCurrent(request.serial)) {
      this.status.set(
        t("workbench-document-activate-failed", { detail: diagnostic }),
        "error",
      );
      this.settleActivation(request, "failed", timings, "unknown", diagnostic);
    } else {
      this.settleStaleActivation(request, timings);
    }
  }

  private settleActivation(
    request: PendingDocumentActivation,
    phase: "ready" | "failed",
    timings: DocumentActivationTimings,
    cacheOutcome: WorkbenchDocumentActivationCacheOutcome,
    diagnostic: string | null,
  ) {
    const totalMs = Math.max(0, monotonicNow() - request.startedAt);
    this.commands.updateDocumentActivation?.(request.serial, {
      phase,
      cacheOutcome,
      diagnostic,
      metrics: { ...timings, totalMs },
    });
    recordDocumentActivationPerformance(request, phase, timings, cacheOutcome, diagnostic);
    request.resolve();
  }

  async close(groupId: WorkbenchGroupId, document: WorkbenchDocumentSnapshot) {
    const wasActive = this.commands.currentSnapshot()?.groups
      .find((group) => group.groupId === groupId)
      ?.activeDocumentId === document.documentId;
    const serial = wasActive ? ++this.activationSerial : null;
    if (wasActive) {
      this.pendingActivation?.resolve();
      this.pendingActivation = null;
    }
    const startedAt = monotonicNow();
    try {
      const intentStartedAt = monotonicNow();
      const receipt = await this.commands.applyIntent({
        kind: "close_document",
        documentId: document.documentId,
        groupId,
      });
      const intentMs = monotonicNow() - intentStartedAt;
      if (!wasActive) return;
      const nextDocument = activeWorkbenchDocument(receipt.snapshot);
      if (nextDocument && serial !== null) {
        this.commands.beginDocumentActivation?.(serial, nextDocument);
        await this.finishActivation({
          serial,
          groupId: receipt.snapshot.activeGroupId,
          document: nextDocument,
          startedAt,
          resolve: () => {},
        }, intentMs);
      }
    } catch (error) {
      this.status.set(
        t("workbench-document-close-failed", { detail: errorMessage(error) }),
        "error",
      );
    }
  }

  async setSurface(surface: WorkbenchSurface): Promise<void> {
    await this.commands.setCenterView(surface === "code" ? "code" : "preview");
  }
}

function monotonicNow() {
  return globalThis.performance?.now?.() ?? Date.now();
}

function recordDocumentActivationPerformance(
  request: PendingDocumentActivation,
  outcome: "ready" | "failed" | "stale",
  timings: DocumentActivationTimings,
  cacheOutcome: WorkbenchDocumentActivationCacheOutcome,
  diagnostic: string | null = null,
) {
  const performanceApi = globalThis.performance;
  if (!performanceApi?.measure) return;
  const name = `pana.workbench.document_activation.${outcome}`;
  try {
    performanceApi.clearMeasures(name);
    performanceApi.measure(name, {
      start: request.startedAt,
      end: monotonicNow(),
      detail: {
        serial: request.serial,
        documentId: request.document.documentId,
        relativePath: request.document.relativePath,
        surface: request.document.surface,
        cacheOutcome,
        diagnostic,
        ...timings,
      },
    });
  } catch {
    // User Timing is auxiliary observability; navigation settlement remains authoritative.
  }
}
