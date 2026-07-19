import type { MoodBoardItem } from "$lib/mood-board/model";
import { imageFilterValue, imageOpacityValue } from "$lib/mood-board/image-adjustments";
import { imageDrawRect } from "$lib/mood-board/image-framing";
import {
  readMoodBoardImageOriginalSrc,
  requireCurrentMoodBoardIdentity,
  type MoodBoardIdentityGuard,
  type MoodBoardRequestIdentity,
} from "$lib/mood-board/io";
import { buildVectorSvgPath, vectorControlPoint } from "$lib/mood-board/vector";

type RenderOptions = {
  maxEdge?: number;
};

function roundedRectPath(context: CanvasRenderingContext2D, x: number, y: number, width: number, height: number, radius: number) {
  const safeRadius = Math.min(radius, width / 2, height / 2);
  context.beginPath();
  context.moveTo(x + safeRadius, y);
  context.lineTo(x + width - safeRadius, y);
  context.quadraticCurveTo(x + width, y, x + width, y + safeRadius);
  context.lineTo(x + width, y + height - safeRadius);
  context.quadraticCurveTo(x + width, y + height, x + width - safeRadius, y + height);
  context.lineTo(x + safeRadius, y + height);
  context.quadraticCurveTo(x, y + height, x, y + height - safeRadius);
  context.lineTo(x, y + safeRadius);
  context.quadraticCurveTo(x, y, x + safeRadius, y);
  context.closePath();
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error("Imaginea nu a putut fi încărcată pentru randarea exportului."));
    image.src = src;
  });
}

function drawImageWithObjectFit(
  context: CanvasRenderingContext2D,
  image: HTMLImageElement,
  fit: "cover" | "contain",
  width: number,
  height: number,
  framing: Extract<MoodBoardItem, { type: "image" }>["framing"],
) {
  const drawRect = imageDrawRect(image.naturalWidth, image.naturalHeight, fit, width, height, framing);
  context.drawImage(image, drawRect.x, drawRect.y, drawRect.width, drawRect.height);
}

function maskPathToCanvasPath(mask: NonNullable<Extract<MoodBoardItem, { type: "image" }>["mask"]>) {
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

function drawWrappedText(
  context: CanvasRenderingContext2D,
  text: string,
  x: number,
  y: number,
  maxWidth: number,
  lineHeight: number,
  maxHeight: number,
) {
  const words = text.split(/\s+/).filter(Boolean);
  const lines: string[] = [];
  let line = "";
  for (const word of words.length ? words : [""]) {
    const next = line ? `${line} ${word}` : word;
    if (line && context.measureText(next).width > maxWidth) {
      lines.push(line);
      line = word;
    } else {
      line = next;
    }
  }
  if (line) lines.push(line);

  const maxLines = Math.max(1, Math.floor(maxHeight / lineHeight));
  for (const [index, value] of lines.slice(0, maxLines).entries()) {
    context.fillText(value, x, y + index * lineHeight);
  }
}

function drawShape(context: CanvasRenderingContext2D, item: Extract<MoodBoardItem, { type: "shape" }>) {
  context.beginPath();
  if (item.shape === "ellipse") {
    context.ellipse(item.width / 2, item.height / 2, item.width / 2, item.height / 2, 0, 0, Math.PI * 2);
  } else if (item.shape === "diamond") {
    context.moveTo(item.width / 2, 0);
    context.lineTo(item.width, item.height / 2);
    context.lineTo(item.width / 2, item.height);
    context.lineTo(0, item.height / 2);
    context.closePath();
  } else {
    context.rect(0, 0, item.width, item.height);
  }
  if (item.fill && item.fill !== "transparent" && item.fill !== "none") {
    context.fillStyle = item.fill;
    context.fill();
  }
  if (item.strokeWidth > 0 && item.stroke && item.stroke !== "transparent" && item.stroke !== "none") {
    context.strokeStyle = item.stroke;
    context.lineWidth = item.strokeWidth;
    context.stroke();
  }
}

function drawVectorPath(context: CanvasRenderingContext2D, item: Extract<MoodBoardItem, { type: "vectorPath" }>) {
  const path = new Path2D(buildVectorSvgPath(item));
  context.save();
  context.scale(item.width / Math.max(1, item.viewBoxWidth), item.height / Math.max(1, item.viewBoxHeight));
  if (item.fill && item.fill !== "none" && item.fill !== "transparent") {
    context.fillStyle = item.fill;
    context.fill(path);
  }
  if (item.strokeWidth > 0 && item.stroke && item.stroke !== "none" && item.stroke !== "transparent") {
    context.strokeStyle = item.stroke;
    context.lineWidth = item.strokeWidth;
    context.stroke(path);
  }
  context.restore();
}

function drawVectorGroup(context: CanvasRenderingContext2D, item: Extract<MoodBoardItem, { type: "vectorGroup" }>) {
  context.save();
  context.scale(item.width / Math.max(1, item.viewBoxWidth), item.height / Math.max(1, item.viewBoxHeight));
  context.translate(-item.viewBoxX, -item.viewBoxY);
  for (const element of item.elements) {
    const [a, b, c, d, e, f] = element.transform;
    context.save();
    context.globalAlpha = element.opacity;
    context.transform(a, b, c, d, e, f);
    if (element.type === "text") {
      context.fillStyle = element.fill;
      context.font = `${element.fontWeight} ${element.fontSize}px ${element.fontFamily}, sans-serif`;
      context.textBaseline = "alphabetic";
      context.fillText(element.text, element.x, element.y);
    } else {
      const path = new Path2D(element.d);
      if (element.fill && element.fill !== "none" && element.fill !== "transparent") {
        context.fillStyle = element.fill;
        context.fill(path);
      }
      if (element.strokeWidth > 0 && element.stroke && element.stroke !== "none" && element.stroke !== "transparent") {
        context.strokeStyle = element.stroke;
        context.lineWidth = element.strokeWidth;
        context.stroke(path);
      }
    }
    context.restore();
  }
  context.restore();
}

async function renderItem(
  context: CanvasRenderingContext2D,
  item: MoodBoardItem,
  offsetX: number,
  offsetY: number,
  identity: MoodBoardRequestIdentity,
  isCurrent: MoodBoardIdentityGuard,
): Promise<void> {
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  context.save();
  context.translate(offsetX + item.x, offsetY + item.y);

  if (item.type === "frame") {
    if (item.background && item.background !== "transparent") {
      context.fillStyle = item.background;
      context.fillRect(0, 0, item.width, item.height);
    }
    context.strokeStyle = item.tone;
    context.lineWidth = 1;
    context.strokeRect(0, 0, item.width, item.height);
    for (const child of item.children) {
      await renderItem(context, child, 0, 0, identity, isCurrent);
      requireCurrentMoodBoardIdentity(identity, isCurrent);
    }
  } else if (item.type === "group") {
    for (const child of item.children) {
      await renderItem(context, child, 0, 0, identity, isCurrent);
      requireCurrentMoodBoardIdentity(identity, isCurrent);
    }
  } else if (item.type === "image") {
    const source = await readMoodBoardImageOriginalSrc(identity, isCurrent, item.path);
    requireCurrentMoodBoardIdentity(identity, isCurrent);
    const image = await loadImage(source);
    requireCurrentMoodBoardIdentity(identity, isCurrent);
    context.save();
    if (item.mask) context.clip(maskPathToCanvasPath(item.mask));
    else if (item.radius > 0) {
      roundedRectPath(context, 0, 0, item.width, item.height, item.radius);
      context.clip();
    }
    context.globalAlpha = imageOpacityValue(item.adjustments);
    context.filter = imageFilterValue(item.adjustments);
    drawImageWithObjectFit(context, image, item.fit, item.width, item.height, item.framing);
    context.restore();
  } else if (item.type === "shape") {
    drawShape(context, item);
  } else if (item.type === "vectorPath") {
    drawVectorPath(context, item);
  } else if (item.type === "vectorGroup") {
    drawVectorGroup(context, item);
  } else if (item.type === "text") {
    context.fillStyle = item.color;
    context.font = `${item.fontWeight} ${item.fontSize}px Inter, system-ui, sans-serif`;
    context.textAlign = item.textAlign;
    context.textBaseline = "top";
    const x = item.textAlign === "center" ? item.width / 2 : item.textAlign === "right" ? item.width : 0;
    drawWrappedText(context, item.text, x, 0, item.width, item.fontSize * 1.18, item.height);
  } else if (item.type === "color") {
    roundedRectPath(context, 0, 0, item.width, item.height, 8);
    context.fillStyle = "#ffffff";
    context.fill();
    roundedRectPath(context, 12, 12, Math.max(20, item.width - 24), Math.max(20, item.height - 64), 6);
    context.fillStyle = item.color;
    context.fill();
    context.fillStyle = "#1a2825";
    context.font = "800 18px Inter, system-ui, sans-serif";
    context.fillText(item.label, 12, item.height - 44);
    context.fillStyle = "#68766f";
    context.font = "500 14px Inter, system-ui, sans-serif";
    context.fillText(item.color, 12, item.height - 22);
  } else if (item.type === "note" || item.type === "reference") {
    roundedRectPath(context, 0, 0, item.width, item.height, 8);
    context.fillStyle = "#ffffff";
    context.fill();
    context.strokeStyle = "#c8d4ce";
    context.lineWidth = 1;
    context.stroke();
    context.fillStyle = "#1a2825";
    context.font = "500 18px Inter, system-ui, sans-serif";
    drawWrappedText(context, item.type === "note" ? item.text : item.title, 16, 16, item.width - 32, 24, item.height - 32);
  }

  context.restore();
}

export async function renderMoodBoardItemToPngDataUrl(
  item: MoodBoardItem,
  identity: MoodBoardRequestIdentity,
  isCurrent: MoodBoardIdentityGuard,
  options: RenderOptions = {},
) {
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  const maxEdge = options.maxEdge ?? 1920;
  const width = Math.max(1, item.width);
  const height = Math.max(1, item.height);
  const scale = maxEdge / Math.max(width, height);
  const outputWidth = Math.max(1, Math.round(width * scale));
  const outputHeight = Math.max(1, Math.round(height * scale));
  const canvas = document.createElement("canvas");
  canvas.width = outputWidth;
  canvas.height = outputHeight;
  const context = canvas.getContext("2d");
  if (!context) throw new Error("Canvas export indisponibil.");

  context.clearRect(0, 0, outputWidth, outputHeight);
  context.save();
  context.scale(scale, scale);
  await renderItem(context, item, -item.x, -item.y, identity, isCurrent);
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  context.restore();

  const dataUrl = canvas.toDataURL("image/png");
  if (!dataUrl.startsWith("data:image/png")) {
    throw new Error("WebView-ul nu a putut genera randarea PNG intermediară.");
  }
  return dataUrl;
}
