import type {
  SelectionEntry,
  SelectionResolution,
  SelectionSnapshot,
  SourceRange,
} from "$lib/types";

export function primarySelectionEntry(
  selection: SelectionSnapshot | null | undefined,
): SelectionEntry | null {
  if (!selection?.primaryMemberId) return null;
  return selection.members.find(
    (member) => member.memberId === selection.primaryMemberId,
  ) ?? null;
}

export function selectionResolution(
  selection: SelectionSnapshot | null | undefined,
): SelectionResolution {
  if (!selection || selection.members.length === 0) return "cleared";
  if (selection.members.some((member) => member.resolution === "ambiguous")) {
    return "ambiguous";
  }
  if (selection.members.some((member) => member.resolution === "notRendered")) {
    return "notRendered";
  }
  return "resolved";
}

export function primarySelectionRenderInstanceId(
  selection: SelectionSnapshot | null | undefined,
): string | null {
  const anchor = primarySelectionEntry(selection)?.anchor;
  return anchor?.renderInstanceId ?? anchor?.renderInstanceIds[0] ?? null;
}

export function primarySelectionEditorNodeId(
  selection: SelectionSnapshot | null | undefined,
): string | null {
  return primarySelectionEntry(selection)?.anchor.editorNodeId ?? null;
}

export function selectionCodeTarget(
  selection: SelectionSnapshot | null | undefined,
): {
  file: string | null;
  range: SourceRange | null;
  primaryMemberId: string | null;
  memberIds: string[];
} {
  const setIdentity = {
    primaryMemberId: selection?.primaryMemberId ?? null,
    memberIds: selection?.members.map((member) => member.memberId) ?? [],
  };
  const focus = selection?.focus;
  if (focus && focus.kind !== "element") {
    return {
      file: focus.file,
      range: "range" in focus ? (focus.range ?? null) : null,
      ...setIdentity,
    };
  }
  const anchor = primarySelectionEntry(selection)?.anchor;
  return {
    file: anchor?.file ?? null,
    range: anchor?.range ?? null,
    ...setIdentity,
  };
}
