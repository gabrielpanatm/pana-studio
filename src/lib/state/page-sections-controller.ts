import { hydratePageSectionSources } from "$lib/source-graph/location";
import type { PageSection, SourceGraph } from "$lib/types";

export type PageSectionsHost = {
  pageSections: PageSection[];
  sourceGraph: SourceGraph | null;
};

export function hydratePageSections(host: PageSectionsHost, sections: PageSection[]) {
  return hydratePageSectionSources(sections, host.sourceGraph);
}

export function setPageSections(host: PageSectionsHost, sections: PageSection[]) {
  host.pageSections = hydratePageSections(host, sections);
}

export function resetPageSections(host: PageSectionsHost) {
  host.pageSections = [];
}
