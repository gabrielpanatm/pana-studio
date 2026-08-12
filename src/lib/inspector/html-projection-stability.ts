import type {
  EditableAttributes,
  InspectorHtmlPhysicalFacts,
  InspectorSelectionSummarySnapshot,
  SelectionSnapshot,
} from "$lib/types";
import { primarySelectionEntry, selectionResolution } from "$lib/kernel/selection-read-model";
import { normalizeClassTokens } from "$lib/html/mutations";

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

/**
 * Presents the class value accepted by ProjectModel as one coherent Inspector
 * snapshot while the Canvas observation catches up. The semantic summary
 * remains Rust-owned; this function only derives its visual representation
 * from the mutation receipt already projected into `classEditorValue`.
 */
export function projectHtmlInspectorClassSummary(
  summary: InspectorSelectionSummarySnapshot | null,
  classEditorValue: string,
): InspectorSelectionSummarySnapshot | null {
  if (
    summary?.state !== "resolved"
    || (
      summary.subjectKind !== "htmlElement"
      && summary.subjectKind !== "runtimeElement"
    )
  ) return summary;

  const classes = normalizeClassTokens(classEditorValue);
  const activeCssClass = summary.activeCssClass && classes.includes(summary.activeCssClass)
    ? summary.activeCssClass
    : null;
  const selector = displaySelector(summary.tag, summary.elementId, classes);

  return {
    ...summary,
    classes,
    activeCssClass,
    selector,
  };
}

function displaySelector(tag: string | null, elementId: string | null, classes: string[]) {
  if (!tag) return null;
  if (elementId) return `${tag}#${escapeCssIdentifier(elementId)}`;
  if (classes.length === 0) return tag;
  return `${tag}.${classes.map(escapeCssIdentifier).join(".")}`;
}

function escapeCssIdentifier(value: string) {
  return value.replace(/[^a-zA-Z0-9_-]/g, "\\$&");
}

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
    || selectionResolution(selection) !== "resolved"
    || !physicalFacts
  ) return false;

  return (
    summary.projectRoot === selection.projectRoot
    && summary.runtimeSessionId === selection.runtimeSessionId
    && summary.selectionRevision === selection.selectionRevision
    && physicalFacts.selectionRevision === summary.selectionRevision
    && Boolean(summary.renderInstanceId)
    && physicalFacts.renderInstanceId === summary.renderInstanceId
    && primarySelectionEntry(selection)?.anchor.renderInstanceId === summary.renderInstanceId
  );
}
