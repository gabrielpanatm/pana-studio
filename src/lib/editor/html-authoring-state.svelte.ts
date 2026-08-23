import { untrack } from "svelte";
import type { SourceEditLocation } from "$lib/source-graph/contracts";
import type {
  HtmlPendingArea,
  InspectorPendingArea,
} from "$lib/canvas/contracts";
import { createEmptyHtmlPending, createEmptyInspectorPending } from "$lib/state/app-helpers";
import {
  createInspectorPendingSourceRegistry,
  updateInspectorPendingSource,
  type InspectorPendingSource,
  type InspectorPendingSourceRegistry,
} from "$lib/state/inspector-pending";

/** Owns transient HTML and Inspector drafts independently from project state. */
export class HtmlAuthoringState {
  classEditorValue = $state("");
  classStatus = $state("");
  imageSourceValue = $state("");
  imageStatus = $state("");
  pendingTag = $state<string | null>(null);
  pendingTagOriginal = $state<string | null>(null);
  pendingTagSourceLocation = $state<SourceEditLocation | null>(null);
  tagStatus = $state("");
  structureStatus = $state("");
  htmlPending = $state<Record<HtmlPendingArea, boolean>>(createEmptyHtmlPending());
  inspectorPending = $state<Record<InspectorPendingArea, boolean>>(createEmptyInspectorPending());
  inspectorPendingSources: InspectorPendingSourceRegistry = createInspectorPendingSourceRegistry();
  mutationRevision = 0;

  private readonly markMutation: () => void;

  constructor(markMutation: () => void) {
    this.markMutation = markMutation;
  }

  setInspectorPending(
    area: InspectorPendingArea,
    pending: boolean,
    source: InspectorPendingSource = "session",
  ) {
    const aggregate = updateInspectorPendingSource(
      this.inspectorPendingSources,
      area,
      source,
      pending,
    );
    const current = untrack(() => this.inspectorPending);
    if (current[area] === aggregate) return;
    this.markMutation();
    this.inspectorPending = { ...current, [area]: aggregate };
  }

  setHtmlPending(area: HtmlPendingArea, pending: boolean) {
    if (this.htmlPending[area] === pending) return;
    const next = { ...this.htmlPending, [area]: pending };
    this.htmlPending = next;
    this.setInspectorPending("html", Object.values(next).some(Boolean));
  }

  clearHtmlPending() {
    if (Object.values(this.htmlPending).some(Boolean)) this.markMutation();
    this.htmlPending = createEmptyHtmlPending();
    this.setInspectorPending("html", false);
  }

  resetPendingSources() {
    this.inspectorPendingSources = createInspectorPendingSourceRegistry();
  }

  resetPending() {
    this.htmlPending = createEmptyHtmlPending();
    this.resetPendingSources();
    this.inspectorPending = createEmptyInspectorPending();
  }

  reset() {
    this.classEditorValue = "";
    this.classStatus = "";
    this.imageSourceValue = "";
    this.imageStatus = "";
    this.pendingTag = null;
    this.pendingTagOriginal = null;
    this.pendingTagSourceLocation = null;
    this.tagStatus = "";
    this.structureStatus = "";
    this.mutationRevision += 1;
    this.resetPending();
  }
}
