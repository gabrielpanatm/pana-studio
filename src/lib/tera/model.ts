import type { DropPosition } from "$lib/ui/drag";
import type { SourceGraphNode, SourceNodeKind } from "$lib/types";

export type TeraConstructKind =
  | "extends"
  | "block"
  | "include"
  | "import"
  | "macro"
  | "for"
  | "if"
  | "set"
  | "teraVariable"
  | "teraComment"
  | "raw";

export type TeraPaletteFamily = "composition" | "logic" | "data" | "reuse" | "safe";

export type TeraPaletteItem = {
  id: string;
  kind: TeraConstructKind;
  family: TeraPaletteFamily;
  label: string;
  description: string;
  snippet: string;
  target?: string;
  name?: string;
  expression?: string;
  sourceNodeId?: string;
};

export type TeraPaletteGroup = {
  label: string;
  description: string;
  items: TeraPaletteItem[];
};

export type TeraDropRequest = {
  targetSelector: string;
  targetSessionId: string | null;
  targetSourceId: string | null;
  targetTemplateSourceId: string | null;
  targetTag: string;
  position: DropPosition;
  item: TeraPaletteItem;
};

export type TeraMoveRequest = {
  sourceId: string;
  targetSelector: string;
  targetSourceId: string | null;
  targetTemplateSourceId: string | null;
  targetTag: string;
  targetKind: "html" | "tera" | "preview";
  position: DropPosition;
};

export type TeraDropResolution =
  | {
      allowed: true;
      anchor: SourceGraphNode;
      position: DropPosition;
      item: TeraPaletteItem;
      snippet: string;
      label: string;
    }
  | {
      allowed: false;
      reason: string;
      anchor?: SourceGraphNode | null;
    };

export const teraConstructKinds: TeraConstructKind[] = [
  "extends",
  "block",
  "include",
  "import",
  "macro",
  "for",
  "if",
  "set",
  "teraVariable",
  "teraComment",
  "raw",
];

export function isTeraConstructKind(value: unknown): value is TeraConstructKind {
  return typeof value === "string" && (teraConstructKinds as string[]).includes(value);
}

export function isTeraSourceNodeKind(kind: SourceNodeKind) {
  return (teraConstructKinds as string[]).includes(kind);
}
