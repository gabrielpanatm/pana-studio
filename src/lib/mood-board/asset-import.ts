import {
  createMoodBoardImageItem,
  isSvgPath,
  type MoodBoardPoint,
} from "$lib/mood-board/factory";
import type { MoodBoardItem } from "$lib/mood-board/model";
import {
  isMoodBoardStaleSessionError,
  readMoodBoardSvgSource,
  requireCurrentMoodBoardIdentity,
  type MoodBoardIdentityGuard,
  type MoodBoardRequestIdentity,
} from "$lib/mood-board/io";
import { parseEditableSvg } from "$lib/mood-board/svg";
import { errorMessage } from "$lib/util";

export type MoodBoardAssetImportResult = {
  item: MoodBoardItem | null;
  status?: {
    text: string;
    kind: "idle" | "saved" | "error";
  };
};

export async function moodBoardAssetItemFromPath(
  path: string,
  point: MoodBoardPoint,
  identity: MoodBoardRequestIdentity,
  isCurrent: MoodBoardIdentityGuard,
): Promise<MoodBoardAssetImportResult> {
  requireCurrentMoodBoardIdentity(identity, isCurrent);
  const normalizedPath = path.trim().replaceAll("\\", "/");
  if (!normalizedPath) return { item: null };

  if (isSvgPath(normalizedPath)) {
    const svgResult = await editableSvgItemFromPath(normalizedPath, point, identity, isCurrent);
    requireCurrentMoodBoardIdentity(identity, isCurrent);
    if (svgResult.item) return svgResult;
    return {
      item: createMoodBoardImageItem(normalizedPath, point),
      status: svgResult.status?.kind === "error"
        ? svgResult.status
        : {
          text: "SVG complex importat ca imagine; structura nu este încă editabilă.",
          kind: "idle",
        },
    };
  }

  return { item: createMoodBoardImageItem(normalizedPath, point) };
}

async function editableSvgItemFromPath(
  path: string,
  point: MoodBoardPoint,
  identity: MoodBoardRequestIdentity,
  isCurrent: MoodBoardIdentityGuard,
): Promise<MoodBoardAssetImportResult> {
  try {
    const source = await readMoodBoardSvgSource(identity, isCurrent, path);
    requireCurrentMoodBoardIdentity(identity, isCurrent);
    const svgItem = parseEditableSvg(path, source, point);
    requireCurrentMoodBoardIdentity(identity, isCurrent);
    if (!svgItem) return { item: null };
    return {
      item: svgItem,
      status: { text: `SVG importat ca vector editabil: ${path}`, kind: "saved" },
    };
  } catch (error) {
    if (isMoodBoardStaleSessionError(error)) throw error;
    return {
      item: null,
      status: { text: `Import SVG editabil eșuat: ${errorMessage(error)}`, kind: "error" },
    };
  }
}
