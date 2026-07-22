import type {
  PreviewProjectionIntentReceipt,
  PreviewStructuralCommandIdentity,
} from "$lib/types";

const MAX_PENDING_STRUCTURAL_OPERATIONS_PER_SESSION = 32;

export type PreviewStructuralSessionHost = {
  sessionProjectRoot: string;
  kernelProjectSessionId: string;
  projectSessionEpoch: number;
  projectTransitionFrontendLeaseActive?: boolean;
  kernelUndoRedoFrontendLeaseActive?: boolean;
  aiEditLeaseFrontendLockActive?: boolean;
  beginPreviewStructuralWriteBoundary: () => Promise<void>;
  endPreviewStructuralWriteBoundary: () => void;
};

export type PreviewStructuralSessionLease = {
  projectRoot: string;
  sessionId: string;
  projectSessionEpoch: number;
};

type StructuralLane = {
  tail: Promise<void>;
  pendingCount: number;
};

const structuralLanes = new Map<string, StructuralLane>();

export class PreviewStructuralCancellationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "PreviewStructuralCancellationError";
  }
}

export function isPreviewStructuralCancellation(
  error: unknown,
): error is PreviewStructuralCancellationError {
  return error instanceof PreviewStructuralCancellationError;
}

function laneKey(lease: PreviewStructuralSessionLease) {
  return `${lease.projectRoot}\u0000${lease.sessionId}`;
}

export function capturePreviewStructuralSessionLease(
  host: PreviewStructuralSessionHost,
): PreviewStructuralSessionLease {
  const projectRoot = host.sessionProjectRoot.trim();
  const sessionId = host.kernelProjectSessionId.trim();
  if (!projectRoot || !sessionId) {
    throw new PreviewStructuralCancellationError(
      "Mutația structurală cere un ProjectSession activ și identificabil.",
    );
  }
  if (host.projectTransitionFrontendLeaseActive) {
    throw new PreviewStructuralCancellationError(
      "Mutația structurală este blocată cât timp tranziția proiectului rezervă sesiunea.",
    );
  }
  if (host.kernelUndoRedoFrontendLeaseActive) {
    throw new PreviewStructuralCancellationError(
      "Mutația structurală este blocată cât timp Undo/Redo rezervă sesiunea.",
    );
  }
  if (host.aiEditLeaseFrontendLockActive) {
    throw new PreviewStructuralCancellationError(
      "Mutația structurală este blocată cât timp AI deține autoritatea de editare.",
    );
  }
  return {
    projectRoot,
    sessionId,
    projectSessionEpoch: host.projectSessionEpoch,
  };
}

export function previewStructuralSessionLeaseMatches(
  host: PreviewStructuralSessionHost,
  lease: PreviewStructuralSessionLease,
) {
  return host.sessionProjectRoot === lease.projectRoot
    && host.kernelProjectSessionId === lease.sessionId
    && host.projectSessionEpoch === lease.projectSessionEpoch;
}

export function requireCurrentPreviewStructuralSession(
  host: PreviewStructuralSessionHost,
  lease: PreviewStructuralSessionLease,
) {
  if (!previewStructuralSessionLeaseMatches(host, lease)) {
    throw new PreviewStructuralCancellationError(
      "Mutația structurală a fost anulată deoarece ProjectSession s-a schimbat.",
    );
  }
}

export function previewStructuralCommandIdentity(
  lease: PreviewStructuralSessionLease,
): PreviewStructuralCommandIdentity {
  return {
    expectedProjectRoot: lease.projectRoot,
    expectedSessionId: lease.sessionId,
  };
}

export function requirePreviewStructuralReceiptIdentity(
  receipt: Pick<
    PreviewProjectionIntentReceipt,
    "projectRoot" | "runtimeSessionId"
  >,
  lease: PreviewStructuralSessionLease,
) {
  if (
    receipt.projectRoot !== lease.projectRoot
    || receipt.runtimeSessionId !== lease.sessionId
  ) {
    throw new Error("Receipt-ul mutației structurale aparține altei instanțe ProjectSession.");
  }
}

/**
 * Serializes the complete structural write lifecycle for one ProjectSession:
 * Rust mutation, local state projection and immutable Preview generation publication.
 *
 * ProjectWorkspace revision is the single project-wide CAS. Running only the
 * Rust commands sequentially is insufficient because a second commit can
 * otherwise overtake the first command's asynchronous acknowledgement.
 */
export async function runInPreviewStructuralLane<T>(
  host: PreviewStructuralSessionHost,
  operation: (lease: PreviewStructuralSessionLease) => Promise<T>,
): Promise<T> {
  let lease: PreviewStructuralSessionLease;
  try {
    lease = capturePreviewStructuralSessionLease(host);
  } catch (error) {
    if (isPreviewStructuralCancellation(error)) return undefined as T;
    throw error;
  }
  const key = laneKey(lease);
  const existing = structuralLanes.get(key);
  if (
    existing
    && existing.pendingCount >= MAX_PENDING_STRUCTURAL_OPERATIONS_PER_SESSION
  ) {
    throw new Error(
      `Coada mutațiilor structurale a atins limita de ${MAX_PENDING_STRUCTURAL_OPERATIONS_PER_SESSION} operații.`,
    );
  }

  const lane = existing ?? { tail: Promise.resolve(), pendingCount: 0 };
  const previous = lane.tail;
  let release!: () => void;
  const completion = new Promise<void>((resolve) => {
    release = resolve;
  });
  const tail = previous.catch(() => undefined).then(() => completion);
  lane.tail = tail;
  lane.pendingCount += 1;
  structuralLanes.set(key, lane);

  await previous.catch(() => undefined);
  let writeBoundaryAcquired = false;
  try {
    requireCurrentPreviewStructuralSession(host, lease);
    await host.beginPreviewStructuralWriteBoundary();
    writeBoundaryAcquired = true;
    requireCurrentPreviewStructuralSession(host, lease);
    return await operation(lease);
  } catch (error) {
    // Cancellation is a no-op, not an error projection into the replacement
    // session. The initiating UI may already have been unmounted/reset.
    if (isPreviewStructuralCancellation(error)) return undefined as T;
    throw error;
  } finally {
    try {
      if (writeBoundaryAcquired) host.endPreviewStructuralWriteBoundary();
    } finally {
      lane.pendingCount -= 1;
      release();
      if (lane.tail === tail && lane.pendingCount === 0) {
        structuralLanes.delete(key);
      }
    }
  }
}

/**
 * Project Transition raises its frontend reservation before awaiting this
 * drain, so no new structural work can enter while existing work finishes.
 */
export async function drainPreviewStructuralLanes() {
  while (structuralLanes.size > 0) {
    const tails = [...structuralLanes.values()].map((lane) => lane.tail);
    await Promise.all(tails.map((tail) => tail.catch(() => undefined)));
  }
}

export function previewStructuralLaneSnapshot() {
  return {
    sessionCount: structuralLanes.size,
    pendingCount: [...structuralLanes.values()].reduce(
      (total, lane) => total + lane.pendingCount,
      0,
    ),
  };
}
