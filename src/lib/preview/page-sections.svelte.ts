import { hydratePageSectionSources } from "$lib/source-graph/location";
import type { PageSection } from "$lib/canvas/contracts";
import type { SourceGraph } from "$lib/source-graph/graph-contract";

/** Owns the page-section projection hydrated against the current SourceGraph. */
export class PageSectionsState {
  sections = $state<PageSection[]>([]);
  private readonly sourceGraph: () => SourceGraph | null;

  constructor(sourceGraph: () => SourceGraph | null) {
    this.sourceGraph = sourceGraph;
  }

  hydrate(sections: PageSection[]) {
    return hydratePageSectionSources(sections, this.sourceGraph());
  }

  set(sections: PageSection[]) {
    this.sections = this.hydrate(sections);
  }

  reset() {
    this.sections = [];
  }
}
