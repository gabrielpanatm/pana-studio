import type { SelectionMutationIdentity } from "$lib/types";

function normalizedAnchor(value: string | null | undefined): string | null {
  return value?.trim() || null;
}

export function sameCssSemanticSelection(
  left: SelectionMutationIdentity | null,
  right: SelectionMutationIdentity | null,
): boolean {
  if (!left || !right) return false;

  const leftAnchors = [
    normalizedAnchor(left.editorNodeId),
    normalizedAnchor(left.sourceNodeId),
    normalizedAnchor(left.renderInstanceId),
  ] as const;
  const rightAnchors = [
    normalizedAnchor(right.editorNodeId),
    normalizedAnchor(right.sourceNodeId),
    normalizedAnchor(right.renderInstanceId),
  ] as const;

  return (
    leftAnchors.some(Boolean)
    && rightAnchors.some(Boolean)
    && leftAnchors.every((anchor, index) => anchor === rightAnchors[index])
  );
}

export function cssSemanticSelectionKey(
  identity: SelectionMutationIdentity | null,
): string {
  if (!identity) return "";
  const anchors = [
    normalizedAnchor(identity.editorNodeId),
    normalizedAnchor(identity.sourceNodeId),
    normalizedAnchor(identity.renderInstanceId),
  ];
  if (!anchors.some(Boolean)) return "";
  return anchors.map((anchor) => anchor ?? "").join("\u0000");
}

export function cssInspectorSubjectKey(
  identity: SelectionMutationIdentity | null,
): string {
  if (!identity) return "";
  const sourceNodeId = normalizedAnchor(identity.sourceNodeId);
  if (sourceNodeId) return `source\u0000${sourceNodeId}`;
  const editorNodeId = normalizedAnchor(identity.editorNodeId);
  if (editorNodeId) return `editor\u0000${editorNodeId}`;
  const renderInstanceId = normalizedAnchor(identity.renderInstanceId);
  return renderInstanceId ? `render\u0000${renderInstanceId}` : "";
}
