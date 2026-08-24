import type {
  UiBlockGraphSnapshot,
  UiBlockSourceInstance,
} from "$lib/blocks/contracts";
import type { BlockSelectionContext } from "$lib/canvas/contracts";

function sourceInstanceMatchesSelection(
  instance: UiBlockSourceInstance,
  selection: BlockSelectionContext,
) {
  return instance.providerId === selection.providerId;
}

export function resolveUiBlockSourceInstanceForSelection(
  snapshot: UiBlockGraphSnapshot | null | undefined,
  selection: BlockSelectionContext | null | undefined,
): UiBlockSourceInstance | null {
  if (!snapshot || !selection) return null;

  if (selection.rootSourceId) {
    const exactRoot = snapshot.sourceInstances.find(
      (instance) => instance.rootSourceNodeId === selection.rootSourceId
        && sourceInstanceMatchesSelection(instance, selection),
    );
    if (exactRoot) return exactRoot;
  }

  const sourceInstancesById = new Map(
    snapshot.sourceInstances.map((instance) => [instance.id, instance] as const),
  );
  // CanvasGraph păstrează proveniența de la exterior spre interior. Ultima
  // instanță compatibilă este blocul cel mai apropiat de elementul selectat.
  const sourceInstanceIds = Array.isArray(selection.sourceInstanceIds)
    ? selection.sourceInstanceIds
    : [];
  for (let index = sourceInstanceIds.length - 1; index >= 0; index -= 1) {
    const instance = sourceInstancesById.get(sourceInstanceIds[index]);
    if (instance && sourceInstanceMatchesSelection(instance, selection)) {
      return instance;
    }
  }
  return null;
}
