import type { CssProperty } from "$lib/css/property-contract";
import type { CssBackground } from "$lib/inspector/background-model";
import type { CssGrid } from "$lib/inspector/grid-model";

export type CssViewport = "desktop" | "tablet" | "mobile";

export type StyleRow = {
  label: string;
  value: string;
};

export type CssVariableRow = {
  name: string;
  value: string;
};

export type CssRuleMatch = {
  selector: string;
  source: string;
  media: string | null;
  declarations: number;
  kind: string;
  score: number;
};

export type CssSelectorOption = {
  selector: string;
  label: string;
  source: "class" | "compound" | "id" | "tag" | "matched";
  detailKind:
    | "matched_rule"
    | "all_element_classes"
    | "element_class"
    | "element_id"
    | "generated_without_class_or_id"
    | "tag_fallback";
  detailSource?: string;
};

export type EditableStyles = {
  color: string;
  backgroundColor: string;
  fontSize: string;
  lineHeight: string;
  textAlign: string;
  margin: string;
  padding: string;
  borderRadius: string;
  display: string;
  flexDirection: string;
  gap: string;
  justifyContent: string;
  alignItems: string;
};

export type ScssVariable = {
  name: string;
  value: string;
  file: string;
};

export type CssPropertySuggestion = ScssVariable & {
  insertValue?: string;
  directValue?: boolean;
};

export type CssRuleContext = {
  file: string;
  selector: string;
  viewport: CssViewport;
  resolvedBreakpoint: string | null;
  baseRules: CssProperty[];
  viewportRules: CssProperty[];
  hasBaseRule: boolean;
  hasViewportRule: boolean;
  background: CssBackground;
  grid: CssGrid;
};

export type PageCssTarget = {
  file: string;
  selector: string;
  targetKind: "existing" | "page" | "fallback" | string;
  exists: boolean;
  linked: boolean;
  href: string | null;
  templatePath: string | null;
  pageOwned: boolean;
  consumerFiles: string[];
  consumerTemplates: string[];
  reason: string;
};

export const CSS_INSPECTOR_CONTEXT_SCHEMA_VERSION = 4;

export type CssInspectorContextState = "existing" | "creation" | "ambiguous";

export type CssInspectorSourceCandidate = {
  file: string;
  ruleContext: CssRuleContext;
};

export type CssInspectorContextResolution = {
  schemaVersion: typeof CSS_INSPECTOR_CONTEXT_SCHEMA_VERSION;
  selectionRevision: number;
  selector: string;
  viewport: CssViewport;
  state: CssInspectorContextState;
  target: PageCssTarget | null;
  ruleContext: CssRuleContext | null;
  candidates: CssInspectorSourceCandidate[];
};
