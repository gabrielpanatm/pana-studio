import type { InspectorPendingArea } from "$lib/types";

export type InspectorPendingSource = "session" | "inspector-pane" | "motion-timeline";

export type InspectorPendingSourceRegistry = Record<InspectorPendingArea, Set<InspectorPendingSource>>;

export function createInspectorPendingSourceRegistry(): InspectorPendingSourceRegistry {
  return {
    html: new Set(),
    css: new Set(),
    js: new Set(),
  };
}

export function updateInspectorPendingSource(
  registry: InspectorPendingSourceRegistry,
  area: InspectorPendingArea,
  source: InspectorPendingSource,
  pending: boolean,
) {
  const sources = registry[area];
  if (pending) sources.add(source);
  else sources.delete(source);
  return sources.size > 0;
}
