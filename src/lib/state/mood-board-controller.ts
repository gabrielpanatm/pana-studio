import { readMoodBoard, saveMoodBoard } from "$lib/mood-board/io";
import {
  cloneMoodBoard,
  createEmptyMoodBoard,
  sameMoodBoard,
  type MoodBoard,
  type MoodBoardSaveState,
} from "$lib/mood-board/model";
import type { ScssVariable, SaveState } from "$lib/types";
import { errorMessage } from "$lib/util";
import { createCssRequestIdentity, setScssVariable } from "$lib/project/io";

const MAX_MOOD_HISTORY = 80;
const MOOD_SAVE_DELAY = 450;

export type MoodBoardControllerHost = {
  moodBoard: MoodBoard;
  moodBoardPast: MoodBoard[];
  moodBoardFuture: MoodBoard[];
  moodBoardSaveState: MoodBoardSaveState;
  moodBoardSaveStatus: string;
  moodBoardSaveTimer: number | null;
  moodBoardSaveInFlight: Promise<boolean> | null;
  moodBoardSaveRequested: boolean;
  moodBoardLoadedForRoot: string | null;
  moodBoardLoadedForSessionId: string | null;
  moodBoardLoadSerial: number;
  moodBoardMutationRevision: number;
  moodBoardDocumentRevision: number;
  moodBoardSavedDocumentRevision: number;
  currentProjectPath: string;
  kernelProjectSessionId: string;
  isProjectTransitionFrontendLeaseActive: () => boolean;
  scssVariables: ScssVariable[];
  setGlobalStatus: (text: string, kind: SaveState) => void;
};

function withUpdatedAt(board: MoodBoard): MoodBoard {
  return { ...cloneMoodBoard(board), updatedAt: Date.now() };
}

function sameMoodBoardDocument(left: MoodBoard, right: MoodBoard) {
  return JSON.stringify(left.items) === JSON.stringify(right.items);
}

function withViewport(board: MoodBoard, viewport: MoodBoard["viewport"]): MoodBoard {
  return {
    ...cloneMoodBoard(board),
    viewport: { ...viewport },
  };
}

type MoodBoardIdentity = {
  expectedProjectRoot: string;
  expectedSessionId: string;
};

function currentMoodBoardIdentity(host: MoodBoardControllerHost): MoodBoardIdentity | null {
  if (!host.currentProjectPath || !host.kernelProjectSessionId) return null;
  return {
    expectedProjectRoot: host.currentProjectPath,
    expectedSessionId: host.kernelProjectSessionId,
  };
}

function isCurrentMoodBoardIdentity(host: MoodBoardControllerHost, identity: MoodBoardIdentity) {
  return host.currentProjectPath === identity.expectedProjectRoot
    && host.kernelProjectSessionId === identity.expectedSessionId;
}

function isLoadedMoodBoardIdentity(host: MoodBoardControllerHost, identity: MoodBoardIdentity) {
  return host.moodBoardLoadedForRoot === identity.expectedProjectRoot
    && host.moodBoardLoadedForSessionId === identity.expectedSessionId;
}

function currentLoadedMoodBoardIdentity(host: MoodBoardControllerHost): MoodBoardIdentity | null {
  const identity = currentMoodBoardIdentity(host);
  return identity && isLoadedMoodBoardIdentity(host, identity) ? identity : null;
}

function receiptMatchesMoodBoardIdentity(
  receipt: { projectRoot: string; sessionId: string },
  identity: MoodBoardIdentity,
) {
  return receipt.projectRoot === identity.expectedProjectRoot
    && receipt.sessionId === identity.expectedSessionId;
}

function allowMoodBoardMutation(host: MoodBoardControllerHost) {
  if (host.isProjectTransitionFrontendLeaseActive()) {
    host.setGlobalStatus(
      "Editarea planșei vizuale este temporar blocată: tranziția proiectului a rezervat sesiunea curentă.",
      "error",
    );
    return false;
  }
  const identity = currentMoodBoardIdentity(host);
  if (identity && isLoadedMoodBoardIdentity(host, identity)) return true;
  host.setGlobalStatus(
    "Editarea Mood Board este blocată până când documentul sesiunii curente este încărcat.",
    "error",
  );
  return false;
}

function recordMoodBoardMutation(host: MoodBoardControllerHost, documentChanged: boolean) {
  host.moodBoardMutationRevision += 1;
  if (documentChanged) host.moodBoardDocumentRevision += 1;
}

function failMoodBoardLoad(host: MoodBoardControllerHost, message: string) {
  host.moodBoardLoadedForRoot = null;
  host.moodBoardLoadedForSessionId = null;
  host.moodBoardSaveState = "error";
  host.moodBoardSaveStatus = message;
  host.setGlobalStatus(message, "error");
}

export async function loadMoodBoard(host: MoodBoardControllerHost) {
  const identity = currentMoodBoardIdentity(host);
  if (!identity) {
    host.moodBoard = createEmptyMoodBoard();
    host.moodBoardLoadedForRoot = null;
    host.moodBoardLoadedForSessionId = null;
    return;
  }
  if (
    host.moodBoardLoadedForRoot === identity.expectedProjectRoot
    && host.moodBoardLoadedForSessionId === identity.expectedSessionId
  ) return;

  const requestSerial = host.moodBoardLoadSerial + 1;
  const mutationRevision = host.moodBoardMutationRevision;
  host.moodBoardLoadSerial = requestSerial;
  host.moodBoardLoadedForRoot = null;
  host.moodBoardLoadedForSessionId = null;
  host.moodBoardSaveState = "idle";
  host.moodBoardSaveStatus = "Se încarcă mood board-ul...";
  try {
    const receipt = await readMoodBoard(identity);
    if (
      host.moodBoardLoadSerial !== requestSerial
      || !isCurrentMoodBoardIdentity(host, identity)
    ) return;
    if (!receiptMatchesMoodBoardIdentity(receipt, identity)) {
      failMoodBoardLoad(
        host,
        "Mood Board a refuzat un răspuns de încărcare din altă sesiune.",
      );
      return;
    }
    if (host.moodBoardMutationRevision !== mutationRevision) {
      failMoodBoardLoad(
        host,
        "Încărcarea Mood Board a fost blocată deoarece proiecția locală s-a modificat între timp.",
      );
      return;
    }
    host.moodBoard = receipt.board;
    host.moodBoardPast = [];
    host.moodBoardFuture = [];
    recordMoodBoardMutation(host, true);
    host.moodBoardSavedDocumentRevision = host.moodBoardDocumentRevision;
    // Publish the loaded identity only after the validated document and its
    // clean revision have been installed. This flag unlocks every mutation.
    host.moodBoardLoadedForRoot = identity.expectedProjectRoot;
    host.moodBoardLoadedForSessionId = identity.expectedSessionId;
    host.moodBoardSaveState = "saved";
    host.moodBoardSaveStatus = "Mood board încărcat.";
  } catch (error) {
    if (
      host.moodBoardLoadSerial !== requestSerial
      || !isCurrentMoodBoardIdentity(host, identity)
    ) return;
    // Preserve the last in-memory projection and all history. It remains
    // unmounted and immutable because no loaded identity is published.
    failMoodBoardLoad(
      host,
      `Nu am putut încărca mood board-ul: ${errorMessage(error)}`,
    );
  }
}

export function resetMoodBoard(host: MoodBoardControllerHost) {
  if (host.moodBoardSaveTimer !== null) {
    window.clearTimeout(host.moodBoardSaveTimer);
    host.moodBoardSaveTimer = null;
  }
  host.moodBoardLoadSerial += 1;
  host.moodBoard = createEmptyMoodBoard();
  host.moodBoardPast = [];
  host.moodBoardFuture = [];
  host.moodBoardLoadedForRoot = null;
  host.moodBoardLoadedForSessionId = null;
  host.moodBoardSaveRequested = false;
  recordMoodBoardMutation(host, true);
  host.moodBoardSavedDocumentRevision = host.moodBoardDocumentRevision;
  host.moodBoardSaveState = "idle";
  host.moodBoardSaveStatus = "Mood board nesalvat încă.";
}

export function commitMoodBoard(host: MoodBoardControllerHost, nextBoard: MoodBoard) {
  if (!allowMoodBoardMutation(host)) return;
  const current = cloneMoodBoard(host.moodBoard);
  const next = withUpdatedAt(nextBoard);
  if (sameMoodBoard(current, next)) return;
  if (sameMoodBoardDocument(current, next)) {
    host.moodBoard = next;
    recordMoodBoardMutation(host, false);
    return;
  }
  host.moodBoardPast = [...host.moodBoardPast, current].slice(-MAX_MOOD_HISTORY);
  host.moodBoardFuture = [];
  host.moodBoard = next;
  recordMoodBoardMutation(host, true);
  scheduleMoodBoardSave(host);
}

export function setMoodBoardTransient(host: MoodBoardControllerHost, nextBoard: MoodBoard) {
  if (!allowMoodBoardMutation(host)) return;
  host.moodBoard = { ...nextBoard, updatedAt: Date.now() };
  recordMoodBoardMutation(host, false);
}

export function undoMoodBoard(host: MoodBoardControllerHost) {
  if (!allowMoodBoardMutation(host)) return;
  const current = cloneMoodBoard(host.moodBoard);
  const viewport = { ...current.viewport };
  const past = [...host.moodBoardPast];
  let previous = past.pop();
  while (previous && sameMoodBoardDocument(current, previous)) {
    previous = past.pop();
  }
  if (!previous) {
    host.moodBoardPast = past;
    return;
  }
  host.moodBoardPast = past;
  host.moodBoardFuture = [withViewport(current, viewport), ...host.moodBoardFuture].slice(0, MAX_MOOD_HISTORY);
  host.moodBoard = withUpdatedAt(withViewport(previous, viewport));
  recordMoodBoardMutation(host, true);
  scheduleMoodBoardSave(host);
}

export function redoMoodBoard(host: MoodBoardControllerHost) {
  if (!allowMoodBoardMutation(host)) return;
  const current = cloneMoodBoard(host.moodBoard);
  const viewport = { ...current.viewport };
  const future = [...host.moodBoardFuture];
  let next = future.shift();
  while (next && sameMoodBoardDocument(current, next)) {
    next = future.shift();
  }
  if (!next) {
    host.moodBoardFuture = future;
    return;
  }
  host.moodBoardFuture = future;
  host.moodBoardPast = [...host.moodBoardPast, withViewport(current, viewport)].slice(-MAX_MOOD_HISTORY);
  host.moodBoard = withUpdatedAt(withViewport(next, viewport));
  recordMoodBoardMutation(host, true);
  scheduleMoodBoardSave(host);
}

export function scheduleMoodBoardSave(host: MoodBoardControllerHost) {
  if (!currentMoodBoardIdentity(host) || !allowMoodBoardMutation(host)) return;
  if (host.moodBoardSaveTimer !== null) {
    window.clearTimeout(host.moodBoardSaveTimer);
  }
  host.moodBoardSaveRequested = true;
  host.moodBoardSaveState = "saving";
  host.moodBoardSaveStatus = "Se salvează mood board-ul...";
  host.moodBoardSaveTimer = window.setTimeout(() => {
    host.moodBoardSaveTimer = null;
    void saveMoodBoardNow(host);
  }, MOOD_SAVE_DELAY);
}

async function runMoodBoardSaveLoop(host: MoodBoardControllerHost): Promise<boolean> {
  while (
    host.moodBoardSaveRequested
    || host.moodBoardSavedDocumentRevision < host.moodBoardDocumentRevision
  ) {
    const identity = currentLoadedMoodBoardIdentity(host);
    if (!identity) return false;
    host.moodBoardSaveRequested = false;
    const documentRevision = host.moodBoardDocumentRevision;
    const mutationRevision = host.moodBoardMutationRevision;
    const board = withUpdatedAt(host.moodBoard);
    host.moodBoardSaveState = "saving";
    host.moodBoardSaveStatus = "Se salvează mood board-ul...";
    try {
      const receipt = await saveMoodBoard(board, identity);
      if (!isCurrentMoodBoardIdentity(host, identity)) return false;
      if (!receiptMatchesMoodBoardIdentity(receipt, identity)) {
        host.moodBoardSaveRequested = true;
        host.moodBoardSaveState = "error";
        host.moodBoardSaveStatus = "Mood Board a refuzat un răspuns de salvare din altă sesiune.";
        host.setGlobalStatus(host.moodBoardSaveStatus, "error");
        return false;
      }
      if (host.moodBoardMutationRevision === mutationRevision) {
        host.moodBoard = receipt.board;
      }
      if (host.moodBoardDocumentRevision === documentRevision) {
        host.moodBoardSavedDocumentRevision = documentRevision;
      } else {
        host.moodBoardSaveRequested = true;
      }
    } catch (error) {
      if (isCurrentMoodBoardIdentity(host, identity)) {
        host.moodBoardSaveRequested = true;
        host.moodBoardSaveState = "error";
        host.moodBoardSaveStatus = `Nu am putut salva mood board-ul: ${errorMessage(error)}`;
        host.setGlobalStatus(host.moodBoardSaveStatus, "error");
      }
      return false;
    }
  }
  host.moodBoardSaveState = "saved";
  host.moodBoardSaveStatus = "Mood board salvat.";
  return true;
}

export async function saveMoodBoardNow(host: MoodBoardControllerHost): Promise<boolean> {
  if (host.moodBoardSaveTimer !== null) {
    window.clearTimeout(host.moodBoardSaveTimer);
    host.moodBoardSaveTimer = null;
  }
  const identity = currentMoodBoardIdentity(host);
  if (!identity) return false;
  if (!isLoadedMoodBoardIdentity(host, identity)) {
    host.moodBoardSaveState = "error";
    host.moodBoardSaveStatus = "Salvarea Mood Board este blocată: documentul sesiunii curente nu a fost încărcat valid.";
    host.setGlobalStatus(host.moodBoardSaveStatus, "error");
    return false;
  }
  if (host.moodBoardSavedDocumentRevision < host.moodBoardDocumentRevision) {
    host.moodBoardSaveRequested = true;
  }
  if (host.moodBoardSaveInFlight) return await host.moodBoardSaveInFlight;
  if (!host.moodBoardSaveRequested) return true;

  const operation = runMoodBoardSaveLoop(host);
  host.moodBoardSaveInFlight = operation;
  try {
    return await operation;
  } finally {
    if (host.moodBoardSaveInFlight === operation) host.moodBoardSaveInFlight = null;
  }
}

export async function drainMoodBoardSaveBeforeTransition(host: MoodBoardControllerHost) {
  if (host.moodBoardSaveTimer !== null) {
    window.clearTimeout(host.moodBoardSaveTimer);
    host.moodBoardSaveTimer = null;
    host.moodBoardSaveRequested = true;
  }
  if (host.moodBoardSavedDocumentRevision < host.moodBoardDocumentRevision) {
    host.moodBoardSaveRequested = true;
  }
  if (!host.moodBoardSaveRequested && !host.moodBoardSaveInFlight) return;

  const saved = await saveMoodBoardNow(host);
  if (
    !saved
    || host.moodBoardSaveRequested
    || host.moodBoardSavedDocumentRevision < host.moodBoardDocumentRevision
  ) {
    throw new Error(host.moodBoardSaveStatus || "Mood Board nu a putut fi salvat înainte de tranziție.");
  }
}

export function applyMoodBoardColorToScssVariable(
  host: MoodBoardControllerHost,
  color: string,
  label = "culoare",
  variableName?: string,
) {
  if (!allowMoodBoardMutation(host)) return;
  const colorVariables = host.scssVariables.filter((variable) => {
    const name = variable.name.toLowerCase();
    const value = variable.value.trim().toLowerCase();
    return name.includes("color")
      || name.startsWith("bg-")
      || name.startsWith("text-")
      || name.startsWith("border-")
      || value.startsWith("#")
      || value.startsWith("rgb")
      || value.startsWith("hsl");
  });

  if (colorVariables.length === 0) {
    host.setGlobalStatus("Nu am găsit variabile SCSS de culoare în proiect.", "error");
    return;
  }

  const name = (variableName ?? "").trim().replace(/^\$/, "");
  if (!name) return;

  const variable = host.scssVariables.find((entry) => entry.name === name);
  if (!variable) {
    host.setGlobalStatus(`Variabila $${name} nu există încă. Momentan Mood Board poate seta doar variabile existente.`, "error");
    return;
  }

  const identity = createCssRequestIdentity(
    host.currentProjectPath,
    host.kernelProjectSessionId,
  );
  host.scssVariables = host.scssVariables.map((entry) => (
    entry.file === variable.file && entry.name === variable.name
      ? { ...entry, value: color }
      : entry
  ));
  void setScssVariable(variable.file, variable.name, color, identity)
    .then(() => {
      if (!isCurrentMoodBoardIdentity(host, identity)) return;
      host.setGlobalStatus(
        `Culoarea ${color} este în ProjectWorkspace pentru $${variable.name}. Ctrl+S persistă pe disc.`,
        "unsaved",
      );
    })
    .catch((error) => {
      if (!isCurrentMoodBoardIdentity(host, identity)) return;
      host.setGlobalStatus(
        `Culoarea nu a putut fi aplicată în ProjectWorkspace: ${errorMessage(error)}`,
        "error",
      );
    });
}
