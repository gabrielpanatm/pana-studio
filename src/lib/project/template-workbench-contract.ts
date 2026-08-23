import type { LocalizedDiagnostic } from "$lib/contracts/localized-diagnostic";
import type { SourceOrigin } from "$lib/source-graph/contracts";

type TemplateWorkbenchDependencyKind = "extends" | "includes" | "imports";

type TemplateWorkbenchTemplate = {
  sourceId: string;
  file: string;
  name: string;
  origin: SourceOrigin;
  themeName: string | null;
  isPartial: boolean;
  definesMacros: boolean;
};

type TemplateWorkbenchDependencyStep = {
  fromSourceId: string;
  fromFile: string;
  toSourceId: string;
  toFile: string;
  kind: TemplateWorkbenchDependencyKind;
};

type TemplateWorkbenchConsumer = {
  pageId: string;
  pageFile: string;
  pageTitle: string;
  pageUrl: string;
  rootTemplateSourceId: string;
  rootTemplateFile: string;
  dependencyPath: TemplateWorkbenchDependencyStep[];
};

type TemplateWorkbenchNavigatorEntry = {
  role: "directParent" | "active";
  template: TemplateWorkbenchTemplate;
  expanded: boolean;
  editable: boolean;
};

type TemplateWorkbenchRenderMode =
  | "page"
  | "includedTemplate"
  | "listingItemScenario"
  | "canonicalRoute"
  | "macroScenario"
  | "orphanTemplate";

type TemplateWorkbenchRenderContextKind =
  | "realZolaPage"
  | "realZolaConsumer"
  | "realZolaRoute"
  | "controlledMacroScenario"
  | "controlledListingItemScenario"
  | "controlledTemplateFixture";

type TemplateWorkbenchRouteContext = {
  kind: "taxonomy_list" | "taxonomy_term" | "not_found";
  label: string;
  url: string;
};

type TemplateWorkbenchRenderContext = {
  kind: TemplateWorkbenchRenderContextKind;
  canonicalTruth: boolean;
  label: string;
  explanation: string;
};

export type TemplateWorkbenchPlan = {
  schemaVersion: 4;
  projectModelRevision: string;
  activeTemplate: TemplateWorkbenchTemplate;
  directParent: TemplateWorkbenchTemplate | null;
  navigator: TemplateWorkbenchNavigatorEntry[];
  consumers: TemplateWorkbenchConsumer[];
  selectedContext: TemplateWorkbenchConsumer | null;
  selectedRoute: TemplateWorkbenchRouteContext | null;
  renderMode: TemplateWorkbenchRenderMode;
  renderContext: TemplateWorkbenchRenderContext;
  diagnostics: Array<{ code: string; messageDiagnostic: LocalizedDiagnostic }>;
};
