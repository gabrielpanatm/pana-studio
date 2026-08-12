import type { SelectionMutationIdentity } from "$lib/types";

function normalizedAnchor(value: string | null | undefined): string | null {
  return value?.trim() || null;
}

export function sameCssSemanticSelection(
  left: SelectionMutationIdentity | null,
  right: SelectionMutationIdentity | null,
): boolean {
  if (!left || !right) return false;

  const leftKey = cssSemanticSelectionKey(left);
  return Boolean(leftKey && leftKey === cssSemanticSelectionKey(right));
}

export function cssSemanticSelectionKey(
  identity: SelectionMutationIdentity | null,
): string {
  if (!identity) return "";
  if (!identity.primaryMemberId || identity.members.length === 0) return "";
  return [
    identity.primaryMemberId,
    ...identity.members.flatMap((member) => [
      member.memberId,
      normalizedAnchor(member.editorNodeId) ?? "",
      normalizedAnchor(member.sourceNodeId) ?? "",
      normalizedAnchor(member.renderInstanceId) ?? "",
    ]),
  ].join("\u0000");
}

export function cssInspectorSubjectKey(
  identity: SelectionMutationIdentity | null,
): string {
  if (!identity) return "";
  const primary = identity.members.find(
    (member) => member.memberId === identity.primaryMemberId,
  );
  if (!primary) return "";
  const sourceNodeId = normalizedAnchor(primary.sourceNodeId);
  if (sourceNodeId) return `source\u0000${sourceNodeId}`;
  const editorNodeId = normalizedAnchor(primary.editorNodeId);
  if (editorNodeId) return `editor\u0000${editorNodeId}`;
  const renderInstanceId = normalizedAnchor(primary.renderInstanceId);
  return renderInstanceId ? `render\u0000${renderInstanceId}` : "";
}
