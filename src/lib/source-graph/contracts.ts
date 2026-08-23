import type { LocalizedDiagnostic } from "$lib/contracts/localized-diagnostic";

export type SourcePageKind = "page" | "section" | "home";

type SourceStyleScope = "global" | "page" | "partial" | "other";

export type SourceOrigin = "local" | "theme";

export type SourceNodeKind =
  | "page"
  | "template"
  | "partial"
  | "style"
  | "script"
  | "asset"
  | "dataFile"
  | "dataTable"
  | "dataArray"
  | "dataValue"
  | "dataComment"
  | "configFile"
  | "html"
  | "blockMarker"
  | "macroCall"
  | "functionCall"
  | "shortcode"
  | "extends"
  | "block"
  | "include"
  | "import"
  | "macro"
  | "for"
  | "if"
  | "elif"
  | "else"
  | "set"
  | "setGlobal"
  | "filter"
  | "break"
  | "continue"
  | "super"
  | "teraVariable"
  | "teraComment"
  | "raw"
  | "tera";

export type SourceRelationKind =
  | "pageTemplate"
  | "sectionPageTemplate"
  | "getsPage"
  | "getsSection"
  | "internalContentLink"
  | "assetUrl"
  | "assetHash"
  | "assetReference"
  | "dataLoad"
  | "dataFileLoad"
  | "contentDataLoad"
  | "imageMetadata"
  | "imageResize"
  | "extends"
  | "includes"
  | "imports"
  | "definesBlock"
  | "overridesBlock"
  | "usesStyle"
  | "usesScript";

export type SourceDiagnosticSeverity = "warning" | "error";

export type SourceCapabilityReason =
  | "structuredConfig"
  | "structuredDataNode"
  | "styleFile"
  | "teraTemplateFile"
  | "teraExtends"
  | "teraBlock"
  | "teraInclude"
  | "teraImport"
  | "teraMacro"
  | "teraFor"
  | "teraIf"
  | "teraElif"
  | "teraElse"
  | "teraSet"
  | "teraSetGlobal"
  | "teraFilter"
  | "teraBreak"
  | "teraContinue"
  | "teraSuper"
  | "teraVariable"
  | "teraMacroCall"
  | "teraFunctionCall"
  | "zolaShortcode"
  | "nativeBlockMarker"
  | "teraComment"
  | "teraRaw"
  | "teraSyntax"
  | "htmlInTeraLoop"
  | "htmlInTeraCondition"
  | "htmlInTeraMacro"
  | "htmlInTeraLocalScope"
  | "htmlInTeraRaw"
  | "markdownPage"
  | "markdownShortcode"
  | "staticJavaScript"
  | "staticAsset"
  | "dataOutputReadOnly"
  | "dataThemeReadOnly"
  | "dataFormatVisualUnsupported"
  | "markdownRenderedBoundary"
  | "markdownSourceUnresolved";

export type SourceCapabilities = {
  canOpenInCode: boolean;
  canEditVisual: boolean;
  canEditText: boolean;
  canEditAttributes: boolean;
  canMove: boolean;
  canExtractPartial: boolean;
  reasonCode: SourceCapabilityReason | null;
};

export type SourceRange = {
  start: number;
  end: number;
  line: number;
  column: number;
  endLine: number;
  endColumn: number;
};

export type SourceEditLocation = {
  file: string;
  line: number;
  column?: number;
};

export type ProjectSourceEditLocation = {
  file: string;
  line: number;
  column: number;
};

export type SourceEditTarget = {
  sourceId: string;
  file: string;
  location: SourceEditLocation;
  range: SourceRange;
  kind: SourceNodeKind;
  label: string;
  capabilities: SourceCapabilities;
};

export type SourceGraphNode = {
  id: string;
  kind: SourceNodeKind;
  file: string;
  origin: SourceOrigin;
  themeName: string | null;
  label: string;
  range: SourceRange | null;
  parent: string | null;
  children: string[];
  capabilities: SourceCapabilities;
};

export type SourceGraphRelation = {
  id: string;
  from: string;
  to: string;
  kind: SourceRelationKind;
  label: string;
};

export type SourceGraphDiagnostic = {
  severity: SourceDiagnosticSeverity;
  diagnostic: LocalizedDiagnostic;
  file: string | null;
  range: SourceRange | null;
};

export type SourceGraphPage = {
  id: string;
  file: string;
  title: string;
  url: string;
  pageKind: SourcePageKind;
  frontmatterTemplate: string | null;
  frontmatterPageTemplate: string | null;
  resolvedTemplate: string | null;
  contentNodeId: string;
  templateNodeId: string | null;
  pageTemplateNodeId: string | null;
  frontmatterFormat: SourceDataFormat | null;
  frontmatterParseError: string | null;
  frontmatterNodes: SourceDataNode[];
  taxonomies: Record<string, string[]>;
  shortcodeParseError: string | null;
  shortcodes: ZolaShortcodeInvocation[];
};

type ZolaShortcodeRange = {
  start: number;
  end: number;
};

type ZolaShortcodeValue =
  | { kind: "string"; value: string }
  | { kind: "integer"; value: number }
  | { kind: "float"; value: number }
  | { kind: "boolean"; value: boolean }
  | { kind: "array"; value: ZolaShortcodeValue[] };

type ZolaShortcodeInvocation = {
  name: string;
  arguments: Record<string, ZolaShortcodeValue>;
  range: ZolaShortcodeRange;
  callRange: ZolaShortcodeRange;
  bodyRange: ZolaShortcodeRange | null;
  nth: number;
  inner: ZolaShortcodeInvocation[];
  sourceNodeId: string | null;
};

export type SourceGraphTemplate = {
  id: string;
  file: string;
  name: string;
  origin: SourceOrigin;
  themeName: string | null;
  isPartial: boolean;
  extends: string | null;
  includes: string[];
  includeGroups: Array<{
    targets: string[];
    ignoreMissing: boolean;
  }>;
  imports: string[];
  getPages: string[];
  getSections: string[];
  internalLinks: string[];
  assetUrls: string[];
  assetHashes: string[];
  literalAssetReferences: string[];
  assetReferenceEligible: number;
  assetReferenceUnanalysable: number;
  dataLoads: string[];
  imageMetadata: string[];
  imageResizes: string[];
  blocks: string[];
  macros: string[];
  semantics: TeraSemanticDocument | null;
  markdownProjections: MarkdownProjection[];
  nodeId: string;
};

type MarkdownProjectionKind = "body" | "summary" | "filter" | "toc" | "shortcode";

type MarkdownSourceBindingKind =
  | "currentPage"
  | "currentSection"
  | "staticPage"
  | "staticSection"
  | "runtimePage"
  | "runtimeSection"
  | "shortcodeInvocation"
  | "unresolved";

export type MarkdownProjection = {
  id: string;
  kind: MarkdownProjectionKind;
  templateSourceNodeId: string;
  templateFile: string;
  templateRange: SourceRange | null;
  bindingKind: MarkdownSourceBindingKind;
  staticContentPath: string | null;
  runtimeSourceExpression: string | null;
};

type TeraSemanticDocument = {
  nodes: TeraSemanticNode[];
};

type TeraSemanticNode =
  | { kind: "super" }
  | { kind: "text"; value: string }
  | { kind: "variable"; expression: TeraSemanticExpression }
  | {
      kind: "macroDefinition";
      name: string;
      arguments: Record<string, TeraSemanticExpression | null>;
      body: TeraSemanticNode[];
    }
  | { kind: "extends"; template: string }
  | { kind: "include"; templates: string[]; ignoreMissing: boolean }
  | { kind: "import"; template: string; namespace: string }
  | { kind: "set"; key: string; global: boolean; value: TeraSemanticExpression }
  | { kind: "raw"; value: string }
  | { kind: "filterSection"; filter: TeraSemanticCall; body: TeraSemanticNode[] }
  | { kind: "block"; name: string; body: TeraSemanticNode[] }
  | {
      kind: "for";
      key: string | null;
      value: string;
      container: TeraSemanticExpression;
      body: TeraSemanticNode[];
      emptyBody: TeraSemanticNode[] | null;
    }
  | {
      kind: "if";
      branches: Array<{ condition: TeraSemanticExpression; body: TeraSemanticNode[] }>;
      otherwise: TeraSemanticNode[] | null;
    }
  | { kind: "break" }
  | { kind: "continue" }
  | { kind: "comment"; value: string };

type TeraSemanticExpression = {
  value: TeraSemanticValue;
  negated: boolean;
  filters: TeraSemanticCall[];
};

type TeraSemanticCall = {
  namespace: string | null;
  name: string;
  arguments: Record<string, TeraSemanticExpression>;
};

type TeraSemanticValue =
  | { kind: "string"; value: string }
  | { kind: "integer"; value: number }
  | { kind: "float"; value: number }
  | { kind: "boolean"; value: boolean }
  | { kind: "identifier"; value: string }
  | {
      kind: "math";
      value: {
        operator: string;
        left: TeraSemanticExpression;
        right: TeraSemanticExpression;
      };
    }
  | {
      kind: "logic";
      value: {
        operator: string;
        left: TeraSemanticExpression;
        right: TeraSemanticExpression;
      };
    }
  | {
      kind: "test";
      value: {
        identifier: string;
        name: string;
        negated: boolean;
        arguments: TeraSemanticExpression[];
      };
    }
  | { kind: "macroCall"; value: TeraSemanticCall }
  | { kind: "functionCall"; value: TeraSemanticCall }
  | { kind: "array"; value: TeraSemanticExpression[] }
  | { kind: "stringConcat"; value: TeraSemanticValue[] }
  | {
      kind: "in";
      value: {
        negated: boolean;
        needle: TeraSemanticExpression;
        haystack: TeraSemanticExpression;
      };
    };

export type SourceGraphAsset = {
  id: string;
  file: string;
  origin: SourceOrigin;
  themeName: string | null;
  logicalPath: string;
  nodeId: string;
};

export type SourceGraphScript = {
  id: string;
  file: string;
  origin: SourceOrigin;
  themeName: string | null;
  logicalPath: string;
  nodeId: string;
};

export type SourceGraphDataFile = {
  id: string;
  file: string;
  origin: SourceOrigin;
  themeName: string | null;
  logicalPath: string;
  loadPaths: string[];
  location: SourceDataLocation;
  nodeId: string;
  format: SourceDataFormat;
  parseError: string | null;
  nodes: SourceDataNode[];
  capabilities: SourceCapabilities;
};

type SourceDataLocation =
  | "date"
  | "project"
  | "static"
  | "content"
  | "output"
  | "theme";

type SourceStructuredDocumentKind = "zolaConfig" | "themeConfig";

export type SourceStructuredDocument = {
  id: string;
  file: string;
  kind: SourceStructuredDocumentKind;
  nodeId: string;
  parseError: string | null;
  nodes: SourceDataNode[];
};

type SourceDataFormat = "toml" | "json" | "yaml" | "csv" | "bibtex" | "xml" | "unknown";

type SourceDataNodeKind =
  | "document"
  | "table"
  | "arrayOfTables"
  | "tableElement"
  | "array"
  | "arrayElement"
  | "inlineTable"
  | "value"
  | "comment"
  | "opaque";

type SourceDataValueKind =
  | "string"
  | "integer"
  | "float"
  | "boolean"
  | "datetime"
  | "array"
  | "inlineTable"
  | "table"
  | "arrayOfTables"
  | "null"
  | "unknown";

type SourceDataPathSegment =
  | { kind: "key"; value: string }
  | { kind: "index"; value: number };

export type SourceDataNode = {
  id: string;
  kind: SourceDataNodeKind;
  path: SourceDataPathSegment[];
  key: string | null;
  valueKind: SourceDataValueKind | null;
  valuePreview: string | null;
  range: SourceRange | null;
  keyRange: SourceRange | null;
  parentId: string | null;
  children: string[];
};

export type SourceGraphStyle = {
  id: string;
  file: string;
  origin: SourceOrigin;
  themeName: string | null;
  scope: SourceStyleScope;
  nodeId: string;
};

export type ComponentDefinitionKind =
  | "templateFile"
  | "partial"
  | "macroLibrary"
  | "macro"
  | "shortcode"
  | "templateBlock"
  | "inlineRepeat"
  | "inlineConditional"
  | "inlineTransform";

type ComponentInvocationKind =
  | "include"
  | "macroCall"
  | "shortcode"
  | "repeat"
  | "conditional"
  | "transform";

type ComponentOrigin = "project" | "theme";

type ComponentResolutionStatus =
  | "resolved"
  | "fallbackResolved"
  | "ambiguous"
  | "dynamic"
  | "external"
  | "unresolved";

type ComponentDependencyKind =
  | "template"
  | "data"
  | "content"
  | "style"
  | "script"
  | "asset"
  | "context"
  | "runtime";

type ComponentParameter = {
  name: string;
  required: boolean;
  defaultValue: TeraSemanticExpression | null;
};

type ComponentArgument = {
  name: string;
  expression: TeraSemanticExpression;
};

type ComponentDataBinding = {
  name: string;
  path: string;
  producer: string;
  sourceNodeId: string | null;
};

type ComponentDependency = {
  kind: ComponentDependencyKind;
  reference: string;
  sourceNodeId: string | null;
  targetNodeId: string | null;
  resolved: boolean;
};

type ComponentCapabilities = {
  canCreate: boolean;
  canEdit: boolean;
  canDuplicate: boolean;
  canMove: boolean;
  canRename: boolean;
  canExtract: boolean;
  canDelete: boolean;
  reasonDiagnostic: LocalizedDiagnostic | null;
};

type ComponentDiagnostic = {
  code: string;
  diagnostic: LocalizedDiagnostic;
  severity: SourceDiagnosticSeverity;
  file: string | null;
  sourceNodeId: string | null;
};

export type ComponentDefinition = {
  id: string;
  kind: ComponentDefinitionKind;
  name: string;
  displayName: string;
  origin: ComponentOrigin;
  themeName: string | null;
  file: string | null;
  templateName: string | null;
  sourceNodeId: string | null;
  ownerDefinitionId: string | null;
  symbol: string | null;
  parameters: ComponentParameter[];
  contextDependencies: string[];
  dataBindings: ComponentDataBinding[];
  dependencies: ComponentDependency[];
  consumerInvocationIds: string[];
  shadowedBy: string | null;
  active: boolean;
  capabilities: ComponentCapabilities;
  diagnostics: ComponentDiagnostic[];
};

type ComponentInvocation = {
  id: string;
  kind: ComponentInvocationKind;
  name: string;
  file: string;
  sourceNodeId: string | null;
  ownerDefinitionId: string | null;
  parentInvocationId: string | null;
  targetReference: string;
  resolvedDefinitionIds: string[];
  fallbackReferences: string[];
  arguments: ComponentArgument[];
  contextDependencies: string[];
  dataBindings: ComponentDataBinding[];
  status: ComponentResolutionStatus;
  diagnostics: ComponentDiagnostic[];
};

type RenderedComponentInstance = {
  id: string;
  definitionId: string | null;
  invocationId: string | null;
  renderInstanceId: string;
  route: string;
  sourceNodeId: string | null;
  parentInstanceId: string | null;
  templateStack: string[];
  scopePath: string[];
  bindingKey: string | null;
  bindingPath: string | null;
};

export type ComponentGraph = {
  schemaVersion: number;
  definitions: ComponentDefinition[];
  invocations: ComponentInvocation[];
  renderedInstances: RenderedComponentInstance[];
  diagnostics: ComponentDiagnostic[];
};
