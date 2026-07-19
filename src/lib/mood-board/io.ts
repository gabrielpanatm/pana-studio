import { invoke } from "@tauri-apps/api/core";
import {
  createEmptyMoodBoard,
  parsePersistedMoodBoard,
  type MoodBoard,
} from "$lib/mood-board/model";

const imagePreviewCache = new Map<string, Promise<string>>();
const MAX_IMAGE_PREVIEW_CACHE_ITEMS = 80;
const MAX_IMAGE_PREVIEW_REQUESTS = 1;
const MAX_QUEUED_IMAGE_PREVIEW_REQUESTS = 80;
let activeImagePreviewRequests = 0;
type ImagePreviewQueueEntry = {
  identity: MoodBoardRequestIdentity;
  isCurrent: MoodBoardIdentityGuard;
  run: () => void;
  reject: (reason: unknown) => void;
};
const imagePreviewQueue: ImagePreviewQueueEntry[] = [];

function runNextImagePreviewRequest() {
  if (activeImagePreviewRequests >= MAX_IMAGE_PREVIEW_REQUESTS) return;
  while (imagePreviewQueue.length > 0) {
    const next = imagePreviewQueue.shift();
    if (!next) return;
    if (!next.isCurrent(next.identity)) {
      next.reject(new MoodBoardStaleSessionError());
      continue;
    }
    activeImagePreviewRequests += 1;
    next.run();
    return;
  }
}

function enqueueImagePreviewRequest<T>(
  identity: MoodBoardRequestIdentity,
  isCurrent: MoodBoardIdentityGuard,
  task: () => Promise<T>,
): Promise<T> {
  return new Promise((resolve, reject) => {
    if (imagePreviewQueue.length >= MAX_QUEUED_IMAGE_PREVIEW_REQUESTS) {
      reject(new Error("Coada de preview Mood Board a atins limita de resurse."));
      return;
    }
    imagePreviewQueue.push({
      identity,
      isCurrent,
      reject,
      run: () => {
        task()
          .then(resolve, reject)
          .finally(() => {
            activeImagePreviewRequests = Math.max(0, activeImagePreviewRequests - 1);
            runNextImagePreviewRequest();
          });
      },
    });
    runNextImagePreviewRequest();
  });
}

export type MoodBoardRequestIdentity = {
  expectedProjectRoot: string;
  expectedSessionId: string;
};

export type MoodBoardIdentityGuard = (identity: MoodBoardRequestIdentity) => boolean;

export class MoodBoardStaleSessionError extends Error {
  constructor() {
    super("Operația Mood Board aparține unei sesiuni care nu mai este activă.");
    this.name = "MoodBoardStaleSessionError";
  }
}

export function isMoodBoardStaleSessionError(error: unknown): error is MoodBoardStaleSessionError {
  return error instanceof MoodBoardStaleSessionError;
}

export function requireCurrentMoodBoardIdentity(
  identity: MoodBoardRequestIdentity,
  isCurrent: MoodBoardIdentityGuard,
) {
  if (!isCurrent(identity)) throw new MoodBoardStaleSessionError();
}

export function moodBoardReceiptMatchesIdentity(
  receipt: { projectRoot: string; sessionId: string },
  identity: MoodBoardRequestIdentity,
) {
  return receipt.projectRoot === identity.expectedProjectRoot
    && receipt.sessionId === identity.expectedSessionId;
}

export type MoodBoardAssetReceipt = {
  projectRoot: string;
  sessionId: string;
  relativePath: string;
};

export type MoodBoardImageReceipt = MoodBoardAssetReceipt & {
  dataUrl: string;
};

export type MoodBoardPaletteReceipt = MoodBoardAssetReceipt & {
  colors: string[];
};

export type MoodBoardSvgSourceReceipt = MoodBoardAssetReceipt & {
  source: string;
};

function normalizedMoodBoardAssetPath(relativePath: string) {
  const normalizedPath = relativePath.trim().replaceAll("\\", "/");
  if (!normalizedPath) throw new Error("Path asset Mood Board lipsă.");
  return normalizedPath;
}

function requireMoodBoardAssetReceipt<T extends MoodBoardAssetReceipt>(
  receipt: T,
  identity: MoodBoardRequestIdentity,
  expectedRelativePath?: string,
) {
  if (!moodBoardReceiptMatchesIdentity(receipt, identity)) {
    throw new Error("Mood Board a refuzat un receipt de asset din altă sesiune.");
  }
  const relativePath = normalizedMoodBoardAssetPath(receipt.relativePath);
  if (expectedRelativePath !== undefined && relativePath !== expectedRelativePath) {
    throw new Error("Mood Board a refuzat un receipt pentru alt asset decât cel solicitat.");
  }
  return { ...receipt, relativePath };
}

function imagePreviewCacheKey(identity: MoodBoardRequestIdentity, relativePath: string) {
  return JSON.stringify([
    identity.expectedProjectRoot,
    identity.expectedSessionId,
    relativePath,
  ]);
}

export type MoodBoardReadReceipt = {
  projectRoot: string;
  sessionId: string;
  board: MoodBoard;
};

export type MoodBoardSaveReceipt = MoodBoardReadReceipt;

type RawMoodBoardReadReceipt = {
  projectRoot: string;
  sessionId: string;
  board: unknown | null;
};

type RawMoodBoardSaveReceipt = {
  projectRoot: string;
  sessionId: string;
  board: unknown;
};

export async function readMoodBoard(identity: MoodBoardRequestIdentity): Promise<MoodBoardReadReceipt> {
  const receipt = await invoke<RawMoodBoardReadReceipt>("read_mood_board", {
    input: identity,
  });
  return {
    projectRoot: receipt.projectRoot,
    sessionId: receipt.sessionId,
    board: receipt.board === null
      ? createEmptyMoodBoard()
      : parsePersistedMoodBoard(receipt.board),
  };
}

export async function saveMoodBoard(
  board: MoodBoard,
  identity: MoodBoardRequestIdentity,
): Promise<MoodBoardSaveReceipt> {
  const receipt = await invoke<RawMoodBoardSaveReceipt>("save_mood_board", {
    input: { ...identity, board },
  });
  return {
    projectRoot: receipt.projectRoot,
    sessionId: receipt.sessionId,
    board: parsePersistedMoodBoard(receipt.board),
  };
}

export async function exportMoodBoardSvgAsset(
  identity: MoodBoardRequestIdentity,
  isCurrent: MoodBoardIdentityGuard,
  relativePath: string,
  svg: string,
): Promise<MoodBoardAssetReceipt> {
  const normalizedPath = normalizedMoodBoardAssetPath(relativePath);
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  const receipt = await invoke<MoodBoardAssetReceipt>("export_mood_board_svg_asset", {
    input: { ...identity, relativePath: normalizedPath, svg },
  });
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  return requireMoodBoardAssetReceipt(receipt, identity, normalizedPath);
}

export async function resolveMoodBoardImageSrc(
  identity: MoodBoardRequestIdentity,
  isCurrent: MoodBoardIdentityGuard,
  relativePath: string,
): Promise<string> {
  const normalizedPath = normalizedMoodBoardAssetPath(relativePath);
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  const cacheKey = imagePreviewCacheKey(identity, normalizedPath);
  const cached = imagePreviewCache.get(cacheKey);
  if (cached) {
    const src = await cached;
    requireCurrentMoodBoardIdentity(identity, isCurrent);
    return src;
  }

  const request = enqueueImagePreviewRequest(identity, isCurrent, async () => {
    requireCurrentMoodBoardIdentity(identity, isCurrent);
    const receipt = await invoke<MoodBoardImageReceipt>("read_mood_board_image_data_url", {
      input: { ...identity, relativePath: normalizedPath },
    });
    requireCurrentMoodBoardIdentity(identity, isCurrent);
    const verified = requireMoodBoardAssetReceipt(receipt, identity, normalizedPath);
    if (!verified.dataUrl) throw new Error("Preview-ul imaginii Mood Board este gol.");
    return verified.dataUrl;
  }).catch((error) => {
    imagePreviewCache.delete(cacheKey);
    throw error;
  });
  imagePreviewCache.set(cacheKey, request);

  if (imagePreviewCache.size > MAX_IMAGE_PREVIEW_CACHE_ITEMS) {
    const firstKey = imagePreviewCache.keys().next().value;
    if (firstKey) imagePreviewCache.delete(firstKey);
  }

  const src = await request;
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  return src;
}

export async function readMoodBoardImageOriginalSrc(
  identity: MoodBoardRequestIdentity,
  isCurrent: MoodBoardIdentityGuard,
  relativePath: string,
): Promise<string> {
  const normalizedPath = normalizedMoodBoardAssetPath(relativePath);
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  const receipt = await invoke<MoodBoardImageReceipt>("read_mood_board_image_original_data_url", {
    input: { ...identity, relativePath: normalizedPath },
  });
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  const verified = requireMoodBoardAssetReceipt(receipt, identity, normalizedPath);
  if (!verified.dataUrl) throw new Error("Sursa originală Mood Board este goală.");
  return verified.dataUrl;
}

export async function extractMoodBoardImagePalette(
  identity: MoodBoardRequestIdentity,
  isCurrent: MoodBoardIdentityGuard,
  relativePath: string,
  maxColors = 5,
): Promise<string[]> {
  const normalizedPath = normalizedMoodBoardAssetPath(relativePath);
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  const receipt = await invoke<MoodBoardPaletteReceipt>("extract_mood_board_image_palette", {
    input: { ...identity, relativePath: normalizedPath, maxColors },
  });
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  return requireMoodBoardAssetReceipt(receipt, identity, normalizedPath).colors;
}

export async function readMoodBoardSvgSource(
  identity: MoodBoardRequestIdentity,
  isCurrent: MoodBoardIdentityGuard,
  relativePath: string,
): Promise<string> {
  const normalizedPath = normalizedMoodBoardAssetPath(relativePath);
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  const receipt = await invoke<MoodBoardSvgSourceReceipt>("read_mood_board_svg_source", {
    input: { ...identity, relativePath: normalizedPath },
  });
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  return requireMoodBoardAssetReceipt(receipt, identity, normalizedPath).source;
}
