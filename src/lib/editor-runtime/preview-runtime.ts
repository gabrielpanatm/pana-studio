import type { SaveState } from "$lib/types";
import type {
  CanvasProjectionIdentity,
  PreviewPhaseReceipt,
} from "$lib/project/io";
import type { CanvasPatch } from "$lib/types";

export type PreviewOperationPayload = Record<string, unknown> & {
  type: string;
};

export type PreviewOperation = {
  revision: number;
  type: string;
  startedAt: number;
  completedAt?: number;
  ok?: boolean;
  error?: string | null;
};

export type PreviewOperationAck = {
  revision: number;
  type: string;
  ok: boolean;
  error?: string | null;
  canvasIdentity?: CanvasProjectionIdentity | null;
  canvasPhaseReceipts?: PreviewPhaseReceipt[];
  canvasPatchReceipt?: {
    schemaVersion: 1;
    patchId: string;
    workspaceRevision: number;
    workspaceTransactionId: string;
    bridgeCommitDurationMs: number;
    receiptToCommitDurationMs?: number;
    roundTripDurationMs?: number;
  } | null;
  canvasPatchRollbackReceipt?: {
    schemaVersion: 1;
    patchId: string;
    workspaceRevision: number;
    workspaceTransactionId: string;
  } | null;
};

export type PreviewRuntimeHost = {
  postPreviewMessage: (payload: Record<string, unknown>) => void;
  setGlobalStatus: (text: string, kind: SaveState) => void;
};

type PreviewRuntimeTimer = ReturnType<typeof globalThis.setTimeout>;

type PendingPreviewOperation = {
  operation: PreviewOperation;
  timeout: PreviewRuntimeTimer;
};

type PreviewOperationWaiter = {
  resolve: (ack: PreviewOperationAck) => void;
  reject: (error: Error) => void;
};

export type PreviewRuntimeOptions = {
  ackTimeoutMs?: number;
  maxPendingOperations?: number;
  incomingWindowMs?: number;
  maxIncomingMessagesPerWindow?: number;
  now?: () => number;
  scheduleTimeout?: (callback: () => void, delayMs: number) => PreviewRuntimeTimer;
  cancelTimeout?: (timer: PreviewRuntimeTimer) => void;
};

export type PreviewRuntimeTransportFailureCode =
  | "ack_timeout"
  | "capacity"
  | "runtime_reset";

/**
 * Separă un bridge absent/resetat de un ACK canonic care refuză operația.
 * Primul permite navigarea iframe-ului ca recovery; al doilea trebuie să
 * rămână fail-closed și să păstreze ultimul Canvas confirmat.
 */
export class PreviewRuntimeTransportError extends Error {
  readonly code: PreviewRuntimeTransportFailureCode;

  constructor(code: PreviewRuntimeTransportFailureCode, message: string) {
    super(message);
    this.name = "PreviewRuntimeTransportError";
    this.code = code;
  }
}

const DEFAULT_ACK_TIMEOUT_MS = 15_000;
const DEFAULT_MAX_PENDING_OPERATIONS = 64;
const DEFAULT_INCOMING_WINDOW_MS = 1_000;
const DEFAULT_MAX_INCOMING_MESSAGES_PER_WINDOW = 512;

export type PreviewIngressWindow = Readonly<{
  startedAt: number | null;
  accepted: number;
  rejected: number;
}>;

export type CanvasPatchPerformanceSnapshot = Readonly<{
  sampleCount: number;
  receiptToCommitP50Ms: number | null;
  receiptToCommitP95Ms: number | null;
  receiptToCommitMaxMs: number | null;
  bridgeCommitP95Ms: number | null;
  budgetMs: 50;
  budgetMet: boolean | null;
}>;

type CanvasPatchPerformanceSample = {
  receiptToCommitDurationMs: number;
  bridgeCommitDurationMs: number;
  roundTripDurationMs: number;
};

const MAX_CANVAS_PATCH_PERFORMANCE_SAMPLES = 256;

export class PreviewRuntime {
  private revision = 0;
  private pending = new Map<number, PendingPreviewOperation>();
  private waiters = new Map<number, PreviewOperationWaiter>();
  private readonly host: PreviewRuntimeHost;
  private readonly ackTimeoutMs: number;
  private readonly maxPendingOperations: number;
  private readonly incomingWindowMs: number;
  private readonly maxIncomingMessagesPerWindow: number;
  private readonly now: () => number;
  private readonly scheduleTimeout: (callback: () => void, delayMs: number) => PreviewRuntimeTimer;
  private readonly cancelTimeout: (timer: PreviewRuntimeTimer) => void;
  private incomingWindowStartedAt: number | null = null;
  private incomingAccepted = 0;
  private incomingRejected = 0;
  private incomingOverflowReported = false;
  private canvasPatchPerformanceSamples: CanvasPatchPerformanceSample[] = [];
  lastAck: PreviewOperationAck | null = null;
  lastOperation: PreviewOperation | null = null;

  constructor(host: PreviewRuntimeHost, options: PreviewRuntimeOptions = {}) {
    this.host = host;
    this.ackTimeoutMs = positiveInteger(options.ackTimeoutMs, DEFAULT_ACK_TIMEOUT_MS);
    this.maxPendingOperations = positiveInteger(
      options.maxPendingOperations,
      DEFAULT_MAX_PENDING_OPERATIONS,
    );
    this.incomingWindowMs = positiveInteger(
      options.incomingWindowMs,
      DEFAULT_INCOMING_WINDOW_MS,
    );
    this.maxIncomingMessagesPerWindow = positiveInteger(
      options.maxIncomingMessagesPerWindow,
      DEFAULT_MAX_INCOMING_MESSAGES_PER_WINDOW,
    );
    this.now = options.now ?? Date.now;
    this.scheduleTimeout = options.scheduleTimeout
      ?? ((callback, delayMs) => globalThis.setTimeout(callback, delayMs));
    this.cancelTimeout = options.cancelTimeout
      ?? ((timer) => globalThis.clearTimeout(timer));
  }

  currentRevision() {
    return this.revision;
  }

  /**
   * O(1) admission gate before any preview payload reaches selection,
   * projection preflight or a structural command. Overflow is fail-closed and
   * reported once per window so a hostile message flood cannot amplify into a
   * status-update flood of its own.
   *
   * This is a resource guard, not author authentication. Native document and
   * gesture provenance remains mandatory for privileged intents.
   */
  acceptIncomingMessage() {
    const now = this.now();
    if (
      this.incomingWindowStartedAt === null
      || now < this.incomingWindowStartedAt
      || now - this.incomingWindowStartedAt >= this.incomingWindowMs
    ) {
      this.incomingWindowStartedAt = now;
      this.incomingAccepted = 0;
      this.incomingRejected = 0;
      this.incomingOverflowReported = false;
    }

    if (this.incomingAccepted < this.maxIncomingMessagesPerWindow) {
      this.incomingAccepted += 1;
      return true;
    }

    this.incomingRejected += 1;
    if (!this.incomingOverflowReported) {
      this.incomingOverflowReported = true;
      this.host.setGlobalStatus(
        "Preview a depășit bugetul de mesaje; surplusul a fost refuzat fără retry.",
        "error",
      );
    }
    return false;
  }

  incomingWindow(): PreviewIngressWindow {
    return {
      startedAt: this.incomingWindowStartedAt,
      accepted: this.incomingAccepted,
      rejected: this.incomingRejected,
    };
  }

  send(payload: PreviewOperationPayload) {
    while (this.pending.size >= this.maxPendingOperations) {
      const oldestRevision = this.pending.keys().next().value;
      if (typeof oldestRevision !== "number") break;
      this.failPending(
        oldestRevision,
        "Preview a depășit limita operațiilor fără ACK; operația cea mai veche a fost invalidată.",
        "capacity",
      );
    }
    const revision = ++this.revision;
    const operation: PreviewOperation = {
      revision,
      type: payload.type,
      startedAt: this.now(),
    };
    const timeout = this.scheduleTimeout(() => {
      this.failPending(
        revision,
        `Preview nu a confirmat operația ${payload.type} în ${this.ackTimeoutMs} ms.`,
        "ack_timeout",
      );
    }, this.ackTimeoutMs);
    this.pending.set(revision, { operation, timeout });
    this.lastOperation = operation;
    this.host.postPreviewMessage({
      ...payload,
      previewRevision: revision,
    });
    return operation;
  }

  sendAndWait(payload: PreviewOperationPayload): Promise<PreviewOperationAck> {
    const operation = this.send(payload);
    return new Promise<PreviewOperationAck>((resolve, reject) => {
      this.waiters.set(operation.revision, { resolve, reject });
    });
  }

  handleAck(data: Record<string, unknown>) {
    if (data.type !== "preview-operation-complete") return null;
    const revision = typeof data.previewRevision === "number" ? data.previewRevision : null;
    const operationType = typeof data.operation === "string" ? data.operation : "";
    if (revision === null || revision <= 0 || !operationType) return null;

    const pending = this.pending.get(revision);
    if (!pending || pending.operation.type !== operationType) return null;

    const ok = data.ok !== false;
    const error = typeof data.error === "string" && data.error ? data.error : null;
    this.cancelTimeout(pending.timeout);
    pending.operation.completedAt = this.now();
    pending.operation.ok = ok;
    pending.operation.error = error;
    this.pending.delete(revision);
    this.lastOperation = pending.operation;

    const ack: PreviewOperationAck = {
      revision,
      type: operationType,
      ok,
      error,
      canvasIdentity: data.canvasIdentity && typeof data.canvasIdentity === "object"
        ? data.canvasIdentity as CanvasProjectionIdentity
        : null,
      canvasPhaseReceipts: Array.isArray(data.canvasPhaseReceipts)
        ? data.canvasPhaseReceipts.filter((receipt) => receipt && typeof receipt === "object") as PreviewPhaseReceipt[]
        : [],
      canvasPatchReceipt: data.canvasPatchReceipt && typeof data.canvasPatchReceipt === "object"
        ? data.canvasPatchReceipt as PreviewOperationAck["canvasPatchReceipt"]
        : null,
      canvasPatchRollbackReceipt: data.canvasPatchRollbackReceipt && typeof data.canvasPatchRollbackReceipt === "object"
        ? data.canvasPatchRollbackReceipt as PreviewOperationAck["canvasPatchRollbackReceipt"]
        : null,
    };
    this.lastAck = ack;
    this.waiters.get(revision)?.resolve(ack);
    this.waiters.delete(revision);
    // CanvasPatch este o accelerație speculativă. Apelantul deține fallback-ul
    // canonic și decide dacă eșecul final trebuie afișat; ACK-ul provizoriu nu
    // trebuie să sperie utilizatorul înainte ca proiecția Rust să fie încercată.
    if (!ok && error && operationType !== "apply-canvas-patch") {
      this.host.setGlobalStatus(`Preview update eșuat: ${error}`, "error");
    }
    return ack;
  }

  async applyCanvasPatch(patch: CanvasPatch) {
    const roundTripStartedAt = this.now();
    const ack = await this.sendAndWait({ type: "apply-canvas-patch", patch });
    const receipt = ack.canvasPatchReceipt;
    const bridgeCommitDurationMs = receipt?.bridgeCommitDurationMs;
    if (
      !ack.ok
      || !receipt
      || receipt.schemaVersion !== 1
      || receipt.patchId !== patch.patchId
      || receipt.workspaceRevision !== patch.workspaceRevision
      || receipt.workspaceTransactionId !== patch.workspaceTransactionId
      || !Number.isFinite(bridgeCommitDurationMs)
      || (bridgeCommitDurationMs ?? -1) < 0
      || (bridgeCommitDurationMs ?? 0) > 600_000
      || !Number.isSafeInteger(patch.issuedAtMs)
      || patch.issuedAtMs <= 0
    ) {
      throw new Error(ack.error || "Preview nu a confirmat CanvasPatch-ul exact.");
    }
    const roundTripDurationMs = Math.max(0, this.now() - roundTripStartedAt);
    const receiptToCommitDurationMs = Math.max(0, Date.now() - patch.issuedAtMs);
    const measured = {
      ...receipt,
      bridgeCommitDurationMs: bridgeCommitDurationMs ?? 0,
      receiptToCommitDurationMs,
      roundTripDurationMs,
    };
    this.canvasPatchPerformanceSamples.push(measured);
    if (this.canvasPatchPerformanceSamples.length > MAX_CANVAS_PATCH_PERFORMANCE_SAMPLES) {
      this.canvasPatchPerformanceSamples.shift();
    }
    return measured;
  }

  async rollbackCanvasPatch(patch: CanvasPatch) {
    const ack = await this.sendAndWait({ type: "rollback-canvas-patch", patch });
    const receipt = ack.canvasPatchRollbackReceipt;
    if (
      !ack.ok
      || !receipt
      || receipt.schemaVersion !== 1
      || receipt.patchId !== patch.patchId
      || receipt.workspaceRevision !== patch.baseWorkspaceRevision
      || receipt.workspaceTransactionId !== patch.workspaceTransactionId
    ) {
      throw new Error(ack.error || "Preview nu a confirmat rollback-ul CanvasPatch exact.");
    }
    return receipt;
  }

  hasPending(type?: string) {
    for (const pending of this.pending.values()) {
      if (!type || pending.operation.type === type) return true;
    }
    return false;
  }

  canvasPatchPerformance(): CanvasPatchPerformanceSnapshot {
    const receiptDurations = this.canvasPatchPerformanceSamples
      .map((sample) => sample.receiptToCommitDurationMs);
    const bridgeDurations = this.canvasPatchPerformanceSamples
      .map((sample) => sample.bridgeCommitDurationMs);
    const p95 = percentile(receiptDurations, 0.95);
    return {
      sampleCount: receiptDurations.length,
      receiptToCommitP50Ms: percentile(receiptDurations, 0.5),
      receiptToCommitP95Ms: p95,
      receiptToCommitMaxMs: receiptDurations.length > 0 ? Math.max(...receiptDurations) : null,
      bridgeCommitP95Ms: percentile(bridgeDurations, 0.95),
      budgetMs: 50,
      budgetMet: p95 === null ? null : p95 < 50,
    };
  }

  reset() {
    for (const waiter of this.waiters.values()) {
      waiter.reject(new PreviewRuntimeTransportError(
        "runtime_reset",
        "Preview runtime a fost resetat înainte de ACK.",
      ));
    }
    this.waiters.clear();
    for (const pending of this.pending.values()) {
      this.cancelTimeout(pending.timeout);
    }
    this.pending.clear();
    this.lastAck = null;
    this.lastOperation = null;
    this.incomingWindowStartedAt = null;
    this.incomingAccepted = 0;
    this.incomingRejected = 0;
    this.incomingOverflowReported = false;
    this.canvasPatchPerformanceSamples = [];
  }

  pendingCount() {
    return this.pending.size;
  }

  private failPending(
    revision: number,
    error: string,
    code: PreviewRuntimeTransportFailureCode,
  ) {
    const pending = this.pending.get(revision);
    if (!pending) return;
    this.cancelTimeout(pending.timeout);
    this.pending.delete(revision);
    const waiter = this.waiters.get(revision);
    waiter?.reject(new PreviewRuntimeTransportError(code, error));
    this.waiters.delete(revision);
    pending.operation.completedAt = this.now();
    pending.operation.ok = false;
    pending.operation.error = error;
    this.lastOperation = pending.operation;
    // Operațiile await-ed au un apelant care decide fallback-ul și mesajul
    // final. Doar mesajele fire-and-forget își publică direct timeout-ul.
    if (!waiter) this.host.setGlobalStatus(`Preview update eșuat: ${error}`, "error");
  }
}

function percentile(values: number[], quantile: number): number | null {
  if (values.length === 0) return null;
  const sorted = [...values].sort((left, right) => left - right);
  const index = Math.max(0, Math.ceil(quantile * sorted.length) - 1);
  return sorted[Math.min(index, sorted.length - 1)];
}

function positiveInteger(value: number | undefined, fallback: number) {
  return typeof value === "number" && Number.isInteger(value) && value > 0 ? value : fallback;
}

export function createPreviewRuntime(host: PreviewRuntimeHost, options: PreviewRuntimeOptions = {}) {
  return new PreviewRuntime(host, options);
}
