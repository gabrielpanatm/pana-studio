import type { EditableAttributes } from "$lib/canvas/contracts";
import type { ProjectHtmlAttributeMutation } from "$lib/preview/contracts";

function normalizedAttributeDraft(attributes: Readonly<EditableAttributes>) {
  return Object.fromEntries(
    Object.entries(attributes)
      .filter(([name]) => !name.toLowerCase().startsWith("data-pana-"))
      .map(([name, value]) => [name, value] as const)
      .sort(([left], [right]) => left.localeCompare(right)),
  );
}

export function attributeDraftMatches(
  current: Readonly<EditableAttributes>,
  submitted: Readonly<EditableAttributes>,
) {
  return JSON.stringify(normalizedAttributeDraft(current))
    === JSON.stringify(normalizedAttributeDraft(submitted));
}

export function attributeDraftToken(attributes: Readonly<EditableAttributes>) {
  return JSON.stringify(normalizedAttributeDraft(attributes));
}

export function attributeMutationsFromRecord(
  attributes: Record<string, string | null>,
): ProjectHtmlAttributeMutation[] {
  return Object.entries(attributes).map(([name, value]) => value === null
    ? { kind: "removeAttribute", name }
    : { kind: "setAttribute", name, value });
}

export function htmlAttributeRecordForKernel(
  attributes: Readonly<EditableAttributes>,
  targetAttributes: Readonly<Record<string, string>> = {},
  zolaImageManaged = false,
): Record<string, string | null> {
  const next: Record<string, string | null> = Object.fromEntries(
    Object.entries(attributes)
      .filter(([name]) => !name.toLowerCase().startsWith("data-pana-"))
      .map(([name, value]) => [name, value]),
  );
  if (zolaImageManaged) {
    delete next.src;
    delete next.width;
    delete next.height;
  }
  for (const name of Object.keys(targetAttributes)) {
    if (
      !(name in attributes)
      && !name.toLowerCase().startsWith("data-pana-")
      && !["class", "style"].includes(name)
      && !(zolaImageManaged && ["src", "width", "height"].includes(name.toLowerCase()))
    ) {
      next[name] = null;
    }
  }
  return next;
}

const BATCH_COMMON_HTML_ATTRIBUTES = new Set([
  "title",
  "lang",
  "dir",
  "tabindex",
  "hidden",
  "inert",
  "contenteditable",
  "draggable",
  "spellcheck",
  "translate",
  "role",
]);

function isBatchCommonHtmlAttribute(name: string) {
  const normalized = name.trim().toLowerCase();
  return BATCH_COMMON_HTML_ATTRIBUTES.has(normalized)
    || normalized.startsWith("aria-")
    || (normalized.startsWith("data-") && !normalized.startsWith("data-pana-"));
}

export function batchCommonAttributeMutations(
  attributes: Readonly<EditableAttributes>,
  primaryAttributes: Readonly<Record<string, string>>,
): ProjectHtmlAttributeMutation[] {
  const next = htmlAttributeRecordForKernel(attributes, primaryAttributes, false);
  const names = new Set([...Object.keys(next), ...Object.keys(primaryAttributes)]);
  const mutations: ProjectHtmlAttributeMutation[] = [];
  for (const name of names) {
    const normalized = name.trim().toLowerCase();
    if (!isBatchCommonHtmlAttribute(normalized)) continue;
    const nextValue = next[name] ?? next[normalized] ?? null;
    const previousValue = primaryAttributes[name] ?? primaryAttributes[normalized] ?? null;
    if (nextValue === previousValue) continue;
    mutations.push(nextValue === null
      ? { kind: "removeAttribute", name: normalized }
      : { kind: "setAttribute", name: normalized, value: nextValue });
  }
  return mutations;
}
