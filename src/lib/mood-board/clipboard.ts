import { exportProjectAssetDataUrl } from "$lib/project/io";
import {
  moodBoardReceiptMatchesIdentity,
  requireCurrentMoodBoardIdentity,
  type MoodBoardIdentityGuard,
  type MoodBoardRequestIdentity,
} from "$lib/mood-board/io";

const pastedImageDirectory = "design/imagini";
const MAX_CLIPBOARD_IMAGE_BYTES = 32 * 1024 * 1024;

export type PastedMoodBoardImage = {
  projectRoot: string;
  sessionId: string;
  relativePath: string;
  fileName: string;
  mimeType: string;
};

function clipboardImageExtension(mimeType: string) {
  const normalized = mimeType.toLowerCase().split(";", 1)[0];
  if (normalized === "image/jpeg" || normalized === "image/jpg") return "jpg";
  if (normalized === "image/webp") return "webp";
  if (normalized === "image/gif") return "gif";
  if (normalized === "image/svg+xml") return "svg";
  return "png";
}

function timestampSlug(date = new Date()) {
  const pad = (value: number) => String(value).padStart(2, "0");
  return [
    date.getFullYear(),
    pad(date.getMonth() + 1),
    pad(date.getDate()),
    pad(date.getHours()),
    pad(date.getMinutes()),
    pad(date.getSeconds()),
  ].join("-");
}

function imageFileName(file: File) {
  const extension = clipboardImageExtension(file.type || "image/png");
  const entropy = Math.random().toString(36).slice(2, 6);
  return `imagine-lipita-${timestampSlug()}-${entropy}.${extension}`;
}

function fileToDataUrl(file: File) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      if (typeof reader.result === "string") resolve(reader.result);
      else reject(new Error("Clipboard-ul nu a putut fi convertit în data URL."));
    };
    reader.onerror = () => reject(new Error("Nu am putut citi imaginea din clipboard."));
    reader.readAsDataURL(file);
  });
}

export function clipboardImageFile(event: ClipboardEvent) {
  const files = Array.from(event.clipboardData?.files ?? []);
  const directFile = files.find((file) => file.type.toLowerCase().startsWith("image/"));
  if (directFile) return directFile;

  const items = Array.from(event.clipboardData?.items ?? []);
  for (const item of items) {
    if (item.kind !== "file" || !item.type.toLowerCase().startsWith("image/")) continue;
    const file = item.getAsFile();
    if (file) return file;
  }
  return null;
}

export async function readClipboardImageFile(
  identity: MoodBoardRequestIdentity,
  isCurrent: MoodBoardIdentityGuard,
) {
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  if (typeof navigator === "undefined") return null;
  if (!navigator.clipboard || typeof navigator.clipboard.read !== "function") return null;

  const items = await navigator.clipboard.read();
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  for (const item of items) {
    const type = item.types.find((candidate) => candidate.toLowerCase().startsWith("image/"));
    if (!type) continue;

    const blob = await item.getType(type);
    requireCurrentMoodBoardIdentity(identity, isCurrent);
    const extension = clipboardImageExtension(blob.type || type || "image/png");
    return new File([blob], `imagine-lipita.${extension}`, { type: blob.type || type || "image/png" });
  }

  return null;
}

export async function saveClipboardImageToDesign(
  file: File,
  identity: MoodBoardRequestIdentity,
  isCurrent: MoodBoardIdentityGuard,
): Promise<PastedMoodBoardImage> {
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  if (file.size > MAX_CLIPBOARD_IMAGE_BYTES) {
    throw new Error("Imaginea din clipboard depășește limita de 32 MiB.");
  }
  const fileName = imageFileName(file);
  const relativePath = `${pastedImageDirectory}/${fileName}`;
  const dataUrl = await fileToDataUrl(file);
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  const receipt = await exportProjectAssetDataUrl(identity, relativePath, dataUrl);
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  if (!moodBoardReceiptMatchesIdentity(receipt, identity) || receipt.relativePath !== relativePath) {
    throw new Error("Mood Board a refuzat receipt-ul imaginii lipite.");
  }

  return {
    projectRoot: receipt.projectRoot,
    sessionId: receipt.sessionId,
    relativePath: receipt.relativePath,
    fileName,
    mimeType: file.type || "image/png",
  };
}
