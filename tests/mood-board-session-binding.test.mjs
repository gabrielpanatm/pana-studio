import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { createEmptyMoodBoard, parsePersistedMoodBoard } from "$lib/mood-board/model";
import {
  commitMoodBoard,
  drainMoodBoardSaveBeforeTransition,
  loadMoodBoard,
  saveMoodBoardNow,
} from "$lib/state/mood-board-controller";
import { handleNativeWindowCloseRequest } from "$lib/state/native-window-close-controller";
import { closeCurrentProject } from "$lib/state/project-controller";
import { resetFileBufferDraftSyncState } from "$lib/session/file-buffer-draft-sync";
import { resetPageJsDraftSyncState } from "$lib/session/page-js-draft-sync";
import {
  exportMoodBoardSvgAsset,
  MoodBoardStaleSessionError,
  resolveMoodBoardImageSrc,
} from "$lib/mood-board/io";
import {
  addMoodBoardVisualAssetAtPath,
  applyMoodBoardPaletteColors,
  applyMoodBoardVisualAssetItem,
  exportMoodBoardVectorPathWorkflow,
  extractMoodBoardPaletteItems,
} from "$lib/mood-board/canvas-assets";
import {
  createMoodBoardImageItem,
  createMoodBoardVectorPath,
} from "$lib/mood-board/factory";

if (!globalThis.window) globalThis.window = globalThis;

afterEach(() => {
  clearMocks();
  resetFileBufferDraftSyncState();
  resetPageJsDraftSyncState();
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

async function nextTurn() {
  await new Promise((resolve) => setImmediate(resolve));
}

async function waitUntil(predicate, message) {
  for (let attempt = 0; attempt < 40; attempt += 1) {
    if (predicate()) return;
    await nextTurn();
  }
  assert.fail(message);
}

function noteBoard(text) {
  return {
    ...createEmptyMoodBoard(),
    items: [{
      id: `note-${text}`,
      type: "note",
      x: 10,
      y: 20,
      width: 240,
      height: 160,
      text,
    }],
  };
}

function sessionGuard(active) {
  return (identity) => active.current?.expectedProjectRoot === identity.expectedProjectRoot
    && active.current?.expectedSessionId === identity.expectedSessionId;
}

function controllerHost(overrides = {}) {
  const statuses = [];
  return {
    moodBoard: createEmptyMoodBoard(),
    moodBoardPast: [],
    moodBoardFuture: [],
    moodBoardSaveState: "idle",
    moodBoardSaveStatus: "",
    moodBoardSaveTimer: null,
    moodBoardSaveInFlight: null,
    moodBoardSaveRequested: false,
    moodBoardLoadedForRoot: "/project",
    moodBoardLoadedForSessionId: "session-a",
    moodBoardLoadSerial: 0,
    moodBoardMutationRevision: 0,
    moodBoardDocumentRevision: 0,
    moodBoardSavedDocumentRevision: 0,
    currentProjectPath: "/project",
    kernelProjectSessionId: "session-a",
    transitionLease: false,
    statuses,
    isProjectTransitionFrontendLeaseActive() {
      return this.transitionLease;
    },
    scssVariables: [],
    scssVariableEdits: {},
    replaceScssVariableEdits() {},
    scheduleSessionAutosave() {},
    scheduleSessionHistorySnapshot() {},
    recordSessionChange() {},
    setGlobalStatus(text, kind) {
      statuses.push({ text, kind });
    },
    ...overrides,
  };
}

test("same-root read receipt from the previous runtime session is ignored", async () => {
  const readA = deferred();
  const readB = deferred();
  const calls = [];
  mockIPC((command, payload) => {
    assert.equal(command, "read_mood_board");
    calls.push(payload.input);
    if (payload.input.expectedSessionId === "session-a") return readA.promise;
    if (payload.input.expectedSessionId === "session-b") return readB.promise;
    throw new Error(`Sesiune neașteptată: ${payload.input.expectedSessionId}`);
  });

  const host = controllerHost({
    moodBoard: noteBoard("local-before-load"),
    moodBoardLoadedForRoot: null,
    moodBoardLoadedForSessionId: null,
  });
  const loadingA = loadMoodBoard(host);
  await waitUntil(() => calls.length === 1, "read A nu a pornit");

  host.kernelProjectSessionId = "session-b";
  const loadingB = loadMoodBoard(host);
  await waitUntil(() => calls.length === 2, "read B nu a pornit");

  readA.resolve({
    projectRoot: "/project",
    sessionId: "session-a",
    board: noteBoard("disk-a"),
  });
  await loadingA;
  assert.equal(host.moodBoard.items[0].text, "local-before-load");

  readB.resolve({
    projectRoot: "/project",
    sessionId: "session-b",
    board: noteBoard("disk-b"),
  });
  await loadingB;
  assert.equal(host.moodBoard.items[0].text, "disk-b");
  assert.equal(host.moodBoardLoadedForRoot, "/project");
  assert.equal(host.moodBoardLoadedForSessionId, "session-b");
});

test("persisted Mood Board parsing rejects incompatible envelopes and unknown nested items", () => {
  assert.throws(
    () => parsePersistedMoodBoard({}),
    /versiunea 2/,
  );
  assert.throws(
    () => parsePersistedMoodBoard({
      ...createEmptyMoodBoard(),
      version: 3,
    }),
    /versiunea 2/,
  );
  assert.throws(
    () => parsePersistedMoodBoard({
      ...createEmptyMoodBoard(),
      items: [{
        id: "unknown-item",
        type: "future-widget",
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      }],
    }),
    /nu este cunoscut/,
  );
  assert.equal(parsePersistedMoodBoard(noteBoard("valid-v2")).items[0].text, "valid-v2");
});

test("failed Mood Board load preserves memory, stays locked and cannot issue a save", async () => {
  const commands = [];
  mockIPC((command) => {
    commands.push(command);
    assert.equal(command, "read_mood_board");
    return {
      projectRoot: "/project",
      sessionId: "session-a",
      board: {
        ...createEmptyMoodBoard(),
        items: [{
          id: "unknown-item",
          type: "future-widget",
          x: 0,
          y: 0,
          width: 100,
          height: 100,
        }],
      },
    };
  });
  const preserved = noteBoard("must-survive-load-error");
  const preservedHistory = [noteBoard("history-must-survive")];
  const host = controllerHost({
    moodBoard: preserved,
    moodBoardPast: preservedHistory,
    moodBoardLoadedForRoot: null,
    moodBoardLoadedForSessionId: null,
    moodBoardMutationRevision: 7,
    moodBoardDocumentRevision: 5,
    moodBoardSavedDocumentRevision: 5,
  });

  await loadMoodBoard(host);
  assert.equal(host.moodBoard.items[0].text, "must-survive-load-error");
  assert.equal(host.moodBoardPast[0].items[0].text, "history-must-survive");
  assert.equal(host.moodBoardMutationRevision, 7);
  assert.equal(host.moodBoardDocumentRevision, 5);
  assert.equal(host.moodBoardLoadedForRoot, null);
  assert.equal(host.moodBoardLoadedForSessionId, null);
  assert.equal(host.moodBoardSaveState, "error");
  assert.match(host.moodBoardSaveStatus, /nu este cunoscut/);

  commitMoodBoard(host, noteBoard("must-not-commit"));
  assert.equal(host.moodBoard.items[0].text, "must-survive-load-error");
  assert.equal(host.moodBoardSaveTimer, null);
  host.moodBoardSaveRequested = true;
  assert.equal(await saveMoodBoardNow(host), false);
  assert.deepEqual(commands, ["read_mood_board"]);
  assert.match(host.moodBoardSaveStatus, /nu a fost încărcat valid/);
});

test("a load whose local projection revision changed never publishes the loaded identity", async () => {
  const read = deferred();
  mockIPC((command) => {
    assert.equal(command, "read_mood_board");
    return read.promise;
  });
  const host = controllerHost({
    moodBoard: noteBoard("local-projection"),
    moodBoardLoadedForRoot: null,
    moodBoardLoadedForSessionId: null,
  });
  const loading = loadMoodBoard(host);
  host.moodBoardMutationRevision += 1;
  read.resolve({
    projectRoot: "/project",
    sessionId: "session-a",
    board: noteBoard("disk-projection"),
  });
  await loading;

  assert.equal(host.moodBoard.items[0].text, "local-projection");
  assert.equal(host.moodBoardLoadedForRoot, null);
  assert.equal(host.moodBoardLoadedForSessionId, null);
  assert.equal(host.moodBoardSaveState, "error");
  assert.match(host.moodBoardSaveStatus, /proiecția locală s-a modificat/);
});

test("edit made while save is in flight survives and is persisted by a second single-flight request", async () => {
  const firstSave = deferred();
  const secondSave = deferred();
  const calls = [];
  mockIPC((command, payload) => {
    assert.equal(command, "save_mood_board");
    calls.push(payload.input);
    return calls.length === 1 ? firstSave.promise : secondSave.promise;
  });

  const host = controllerHost();
  commitMoodBoard(host, noteBoard("first"));
  const saving = saveMoodBoardNow(host);
  await waitUntil(() => calls.length === 1, "primul save nu a pornit");

  commitMoodBoard(host, noteBoard("second"));
  firstSave.resolve({
    projectRoot: "/project",
    sessionId: "session-a",
    board: noteBoard("first"),
  });
  await waitUntil(() => calls.length === 2, "editarea concurentă nu a produs al doilea save");
  assert.equal(host.moodBoard.items[0].text, "second");

  secondSave.resolve({
    projectRoot: "/project",
    sessionId: "session-a",
    board: noteBoard("second"),
  });
  assert.equal(await saving, true);
  assert.equal(calls[0].board.items[0].text, "first");
  assert.equal(calls[1].board.items[0].text, "second");
  assert.equal(host.moodBoard.items[0].text, "second");
  assert.equal(host.moodBoardSavedDocumentRevision, host.moodBoardDocumentRevision);
  assert.equal(host.moodBoardSaveInFlight, null);
  if (host.moodBoardSaveTimer !== null) window.clearTimeout(host.moodBoardSaveTimer);
});

test("a malformed save receipt cannot replace the live Mood Board with sanitized empty state", async () => {
  mockIPC((command) => {
    assert.equal(command, "save_mood_board");
    return {
      projectRoot: "/project",
      sessionId: "session-a",
      board: { version: 2, updatedAt: 1, viewport: { x: 0, y: 0, zoom: 1 } },
    };
  });
  const host = controllerHost();
  commitMoodBoard(host, noteBoard("live-must-survive-malformed-save-receipt"));

  assert.equal(await saveMoodBoardNow(host), false);
  assert.equal(host.moodBoard.items[0].text, "live-must-survive-malformed-save-receipt");
  assert.equal(host.moodBoardSaveState, "error");
  assert.equal(host.moodBoardSaveRequested, true);
  assert.match(host.moodBoardSaveStatus, /document\.items trebuie să fie listă/);
});

test("Mood Board drain surfaces save failure instead of allowing a transition barrier to pass", async () => {
  const calls = [];
  mockIPC((command, payload) => {
    assert.equal(command, "save_mood_board");
    calls.push(payload.input);
    throw new Error("mood save failed");
  });
  const host = controllerHost();
  commitMoodBoard(host, noteBoard("dirty-before-transition"));

  await assert.rejects(
    drainMoodBoardSaveBeforeTransition(host),
    /Nu am putut salva mood board-ul: mood save failed/,
  );
  assert.equal(calls.length, 1);
  assert.equal(host.moodBoardSaveState, "error");
  assert.equal(host.moodBoardSaveRequested, true);
  assert.ok(host.moodBoardSavedDocumentRevision < host.moodBoardDocumentRevision);
});

test("Mood Board mutations are rejected until the exact root and runtime session finished loading", () => {
  const host = controllerHost({
    moodBoardLoadedForRoot: null,
    moodBoardLoadedForSessionId: null,
  });
  const before = JSON.stringify(host.moodBoard);
  commitMoodBoard(host, noteBoard("must-not-enter"));
  assert.equal(JSON.stringify(host.moodBoard), before);
  assert.equal(host.moodBoardDocumentRevision, 0);
  assert.match(host.statuses.at(-1).text, /până când documentul sesiunii curente este încărcat/);

  host.moodBoardLoadedForRoot = "/project";
  host.moodBoardLoadedForSessionId = "session-a";
  commitMoodBoard(host, noteBoard("accepted"));
  assert.equal(host.moodBoard.items[0].text, "accepted");
  assert.equal(host.moodBoardDocumentRevision, 1);
  if (host.moodBoardSaveTimer !== null) window.clearTimeout(host.moodBoardSaveTimer);
});

test("native close remains pending-safe when the project pre-close drain rejects", async () => {
  const statuses = [];
  const app = {
    nativeWindowClosePending: false,
    nativeWindowCloseInProgress: false,
    scannedProject: { root: "/project" },
    projectTransitionDecisionRequest: null,
    async closeCurrentProject() {
      throw new Error("mood drain failed");
    },
    setGlobalStatus(text, kind) {
      statuses.push({ text, kind });
    },
  };

  await handleNativeWindowCloseRequest(app);
  assert.deepEqual(app.scannedProject, { root: "/project" });
  assert.equal(app.nativeWindowClosePending, false);
  assert.equal(app.nativeWindowCloseInProgress, false);
  assert.match(statuses.at(-1).text, /mood drain failed/);
  assert.equal(statuses.at(-1).kind, "error");
});

test("native close routes a detached Rust session directly to close policy", async () => {
  const closeRoots = [];
  const app = {
    nativeWindowClosePending: false,
    nativeWindowCloseInProgress: false,
    scannedProject: null,
    projectTransitionDecisionRequest: null,
    async closeCurrentProject(projectRoot) {
      closeRoots.push(projectRoot);
      this.projectTransitionDecisionRequest = {
        continuation: { kind: "close_project" },
      };
    },
    setGlobalStatus() {},
  };

  await handleNativeWindowCloseRequest(app, "/project-a");

  assert.deepEqual(closeRoots, ["/project-a"]);
  assert.equal(app.scannedProject, null);
  assert.equal(app.nativeWindowClosePending, true);
  assert.equal(app.nativeWindowCloseInProgress, false);
  assert.equal(app.projectTransitionDecisionRequest.continuation.kind, "close_project");
});

test("detached close evaluates kernel policy without rebuilding the frontend project", async () => {
  const calls = [];
  mockIPC((command, payload) => {
    calls.push({ command, payload });
    assert.equal(command, "read_kernel_project_transition_policy");
    assert.equal(payload.action, "close_project");
    return {
      decision: "confirm",
      reason: "undo_redo_dirty",
      title: "Confirmă închiderea",
      message: "Istoricul sesiunii este departe de saved point.",
      recommendedAction: "Confirmă sau anulează.",
      sessionId: "runtime-session-a",
    };
  });
  const statuses = [];
  const notifications = [];
  const host = {
    scannedProject: null,
    projectTransitionDecisionRequest: null,
    projectStatus: "",
    async beginProjectTransitionFrontendLease() {},
    endProjectTransitionFrontendLease() {},
    setGlobalStatus(text, kind) {
      statuses.push({ text, kind });
    },
    notify(notification) {
      notifications.push(notification);
    },
  };

  const closed = await closeCurrentProject(host, { detachedProjectRoot: "/project-a" });

  assert.equal(closed, false);
  assert.equal(calls.length, 1);
  assert.equal(host.scannedProject, null);
  assert.equal(host.projectTransitionDecisionRequest.targetRoot, "/project-a");
  assert.equal(host.projectTransitionDecisionRequest.action, "close_project");
  assert.equal(host.projectTransitionDecisionRequest.continuation.kind, "close_project");
  assert.equal(statuses.at(-1).kind, "idle");
  assert.equal(notifications.at(-1).level, "warning");
});

test("Mood image preview cache isolates the same relative path by project root and runtime session", async () => {
  const identityA = { expectedProjectRoot: "/project-a", expectedSessionId: "session-a-cache" };
  const identityB = { expectedProjectRoot: "/project-b", expectedSessionId: "session-b-cache" };
  const active = { current: identityA };
  const isCurrent = sessionGuard(active);
  const calls = [];
  mockIPC((command, payload) => {
    assert.equal(command, "read_mood_board_image_data_url");
    calls.push(payload.input);
    const identity = payload.input.expectedSessionId === identityA.expectedSessionId ? identityA : identityB;
    return {
      projectRoot: identity.expectedProjectRoot,
      sessionId: identity.expectedSessionId,
      relativePath: payload.input.relativePath,
      dataUrl: `data:image/png;base64,${identity.expectedSessionId}`,
    };
  });

  const srcA = await resolveMoodBoardImageSrc(identityA, isCurrent, "static/shared-cache.png");
  active.current = identityB;
  const srcB = await resolveMoodBoardImageSrc(identityB, isCurrent, "static/shared-cache.png");

  assert.notEqual(srcA, srcB);
  assert.equal(calls.length, 2);
  assert.deepEqual(calls.map((input) => input.expectedProjectRoot), ["/project-a", "/project-b"]);
});

test("deferred Mood preview from A is rejected after an A to B transition", async () => {
  const identityA = { expectedProjectRoot: "/project-a", expectedSessionId: "session-a-deferred" };
  const identityB = { expectedProjectRoot: "/project-b", expectedSessionId: "session-b-deferred" };
  const active = { current: identityA };
  const isCurrent = sessionGuard(active);
  const readA = deferred();
  mockIPC((command, payload) => {
    assert.equal(command, "read_mood_board_image_data_url");
    assert.deepEqual(payload.input, { ...identityA, relativePath: "static/deferred-a.png" });
    return readA.promise;
  });

  let projectedSrc = "";
  const operation = resolveMoodBoardImageSrc(identityA, isCurrent, "static/deferred-a.png")
    .then((src) => {
      projectedSrc = src;
    });
  active.current = identityB;
  readA.resolve({
    projectRoot: identityA.expectedProjectRoot,
    sessionId: identityA.expectedSessionId,
    relativePath: "static/deferred-a.png",
    dataUrl: "data:image/png;base64,AAAA",
  });

  await assert.rejects(operation, MoodBoardStaleSessionError);
  assert.equal(projectedSrc, "");
});

test("queued Mood preview from a stale session never starts its IPC in the next project", async () => {
  const identityA = { expectedProjectRoot: "/project-a", expectedSessionId: "session-a-queue" };
  const identityB = { expectedProjectRoot: "/project-b", expectedSessionId: "session-b-queue" };
  const active = { current: identityA };
  const isCurrent = sessionGuard(active);
  const firstRead = deferred();
  const calls = [];
  mockIPC((command, payload) => {
    assert.equal(command, "read_mood_board_image_data_url");
    calls.push(payload.input.relativePath);
    if (payload.input.relativePath === "static/queue-first.png") return firstRead.promise;
    throw new Error("Cererea queued stale nu trebuia să ajungă la IPC.");
  });

  const first = resolveMoodBoardImageSrc(identityA, isCurrent, "static/queue-first.png");
  const queued = resolveMoodBoardImageSrc(identityA, isCurrent, "static/queue-stale.png");
  assert.deepEqual(calls, ["static/queue-first.png"]);

  active.current = identityB;
  firstRead.resolve({
    projectRoot: identityA.expectedProjectRoot,
    sessionId: identityA.expectedSessionId,
    relativePath: "static/queue-first.png",
    dataUrl: "data:image/png;base64,FIRST",
  });

  const [firstResult, queuedResult] = await Promise.allSettled([first, queued]);
  assert.equal(firstResult.status, "rejected");
  assert.equal(queuedResult.status, "rejected");
  assert.ok(firstResult.reason instanceof MoodBoardStaleSessionError);
  assert.ok(queuedResult.reason instanceof MoodBoardStaleSessionError);
  assert.deepEqual(calls, ["static/queue-first.png"]);
});

test("stale editable SVG result has zero mutation after same-root session reopen", async () => {
  const identityA = { expectedProjectRoot: "/same-project", expectedSessionId: "session-a-svg" };
  const identityB = { expectedProjectRoot: "/same-project", expectedSessionId: "session-b-svg" };
  const active = { current: identityA };
  const isCurrent = sessionGuard(active);
  const readSvg = deferred();
  mockIPC((command, payload) => {
    assert.equal(command, "read_mood_board_svg_source");
    assert.deepEqual(payload.input, { ...identityA, relativePath: "static/deferred.svg" });
    return readSvg.promise;
  });

  const board = noteBoard("before-svg");
  let projectedBoard = board;
  const operation = addMoodBoardVisualAssetAtPath(
    "static/deferred.svg",
    { x: 100, y: 120 },
    identityA,
    isCurrent,
  ).then((result) => {
    if (result.board) projectedBoard = result.board;
  });

  active.current = identityB;
  readSvg.resolve({
    projectRoot: identityA.expectedProjectRoot,
    sessionId: identityA.expectedSessionId,
    relativePath: "static/deferred.svg",
    source: '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><path d="M0 0 L20 20"/></svg>',
  });

  await assert.rejects(operation, MoodBoardStaleSessionError);
  assert.equal(projectedBoard, board);
  assert.equal(projectedBoard.items.length, 1);
});

test("mutating SVG export started in A cannot publish status after switching to B", async () => {
  const identityA = { expectedProjectRoot: "/project-a", expectedSessionId: "session-a-export" };
  const identityB = { expectedProjectRoot: "/project-b", expectedSessionId: "session-b-export" };
  const active = { current: identityA };
  const isCurrent = sessionGuard(active);
  const exportA = deferred();
  let input = null;
  mockIPC((command, payload) => {
    assert.equal(command, "export_mood_board_svg_asset");
    input = payload.input;
    return exportA.promise;
  });

  const vector = createMoodBoardVectorPath({ x: 20, y: 30 });
  const board = { ...createEmptyMoodBoard(), items: [vector] };
  const boardBefore = JSON.stringify(board);
  const statuses = [];
  const operation = exportMoodBoardVectorPathWorkflow(
    board,
    vector.id,
    () => "export-session-test",
    (text, kind) => statuses.push({ text, kind }),
    identityA,
    isCurrent,
  );
  await waitUntil(() => input !== null, "exportul SVG din A nu a pornit");
  assert.equal(input.expectedProjectRoot, identityA.expectedProjectRoot);
  assert.equal(input.expectedSessionId, identityA.expectedSessionId);
  assert.equal(input.relativePath, "resurse/imagini/export-session-test.svg");
  assert.match(input.svg, /<svg/);

  active.current = identityB;
  exportA.resolve({
    projectRoot: identityA.expectedProjectRoot,
    sessionId: identityA.expectedSessionId,
    relativePath: input.relativePath,
  });
  await operation;

  assert.deepEqual(statuses, []);
  assert.equal(JSON.stringify(board), boardBefore);
});

test("mutating Mood asset rejects a receipt with the wrong root, session or path", async () => {
  const identity = { expectedProjectRoot: "/project", expectedSessionId: "session-receipt" };
  const active = { current: identity };
  const isCurrent = sessionGuard(active);
  mockIPC((command) => {
    assert.equal(command, "export_mood_board_svg_asset");
    return {
      projectRoot: identity.expectedProjectRoot,
      sessionId: identity.expectedSessionId,
      relativePath: "resurse/imagini/wrong.svg",
    };
  });

  await assert.rejects(
    exportMoodBoardSvgAsset(
      identity,
      isCurrent,
      "resurse/imagini/expected.svg",
      '<svg xmlns="http://www.w3.org/2000/svg"/>',
    ),
    /alt asset decât cel solicitat/,
  );
});

test("deferred SVG intent merges over the live same-session board instead of replacing concurrent edits", async () => {
  const identity = { expectedProjectRoot: "/project", expectedSessionId: "session-svg-merge" };
  const active = { current: identity };
  const isCurrent = sessionGuard(active);
  const readSvg = deferred();
  mockIPC((command) => {
    assert.equal(command, "read_mood_board_svg_source");
    return readSvg.promise;
  });

  let liveBoard = noteBoard("before-await");
  const operation = addMoodBoardVisualAssetAtPath(
    "static/concurrent.svg",
    { x: 50, y: 60 },
    identity,
    isCurrent,
  );
  liveBoard = {
    ...liveBoard,
    items: [...liveBoard.items, noteBoard("concurrent-edit").items[0]],
  };
  readSvg.resolve({
    projectRoot: identity.expectedProjectRoot,
    sessionId: identity.expectedSessionId,
    relativePath: "static/concurrent.svg",
    source: '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><path d="M0 0 L20 20"/></svg>',
  });

  const intent = await operation;
  assert.ok(intent.item);
  const applied = applyMoodBoardVisualAssetItem(liveBoard, intent.item);
  assert.equal(applied.board.items.length, 3);
  assert.equal(applied.board.items[1].text, "concurrent-edit");
});

test("deferred palette merges over live edits and refuses a deleted or changed source", async () => {
  const identity = { expectedProjectRoot: "/project", expectedSessionId: "session-palette-merge" };
  const active = { current: identity };
  const isCurrent = sessionGuard(active);
  const palette = deferred();
  mockIPC((command, payload) => {
    assert.equal(command, "extract_mood_board_image_palette");
    assert.equal(payload.input.relativePath, "static/palette.png");
    return palette.promise;
  });

  const image = createMoodBoardImageItem("static/palette.png", { x: 10, y: 20 });
  const initial = { ...createEmptyMoodBoard(), items: [image] };
  const operation = extractMoodBoardPaletteItems(
    initial,
    image.id,
    image.path,
    identity,
    isCurrent,
  );
  const liveImage = { ...image, x: 300, y: 400 };
  const concurrentNote = noteBoard("palette-concurrent-edit").items[0];
  const liveBoard = { ...initial, items: [liveImage, concurrentNote] };
  palette.resolve({
    projectRoot: identity.expectedProjectRoot,
    sessionId: identity.expectedSessionId,
    relativePath: image.path,
    colors: ["#112233", "#445566"],
  });

  const intent = await operation;
  assert.deepEqual(intent.colors, ["#112233", "#445566"]);
  const applied = applyMoodBoardPaletteColors(liveBoard, image.id, image.path, intent.colors);
  assert.ok(applied);
  assert.equal(applied.board.items.length, 4);
  assert.equal(applied.board.items[1].text, "palette-concurrent-edit");
  assert.equal(applied.board.items[2].x, 300);

  const deletedSourceBoard = { ...liveBoard, items: [concurrentNote] };
  assert.equal(
    applyMoodBoardPaletteColors(deletedSourceBoard, image.id, image.path, intent.colors),
    null,
  );
  const changedSourceBoard = { ...liveBoard, items: [{ ...liveImage, path: "static/replaced.png" }, concurrentNote] };
  assert.equal(
    applyMoodBoardPaletteColors(changedSourceBoard, image.id, image.path, intent.colors),
    null,
  );
});
