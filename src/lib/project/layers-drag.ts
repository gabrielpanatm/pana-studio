import type { DropPosition } from "$lib/ui/drag";
import type { PageSection } from "$lib/types";

export type LayerMoveRequest = {
  sourceSelector: string;
  targetSelector: string;
  sourceSessionId?: string | null;
  sourceSourceId?: string | null;
  sourceTemplateSourceId?: string | null;
  targetSessionId?: string | null;
  targetSourceId?: string | null;
  targetTemplateSourceId?: string | null;
  targetKind?: "html" | "empty-tera-slot";
  position: DropPosition;
};

export type LayerDropValidation = {
  allowed: boolean;
  reason?: string;
};

const layerContainerTags = new Set([
  "main",
  "section",
  "article",
  "header",
  "footer",
  "nav",
  "aside",
  "div",
  "ul",
  "ol",
  "li",
  "form",
  "fieldset",
]);

export function canLayerReceiveChildren(tag: string) {
  return layerContainerTags.has(tag.toLowerCase());
}

export function validateLayerDrop(
  source: Pick<PageSection, "selector" | "tag" | "sourceLocation"> | null | undefined,
  target: Pick<PageSection, "selector" | "tag" | "sourceLocation"> | null | undefined,
  position: DropPosition,
): LayerDropValidation {
  const structural = validateLayerStructureDrop(source, target, position);
  if (!structural.allowed) return structural;

  const sourceTpl = source?.sourceLocation ?? null;
  const targetTpl = target?.sourceLocation ?? null;
  if (!sourceTpl || !targetTpl) {
    return { allowed: false, reason: "Elementul nu are sursă template editabilă." };
  }
  if (sourceTpl.file !== targetTpl.file) {
    return { allowed: false, reason: "Mutarea între template-uri diferite nu este activă încă." };
  }
  if (sourceTpl.line === targetTpl.line) {
    return { allowed: false, reason: "Sursa și destinația sunt pe aceeași linie template." };
  }

  return { allowed: true };
}

export function validateLayerStructureDrop(
  source: Pick<PageSection, "selector" | "tag"> | null | undefined,
  target: Pick<PageSection, "selector" | "tag"> | null | undefined,
  position: DropPosition,
): LayerDropValidation {
  if (!source || !target) {
    return { allowed: false, reason: "Nu am găsit sursa sau destinația." };
  }
  if (source.selector === target.selector) {
    return { allowed: false, reason: "Elementul este deja pe această țintă." };
  }
  if (target.selector.startsWith(`${source.selector} > `)) {
    return { allowed: false, reason: "Nu poate fi mutat în propriul copil." };
  }
  if (position === "inside" && !canLayerReceiveChildren(target.tag)) {
    return { allowed: false, reason: `<${target.tag}> nu este container pentru copii.` };
  }

  return { allowed: true };
}
