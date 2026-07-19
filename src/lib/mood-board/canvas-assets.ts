import { moodBoardAssetItemFromPath } from "$lib/mood-board/asset-import";
import {
  pasteClipboardImageToDesignFromWebView,
  saveClipboardEventImageToDesign,
} from "$lib/mood-board/clipboard-actions";
import {
  exportMoodBoardCompositionWebp,
  exportMoodBoardImageWebp,
  exportMoodBoardVectorSvg,
  slugifyAssetName,
} from "$lib/mood-board/export";
import {
  createMoodBoardPaletteItems,
  imageTitleFromPath,
  type MoodBoardPoint,
} from "$lib/mood-board/factory";
import {
  extractMoodBoardImagePalette,
  isMoodBoardStaleSessionError,
  requireCurrentMoodBoardIdentity,
  type MoodBoardIdentityGuard,
  type MoodBoardRequestIdentity,
} from "$lib/mood-board/io";
import {
  cloneMoodBoard,
  findMoodBoardItem,
  type MoodBoard,
  type MoodBoardItem,
} from "$lib/mood-board/model";
import { errorMessage } from "$lib/util";

type MoodBoardCanvasStatusKind = "idle" | "saved" | "error";
type MoodBoardCanvasStatusUpdate = (text: string, kind: MoodBoardCanvasStatusKind) => void;
type MoodBoardCanvasPrompt = (message: string, defaultValue?: string) => string | null;

export function applyMoodBoardVisualAssetItem(board: MoodBoard, item: MoodBoardItem) {
  const next = cloneMoodBoard(board);
  next.items = [...next.items, item];
  return { board: next, selectedItemId: item.id };
}

export function applyMoodBoardPaletteColors(
  board: MoodBoard,
  sourceItemId: string,
  sourcePath: string,
  colors: string[],
) {
  const sourceItem = findMoodBoardItem(board.items, sourceItemId);
  if (!sourceItem || sourceItem.type !== "image" || sourceItem.path !== sourcePath) return null;
  const created = createMoodBoardPaletteItems(sourceItem, colors);
  if (!created.length) return null;
  const next = cloneMoodBoard(board);
  next.items = [...next.items, ...created];
  return { board: next, selectedItemId: created[0]?.id ?? null };
}

export async function addMoodBoardVisualAssetAtPath(
  path: string,
  point: MoodBoardPoint,
  identity: MoodBoardRequestIdentity,
  isCurrent: MoodBoardIdentityGuard,
) {
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  const result = await moodBoardAssetItemFromPath(path, point, identity, isCurrent);
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  if (!result.item) return { item: null, selectedItemId: null, status: result.status };
  return {
    item: result.item,
    selectedItemId: result.item.id,
    status: result.status,
  };
}

export function saveMoodBoardClipboardEventImageToDesign(
  event: ClipboardEvent,
  identity: MoodBoardRequestIdentity,
  isCurrent: MoodBoardIdentityGuard,
) {
  return saveClipboardEventImageToDesign(event, identity, isCurrent);
}

export async function pasteMoodBoardClipboardImageToDesign(
  identity: MoodBoardRequestIdentity,
  isCurrent: MoodBoardIdentityGuard,
) {
  const relativePath = await pasteClipboardImageToDesignFromWebView(identity, isCurrent);
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  return relativePath;
}

export async function extractMoodBoardPaletteItems(
  board: MoodBoard,
  itemId: string,
  path: string,
  identity: MoodBoardRequestIdentity,
  isCurrent: MoodBoardIdentityGuard,
) {
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  const sourceItem = findMoodBoardItem(board.items, itemId);
  if (!sourceItem || sourceItem.type !== "image") return null;

  const colors = await extractMoodBoardImagePalette(identity, isCurrent, path, 5);
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  if (!colors.length) return null;
  return { colors };
}

export async function exportMoodBoardVectorPathWorkflow(
  board: MoodBoard,
  itemId: string,
  prompt: MoodBoardCanvasPrompt,
  onStatusUpdate: MoodBoardCanvasStatusUpdate,
  identity: MoodBoardRequestIdentity,
  isCurrent: MoodBoardIdentityGuard,
) {
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  const item = findMoodBoardItem(board.items, itemId);
  if (!item || (item.type !== "vectorPath" && item.type !== "vectorGroup")) return;
  const suggested = slugifyAssetName(`vector-${new Date().toISOString().slice(0, 19)}`);
  const name = prompt("Nume fișier SVG", suggested);
  if (!name) return;
  requireCurrentMoodBoardIdentity(identity, isCurrent);

  try {
    const relativePath = await exportMoodBoardVectorSvg(item, name, identity, isCurrent);
    requireCurrentMoodBoardIdentity(identity, isCurrent);
    onStatusUpdate(`SVG exportat: ${relativePath}`, "saved");
  } catch (error) {
    if (isMoodBoardStaleSessionError(error) || !isCurrent(identity)) return;
    onStatusUpdate(`Export SVG eșuat: ${errorMessage(error)}`, "error");
  }
}

export async function exportMoodBoardImageWebpWorkflow(
  board: MoodBoard,
  itemId: string,
  prompt: MoodBoardCanvasPrompt,
  onStatusUpdate: MoodBoardCanvasStatusUpdate,
  identity: MoodBoardRequestIdentity,
  isCurrent: MoodBoardIdentityGuard,
) {
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  const imageItem = findMoodBoardItem(board.items, itemId);
  if (!imageItem || imageItem.type !== "image") return;
  const suggested = slugifyAssetName(imageItem.title || imageTitleFromPath(imageItem.path));
  const name = prompt("Nume fișier WebP", suggested);
  if (!name) return;
  requireCurrentMoodBoardIdentity(identity, isCurrent);

  try {
    onStatusUpdate("Se exportă WebP din canvas...", "idle");
    const relativePath = await exportMoodBoardImageWebp(imageItem, name, identity, isCurrent);
    requireCurrentMoodBoardIdentity(identity, isCurrent);
    onStatusUpdate(`WebP exportat: ${relativePath}`, "saved");
  } catch (error) {
    if (isMoodBoardStaleSessionError(error) || !isCurrent(identity)) return;
    onStatusUpdate(`Export WebP eșuat: ${errorMessage(error)}`, "error");
  }
}

export async function exportMoodBoardCompositionWebpWorkflow(
  board: MoodBoard,
  itemId: string,
  prompt: MoodBoardCanvasPrompt,
  onStatusUpdate: MoodBoardCanvasStatusUpdate,
  identity: MoodBoardRequestIdentity,
  isCurrent: MoodBoardIdentityGuard,
) {
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  const item = findMoodBoardItem(board.items, itemId);
  if (!item) return;
  const suggested = slugifyAssetName(item.type === "frame" || item.type === "group" ? item.title : item.type);
  const name = prompt("Nume fișier WebP", suggested);
  if (!name) return;
  requireCurrentMoodBoardIdentity(identity, isCurrent);

  try {
    onStatusUpdate("Se exportă compoziția WebP...", "idle");
    const relativePath = await exportMoodBoardCompositionWebp(item, name, identity, isCurrent);
    requireCurrentMoodBoardIdentity(identity, isCurrent);
    onStatusUpdate(`Compoziție exportată: ${relativePath}`, "saved");
  } catch (error) {
    if (isMoodBoardStaleSessionError(error) || !isCurrent(identity)) return;
    onStatusUpdate(`Export compoziție eșuat: ${errorMessage(error)}`, "error");
  }
}
