import {
  applyFileBufferChangeSet,
  readFileBufferText,
} from "$lib/project/io/workspace";
import { t } from "$lib/i18n/runtime.svelte";
import type { FileBufferRequestIdentity } from "$lib/project/workspace-contract";
import type {
  FileBufferTextChange,
  FileBufferTextSnapshot,
} from "$lib/project/workspace-contract";

type FileBufferDraftSyncLease = {
  expectedProjectRoot: string;
  expectedSessionId: string;
  generation: number;
};

type FileBufferChangeSetDraftSyncTask = FileBufferDraftSyncLease & {
  relativePath: string;
  base: string;
  contents: string;
  source: string;
  changes: FileBufferTextChange[];
};

type FileBufferDraftSyncTask = FileBufferChangeSetDraftSyncTask;

type FileBufferSyncCursor = {
  revision: number;
  hash: string;
};

type FileBufferChangeSetFailureKind = "conflict" | "invalid_change_set" | "sync_failed";

type FileBufferDraftSyncFailure = {
  relativePath: string;
  message: string;
  sticky?: boolean;
};

const failures = new Map<string, FileBufferDraftSyncFailure>();
const pending = new Map<string, FileBufferDraftSyncTask>();
const confirmedBuffers = new Map<string, FileBufferSyncCursor>();
const desiredBuffers = new Map<string, string>();
let activeSession: Omit<FileBufferDraftSyncLease, "generation"> | null = null;
let syncGeneration = 0;
let drainPromise: Promise<void> | null = null;

export function setFileBufferDraftSyncSession(
  expectedProjectRoot: string | null | undefined,
  expectedSessionId: string | null | undefined,
) {
  const next = {
    expectedProjectRoot: expectedProjectRoot?.trim() ?? "",
    expectedSessionId: expectedSessionId?.trim() ?? "",
  };
  if (!next.expectedProjectRoot || !next.expectedSessionId) {
    resetFileBufferDraftSyncState();
    return;
  }
  if (
    activeSession?.expectedProjectRoot === next.expectedProjectRoot
    && activeSession.expectedSessionId === next.expectedSessionId
  ) return;
  invalidateSyncGeneration();
  activeSession = next;
}

export function queueFileBufferDraftChangeSetForPath(
  relativePath: string,
  beforeText: string,
  afterText: string,
  changes: FileBufferTextChange[],
  source = "codemirror",
) {
  if (!relativePath) return;
  const lease = captureSyncLease(relativePath);
  if (!lease) return;
  const key = taskKey(lease, relativePath);
  const dirty = beforeText !== afterText;
  if (!dirty || changes.length === 0) return;
  if (!acceptContinuousFrontendTransition(key, relativePath, beforeText, afterText)) return;

  const previous = pending.get(key);
  const base = previous?.base ?? beforeText;
  const combinedChanges = previous
    ? textTransitionToChangeSet(base, afterText)
    : changes;
  if (base === afterText || combinedChanges.length === 0) {
    pending.delete(key);
    ensureDrain();
    return;
  }

  pending.set(key, {
    ...lease,
    relativePath,
    base,
    contents: afterText,
    source,
    changes: combinedChanges,
  });
  ensureDrain();
}

export function queueFileBufferDraftTextTransitionForPath(
  relativePath: string,
  beforeText: string,
  afterText: string,
  source: string,
) {
  queueFileBufferDraftChangeSetForPath(
    relativePath,
    beforeText,
    afterText,
    textTransitionToChangeSet(beforeText, afterText),
    source,
  );
}

/**
 * Re-reads the mounted editor's final frontend snapshot into the same
 * continuous queue before Save. It never writes directly and it cannot
 * bypass the FileBufferStore CAS checks used by the normal transition path.
 */
export function queueFileBufferDraftFlushSnapshotForPath(
  relativePath: string,
  contents: string,
  source = "editor.flush",
) {
  if (!relativePath) return;
  const lease = captureSyncLease(relativePath);
  if (!lease) return;
  const key = taskKey(lease, relativePath);
  const previousContents = desiredBuffers.get(key);
  if (previousContents === undefined || previousContents === contents) return;
  queueFileBufferDraftTextTransitionForPath(
    relativePath,
    previousContents,
    contents,
    source,
  );
}

function captureSyncLease(relativePath: string): FileBufferDraftSyncLease | null {
  if (activeSession) {
    return { ...activeSession, generation: syncGeneration };
  }
  failures.set(`unbound\u0000${relativePath}`, {
    relativePath,
    message: t("file-buffer-sync-session-missing"),
  });
  return null;
}

function taskKey(lease: FileBufferDraftSyncLease, relativePath: string) {
  return `${lease.generation}\u0000${lease.expectedProjectRoot}\u0000${lease.expectedSessionId}\u0000${relativePath}`;
}

function acceptContinuousFrontendTransition(
  key: string,
  relativePath: string,
  beforeText: string,
  afterText: string,
) {
  const expectedBefore = desiredBuffers.get(key) ?? pending.get(key)?.contents;
  if (expectedBefore === undefined || expectedBefore === beforeText) {
    desiredBuffers.set(key, afterText);
    return true;
  }

  failures.set(key, {
    relativePath,
    sticky: true,
    message: t("file-buffer-sync-frontend-discontinuity", {
      path: relativePath,
      expectedHash: hashFileBufferText(expectedBefore),
      actualHash: hashFileBufferText(beforeText),
    }),
  });
  console.warn(
    "[Pană Studio] FileBufferStore frontend continuity failed",
    relativePath,
    failures.get(key)?.message,
  );
  return false;
}

function taskIsCurrent(task: FileBufferDraftSyncTask) {
  return Boolean(
    activeSession
      && syncGeneration === task.generation
      && activeSession.expectedProjectRoot === task.expectedProjectRoot
      && activeSession.expectedSessionId === task.expectedSessionId,
  );
}

function taskIdentity(task: FileBufferDraftSyncTask): FileBufferRequestIdentity {
  return {
    expectedProjectRoot: task.expectedProjectRoot,
    expectedSessionId: task.expectedSessionId,
  };
}

export async function flushFileBufferDraftSync(options: { throwOnFailure?: boolean } = {}) {
  ensureDrain();
  while (drainPromise) {
    await drainPromise;
  }
  if ((options.throwOnFailure ?? true) && failures.size > 0) {
    const details = Array.from(failures.values())
      .map(({ relativePath, message }) => `${relativePath}: ${message}`)
      .join("; ");
    throw new Error(t("file-buffer-sync-failures", { details }));
  }
}

export function hasPendingFileBufferDraftSync() {
  return pending.size > 0 || drainPromise !== null || failures.size > 0;
}

export function resetFileBufferDraftSyncState() {
  invalidateSyncGeneration();
  activeSession = null;
}

function invalidateSyncGeneration() {
  syncGeneration = syncGeneration >= Number.MAX_SAFE_INTEGER ? 1 : syncGeneration + 1;
  pending.clear();
  failures.clear();
  confirmedBuffers.clear();
  desiredBuffers.clear();
}

export function invalidateFileBufferDraftSyncCursor(relativePath: string) {
  if (!relativePath || !activeSession) return;
  const key = taskKey({ ...activeSession, generation: syncGeneration }, relativePath);
  failures.delete(key);
  confirmedBuffers.delete(key);
  desiredBuffers.delete(key);
}

/**
 * Reanchors only the frontend CAS cursor after a separately validated Rust
 * receipt/read. It performs no IPC and never changes FileBufferStore state.
 */
export function reanchorFileBufferDraftSyncCursor(
  relativePath: string,
  cursor: FileBufferSyncCursor,
): boolean {
  if (!relativePath || !activeSession) return false;
  if (
    !Number.isSafeInteger(cursor.revision)
    || cursor.revision < 0
    || !/^[0-9a-f]{16}$/.test(cursor.hash)
  ) {
    throw new Error(
      t("file-buffer-sync-invalid-cursor", { path: relativePath }),
    );
  }
  const key = taskKey({ ...activeSession, generation: syncGeneration }, relativePath);
  failures.delete(key);
  confirmedBuffers.set(key, { ...cursor });
  return true;
}

/**
 * Replaces the frontend FileBuffer projection with the exact snapshot
 * published by a validated ProjectWorkspace history receipt. The history
 * command is allowed to call this only after the draft queue has drained.
 * No IPC is performed and no stale pre-history destination is retained.
 */
export function rebaseFileBufferDraftSyncProjection(
  relativePath: string,
  snapshot: FileBufferTextSnapshot | null,
): boolean {
  if (!relativePath || !activeSession) return false;
  const key = taskKey({ ...activeSession, generation: syncGeneration }, relativePath);
  if (pending.has(key) || drainPromise) {
    throw new Error(
      t("file-buffer-sync-history-rebase-pending", { path: relativePath }),
    );
  }

  failures.delete(key);
  confirmedBuffers.delete(key);
  desiredBuffers.delete(key);
  if (snapshot === null) return true;

  if (snapshot.relativePath !== relativePath) {
    throw new Error(
      t("file-buffer-sync-history-path-mismatch", {
        path: relativePath,
        actualPath: snapshot.relativePath,
      }),
    );
  }
  const calculatedHash = hashFileBufferText(snapshot.text);
  const calculatedBytes = utf8ByteLength(snapshot.text);
  if (
    snapshot.hash !== calculatedHash
    || snapshot.bytes !== calculatedBytes
    || !Number.isSafeInteger(snapshot.revision)
    || snapshot.revision < 0
  ) {
    throw new Error(
      t("file-buffer-sync-history-metadata-mismatch", {
        path: relativePath,
        hash: snapshot.hash,
        bytes: snapshot.bytes,
        revision: snapshot.revision,
        calculatedHash,
        calculatedBytes,
      }),
    );
  }

  desiredBuffers.set(key, snapshot.text);
  confirmedBuffers.set(key, {
    revision: snapshot.revision,
    hash: snapshot.hash,
  });
  return true;
}

function ensureDrain() {
  if (drainPromise) return;
  if (pending.size === 0) return;
  drainPromise = drain().finally(() => {
    drainPromise = null;
    if (pending.size > 0) ensureDrain();
  });
}

async function drain() {
  while (pending.size > 0) {
    const tasks = Array.from(pending.values());
    pending.clear();
    for (const task of tasks) {
      await applyTask(task);
    }
  }
}

async function applyTask(task: FileBufferDraftSyncTask) {
  if (!taskIsCurrent(task)) return;
  const key = taskKey(task, task.relativePath);
  try {
    if (!await applyChangeSetTask(task)) return;
    if (!taskIsCurrent(task)) return;
    if (!failures.get(key)?.sticky) failures.delete(key);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (!taskIsCurrent(task)) return;
    confirmedBuffers.delete(key);
    if (isStaleSessionDiagnostic(message)) return;
    if (!failures.get(key)?.sticky) {
      failures.set(key, { relativePath: task.relativePath, message });
    }
    console.warn("[Pană Studio] FileBufferStore draft sync failed", task.relativePath, message);
  }
}

async function applyChangeSetTask(task: FileBufferChangeSetDraftSyncTask): Promise<boolean> {
  const key = taskKey(task, task.relativePath);
  const taskBaseHash = hashFileBufferText(task.base);
  const base = await ensureConfirmedBuffer(task);
  if (!base) return false;

  if (base.hash !== taskBaseHash) {
    confirmedBuffers.delete(key);
    throw changeSetSyncError(
      task.relativePath,
      "conflict",
      t("file-buffer-sync-frontend-hash-mismatch", {
        frontendHash: taskBaseHash,
        confirmedHash: base.hash,
      }),
    );
  }

  try {
    return await applyChangeSetAtCursor(task, base);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (!taskIsCurrent(task) || isStaleSessionDiagnostic(message)) return false;
    const failureKind = classifyChangeSetFailure(message);
    confirmedBuffers.delete(key);

    if (failureKind !== "conflict") {
      throw changeSetSyncError(task.relativePath, failureKind, message);
    }

    let current: FileBufferSyncCursor;
    try {
      const snapshot = await readFileBufferText(task.relativePath, taskIdentity(task));
      if (!taskIsCurrent(task)) return false;
      current = { revision: snapshot.revision, hash: snapshot.hash };
    } catch (refreshError) {
      const refreshMessage = refreshError instanceof Error ? refreshError.message : String(refreshError);
      if (!taskIsCurrent(task) || isStaleSessionDiagnostic(refreshMessage)) return false;
      throw changeSetSyncError(
        task.relativePath,
        "sync_failed",
        t("file-buffer-sync-revalidation-failed", {
          message,
          refreshMessage,
        }),
      );
    }

    if (current.hash !== taskBaseHash) {
      throw changeSetSyncError(
        task.relativePath,
        "conflict",
        t("file-buffer-sync-buffer-revision-advanced", {
          message,
          revision: current.revision,
          hash: current.hash,
        }),
      );
    }

    try {
      return await applyChangeSetAtCursor(task, current);
    } catch (retryError) {
      confirmedBuffers.delete(key);
      const retryMessage = retryError instanceof Error ? retryError.message : String(retryError);
      if (!taskIsCurrent(task) || isStaleSessionDiagnostic(retryMessage)) return false;
      throw changeSetSyncError(
        task.relativePath,
        classifyChangeSetFailure(retryMessage),
        t("file-buffer-sync-cas-retry-failed", { message: retryMessage }),
      );
    }
  }
}

async function applyChangeSetAtCursor(
  task: FileBufferChangeSetDraftSyncTask,
  cursor: FileBufferSyncCursor,
) : Promise<boolean> {
  const result = await applyFileBufferChangeSet(
    {
      relativePath: task.relativePath,
      baseRevision: cursor.revision,
      baseHash: cursor.hash,
      coordinateSpace: "utf16",
      source: task.source,
      changes: task.changes,
    },
    taskIdentity(task),
  );
  if (!taskIsCurrent(task)) return false;
  confirmedBuffers.set(taskKey(task, task.relativePath), {
    revision: result.revision,
    hash: result.currentHash,
  });
  return true;
}

function isStaleSessionDiagnostic(diagnostic: string) {
  return diagnostic.includes("[file_buffer_stale_session]")
    || diagnostic.includes("[file_buffer_stale_receipt]");
}

function classifyChangeSetFailure(diagnostic: string): FileBufferChangeSetFailureKind {
  if (diagnostic.includes("[file_buffer_changeset_conflict]")) {
    return "conflict";
  }
  if (diagnostic.includes("[file_buffer_changeset_invalid]")) {
    return "invalid_change_set";
  }
  return "sync_failed";
}

function changeSetSyncError(
  relativePath: string,
  failureKind: FileBufferChangeSetFailureKind,
  diagnostic: string,
) {
  const localizedDiagnostic = failureKind === "conflict"
    ? t("file-buffer-sync-changeset-conflict")
    : failureKind === "invalid_change_set"
      ? t("file-buffer-sync-changeset-invalid")
      : diagnostic;
  return new Error(
    t("file-buffer-sync-changeset-blocked", {
      path: relativePath,
      kind: failureKind,
      diagnostic: localizedDiagnostic,
    }),
  );
}

async function ensureConfirmedBuffer(
  task: FileBufferDraftSyncTask,
): Promise<FileBufferSyncCursor | null> {
  const key = taskKey(task, task.relativePath);
  const cached = confirmedBuffers.get(key);
  if (cached) return cached;
  const snapshot = await readFileBufferText(task.relativePath, taskIdentity(task));
  if (!taskIsCurrent(task)) return null;
  const cursor = {
    revision: snapshot.revision,
    hash: snapshot.hash,
  };
  confirmedBuffers.set(key, cursor);
  return cursor;
}

function utf8ByteLength(text: string) {
  return new TextEncoder().encode(text).byteLength;
}

export function hashFileBufferText(text: string) {
  let hash = 0xcbf29ce484222325n;
  for (const byte of new TextEncoder().encode(text)) {
    hash ^= BigInt(byte);
    hash = (hash * 0x100000001b3n) & 0xffffffffffffffffn;
  }
  return hash.toString(16).padStart(16, "0");
}

function textTransitionToChangeSet(beforeText: string, afterText: string): FileBufferTextChange[] {
  if (beforeText === afterText) return [];

  let start = 0;
  const sharedLength = Math.min(beforeText.length, afterText.length);
  while (start < sharedLength) {
    const beforeSegment = codePointSegmentAt(beforeText, start);
    const afterSegment = codePointSegmentAt(afterText, start);
    if (!beforeSegment || beforeSegment !== afterSegment) break;
    start += beforeSegment.length;
  }

  let beforeEnd = beforeText.length;
  let afterEnd = afterText.length;
  while (beforeEnd > start && afterEnd > start) {
    const beforeStart = previousCodePointStart(beforeText, beforeEnd);
    const afterStart = previousCodePointStart(afterText, afterEnd);
    if (beforeStart < start || afterStart < start) break;
    const beforeSegment = beforeText.slice(beforeStart, beforeEnd);
    const afterSegment = afterText.slice(afterStart, afterEnd);
    if (beforeSegment !== afterSegment) break;
    beforeEnd = beforeStart;
    afterEnd = afterStart;
  }

  return [{
    from: start,
    to: beforeEnd,
    insert: afterText.slice(start, afterEnd),
  }];
}

function codePointSegmentAt(text: string, index: number) {
  if (index >= text.length) return "";
  const first = text.charCodeAt(index);
  if (isHighSurrogate(first) && index + 1 < text.length && isLowSurrogate(text.charCodeAt(index + 1))) {
    return text.slice(index, index + 2);
  }
  return text.slice(index, index + 1);
}

function previousCodePointStart(text: string, end: number) {
  if (end <= 0) return 0;
  const previous = text.charCodeAt(end - 1);
  if (isLowSurrogate(previous) && end - 2 >= 0 && isHighSurrogate(text.charCodeAt(end - 2))) {
    return end - 2;
  }
  return end - 1;
}

function isHighSurrogate(value: number) {
  return value >= 0xd800 && value <= 0xdbff;
}

function isLowSurrogate(value: number) {
  return value >= 0xdc00 && value <= 0xdfff;
}
