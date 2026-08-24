import type { CanvasProjectionIdentity } from "$lib/contracts/canvas-projection";
import { sameCanvasProjectionIdentity as canvasIdentityEquals } from "$lib/contracts/canvas-identity";
import {
  primarySelectionEditorNodeId,
  primarySelectionEntry,
  selectionResolution,
} from "$lib/kernel/selection-read-model";
import { workbenchSourceStatusFromSelection } from "$lib/source-provenance";
import { EditorSelectionSessionController } from "$lib/state/editor-selection-session.svelte";
import type {
  BlockSelectionContext,
  CanvasElementObservation,
  CoordinatedElementSelection,
  InspectorHtmlPhysicalFacts,
} from "$lib/canvas/contracts";
import type { DynamicWidgetSelectionContext } from "$lib/content-models/contracts";
import type { SelectionSnapshot } from "$lib/editor/contracts";
import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";
import type { SourceGraph } from "$lib/source-graph/graph-contract";
import type { SourceEditTarget } from "$lib/source-graph/contracts";

export type SelectionWorkspaceContext = Readonly<{
  activeCanvasIdentity: CanvasProjectionIdentity | null;
  activeCanvasUrl: string;
  activeScannedPath: string | null;
  browserPreviewRoute: string;
  previewSrc: string;
  workspace: ProjectWorkspaceSnapshot | null;
  sourceGraph: SourceGraph | null;
}>;

export type SelectionWorkspaceCommands = {
  context: () => SelectionWorkspaceContext;
  applySelectionState: (observation: CanvasElementObservation) => void;
  projectSelectionOnCanvas: (selection: SelectionSnapshot) => void;
  resolveSourceEditTarget: (sourceId: string | null | undefined) => SourceEditTarget | null;
};

/** Owns the Rust selection projection and all identity-checked inspector read models. */
export class SelectionWorkspaceState {
  readonly session: EditorSelectionSessionController;
  pendingRestoredTag: string | null = null;
  pendingRestoredTimer: number | null = null;
  private readonly commands: SelectionWorkspaceCommands;

  constructor(commands: SelectionWorkspaceCommands) {
    this.commands = commands;
    const state = this;
    this.session = new EditorSelectionSessionController(() => {
      const context = state.commands.context();
      return {
        activeCanvasIdentity: context.activeCanvasIdentity,
        activeCanvasUrl: context.activeCanvasUrl,
        activeScannedPath: context.activeScannedPath,
        browserPreviewRoute: context.browserPreviewRoute,
        get coordinatedElementSelection() {
          return state.coordinatedElement;
        },
        previewSrc: context.previewSrc,
        projectWorkspaceSnapshot: context.workspace,
        applySelectionState: (observation) => state.commands.applySelectionState(observation),
        projectSelectionSnapshotOnCanvas: (selection) => (
          state.commands.projectSelectionOnCanvas(selection)
        ),
      };
    });
  }

  get coordinatedElement(): CoordinatedElementSelection | null {
    const context = this.commands.context();
    const accepted = this.session.acceptedObservation;
    const semantic = this.session.selectionSnapshot;
    const primary = primarySelectionEntry(semantic);
    if (
      !accepted
      || !semantic
      || selectionResolution(semantic) !== "resolved"
      || (
        primary?.subject.kind !== "htmlElement"
        && primary?.subject.kind !== "runtimeElement"
      )
      || accepted.selectionRevision !== semantic.selectionRevision
      || !canvasIdentityEquals(accepted.canvasIdentity, semantic.canvasIdentity)
      || !canvasIdentityEquals(context.activeCanvasIdentity, semantic.canvasIdentity)
      || accepted.renderInstanceId !== primary?.anchor.renderInstanceId
    ) return null;
    const sourceReference = primary.provenance.definition ?? primary.provenance.composition ?? null;
    return {
      snapshot: semantic,
      documentEpoch: accepted.documentEpoch,
      renderInstanceId: accepted.renderInstanceId,
      sourceNodeId: primary.anchor.sourceNodeId ?? null,
      sourceLocation: sourceReference?.range
        ? {
            file: sourceReference.file,
            line: sourceReference.range.line,
            column: sourceReference.range.column,
          }
        : primary.anchor.file && primary.anchor.range
          ? {
              file: primary.anchor.file,
              line: primary.anchor.range.line,
              column: primary.anchor.range.column,
            }
          : null,
      observation: accepted.observation,
    };
  }

  get htmlPhysicalFacts(): InspectorHtmlPhysicalFacts | null {
    const coordinated = this.coordinatedElement;
    const summary = this.session.inspectorSummary;
    if (
      !coordinated
      || summary?.state !== "resolved"
      || summary.selectionRevision !== coordinated.snapshot.selectionRevision
      || summary.renderInstanceId !== coordinated.renderInstanceId
    ) return null;
    const observation = coordinated.observation;
    return {
      selectionRevision: summary.selectionRevision,
      renderInstanceId: coordinated.renderInstanceId,
      rect: { ...observation.rect },
      hasChildElements: observation.hasChildElements,
      childElementCount: observation.childNodes.length,
      zolaImage: observation.zolaImage ? { ...observation.zolaImage } : null,
    };
  }

  get blockContext(): BlockSelectionContext | null {
    const coordinated = this.coordinatedElement;
    const summary = this.session.inspectorSummary;
    if (
      !coordinated
      || summary?.state !== "resolved"
      || summary.selectionRevision !== coordinated.snapshot.selectionRevision
      || summary.renderInstanceId !== coordinated.renderInstanceId
    ) return null;
    const physical = coordinated.observation.blockContext;
    const bounded = summary.blockContext ?? null;
    if (
      !physical
      || !bounded
      || bounded.providerId !== physical.providerId
      || bounded.rootTag !== physical.rootTag
    ) return null;
    const navigation = this.session.navigationSnapshot;
    const node = this.selectedEditorNavigationNode;
    const ownsSelection = Boolean(
      navigation
      && canvasIdentityEquals(navigation.identity, coordinated.snapshot.canvasIdentity)
      && node?.renderInstanceId === coordinated.renderInstanceId,
    );
    return {
      ...physical,
      sourceInstanceIds: ownsSelection && Array.isArray(node?.blockSourceInstanceIds)
        ? [...node.blockSourceInstanceIds]
        : [],
      rootSourceId: ownsSelection ? node?.sourceNodeId ?? null : null,
      rootTemplateSourceId: null,
      rootSessionId: coordinated.snapshot.runtimeSessionId,
    };
  }

  get dynamicWidgetContext(): DynamicWidgetSelectionContext | null {
    const coordinated = this.coordinatedElement;
    const navigation = this.session.navigationSnapshot;
    const node = this.selectedEditorNavigationNode;
    if (
      !coordinated
      || !navigation
      || !node
      || node.renderInstanceId !== coordinated.renderInstanceId
      || !canvasIdentityEquals(navigation.identity, coordinated.snapshot.canvasIdentity)
    ) return null;
    const sourceInstanceIds = Array.isArray(node.dynamicWidgetSourceInstanceIds)
      ? [...node.dynamicWidgetSourceInstanceIds]
      : [];
    const sourceInstanceId = sourceInstanceIds.at(-1) ?? null;
    if (!sourceInstanceId) return null;
    const sourceInstance = this.commands.context().sourceGraph?.dynamicWidgetGraph.sourceInstances.find(
      (candidate) => candidate.id === sourceInstanceId,
    ) ?? null;
    if (!sourceInstance) return null;
    return {
      sourceInstanceId,
      sourceInstanceIds,
      providerId: sourceInstance.providerId,
      modelRevision: navigation.modelRevision,
      previewRevision: navigation.identity.previewRevision,
      renderInstanceId: coordinated.renderInstanceId,
    };
  }

  get selectionEpoch() {
    return this.session.selectionSnapshot?.selectionRevision ?? 0;
  }

  get activeCssSelector() {
    const focus = this.session.selectionSnapshot?.focus;
    return focus?.kind === "cssRule" || focus?.kind === "cssProperty" ? focus.selector : "";
  }

  get selectedSourceEditTarget() {
    return this.commands.resolveSourceEditTarget(
      primarySelectionEntry(this.session.selectionSnapshot)?.anchor.sourceNodeId,
    );
  }

  get selectedTemplateSourceNode() {
    const primary = primarySelectionEntry(this.session.selectionSnapshot);
    const sourceNodeId = primary?.subject.kind === "boundary"
      && primary.subject.boundaryKind !== "markdown"
      ? primary.anchor.sourceNodeId
      : null;
    return sourceNodeId
      ? this.commands.context().sourceGraph?.nodes.find((node) => node.id === sourceNodeId) ?? null
      : null;
  }

  get selectedEditorNavigationNode() {
    const editorNodeId = primarySelectionEditorNodeId(this.session.selectionSnapshot);
    return editorNodeId
      ? this.session.navigationSnapshot?.nodes.find((node) => node.id === editorNodeId) ?? null
      : null;
  }

  get selectedSemanticSourceLocation() {
    const selection = this.session.selectionSnapshot;
    const primary = primarySelectionEntry(selection);
    return Boolean(
      selection
      && selectionResolution(selection) === "resolved"
      && primary?.anchor.file
      && primary.anchor.range,
    );
  }

  get workbenchSourceStatus() {
    return workbenchSourceStatusFromSelection(this.session.selectionSnapshot);
  }
}
