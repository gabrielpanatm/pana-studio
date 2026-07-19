import { imageFilterValue, imageOpacityValue } from "$lib/mood-board/image-adjustments";
import { imageDrawRect } from "$lib/mood-board/image-framing";
import {
  exportMoodBoardSvgAsset,
  moodBoardReceiptMatchesIdentity,
  readMoodBoardImageOriginalSrc,
  requireCurrentMoodBoardIdentity,
  type MoodBoardAssetReceipt,
  type MoodBoardIdentityGuard,
  type MoodBoardRequestIdentity,
} from "$lib/mood-board/io";
import type { MoodBoardItem } from "$lib/mood-board/model";
import { renderMoodBoardItemToPngDataUrl } from "$lib/mood-board/render";
import { vectorGroupToSvg, vectorPathToSvg } from "$lib/mood-board/svg";
import { vectorControlPoint } from "$lib/mood-board/vector";
import { exportProjectAssetWebpFromDataUrl } from "$lib/project/io";

type MoodBoardImageItem = Extract<MoodBoardItem, { type: "image" }>;
type MoodBoardVectorExportItem = Extract<MoodBoardItem, { type: "vectorPath" | "vectorGroup" }>;

export function slugifyAssetName(value: string) {
  const slug = value
    .trim()
    .toLowerCase()
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return slug || `vector-${Date.now().toString(36)}`;
}

function requireProjectAssetReceipt(
  receipt: MoodBoardAssetReceipt,
  identity: MoodBoardRequestIdentity,
  expectedRelativePath: string,
) {
  if (!moodBoardReceiptMatchesIdentity(receipt, identity) || receipt.relativePath !== expectedRelativePath) {
    throw new Error("Mood Board a refuzat receipt-ul exportului.");
  }
  return receipt.relativePath;
}

export async function exportMoodBoardVectorSvg(
  item: MoodBoardVectorExportItem,
  name: string,
  identity: MoodBoardRequestIdentity,
  isCurrent: MoodBoardIdentityGuard,
) {
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  const filename = `${slugifyAssetName(name.trim().replace(/\.svg$/i, ""))}.svg`;
  const relativePath = `resurse/imagini/${filename}`;
  const receipt = await exportMoodBoardSvgAsset(
    identity,
    isCurrent,
    relativePath,
    item.type === "vectorPath" ? vectorPathToSvg(item) : vectorGroupToSvg(item),
  );
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  return requireProjectAssetReceipt(receipt, identity, relativePath);
}

export async function exportMoodBoardImageWebp(
  imageItem: MoodBoardImageItem,
  name: string,
  identity: MoodBoardRequestIdentity,
  isCurrent: MoodBoardIdentityGuard,
) {
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  const source = await readMoodBoardImageOriginalSrc(identity, isCurrent, imageItem.path);
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  const sourceImage = await loadImage(source);
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  const maxEdge = 1920;
  const ratio = imageItem.width / Math.max(1, imageItem.height);
  const outputWidth = ratio >= 1 ? maxEdge : Math.max(1, Math.round(maxEdge * ratio));
  const outputHeight = ratio >= 1 ? Math.max(1, Math.round(maxEdge / ratio)) : maxEdge;
  const canvas = document.createElement("canvas");
  canvas.width = outputWidth;
  canvas.height = outputHeight;
  const context = canvas.getContext("2d");
  if (!context) throw new Error("Canvas export indisponibil.");

  context.clearRect(0, 0, outputWidth, outputHeight);
  context.save();
  context.scale(outputWidth / imageItem.width, outputHeight / imageItem.height);
  if (imageItem.mask) context.clip(maskPathToCanvasPath(imageItem.mask));
  context.globalAlpha = imageOpacityValue(imageItem.adjustments);
  context.filter = imageFilterValue(imageItem.adjustments);
  drawImageWithObjectFit(context, sourceImage, imageItem.fit, imageItem.width, imageItem.height, imageItem.framing);
  context.restore();

  const dataUrl = canvas.toDataURL("image/png");
  if (!dataUrl.startsWith("data:image/png")) {
    throw new Error("WebView-ul nu a putut genera randarea PNG intermediară.");
  }
  const filename = `${slugifyAssetName(name.trim().replace(/\.webp$/i, ""))}.webp`;
  const relativePath = `resurse/imagini/${filename}`;
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  const receipt = await exportProjectAssetWebpFromDataUrl(identity, relativePath, dataUrl);
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  return requireProjectAssetReceipt(receipt, identity, relativePath);
}

export async function exportMoodBoardCompositionWebp(
  item: MoodBoardItem,
  name: string,
  identity: MoodBoardRequestIdentity,
  isCurrent: MoodBoardIdentityGuard,
) {
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  const dataUrl = await renderMoodBoardItemToPngDataUrl(item, identity, isCurrent, { maxEdge: 1920 });
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  const filename = `${slugifyAssetName(name.trim().replace(/\.webp$/i, ""))}.webp`;
  const relativePath = `resurse/imagini/${filename}`;
  const receipt = await exportProjectAssetWebpFromDataUrl(identity, relativePath, dataUrl);
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  return requireProjectAssetReceipt(receipt, identity, relativePath);
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error("Imaginea nu a putut fi încărcată pentru export."));
    image.src = src;
  });
}

function drawImageWithObjectFit(
  context: CanvasRenderingContext2D,
  image: HTMLImageElement,
  fit: "cover" | "contain",
  width: number,
  height: number,
  framing: MoodBoardImageItem["framing"],
) {
  const drawRect = imageDrawRect(image.naturalWidth, image.naturalHeight, fit, width, height, framing);
  context.drawImage(image, drawRect.x, drawRect.y, drawRect.width, drawRect.height);
}

function maskPathToCanvasPath(mask: NonNullable<MoodBoardImageItem["mask"]>) {
  const path = new Path2D();
  const [first, ...rest] = mask.nodes;
  if (!first) return path;
  path.moveTo(first.x, first.y);
  let previous = first;
  for (const node of rest) {
    const c1 = vectorControlPoint(previous, "out");
    const c2 = vectorControlPoint(node, "in");
    if (previous.out || node.in) path.bezierCurveTo(c1.x, c1.y, c2.x, c2.y, node.x, node.y);
    else path.lineTo(node.x, node.y);
    previous = node;
  }
  if (mask.closed) {
    const c1 = vectorControlPoint(previous, "out");
    const c2 = vectorControlPoint(first, "in");
    if (previous.out || first.in) path.bezierCurveTo(c1.x, c1.y, c2.x, c2.y, first.x, first.y);
    path.closePath();
  }
  return path;
}
