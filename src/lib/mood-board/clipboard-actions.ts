import { clipboardImageFile, readClipboardImageFile, saveClipboardImageToDesign } from "$lib/mood-board/clipboard";
import {
  isMoodBoardStaleSessionError,
  requireCurrentMoodBoardIdentity,
  type MoodBoardIdentityGuard,
  type MoodBoardRequestIdentity,
} from "$lib/mood-board/io";
import { errorMessage } from "$lib/util";

export function saveClipboardEventImageToDesign(
  event: ClipboardEvent,
  identity: MoodBoardRequestIdentity,
  isCurrent: MoodBoardIdentityGuard,
) {
  const file = clipboardImageFile(event);
  if (!file) return null;
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  return saveClipboardImageToDesign(file, identity, isCurrent).then((saved) => {
    requireCurrentMoodBoardIdentity(identity, isCurrent);
    return saved.relativePath;
  });
}

export async function pasteClipboardImageToDesignFromWebView(
  identity: MoodBoardRequestIdentity,
  isCurrent: MoodBoardIdentityGuard,
) {
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  try {
    const file = await readClipboardImageFile(identity, isCurrent);
    requireCurrentMoodBoardIdentity(identity, isCurrent);
    if (file) {
      const relativePath = (await saveClipboardImageToDesign(file, identity, isCurrent)).relativePath;
      requireCurrentMoodBoardIdentity(identity, isCurrent);
      return relativePath;
    }
    throw new Error("Clipboard-ul WebView nu conține o imagine accesibilă.");
  } catch (error) {
    if (isMoodBoardStaleSessionError(error)) throw error;
    throw new Error(`Clipboard-ul WebView nu a putut furniza imaginea: ${errorMessage(error)}`);
  }
}
