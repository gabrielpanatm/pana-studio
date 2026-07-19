export const htmlVoidTags = new Set([
  "area",
  "base",
  "br",
  "col",
  "embed",
  "hr",
  "img",
  "input",
  "link",
  "meta",
  "param",
  "source",
  "track",
  "wbr",
]);

export function normalizeClassTokens(value: string) {
  return Array.from(new Set(value.split(/\s+/).map((token) => token.trim()).filter(Boolean)));
}

export type InsertPosition = "before" | "after" | "child";
export type MovePosition = "before" | "after" | "inside";

export function canElementAcceptChildren(tag: string, htmlVoidTags: ReadonlySet<string>) {
  return !htmlVoidTags.has(tag);
}
