import type { HtmlPaletteElement, HtmlPaletteGroup } from "$lib/html/palette";
import type {
  BlockDefinition,
  BlockScale,
  NativeBlockRegistryItem,
  NativeBlockRegistrySnapshot,
  UiBlockGraphSnapshot,
  UiBlockSourceInstance,
} from "$lib/blocks/contracts";
import type { BlockSelectionContext } from "$lib/canvas/contracts";

const BLOCK_SCALE_ORDER: readonly BlockScale[] = ["element", "section", "composition"];

export function availableNativeBlockScales(
  definitions: ReadonlyArray<Pick<BlockDefinition, "scale">>,
): BlockScale[] {
  const available = new Set(definitions.map((definition) => definition.scale));
  return BLOCK_SCALE_ORDER.filter((scale) => available.has(scale));
}

function paletteElementForBlock(block: NativeBlockRegistryItem): HtmlPaletteElement {
  return {
    id: `block:${block.id}`,
    kind: "block",
    blockId: block.id,
    blockKind: block.kind,
    tag: block.tag,
    label: block.label,
    description: block.description,
    text: block.text,
    className: block.className,
    html: block.html,
  };
}

function nonEmptyBlockPaletteGroups(groups: HtmlPaletteGroup[]) {
  return groups.filter((group) => group.elements.length > 0);
}

export function nativeBlockPaletteGroupsFromRegistry(
  snapshot: NativeBlockRegistrySnapshot | null | undefined,
): HtmlPaletteGroup[] {
  if (!snapshot || snapshot.schemaVersion !== 1) return [];
  const groups = snapshot.groups.map((group) => ({
    label: group.label,
    elements: group.elements.map(paletteElementForBlock),
  }));
  return nonEmptyBlockPaletteGroups(groups);
}

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
