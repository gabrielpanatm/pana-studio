import type { SourceNodeKind } from "$lib/types";

const templateEditGateKinds = new Set<SourceNodeKind>([
  "template",
  "partial",
  "block",
  "include",
  "macro",
  "for",
  "if",
  "with",
]);

export function canRequestTemplateEditGateKind(kind: SourceNodeKind | null | undefined) {
  return Boolean(kind && templateEditGateKinds.has(kind));
}

export function templateEditGateReason(kind: SourceNodeKind | null | undefined, hasPreviewSelector: boolean) {
  if (!hasPreviewSelector) return "Nu există o zonă randată asociată";
  return canRequestTemplateEditGateKind(kind)
    ? "Deblochează HTML-ul randat în acest gate"
    : "Acest nod Tera se editează din cod sau printr-o acțiune dedicată.";
}

export function templateEditGateSelectionStatus(
  canSelectHtml: boolean | null | undefined,
  variant: "element" | "zone" | "node" = "element",
) {
  if (canSelectHtml === false) {
    return variant === "node"
      ? "Nod Tera selectat. Acest scope se editează din cod sau printr-o acțiune dedicată."
      : "Zona este protejată de un gate Tera code-only. Deschide sursa pentru editare.";
  }
  if (variant === "node") return "Nod Tera selectat. Editează deblochează HTML-ul randat; Cod deschide sursa.";
  return variant === "zone"
    ? "Zona este protejată de un gate Tera. Alege Editează ca să deblochezi HTML-ul randat."
    : "Elementul este blocat de un gate Tera. Alege Editează ca să deblochezi HTML-ul randat.";
}
