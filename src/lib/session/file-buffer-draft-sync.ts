import {
  applyFileBufferChangeSet,
  clearFileBufferDraft,
  readFileBufferText,
  setFileBufferDraft,
} from "$lib/project/io";
import type { FileBufferRequestIdentity } from "$lib/types";
import type {
  FileBufferFileSnapshot,
  FileBufferTextChange,
  FileBufferTextSnapshot,
} from "$lib/types";

type FileBufferDraftSyncLease = {
  expectedProjectRoot: string;
  expectedSessionId: string;
  generation: number;
};

type FileBufferFullDraftSyncTask = FileBufferDraftSyncLease & {
  kind: "full";
  relativePath: string;
  source: string;
  base: string;
  dirty: boolean;
  contents: string;
};

type FileBufferChangeSetDraftSyncTask = FileBufferDraftSyncLease & {
  kind: "changeset";
  relativePath: string;
  dirty: true;
  base: string;
  contents: string;
  source: string;
  changes: FileBufferTextChange[];
};

type FileBufferDraftSyncTask = FileBufferFullDraftSyncTask | FileBufferChangeSetDraftSyncTask;

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
    kind: "changeset",
    relativePath,
    dirty: true,
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

export function queueFileBufferDraftSyncForPath(
  relativePath: string,
  beforeText: string,
  afterText: string,
  source = "source_draft",
) {
  if (!relativePath) return;
  const lease = captureSyncLease(relativePath);
  if (!lease) return;
  if (beforeText === afterText) return;
  const key = taskKey(lease, relativePath);
  if (!acceptContinuousFrontendTransition(key, relativePath, beforeText, afterText)) return;
  const previous = pending.get(key);
  const base = previous?.base ?? beforeText;
  if (base === afterText) {
    pending.delete(key);
    ensureDrain();
    return;
  }
  pending.set(key, {
    ...lease,
    kind: "full",
    relativePath,
    source,
    base,
    dirty: true,
    contents: afterText,
  });
  ensureDrain();
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

export function queueFileBufferDraftClear(relativePath: string, source = "clear") {
  if (!relativePath) return;
  const lease = captureSyncLease(relativePath);
  if (!lease) return;
  const key = taskKey(lease, relativePath);
  desiredBuffers.delete(key);
  pending.set(key, {
    ...lease,
    kind: "full",
    relativePath,
    source,
    base: "",
    dirty: false,
    contents: "",
  });
  ensureDrain();
}

function captureSyncLease(relativePath: string): FileBufferDraftSyncLease | null {
  if (activeSession) {
    return { ...activeSession, generation: syncGeneration };
  }
  failures.set(`unbound\u0000${relativePath}`, {
    relativePath,
    message: "[file_buffer_identity_invalid] FileBuffer draft sync nu are o sesiune activă.",
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
    message: `[file_buffer_frontend_discontinuity] Coada FileBuffer a refuzat o tranziție `
      + `necontinuă pentru ${relativePath}: hash-ul ultimei destinații frontend `
      + `(${hashFileBufferText(expectedBefore)}) diferă de baza noului eveniment `
      + `(${hashFileBufferText(beforeText)}). Textul nou a rămas numai în editor; Save este blocat.`,
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
    throw new Error(`FileBufferStore nu a putut sincroniza drafturile active. ${details}`);
  }
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
      `[file_buffer_invalid_cursor] Cursorul FileBuffer pentru ${relativePath} este invalid.`,
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
      `[file_buffer_history_rebase_pending] Proiecția FileBuffer pentru ${relativePath} `
        + "nu poate fi rebazată cât timp coada de drafturi este activă.",
    );
  }

  failures.delete(key);
  confirmedBuffers.delete(key);
  desiredBuffers.delete(key);
  if (snapshot === null) return true;

  if (snapshot.relativePath !== relativePath) {
    throw new Error(
      `[file_buffer_invalid_history_projection] Snapshot-ul pentru ${relativePath} `
        + `declară path-ul ${snapshot.relativePath}.`,
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
      `[file_buffer_invalid_history_projection] Snapshot-ul pentru ${relativePath} `
        + `are hash/bytes/revision ${snapshot.hash}/${snapshot.bytes}/${snapshot.revision}, `
        + `dar textul confirmă ${calculatedHash}/${calculatedBytes}.`,
    );
  }

  desiredBuffers.set(key, snapshot.text);
  confirmedBuffers.set(key, {
    revision: snapshot.revision,
    hash: snapshot.hash,
  });
  return true;
}

export function fileBufferDraftSyncSnapshot() {
  return {
    generation: syncGeneration,
    activeProjectRoot: activeSession?.expectedProjectRoot ?? null,
    activeSessionId: activeSession?.expectedSessionId ?? null,
    pendingCount: pending.size,
    failureCount: failures.size,
    cursorCount: confirmedBuffers.size,
  };
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
    if (task.dirty) {
      if (task.kind === "changeset") {
        if (!await applyChangeSetTask(task)) return;
      } else {
        if (!await applyFullDraftTask(task)) return;
      }
    } else {
      if (!await applyClearDraftTask(task)) return;
    }
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

async function applyFullDraftTask(task: FileBufferFullDraftSyncTask): Promise<boolean> {
  const key = taskKey(task, task.relativePath);
  const previousCursor = confirmedBuffers.get(key);
  const current = await readFileBufferText(task.relativePath, taskIdentity(task));
  if (!taskIsCurrent(task)) return false;
  requireReadReceipt(task, current);

  const currentCursor = { revision: current.revision, hash: current.hash };
  if (current.text === task.contents) {
    confirmedBuffers.set(key, currentCursor);
    return true;
  }

  const taskBaseHash = hashFileBufferText(task.base);
  if (previousCursor && previousCursor.hash !== current.hash) {
    throw fullDraftSyncConflict(
      task.relativePath,
      `bufferul a avansat de la revision/hash ${previousCursor.revision}/${previousCursor.hash} `
        + `la ${currentCursor.revision}/${currentCursor.hash}.`,
    );
  }
  if (current.hash !== taskBaseHash) {
    throw fullDraftSyncConflict(
      task.relativePath,
      `sincronizarea a observat hash ${current.hash}, diferit de baza frontend ${taskBaseHash}.`,
    );
  }

  // Save projection advances the FileBuffer revision even when the current
  // text/hash remains the base of this full-draft task. That revision-only
  // advance is safe to reanchor; the mutation below is still bound to the
  // freshly read revision/hash and therefore cannot overwrite a real change.

  const snapshot = await setFileBufferDraft(
    task.relativePath,
    task.contents,
    mutationExpectation(currentCursor),
    taskIdentity(task),
  );
  if (!taskIsCurrent(task)) return false;
  requireSetReceipt(task, snapshot, currentCursor);
  rememberFileSnapshot(task, snapshot);
  return true;
}

async function applyClearDraftTask(task: FileBufferFullDraftSyncTask): Promise<boolean> {
  const key = taskKey(task, task.relativePath);
  const previousCursor = confirmedBuffers.get(key);
  const current = await readFileBufferText(task.relativePath, taskIdentity(task));
  if (!taskIsCurrent(task)) return false;
  requireReadReceipt(task, current);

  const currentCursor = { revision: current.revision, hash: current.hash };
  if (previousCursor && !sameCursor(previousCursor, currentCursor) && current.dirty) {
    throw fullDraftSyncConflict(
      task.relativePath,
      `clear-ul stale a observat revision/hash ${currentCursor.revision}/${currentCursor.hash}, `
        + `dar ultima stare confirmată era ${previousCursor.revision}/${previousCursor.hash}.`,
    );
  }

  const snapshot = await clearFileBufferDraft(
    task.relativePath,
    mutationExpectation(currentCursor),
    taskIdentity(task),
  );
  if (!taskIsCurrent(task)) return false;
  requireClearReceipt(task, snapshot, currentCursor);
  rememberFileSnapshot(task, snapshot);
  return true;
}

function mutationExpectation(cursor: FileBufferSyncCursor) {
  return {
    expectedRevision: cursor.revision,
    expectedHash: cursor.hash,
  };
}

function sameCursor(left: FileBufferSyncCursor, right: FileBufferSyncCursor) {
  return left.revision === right.revision && left.hash === right.hash;
}

function requireReadReceiptPath(task: FileBufferDraftSyncTask, relativePath: string) {
  if (relativePath !== task.relativePath) {
    throw new Error(
      `[file_buffer_invalid_receipt] FileBufferStore a returnat path ${relativePath} `
        + `pentru requestul ${task.relativePath}.`,
    );
  }
}

function requireReadReceipt(task: FileBufferDraftSyncTask, snapshot: FileBufferTextSnapshot) {
  requireReadReceiptPath(task, snapshot.relativePath);
  const calculatedHash = hashFileBufferText(snapshot.text);
  const calculatedBytes = utf8ByteLength(snapshot.text);
  if (
    snapshot.hash !== calculatedHash
    || snapshot.bytes !== calculatedBytes
    || !Number.isSafeInteger(snapshot.revision)
    || snapshot.revision < 0
  ) {
    throw new Error(
      `[file_buffer_invalid_receipt] Read pentru ${task.relativePath} are metadata inconsistentă: `
        + `hash/bytes/revision ${snapshot.hash}/${snapshot.bytes}/${snapshot.revision}, `
        + `calculate ${calculatedHash}/${calculatedBytes}.`,
    );
  }
}

function requireSetReceipt(
  task: FileBufferFullDraftSyncTask,
  snapshot: FileBufferFileSnapshot,
  expectation: FileBufferSyncCursor,
) {
  requireReadReceiptPath(task, snapshot.relativePath);
  const desiredHash = hashFileBufferText(task.contents);
  const desiredBytes = utf8ByteLength(task.contents);
  if (
    snapshot.currentHash !== desiredHash
    || snapshot.currentBytes !== desiredBytes
    || !Number.isSafeInteger(snapshot.revision)
    || snapshot.revision <= expectation.revision
  ) {
    throw new Error(
      `[file_buffer_invalid_receipt] Set draft pentru ${task.relativePath} nu confirmă starea cerută: `
        + `așteptat hash/bytes ${desiredHash}/${desiredBytes} de la revizia ${expectation.revision}, `
        + `primit ${snapshot.currentHash}/${snapshot.currentBytes}/${snapshot.revision}.`,
    );
  }
}

function requireClearReceipt(
  task: FileBufferFullDraftSyncTask,
  snapshot: FileBufferFileSnapshot,
  expectation: FileBufferSyncCursor,
) {
  requireReadReceiptPath(task, snapshot.relativePath);
  if (
    snapshot.hasDraft
    || snapshot.dirty
    || !Number.isSafeInteger(snapshot.revision)
    || snapshot.revision < expectation.revision
  ) {
    throw new Error(
      `[file_buffer_invalid_receipt] Clear draft pentru ${task.relativePath} nu confirmă `
        + `un buffer curat la sau după revizia ${expectation.revision}.`,
    );
  }
}

function fullDraftSyncConflict(relativePath: string, diagnostic: string) {
  return new Error(
    `[file_buffer_draft_cas_conflict] FileBufferStore a blocat mutația full-draft/clear `
      + `pentru ${relativePath}; suprascrierea fără CAS este interzisă. ${diagnostic}`,
  );
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
      `hash-ul textului frontend (${taskBaseHash}) nu corespunde bufferului confirmat (${base.hash}).`,
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
        `${message} Revalidarea bufferului a eșuat: ${refreshMessage}`,
      );
    }

    if (current.hash !== taskBaseHash) {
      throw changeSetSyncError(
        task.relativePath,
        "conflict",
        `${message} Bufferul a avansat la revizia ${current.revision}, cu un hash diferit (${current.hash}).`,
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
        `Reîncercarea CAS unică a eșuat: ${retryMessage}`,
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
  if (diagnostic.includes("revizia așteptată") || diagnostic.includes("hash-ul de bază")) {
    return "conflict";
  }
  if (
    diagnostic.includes("range")
    || diagnostic.includes("UTF-16")
    || diagnostic.includes("se suprapun")
    || diagnostic.includes("path gol")
    || diagnostic.includes("draftul rezultat")
    || diagnostic.includes("peste limita")
  ) {
    return "invalid_change_set";
  }
  return "sync_failed";
}

function changeSetSyncError(
  relativePath: string,
  failureKind: FileBufferChangeSetFailureKind,
  diagnostic: string,
) {
  return new Error(
    `FileBufferStore a blocat sincronizarea change-set pentru ${relativePath} `
      + `(${failureKind}); fallback-ul full-draft fără CAS este interzis. ${diagnostic}`,
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

function rememberFileSnapshot(task: FileBufferDraftSyncTask, snapshot: FileBufferFileSnapshot) {
  confirmedBuffers.set(taskKey(task, snapshot.relativePath), {
    revision: snapshot.revision,
    hash: snapshot.currentHash,
  });
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
