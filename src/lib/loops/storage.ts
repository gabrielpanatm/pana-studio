import {
  normalizeLoopDefinition,
  type LoopDefinition,
} from "$lib/loops/model";

const storageKey = "pana-studio.loop-definitions.v1";

type StoredLoops = Record<string, LoopDefinition[]>;

export function loadLoopDefinitionsForProject(storage: Storage, projectRoot: string | null | undefined) {
  if (!projectRoot) return [];
  const parsed = parseStoredLoops(storage.getItem(storageKey));
  return (parsed[projectRoot] ?? []).map(normalizeLoopDefinition);
}

export function saveLoopDefinitionsForProject(
  storage: Storage,
  projectRoot: string | null | undefined,
  definitions: LoopDefinition[],
) {
  if (!projectRoot) return;
  const parsed = parseStoredLoops(storage.getItem(storageKey));
  parsed[projectRoot] = definitions.map(normalizeLoopDefinition);
  storage.setItem(storageKey, JSON.stringify(parsed));
}

function parseStoredLoops(payload: string | null): StoredLoops {
  if (!payload) return {};
  try {
    const parsed = JSON.parse(payload) as unknown;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return {};
    const result: StoredLoops = {};
    for (const [projectRoot, value] of Object.entries(parsed)) {
      if (!Array.isArray(value)) continue;
      result[projectRoot] = value
        .filter((item): item is LoopDefinition => Boolean(item && typeof item === "object"))
        .map(normalizeLoopDefinition);
    }
    return result;
  } catch {
    return {};
  }
}
