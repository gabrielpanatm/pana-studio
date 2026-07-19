import assert from "node:assert/strict";
import { afterEach, beforeEach, test } from "node:test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  flushFileBufferDraftSync,
  fileBufferDraftSyncSnapshot,
  queueFileBufferDraftChangeSetForPath,
  queueFileBufferDraftClear,
  queueFileBufferDraftFlushSnapshotForPath,
  queueFileBufferDraftSyncForPath,
  queueFileBufferDraftTextTransitionForPath,
  reanchorFileBufferDraftSyncCursor,
  rebaseFileBufferDraftSyncProjection,
  resetFileBufferDraftSyncState,
  setFileBufferDraftSyncSession,
} from "$lib/session/file-buffer-draft-sync";

if (!globalThis.window) globalThis.window = globalThis;

const relativePath = "sursa/templates/index.html";
const projectRoot = "/project";
const sessionA = "stable-project:runtime-a";
const sessionB = "stable-project:runtime-b";

beforeEach(() => {
  setFileBufferDraftSyncSession(projectRoot, sessionA);
});

afterEach(() => {
  clearMocks();
  resetFileBufferDraftSyncState();
});

for (const conflictKind of ["revision", "hash"]) {
  test(`a stale change-set cannot overwrite a concurrent ${conflictKind} mutation`, async () => {
    const beforeText = "alpha";
    const afterText = "alpha!";
    let serverText = beforeText;
    let serverRevision = 1;
    let readCount = 0;
    let applyCount = 0;
    let fullDraftCount = 0;

    mockIPC((command, payload) => {
      if (command === "read_file_buffer_text") {
        readCount += 1;
        return commandReceipt(textSnapshot(serverText, serverRevision));
      }
      if (command === "apply_file_buffer_changeset") {
        applyCount += 1;
        serverText = "concurrent authority";
        serverRevision = 2;
        if (conflictKind === "revision") {
          throw new Error(
            `FileBufferStore a refuzat change-set-ul pentru ${relativePath}: `
              + `revizia așteptată ${payload.input.baseRevision}, revizia curentă ${serverRevision}.`,
          );
        }
        throw new Error(
          `FileBufferStore a refuzat change-set-ul pentru ${relativePath}: `
            + "hash-ul de bază nu mai corespunde bufferului curent.",
        );
      }
      if (command === "set_file_buffer_draft") {
        fullDraftCount += 1;
        serverText = payload.contents;
        serverRevision += 1;
        return commandReceipt(fileSnapshot(serverText, serverRevision));
      }
      throw new Error(`Comandă IPC neașteptată: ${command}`);
    });

    queueFileBufferDraftChangeSetForPath(
      relativePath,
      beforeText,
      afterText,
      [{ from: beforeText.length, to: beforeText.length, insert: "!" }],
      "concurrency_test",
    );

    await assert.rejects(
      flushFileBufferDraftSync(),
      (error) => error instanceof Error
        && error.message.includes("(conflict)")
        && error.message.includes("fallback-ul full-draft fără CAS este interzis"),
    );
    assert.equal(serverText, "concurrent authority");
    assert.equal(serverRevision, 2);
    assert.equal(applyCount, 1);
    assert.equal(readCount, 2);
    assert.equal(fullDraftCount, 0);
  });
}

test("a revision-only conflict retries once when the authoritative hash is unchanged", async () => {
  const beforeText = "alpha";
  const afterText = "alpha!";
  let serverText = beforeText;
  let serverRevision = 1;
  let readCount = 0;
  let applyCount = 0;
  let fullDraftCount = 0;

  mockIPC((command, payload) => {
    if (command === "read_file_buffer_text") {
      readCount += 1;
      return commandReceipt(textSnapshot(serverText, serverRevision));
    }
    if (command === "apply_file_buffer_changeset") {
      applyCount += 1;
      if (applyCount === 1) {
        serverRevision = 2;
        throw new Error(
          `FileBufferStore a refuzat change-set-ul pentru ${relativePath}: `
            + `revizia așteptată ${payload.input.baseRevision}, revizia curentă ${serverRevision}.`,
        );
      }

      assert.equal(payload.input.baseRevision, 2);
      assert.equal(payload.input.baseHash, hashText(beforeText));
      serverText = applyAsciiChanges(serverText, payload.input.changes);
      serverRevision = 3;
      return commandReceipt(changeSetReceipt(serverText, serverRevision));
    }
    if (command === "set_file_buffer_draft") {
      fullDraftCount += 1;
      throw new Error("full-draft fallback must not run");
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  queueFileBufferDraftChangeSetForPath(
    relativePath,
    beforeText,
    afterText,
    [{ from: beforeText.length, to: beforeText.length, insert: "!" }],
    "revision_retry_test",
  );

  await flushFileBufferDraftSync();
  assert.equal(serverText, afterText);
  assert.equal(serverRevision, 3);
  assert.equal(applyCount, 2);
  assert.equal(readCount, 2);
  assert.equal(fullDraftCount, 0);
});

test("rapid continuous Markdown updates preserve the oldest pending base while IPC is in flight", async () => {
  const beforeText = "Scrie conținutul aici.";
  const firstText = `${beforeText} `;
  const finalText = `${firstText}continut scris frumos`;
  const firstApplyStarted = deferred();
  const releaseFirstApply = deferred();
  let serverText = beforeText;
  let serverRevision = 1;
  let applyCount = 0;
  let fullDraftCount = 0;

  mockIPC(async (command, payload) => {
    if (command === "read_file_buffer_text") {
      return commandReceipt(textSnapshot(serverText, serverRevision));
    }
    if (command === "apply_file_buffer_changeset") {
      applyCount += 1;
      if (applyCount === 1) {
        firstApplyStarted.resolve();
        await releaseFirstApply.promise;
      }
      assert.equal(payload.input.baseRevision, serverRevision);
      assert.equal(payload.input.baseHash, hashText(serverText));
      serverText = applyAsciiChanges(serverText, payload.input.changes);
      serverRevision += 1;
      return commandReceipt(changeSetReceipt(serverText, serverRevision));
    }
    if (command === "set_file_buffer_draft") {
      fullDraftCount += 1;
      throw new Error("tastarea continuă nu trebuie degradată la full-draft");
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  queueFileBufferDraftTextTransitionForPath(
    relativePath,
    beforeText,
    firstText,
    "markdown.editor",
  );
  await firstApplyStarted.promise;

  let frontendText = firstText;
  for (const character of "continut scris frumos") {
    const nextText = frontendText + character;
    queueFileBufferDraftTextTransitionForPath(
      relativePath,
      frontendText,
      nextText,
      "markdown.editor",
    );
    frontendText = nextText;
  }

  releaseFirstApply.resolve();
  await flushFileBufferDraftSync();

  assert.equal(frontendText, finalText);
  assert.equal(serverText, finalText);
  assert.equal(serverRevision, 3);
  assert.equal(applyCount, 2);
  assert.equal(fullDraftCount, 0);
  assert.equal(fileBufferDraftSyncSnapshot().failureCount, 0);
});

test("a Markdown flush snapshot extends the last queued frontend state through CAS", async () => {
  const beforeText = "alpha";
  const queuedText = "alpha beta";
  const finalText = "alpha beta gamma";
  const firstApplyStarted = deferred();
  const releaseFirstApply = deferred();
  let serverText = beforeText;
  let serverRevision = 1;
  let applyCount = 0;

  mockIPC(async (command, payload) => {
    if (command === "read_file_buffer_text") {
      return commandReceipt(textSnapshot(serverText, serverRevision));
    }
    if (command === "apply_file_buffer_changeset") {
      applyCount += 1;
      if (applyCount === 1) {
        firstApplyStarted.resolve();
        await releaseFirstApply.promise;
      }
      assert.equal(payload.input.baseRevision, serverRevision);
      assert.equal(payload.input.baseHash, hashText(serverText));
      serverText = applyAsciiChanges(serverText, payload.input.changes);
      serverRevision += 1;
      return commandReceipt(changeSetReceipt(serverText, serverRevision));
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  queueFileBufferDraftTextTransitionForPath(
    relativePath,
    beforeText,
    queuedText,
    "markdown.editor",
  );
  await firstApplyStarted.promise;
  queueFileBufferDraftFlushSnapshotForPath(
    relativePath,
    finalText,
    "markdown.editor.flush",
  );
  releaseFirstApply.resolve();

  await flushFileBufferDraftSync();
  assert.equal(serverText, finalText);
  assert.equal(serverRevision, 3);
  assert.equal(applyCount, 2);
  assert.equal(fileBufferDraftSyncSnapshot().failureCount, 0);
});

test("a discontinuous frontend transition stays blocked after an older in-flight task succeeds", async () => {
  const beforeText = "alpha";
  const firstText = "alpha beta";
  const firstApplyStarted = deferred();
  const releaseFirstApply = deferred();
  let serverText = beforeText;
  let serverRevision = 1;
  let applyCount = 0;

  mockIPC(async (command, payload) => {
    if (command === "read_file_buffer_text") {
      return commandReceipt(textSnapshot(serverText, serverRevision));
    }
    if (command === "apply_file_buffer_changeset") {
      applyCount += 1;
      firstApplyStarted.resolve();
      await releaseFirstApply.promise;
      serverText = applyAsciiChanges(serverText, payload.input.changes);
      serverRevision += 1;
      return commandReceipt(changeSetReceipt(serverText, serverRevision));
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  queueFileBufferDraftTextTransitionForPath(
    relativePath,
    beforeText,
    firstText,
    "markdown.editor",
  );
  await firstApplyStarted.promise;
  queueFileBufferDraftTextTransitionForPath(
    relativePath,
    "altă bază",
    "text care nu poate fi legat de coadă",
    "markdown.editor",
  );
  releaseFirstApply.resolve();

  await assert.rejects(
    flushFileBufferDraftSync(),
    (error) => error instanceof Error
      && error.message.includes("[file_buffer_frontend_discontinuity]"),
  );
  assert.equal(serverText, firstText);
  assert.equal(serverRevision, 2);
  assert.equal(applyCount, 1);
  assert.equal(fileBufferDraftSyncSnapshot().failureCount, 1);
});

test("an invalid change-set fails closed without retry or full-draft replacement", async () => {
  const beforeText = "alpha";
  let readCount = 0;
  let applyCount = 0;
  let fullDraftCount = 0;

  mockIPC((command) => {
    if (command === "read_file_buffer_text") {
      readCount += 1;
      return commandReceipt(textSnapshot(beforeText, 1));
    }
    if (command === "apply_file_buffer_changeset") {
      applyCount += 1;
      throw new Error(
        "FileBufferStore a refuzat change-set-ul: offsetul UTF-16 20 depășește documentul.",
      );
    }
    if (command === "set_file_buffer_draft") {
      fullDraftCount += 1;
      throw new Error("full-draft fallback must not run");
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  queueFileBufferDraftChangeSetForPath(
    relativePath,
    beforeText,
    "alpha!",
    [{ from: 20, to: 20, insert: "!" }],
    "invalid_changeset_test",
  );

  await assert.rejects(
    flushFileBufferDraftSync(),
    (error) => error instanceof Error && error.message.includes("(invalid_change_set)"),
  );
  assert.equal(readCount, 1);
  assert.equal(applyCount, 1);
  assert.equal(fullDraftCount, 0);
});

test("a full-draft CAS cannot overwrite a mutation committed after its read", async () => {
  const beforeText = "alpha";
  const afterText = "alpha restored";
  let serverText = beforeText;
  let serverRevision = 1;
  let readCount = 0;
  let setCount = 0;

  mockIPC((command, payload) => {
    if (command === "read_file_buffer_text") {
      readCount += 1;
      return commandReceipt(textSnapshot(serverText, serverRevision));
    }
    if (command === "set_file_buffer_draft") {
      setCount += 1;
      assert.deepEqual(payload.expectation, {
        expectedRevision: 1,
        expectedHash: hashText(beforeText),
      });
      serverText = "concurrent authority";
      serverRevision = 2;
      throw new Error(
        `[file_buffer_draft_cas_conflict] expected ${payload.expectation.expectedRevision}/`
          + `${payload.expectation.expectedHash}, current ${serverRevision}/${hashText(serverText)}`,
      );
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  queueFileBufferDraftSyncForPath(
    relativePath,
    beforeText,
    afterText,
    "full_draft_concurrency_test",
  );

  await assert.rejects(
    flushFileBufferDraftSync(),
    (error) => error instanceof Error
      && error.message.includes("[file_buffer_draft_cas_conflict]"),
  );
  assert.equal(serverText, "concurrent authority");
  assert.equal(serverRevision, 2);
  assert.equal(readCount, 1);
  assert.equal(setCount, 1);
});

test("a clear CAS cannot erase a newer draft committed after its read", async () => {
  const observedDraft = "owned draft";
  let serverText = observedDraft;
  let serverRevision = 2;
  let readCount = 0;
  let clearCount = 0;

  mockIPC((command, payload) => {
    if (command === "read_file_buffer_text") {
      readCount += 1;
      return commandReceipt(textSnapshot(serverText, serverRevision));
    }
    if (command === "clear_file_buffer_draft") {
      clearCount += 1;
      assert.deepEqual(payload.expectation, {
        expectedRevision: 2,
        expectedHash: hashText(observedDraft),
      });
      serverText = "newer draft";
      serverRevision = 3;
      throw new Error(
        `[file_buffer_draft_cas_conflict] expected ${payload.expectation.expectedRevision}/`
          + `${payload.expectation.expectedHash}, current ${serverRevision}/${hashText(serverText)}`,
      );
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  queueFileBufferDraftClear(relativePath, "clear_concurrency_test");

  await assert.rejects(
    flushFileBufferDraftSync(),
    (error) => error instanceof Error
      && error.message.includes("[file_buffer_draft_cas_conflict]"),
  );
  assert.equal(serverText, "newer draft");
  assert.equal(serverRevision, 3);
  assert.equal(readCount, 1);
  assert.equal(clearCount, 1);
});

test("a successful full-draft sync binds the mutation to its read receipt", async () => {
  const beforeText = "alpha";
  const afterText = "alpha restored";
  let setCount = 0;

  mockIPC((command, payload) => {
    if (command === "read_file_buffer_text") {
      return commandReceipt(textSnapshot(beforeText, 4));
    }
    if (command === "set_file_buffer_draft") {
      setCount += 1;
      assert.deepEqual(payload.expectation, {
        expectedRevision: 4,
        expectedHash: hashText(beforeText),
      });
      return commandReceipt(fileSnapshot(afterText, 5));
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  queueFileBufferDraftSyncForPath(relativePath, beforeText, afterText, "full_draft_success");
  await flushFileBufferDraftSync();

  assert.equal(setCount, 1);
  assert.equal(fileBufferDraftSyncSnapshot().failureCount, 0);
  assert.equal(fileBufferDraftSyncSnapshot().cursorCount, 1);
});

test("a full draft accepts a Save revision advance when cursor, base and current hash agree", async () => {
  const beforeText = "alpha";
  const afterText = "alpha after Save";
  let setCount = 0;

  assert.equal(reanchorFileBufferDraftSyncCursor(relativePath, {
    revision: 4,
    hash: hashText(beforeText),
  }), true);

  mockIPC((command, payload) => {
    if (command === "read_file_buffer_text") {
      // Save advanced only the revision after the cursor was confirmed.
      return commandReceipt(textSnapshot(beforeText, 5));
    }
    if (command === "set_file_buffer_draft") {
      setCount += 1;
      assert.deepEqual(payload.expectation, {
        expectedRevision: 5,
        expectedHash: hashText(beforeText),
      });
      return commandReceipt(fileSnapshot(afterText, 6));
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  queueFileBufferDraftSyncForPath(
    relativePath,
    beforeText,
    afterText,
    "save_revision_reanchor_test",
  );
  await flushFileBufferDraftSync();

  assert.equal(setCount, 1);
  assert.equal(fileBufferDraftSyncSnapshot().failureCount, 0);
});

test("history projection rebases both the desired text and CAS cursor without a second read", async () => {
  const restoredText = "restored by history";
  const editedText = `${restoredText}!`;
  let readCount = 0;
  let applyCount = 0;

  assert.equal(rebaseFileBufferDraftSyncProjection(
    relativePath,
    textSnapshot(restoredText, 12),
  ), true);

  mockIPC((command, payload) => {
    if (command === "read_file_buffer_text") {
      readCount += 1;
      throw new Error("history rebase must make this read unnecessary");
    }
    if (command === "apply_file_buffer_changeset") {
      applyCount += 1;
      assert.equal(payload.input.baseRevision, 12);
      assert.equal(payload.input.baseHash, hashText(restoredText));
      return commandReceipt(changeSetReceipt(editedText, 13));
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  queueFileBufferDraftTextTransitionForPath(
    relativePath,
    restoredText,
    editedText,
    "history_rebase_test",
  );
  await flushFileBufferDraftSync();

  assert.equal(readCount, 0);
  assert.equal(applyCount, 1);
  assert.equal(fileBufferDraftSyncSnapshot().failureCount, 0);

  assert.equal(rebaseFileBufferDraftSyncProjection(relativePath, null), true);
  assert.equal(fileBufferDraftSyncSnapshot().cursorCount, 0);
});

test("a successful clear binds the mutation to its read receipt and verifies clean state", async () => {
  const draftText = "owned draft";
  let clearCount = 0;

  mockIPC((command, payload) => {
    if (command === "read_file_buffer_text") {
      return commandReceipt(textSnapshot(draftText, 7));
    }
    if (command === "clear_file_buffer_draft") {
      clearCount += 1;
      assert.deepEqual(payload.expectation, {
        expectedRevision: 7,
        expectedHash: hashText(draftText),
      });
      return commandReceipt(fileSnapshot("alpha", 8, { hasDraft: false, dirty: false }));
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  queueFileBufferDraftClear(relativePath, "clear_success");
  await flushFileBufferDraftSync();

  assert.equal(clearCount, 1);
  assert.equal(fileBufferDraftSyncSnapshot().failureCount, 0);
  assert.equal(fileBufferDraftSyncSnapshot().cursorCount, 1);
});

test("same-root reopen invalidates an in-flight continuation without contaminating session B", async () => {
  const readStarted = deferred();
  const releaseRead = deferred();
  let readCount = 0;
  let mutationCount = 0;

  mockIPC(async (command, payload) => {
    if (command === "read_file_buffer_text") {
      readCount += 1;
      assert.deepEqual(payload.identity, {
        expectedProjectRoot: projectRoot,
        expectedSessionId: sessionA,
      });
      readStarted.resolve();
      await releaseRead.promise;
      return commandReceipt(textSnapshot("alpha", 1), sessionA);
    }
    if (
      command === "apply_file_buffer_changeset"
      || command === "set_file_buffer_draft"
      || command === "clear_file_buffer_draft"
    ) {
      mutationCount += 1;
      throw new Error("taskul A nu trebuie să atingă sesiunea B");
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  queueFileBufferDraftChangeSetForPath(
    relativePath,
    "alpha",
    "alpha!",
    [{ from: 5, to: 5, insert: "!" }],
    "same_root_reopen_test",
  );
  await readStarted.promise;
  resetFileBufferDraftSyncState();
  setFileBufferDraftSyncSession(projectRoot, sessionB);
  releaseRead.resolve();
  await flushFileBufferDraftSync();

  assert.equal(readCount, 1);
  assert.equal(mutationCount, 0);
  assert.deepEqual(fileBufferDraftSyncSnapshot(), {
    generation: fileBufferDraftSyncSnapshot().generation,
    activeProjectRoot: projectRoot,
    activeSessionId: sessionB,
    pendingCount: 0,
    failureCount: 0,
    cursorCount: 0,
  });
});

test("a receipt from another runtime session is refused before creating a cursor", async () => {
  let applyCount = 0;
  mockIPC((command) => {
    if (command === "read_file_buffer_text") {
      return commandReceipt(textSnapshot("alpha", 1), sessionB);
    }
    if (command === "apply_file_buffer_changeset") {
      applyCount += 1;
      throw new Error("receiptul stale nu trebuie consumat");
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  queueFileBufferDraftChangeSetForPath(
    relativePath,
    "alpha",
    "alpha!",
    [{ from: 5, to: 5, insert: "!" }],
    "stale_receipt_test",
  );
  await flushFileBufferDraftSync();

  const snapshot = fileBufferDraftSyncSnapshot();
  assert.equal(applyCount, 0);
  assert.equal(snapshot.activeSessionId, sessionA);
  assert.equal(snapshot.failureCount, 0);
  assert.equal(snapshot.cursorCount, 0);
});

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function commandReceipt(payload, runtimeSessionId = sessionA) {
  return {
    projectRoot,
    runtimeSessionId,
    payload,
  };
}

function textSnapshot(text, revision) {
  return {
    relativePath,
    text,
    dirty: revision > 1,
    hash: hashText(text),
    bytes: new TextEncoder().encode(text).byteLength,
    revision,
  };
}

function fileSnapshot(text, revision, { hasDraft = true, dirty = true } = {}) {
  return {
    relativePath,
    hasDraft,
    dirty,
    currentHash: hashText(text),
    currentBytes: new TextEncoder().encode(text).byteLength,
    revision,
  };
}

function changeSetReceipt(text, revision) {
  return {
    relativePath,
    revision,
    currentHash: hashText(text),
  };
}

function applyAsciiChanges(text, changes) {
  return [...changes]
    .sort((left, right) => right.from - left.from)
    .reduce(
      (current, change) => current.slice(0, change.from) + change.insert + current.slice(change.to),
      text,
    );
}

function hashText(text) {
  let hash = 0xcbf29ce484222325n;
  for (const byte of new TextEncoder().encode(text)) {
    hash ^= BigInt(byte);
    hash = (hash * 0x100000001b3n) & 0xffffffffffffffffn;
  }
  return hash.toString(16).padStart(16, "0");
}
