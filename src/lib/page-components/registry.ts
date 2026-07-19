import type { HtmlPaletteElement, HtmlPaletteGroup } from "$lib/project/html-palette";
import type { PageComponentRegistryItem, PageComponentRegistrySnapshot } from "$lib/types";

function paletteElementForComponent(component: PageComponentRegistryItem): HtmlPaletteElement {
  return {
    id: `component:${component.id}`,
    kind: "component",
    componentId: component.id,
    componentKind: component.kind,
    tag: component.tag,
    label: component.label,
    description: component.description,
    text: component.text,
    className: component.className,
    html: component.html,
  };
}

function nonEmptyComponentPaletteGroups(groups: HtmlPaletteGroup[]) {
  return groups.filter((group) => group.elements.length > 0);
}

export function pageComponentPaletteGroupsFromRegistry(
  snapshot: PageComponentRegistrySnapshot | null | undefined,
): HtmlPaletteGroup[] {
  if (!snapshot || snapshot.schemaVersion !== 1) return [];
  const groups = snapshot.groups.map((group) => ({
    label: group.label,
    elements: group.elements.map(paletteElementForComponent),
  }));
  return nonEmptyComponentPaletteGroups(groups);
}
