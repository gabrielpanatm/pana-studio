import type { EditorHtmlTarget } from "$lib/editor-runtime/commands";
import type { CoordinatedElementSelection } from "$lib/canvas/contracts";
import type { SourceEditLocation } from "$lib/source-graph/contracts";
import type { HtmlActionsHost } from "$lib/editor/html-actions/host";

export type HtmlActionTarget = {
  tag: string;
  selectionRevision?: number | null;
  renderInstanceId?: string | null;
  sourceId?: string | null;
  templateSourceId?: string | null;
  sourceLocation?: SourceEditLocation | null;
  sessionId?: string | null;
  hasChildElements?: boolean;
  rawText?: string;
  attributes?: Readonly<Record<string, string>>;
  classes?: readonly string[];
  zolaImage?: import("$lib/canvas/contracts").ZolaImagePresentation | null;
};

export function freezeHtmlActionTarget(target: HtmlActionTarget): HtmlActionTarget {
  return Object.freeze({
    ...target,
    sourceLocation: target.sourceLocation
      ? Object.freeze({ ...target.sourceLocation })
      : null,
    attributes: Object.freeze({ ...(target.attributes ?? {}) }),
    classes: Object.freeze([...(target.classes ?? [])]),
  });
}

/** Captures selection/source identity before an operation can wait in the structural lane. */
export function captureHtmlActionTarget(
  target: CoordinatedElementSelection | EditorHtmlTarget | null | undefined,
): HtmlActionTarget | null {
  if (!target) return null;
  if ("snapshot" in target) {
    const observation = target.observation;
    return freezeHtmlActionTarget({
      tag: observation.tag,
      selectionRevision: target.snapshot.selectionRevision,
      renderInstanceId: target.renderInstanceId,
      sourceId: target.sourceNodeId,
      templateSourceId: null,
      sourceLocation: target.sourceLocation,
      sessionId: target.snapshot.runtimeSessionId,
      hasChildElements: observation.hasChildElements,
      rawText: observation.rawText,
      attributes: observation.attributes,
      zolaImage: observation.zolaImage,
      classes: observation.classes,
    });
  }
  if ("kind" in target) {
    const observation = target.observation ?? null;
    return freezeHtmlActionTarget({
      tag: target.tag,
      selectionRevision: target.selectionRevision ?? null,
      renderInstanceId: target.renderInstanceId ?? null,
      sourceId: target.sourceId ?? null,
      templateSourceId: target.templateSourceId ?? null,
      sourceLocation: target.sourceLocation ?? null,
      sessionId: target.sessionId ?? null,
      hasChildElements: observation?.hasChildElements,
      rawText: observation?.rawText,
      attributes: observation?.attributes,
      zolaImage: observation?.zolaImage ?? null,
      classes: observation?.classes,
    });
  }
  return null;
}

export function currentSelectionMatchesTarget(
  host: HtmlActionsHost,
  target: HtmlActionTarget,
) {
  const current = host.context().coordinatedSelection;
  if (!current) return false;
  if (
    target.selectionRevision
    && target.selectionRevision !== current.snapshot.selectionRevision
  ) return false;
  if (target.renderInstanceId && target.renderInstanceId !== current.renderInstanceId) return false;
  if (target.sessionId && target.sessionId !== current.snapshot.runtimeSessionId) return false;
  if (!target.sourceId || !current.sourceNodeId) return false;
  return target.sourceId === current.sourceNodeId;
}
