import { htmlEditorSchema } from "./editor-schema";

export type HtmlTagGroup = { label: string; tags: string[] };

export const htmlTagGroups: HtmlTagGroup[] = htmlEditorSchema.paletteGroups.map((group) => ({
  label: group.label,
  tags: [...group.tags],
}));
