import schemaDocument from "./editor-schema.json" with { type: "json" };

export type HtmlPreviewMode = "live" | "sourceOnly" | "inert" | "blocked";
export type HtmlAttributeEmptyPolicy = "preserve" | "remove";
export type HtmlAttributeSemantic =
  | "ariaBoolean"
  | "ariaToken"
  | "booleanOrString"
  | "booleanPresence"
  | "dateTime"
  | "enumerated"
  | "enumeratedOrString"
  | "idReference"
  | "idReferenceList"
  | "integer"
  | "mediaQuery"
  | "nonNegativeInteger"
  | "number"
  | "numberList"
  | "numberOrAny"
  | "numberOrDate"
  | "positiveInteger"
  | "script"
  | "string"
  | "stringAllowEmpty"
  | "token"
  | "tokenList"
  | "tokenListAllowEmpty"
  | "url"
  | "urlList";

export type HtmlTagCapability = {
  group: string;
  family: string;
  sourceEditable: boolean;
  liveProjectable: boolean;
  previewMode: HtmlPreviewMode;
  acceptsChildren: boolean;
  attributeEditor: "complete" | "blocked";
  reason?: string;
};

export type HtmlAttributeDefinition = {
  semantic: HtmlAttributeSemantic;
  emptyPolicy: HtmlAttributeEmptyPolicy;
  scope?: "global" | "accessibility";
  elements?: string[];
  values?: string[];
  implicitValue?: string;
  sourceEditable: boolean;
  liveProjectable: boolean;
  previewMode?: HtmlPreviewMode;
  reason?: string;
};

type HtmlEditorSchema = {
  schemaVersion: number;
  designSafe: {
    forbiddenElements: string[];
    forbiddenAttributes: string[];
    forbiddenAttributePrefixes: string[];
    activeSchemes: string[];
    forbiddenMetaHttpEquiv: string[];
  };
  paletteGroups: Array<{ label: string; tags: string[] }>;
  tags: Record<string, HtmlTagCapability>;
  attributes: Record<string, HtmlAttributeDefinition>;
  dynamicAttributes: Record<string, HtmlAttributeDefinition>;
};

export const htmlEditorSchema = schemaDocument as HtmlEditorSchema;

export type HtmlTagOption = {
  value: string;
  label: string;
  group: string;
};

export function htmlTagCapability(tag: string): HtmlTagCapability | null {
  return htmlEditorSchema.tags[tag.trim().toLowerCase()] ?? null;
}

export function htmlAttributeDefinition(name: string): HtmlAttributeDefinition | null {
  const normalized = name.trim().toLowerCase();
  const fixed = htmlEditorSchema.attributes[normalized];
  if (fixed) return fixed;
  if (normalized.startsWith("data-") && normalized.length > 5) {
    return htmlEditorSchema.dynamicAttributes["data-*"] ?? null;
  }
  if (normalized.startsWith("aria-") && normalized.length > 5) {
    return htmlEditorSchema.dynamicAttributes["aria-*"] ?? null;
  }
  if (normalized.startsWith("on") && normalized.length > 2) {
    return htmlEditorSchema.dynamicAttributes["on*"] ?? null;
  }
  return null;
}

export function htmlAttributesForElement(tag: string): string[] {
  const normalizedTag = tag.trim().toLowerCase();
  return Object.entries(htmlEditorSchema.attributes)
    .filter(([, definition]) => definition.elements?.includes(normalizedTag))
    .map(([name]) => name);
}

export function htmlGlobalAttributeNames(): string[] {
  return Object.entries(htmlEditorSchema.attributes)
    .filter(([, definition]) => definition.scope === "global")
    .map(([name]) => name);
}

export function htmlAccessibilityAttributeNames(): string[] {
  return Object.entries(htmlEditorSchema.attributes)
    .filter(([, definition]) => definition.scope === "accessibility")
    .map(([name]) => name);
}

export function htmlTagTransitionOptions(currentTag: string): HtmlTagOption[] {
  const normalizedCurrent = currentTag.trim().toLowerCase();
  const current = htmlTagCapability(normalizedCurrent);
  if (!current || !current.sourceEditable || !current.liveProjectable || !current.acceptsChildren) {
    return [];
  }

  return Object.entries(htmlEditorSchema.tags)
    .filter(([, candidate]) => (
      candidate.sourceEditable
      && candidate.liveProjectable
      && candidate.acceptsChildren
      && candidate.previewMode === "live"
      && candidate.family === current.family
    ))
    .map(([tag, candidate]) => ({ value: tag, label: tag, group: candidate.group }));
}

export function htmlAttributePreviewMode(name: string, tag: string): HtmlPreviewMode {
  const tagMode = htmlTagCapability(tag)?.previewMode ?? "blocked";
  if (tagMode !== "live") return tagMode;
  const definition = htmlAttributeDefinition(name);
  if (!definition || !definition.sourceEditable) return "blocked";
  return definition.previewMode ?? (definition.liveProjectable ? "live" : "sourceOnly");
}

export function htmlAttributeAppliesToTag(name: string, tag: string): boolean {
  const definition = htmlAttributeDefinition(name);
  if (!definition) return name.startsWith("data-") || name.startsWith("aria-");
  if (!definition.elements?.length) return true;
  return definition.elements.includes(tag.trim().toLowerCase());
}

export function htmlAttributeValueError(name: string, value: string): string | null {
  const definition = htmlAttributeDefinition(name);
  if (!definition) return null;
  if (value === "" && definition.emptyPolicy === "remove") return null;

  const normalized = value.trim().toLowerCase();
  if (definition.values?.length && definition.semantic !== "enumeratedOrString") {
    if (!definition.values.includes(normalized)) {
      return `Valoarea trebuie să fie una dintre: ${definition.values.join(", ")}.`;
    }
  }
  if (definition.semantic === "integer" && !/^-?\d+$/.test(value.trim())) {
    return "Valoarea trebuie să fie un număr întreg.";
  }
  if (definition.semantic === "nonNegativeInteger" && !/^\d+$/.test(value.trim())) {
    return "Valoarea trebuie să fie un număr întreg pozitiv sau zero.";
  }
  if (definition.semantic === "positiveInteger" && !/^[1-9]\d*$/.test(value.trim())) {
    return "Valoarea trebuie să fie un număr întreg mai mare ca zero.";
  }
  if (definition.semantic === "number" && !Number.isFinite(Number(value.trim()))) {
    return "Valoarea trebuie să fie numerică.";
  }
  if (definition.semantic === "numberOrAny" && normalized !== "any" && !Number.isFinite(Number(value.trim()))) {
    return "Valoarea trebuie să fie numerică sau «any».";
  }
  if (definition.semantic === "ariaBoolean" && normalized !== "true" && normalized !== "false") {
    return "ARIA boolean acceptă explicit doar «true» sau «false».";
  }
  return null;
}
