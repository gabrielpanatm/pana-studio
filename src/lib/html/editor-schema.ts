import schemaDocument from "./editor-schema.json" with { type: "json" };
import { t } from "$lib/i18n/runtime.svelte";
import type { MessageId } from "$lib/i18n/generated/catalog";

export type HtmlPreviewMode = "live" | "sourceOnly" | "inert" | "blocked";
export type HtmlAttributeEmptyPolicy = "preserve" | "remove";
export type HtmlSchemaGroup =
  | "document"
  | "structure"
  | "text"
  | "lists"
  | "media"
  | "actions"
  | "forms"
  | "tables"
  | "interactive"
  | "indicators";
export type HtmlSchemaReason =
  | "iframeInert"
  | "navigationDisabled"
  | "downloadDisabled"
  | "formSubmissionDisabled"
  | "activeDocumentInjection";
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

type RawHtmlTagCapability = Omit<HtmlTagCapability, "group" | "reason"> & {
  group: HtmlSchemaGroup;
  reasonCode?: HtmlSchemaReason;
};

type RawHtmlAttributeDefinition = Omit<HtmlAttributeDefinition, "reason"> & {
  reasonCode?: HtmlSchemaReason;
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
  paletteGroups: Array<{ label: HtmlSchemaGroup; tags: string[] }>;
  tags: Record<string, RawHtmlTagCapability>;
  attributes: Record<string, RawHtmlAttributeDefinition>;
  dynamicAttributes: Record<string, RawHtmlAttributeDefinition>;
};

export const htmlEditorSchema = schemaDocument as HtmlEditorSchema;

const groupMessageIds: Record<HtmlSchemaGroup, MessageId> = {
  document: "inspector-schema-group-document",
  structure: "inspector-schema-group-structure",
  text: "inspector-schema-group-text",
  lists: "inspector-schema-group-lists",
  media: "inspector-schema-group-media",
  actions: "inspector-schema-group-actions",
  forms: "inspector-schema-group-forms",
  tables: "inspector-schema-group-tables",
  interactive: "inspector-schema-group-interactive",
  indicators: "inspector-schema-group-indicators",
};

const reasonMessageIds: Record<HtmlSchemaReason, MessageId> = {
  iframeInert: "inspector-schema-reason-iframe",
  navigationDisabled: "inspector-schema-reason-navigation",
  downloadDisabled: "inspector-schema-reason-download",
  formSubmissionDisabled: "inspector-schema-reason-form-submit",
  activeDocumentInjection: "inspector-schema-reason-srcdoc",
};

export function htmlSchemaGroupLabel(group: HtmlSchemaGroup): string {
  return t(groupMessageIds[group]);
}

function localizedReason(reasonCode?: HtmlSchemaReason): string | undefined {
  return reasonCode ? t(reasonMessageIds[reasonCode]) : undefined;
}

export type HtmlTagOption = {
  value: string;
  label: string;
  group: string;
};

export function htmlTagCapability(tag: string): HtmlTagCapability | null {
  const capability = htmlEditorSchema.tags[tag.trim().toLowerCase()] ?? null;
  return capability
    ? {
        ...capability,
        group: htmlSchemaGroupLabel(capability.group),
        reason: localizedReason(capability.reasonCode),
      }
    : null;
}

export function htmlTagAcceptsChildren(tag: string): boolean {
  return htmlEditorSchema.tags[tag.trim().toLowerCase()]?.acceptsChildren === true;
}

export function htmlAttributeDefinition(name: string): HtmlAttributeDefinition | null {
  const normalized = name.trim().toLowerCase();
  const fixed = htmlEditorSchema.attributes[normalized];
  if (fixed) return { ...fixed, reason: localizedReason(fixed.reasonCode) };
  if (normalized.startsWith("data-") && normalized.length > 5) {
    const definition = htmlEditorSchema.dynamicAttributes["data-*"] ?? null;
    return definition ? { ...definition, reason: localizedReason(definition.reasonCode) } : null;
  }
  if (normalized.startsWith("aria-") && normalized.length > 5) {
    const definition = htmlEditorSchema.dynamicAttributes["aria-*"] ?? null;
    return definition ? { ...definition, reason: localizedReason(definition.reasonCode) } : null;
  }
  if (normalized.startsWith("on") && normalized.length > 2) {
    const definition = htmlEditorSchema.dynamicAttributes["on*"] ?? null;
    return definition ? { ...definition, reason: localizedReason(definition.reasonCode) } : null;
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
    .map(([tag, candidate]) => ({
      value: tag,
      label: tag,
      group: htmlSchemaGroupLabel(candidate.group),
    }));
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
      return t("inspector-schema-value-enum", { values: definition.values.join(", ") });
    }
  }
  if (definition.semantic === "integer" && !/^-?\d+$/.test(value.trim())) {
    return t("inspector-schema-value-integer");
  }
  if (definition.semantic === "nonNegativeInteger" && !/^\d+$/.test(value.trim())) {
    return t("inspector-schema-value-non-negative");
  }
  if (definition.semantic === "positiveInteger" && !/^[1-9]\d*$/.test(value.trim())) {
    return t("inspector-schema-value-positive");
  }
  if (definition.semantic === "number" && !Number.isFinite(Number(value.trim()))) {
    return t("inspector-schema-value-number");
  }
  if (definition.semantic === "numberOrAny" && normalized !== "any" && !Number.isFinite(Number(value.trim()))) {
    return t("inspector-schema-value-number-any");
  }
  if (definition.semantic === "ariaBoolean" && normalized !== "true" && normalized !== "false") {
    return t("inspector-schema-value-aria-boolean");
  }
  return null;
}
