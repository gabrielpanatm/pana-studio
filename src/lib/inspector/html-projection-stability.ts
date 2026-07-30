import type {
  EditableAttributes,
  InspectorHtmlPhysicalFacts,
  InspectorSelectionSummarySnapshot,
  SelectionSnapshot,
} from "$lib/types";

export type StableHtmlInspectorProjection = {
  summary: InspectorSelectionSummarySnapshot;
  selection: SelectionSnapshot;
  physicalFacts: InspectorHtmlPhysicalFacts;
  attributeValues: EditableAttributes;
  textContentValue: string;
  classEditorValue: string;
  imageSourceValue: string;
  pendingTag: string | null;
  attributeStatus: string;
  textStatus: string;
  classStatus: string;
  imageStatus: string;
  tagStatus: string;
  canEditHtml: boolean;
  isActivePreviewHtmlSource: boolean;
};

export type HtmlInspectorProjectionInput = {
  summary: InspectorSelectionSummarySnapshot | null;
  selection: SelectionSnapshot | null;
  physicalFacts: InspectorHtmlPhysicalFacts | null;
  attributeValues: EditableAttributes;
  textContentValue: string;
  classEditorValue: string;
  imageSourceValue: string;
  pendingTag: string | null;
  attributeStatus: string;
  textStatus: string;
  classStatus: string;
  imageStatus: string;
  tagStatus: string;
  canEditHtml: boolean;
  isActivePreviewHtmlSource: boolean;
};

export type HtmlInspectorProjectionTransition = {
  projection: StableHtmlInspectorProjection | null;
  pending: boolean;
};

export function advanceStableHtmlInspectorProjection(
  previous: StableHtmlInspectorProjection | null,
  input: HtmlInspectorProjectionInput,
): HtmlInspectorProjectionTransition {
  if (completeHtmlProjection(input)) {
    return {
      projection: {
        summary: input.summary,
        selection: input.selection,
        physicalFacts: input.physicalFacts,
        attributeValues: { ...input.attributeValues },
        textContentValue: input.textContentValue,
        classEditorValue: input.classEditorValue,
        imageSourceValue: input.imageSourceValue,
        pendingTag: input.pendingTag,
        attributeStatus: input.attributeStatus,
        textStatus: input.textStatus,
        classStatus: input.classStatus,
        imageStatus: input.imageStatus,
        tagStatus: input.tagStatus,
        canEditHtml: input.canEditHtml,
        isActivePreviewHtmlSource: input.isActivePreviewHtmlSource,
      },
      pending: false,
    };
  }

  const sameRuntime = Boolean(
    previous
    && input.summary
    && previous.summary.projectRoot === input.summary.projectRoot
    && previous.summary.runtimeSessionId === input.summary.runtimeSessionId,
  );
  const awaitingPhysicalProjection = input.summary?.state === "resolving"
    || (
      input.summary?.state === "resolved"
      && (
        input.summary.subjectKind === "htmlElement"
        || input.summary.subjectKind === "runtimeElement"
      )
    );
  if (sameRuntime && awaitingPhysicalProjection) {
    return { projection: previous, pending: true };
  }

  return { projection: null, pending: false };
}

function completeHtmlProjection(
  input: HtmlInspectorProjectionInput,
): input is HtmlInspectorProjectionInput & {
  summary: InspectorSelectionSummarySnapshot;
  selection: SelectionSnapshot;
  physicalFacts: InspectorHtmlPhysicalFacts;
} {
  const { summary, selection, physicalFacts } = input;
  if (
    summary?.state !== "resolved"
    || (
      summary.subjectKind !== "htmlElement"
      && summary.subjectKind !== "runtimeElement"
    )
    || !selection
    || selection.resolution !== "resolved"
    || !physicalFacts
  ) return false;

  return (
    summary.projectRoot === selection.projectRoot
    && summary.runtimeSessionId === selection.runtimeSessionId
    && summary.selectionRevision === selection.selectionRevision
    && physicalFacts.selectionRevision === summary.selectionRevision
    && Boolean(summary.renderInstanceId)
    && physicalFacts.renderInstanceId === summary.renderInstanceId
    && selection.anchor?.renderInstanceId === summary.renderInstanceId
  );
}
