import type { HtmlPaletteElement, HtmlPaletteGroup } from "$lib/project/html-palette";
import type { NativeBlockRegistryItem, NativeBlockRegistrySnapshot } from "$lib/types";

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
