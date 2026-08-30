import type { DropPosition } from "$lib/ui/drag";
import type { DynamicWidgetProperties } from "$lib/content-models/contracts";
import type { SourceGraphNode } from "$lib/source-graph/contracts";

export type TeraConstructKind =
  | "extends"
  | "block"
  | "include"
  | "componentDefinition"
  | "componentCall"
  | "for"
  | "if"
  | "set"
  | "teraVariable"
  | "teraComment"
  | "raw"
  | "dynamicWidget";

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
  dynamicWidget?: DynamicWidgetProperties;
};

export type TeraPaletteGroup = {
  label: string;
  description: string;
  items: TeraPaletteItem[];
};

export type TeraDropRequest = {
  targetSessionId: string | null;
  targetSourceId: string | null;
  targetTemplateSourceId: string | null;
  targetTag: string;
  position: DropPosition;
  item: TeraPaletteItem;
};

export type TeraDropResolution =
  | {
      allowed: true;
      anchor: SourceGraphNode;
      position: DropPosition;
      item: TeraPaletteItem;
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
  "componentDefinition",
  "componentCall",
  "for",
  "if",
  "set",
  "teraVariable",
  "teraComment",
  "raw",
  "dynamicWidget",
];

export function isTeraConstructKind(value: unknown): value is TeraConstructKind {
  return typeof value === "string" && (teraConstructKinds as string[]).includes(value);
}
