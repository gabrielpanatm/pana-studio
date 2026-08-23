import type { CanvasInteractionControllerHost } from "$lib/state/canvas-interaction-host";
import type { HtmlEditingService } from "$lib/editor/html-editing-service";
import type { EditorInteractionRuntime } from "$lib/editor/interaction-runtime.svelte";
import type { ProjectAnalysisState } from "$lib/project/analysis-state.svelte";
import { blockedAction } from "$lib/editor-runtime/action-outcome";
import { contextMenu } from "$lib/context-menu/store.svelte";
import {
  htmlElementContextMenuItems,
  teraContextMenuItems,
} from "$lib/editor-runtime/context-menu";
import type {
  EditorHtmlTarget,
  EditorTeraTarget,
} from "$lib/editor-runtime/commands";
import {
  hoverCanvasNavigationNode,
  selectCanvasNavigationNode,
} from "$lib/state/canvas-interaction-controller";
import {
  editorNavigationDropTargetStatus,
  enterEditorNavigationScope,
  exitEditorNavigationScope,
  hoverEditorNavigationNode,
  moveEditorNavigationNode,
  previewEditorNavigationMove,
  selectEditorNavigationNode,
  editorNavigationNodeSelector,
  type EditorNavigationControllerHost,
  type EditorNavigationDropTarget,
} from "$lib/state/editor-navigation-controller";
import type { SelectionWorkspaceState } from "$lib/editor/selection-workspace.svelte";
import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import type {
  NativeBlockSlotMutationContext,
  NativeBlockSlotMutationRequest,
} from "$lib/blocks/contracts";
import type { EditorMovePlan } from "$lib/editor/contracts";
import type { EditorNavigationNode } from "$lib/editor/contracts";
import type { ProjectMovePosition } from "$lib/preview/contracts";
import type { PreviewTeraSelectionTarget } from "$lib/state/app-helpers";
import { t } from "$lib/i18n/runtime.svelte";
import {
  primarySelectionEditorNodeId,
  selectionResolution,
} from "$lib/kernel/selection-read-model";

export type EditorNavigationServiceDependencies = Readonly<{
  project: ProjectSessionState;
  selection: SelectionWorkspaceState;
  analysis: ProjectAnalysisState;
  canvas: CanvasInteractionControllerHost;
  html: () => HtmlEditingService;
  editor: () => EditorInteractionRuntime;
  status: GlobalStatusState;
  setPreviewTeraSelection: (
    target: PreviewTeraSelectionTarget,
    options?: { status?: string },
  ) => void;
  flushDrafts: EditorNavigationControllerHost["flushInteractiveEditorDrafts"];
  projectCommittedMove: EditorNavigationControllerHost["projectCommittedMove"];
}>;

/** Owns semantic Layers selection, scopes and serialized move commands. */
export class EditorNavigationService {
  private readonly controller: EditorNavigationControllerHost;
  private readonly dependencies: EditorNavigationServiceDependencies;

  constructor(dependencies: EditorNavigationServiceDependencies) {
    this.dependencies = dependencies;
    this.controller = {
      context: () => ({
        activeCanvasIdentity: dependencies.canvas.session.activeCanvasIdentity,
        projectSessionEpoch: dependencies.project.epoch,
      }),
      editorSelection: dependencies.selection.session,
      setGlobalStatus: (text, kind) => dependencies.status.set(text, kind),
      setPreviewTeraSelection: dependencies.setPreviewTeraSelection,
      flushInteractiveEditorDrafts: dependencies.flushDrafts,
      selectCanvasNode: (node, options) => (
        selectCanvasNavigationNode(dependencies.canvas, node, options)
      ),
      hoverCanvasNode: (node) => hoverCanvasNavigationNode(dependencies.canvas, node),
      projectCommittedMove: dependencies.projectCommittedMove,
    };
  }

  host() { return this.controller; }

  select(node: EditorNavigationNode, options: {
    toggle?: boolean;
    extendRange?: boolean;
    setPrimary?: boolean;
  } = {}) {
    return selectEditorNavigationNode(this.controller, node, options);
  }

  hover(node: EditorNavigationNode | null) {
    hoverEditorNavigationNode(this.controller, node);
  }

  enterScope(scopeId: string) {
    return enterEditorNavigationScope(this.controller, scopeId);
  }

  exitScope() {
    exitEditorNavigationScope(this.controller);
  }

  previewMove(
    sourceNodeId: string,
    targetNodeId: string,
    position: ProjectMovePosition,
    nativeBlockSlot: NativeBlockSlotMutationContext | null = null,
  ): Promise<EditorMovePlan> {
    return previewEditorNavigationMove(
      this.controller,
      sourceNodeId,
      targetNodeId,
      position,
      nativeBlockSlot,
    );
  }

  move(
    sourceNodeId: string,
    targetNodeId: string,
    position: ProjectMovePosition,
    preplanned: EditorMovePlan | null = null,
    inputEmittedAtMs = 0,
    nativeBlockSlot: NativeBlockSlotMutationContext | null = null,
  ) {
    const multiSelection = this.dependencies.selection.session.selectionSnapshot?.members ?? [];
    if (
      multiSelection.length > 1
      && !nativeBlockSlot
      && multiSelection.some((member) => member.anchor.editorNodeId === sourceNodeId)
    ) {
      const target = this.dependencies.selection.session.navigationSnapshot?.nodes.find(
        (node) => node.id === targetNodeId,
      );
      if (!target?.sourceNodeId) {
        return Promise.resolve(blockedAction(t("editor-navigation-snapshot-stale")));
      }
      return this.moveMultipleSelection(target.sourceNodeId, target.tag, position);
    }
    return moveEditorNavigationNode(
      this.controller,
      sourceNodeId,
      targetNodeId,
      position,
      preplanned,
      inputEmittedAtMs,
      nativeBlockSlot,
    );
  }

  private async moveMultipleSelection(
    targetSourceNodeId: string,
    targetTag: string | null,
    position: ProjectMovePosition,
  ) {
    await this.dependencies.flushDrafts("snapshot");
    return await this.dependencies.html().moveMultipleSelection(
      targetSourceNodeId,
      targetTag,
      position,
    );
  }

  dropTargetStatus(target: EditorNavigationDropTarget) {
    return editorNavigationDropTargetStatus({
      editorNavigationSnapshot: this.controller.editorSelection.navigationSnapshot,
      editorEditScopeGrant: this.controller.editorSelection.editScopeGrant,
    }, target);
  }

  async selectDynamicWidgetSourceInstance(instanceId: string) {
    const selection = this.dependencies.selection;
    await selection.session.refreshNavigationSnapshot();
    const sourceInstance = this.dependencies.analysis.sourceGraph?.dynamicWidgetGraph.sourceInstances.find(
      (candidate) => candidate.instanceId === instanceId,
    ) ?? null;
    if (!sourceInstance) return false;
    const rootSourceNodeIds = new Set(sourceInstance.rootSourceNodeIds);
    const node = selection.session.navigationSnapshot?.nodes.find((candidate) => (
      candidate.dynamicWidgetSourceInstanceIds.includes(sourceInstance.id)
      && Boolean(candidate.sourceNodeId && rootSourceNodeIds.has(candidate.sourceNodeId))
      && Boolean(candidate.renderInstanceId)
    )) ?? selection.session.navigationSnapshot?.nodes.find((candidate) => (
      candidate.dynamicWidgetSourceInstanceIds.includes(sourceInstance.id)
      && Boolean(candidate.renderInstanceId)
    )) ?? null;
    if (!node) return false;
    await this.select(node);
    return true;
  }

  async openContextMenu(requestedNode: EditorNavigationNode, x: number, y: number) {
    const selection = this.dependencies.selection.session;
    const requestedCurrentNode = selection.navigationSnapshot?.nodes.find(
      (candidate) => candidate.id === requestedNode.id,
    ) ?? null;
    if (!requestedCurrentNode) {
      contextMenu.close();
      return;
    }

    const alreadySelected = selection.selectionSnapshot?.members.some(
      (member) => member.anchor.editorNodeId === requestedCurrentNode.id,
    ) === true;
    const selected = await this.select(
      requestedCurrentNode,
      alreadySelected ? { setPrimary: true } : {},
    );
    const effectiveNodeId = selected && selectionResolution(selected) === "resolved"
      ? primarySelectionEditorNodeId(selected)
      : null;
    const node = effectiveNodeId
      ? selection.navigationSnapshot?.nodes.find((candidate) => candidate.id === effectiveNodeId) ?? null
      : null;
    if (
      !selected
      || selectionResolution(selected) !== "resolved"
      || !node
    ) {
      contextMenu.close();
      this.dependencies.status.set(t("editor-navigation-snapshot-stale"), "error");
      return;
    }

    const selector = editorNavigationNodeSelector(node) ?? "";
    const requiredScopeId = node.capabilities.requiresEditScopeId;
    const sourceIsEditable = node.origin !== "theme"
      && (!node.capabilities.readOnly || (
        requiredScopeId !== null && requiredScopeId === selection.editScopeId
      ));

    if (node.kind === "htmlElement") {
      const canMutate = sourceIsEditable && Boolean(node.sourceNodeId && node.tag);
      const target: EditorHtmlTarget = {
        kind: "html",
        tag: node.tag ?? "",
        label: node.label,
        renderInstanceId: node.renderInstanceId,
        selectionRevision: selected.selectionRevision,
        sourceLocation: node.file && node.range
          ? { file: node.file, line: node.range.line, column: node.range.column }
          : null,
        sourceId: node.sourceNodeId,
        sessionId: this.dependencies.canvas.session.activeCanvasIdentity?.runtimeSessionId ?? null,
      };
      contextMenu.open({
        source: "layers",
        x,
        y,
        title: `<${node.tag ?? "element"}> ${node.label}`,
        subtitle: node.file ?? selector,
        items: htmlElementContextMenuItems(
          this.dependencies.editor().commands,
          target,
          "layers",
          {
            canSelect: node.capabilities.canSelect,
            canOpenInCode: node.capabilities.canOpenInCode,
            canDuplicate: canMutate,
            canDelete: canMutate,
          },
        ),
      });
      return;
    }

    if (node.kind === "boundary" && node.boundary?.kind !== "markdown") {
      const sourceId = node.boundary?.sourceNodeId ?? node.sourceNodeId ?? "";
      const sourceNode = this.dependencies.analysis.sourceGraph?.nodes.find(
        (candidate) => candidate.id === node.sourceNodeId || candidate.id === sourceId,
      ) ?? null;
      const canDelete = sourceIsEditable
        && Boolean(node.sourceNodeId && node.boundary)
        && node.capabilities.canMoveAtomic;
      const target: EditorTeraTarget = {
        kind: "tera",
        editorNodeId: node.id,
        sourceId,
        label: node.label,
        kindLabel: node.sourceKind ?? "Tera",
        file: node.file,
        origin: node.origin === "theme"
          ? "theme"
          : node.origin === "project"
            ? "current"
            : "unknown",
        themeName: node.themeName,
        canEnterBoundary: node.capabilities.canEnterBoundary,
        sourceNode,
      };
      contextMenu.open({
        source: "layers",
        x,
        y,
        title: `${node.sourceKind ?? "Tera"}: ${node.label}`,
        subtitle: node.file ?? sourceId,
        items: teraContextMenuItems(
          this.dependencies.editor().commands,
          target,
          "layers",
          {
            canSelect: node.capabilities.canSelect,
            canEnterBoundary: node.capabilities.canEnterBoundary,
            canOpenInCode: node.capabilities.canOpenInCode,
            canDelete,
          },
        ),
      });
      return;
    }

    contextMenu.close();
  }

  async deleteNode(node: EditorNavigationNode) {
    const selection = await this.select(node);
    if (
      !selection
      || selectionResolution(selection) !== "resolved"
      || primarySelectionEditorNodeId(selection) !== node.id
    ) {
      this.dependencies.status.set(t("editor-navigation-snapshot-stale"), "error");
      return;
    }
    if (node.kind === "htmlElement") {
      return await this.dependencies.editor().commands.dispatch({
        type: "delete-html",
        surface: "layers",
        target: {
          kind: "html",
          tag: node.tag ?? "",
          label: node.label,
          selectionRevision: selection.selectionRevision,
          renderInstanceId: node.renderInstanceId,
          sourceLocation: node.file && node.range
            ? { file: node.file, line: node.range.line, column: node.range.column }
            : null,
          sourceId: node.sourceNodeId,
          sessionId: this.dependencies.canvas.session.activeCanvasIdentity?.runtimeSessionId ?? null,
        },
      });
    }
    if (node.kind !== "boundary" || node.boundary?.kind === "markdown") return;
    const sourceNode = node.sourceNodeId
      ? this.dependencies.analysis.sourceGraph?.nodes.find(
        (candidate) => candidate.id === node.sourceNodeId,
      ) ?? null
      : null;
    return await this.dependencies.editor().commands.dispatch({
      type: "delete-tera",
      surface: "layers",
      target: {
        kind: "tera",
        sourceId: node.sourceNodeId ?? "",
        label: node.label,
        kindLabel: node.sourceKind ?? undefined,
        file: node.file,
        origin: node.origin === "project"
          ? "local"
          : node.origin === "theme"
            ? "theme"
            : "unknown",
        themeName: node.themeName,
        sourceNode,
      },
    });
  }

  async applyNativeBlockSlotMutation(request: NativeBlockSlotMutationRequest) {
    if (request.operation !== "move") {
      return await this.dependencies.html().mutateNativeBlockSlot(request);
    }
    const sourceId = request.item?.sourceNodeId;
    const targetId = request.targetItem?.sourceNodeId;
    const snapshot = this.dependencies.selection.session.navigationSnapshot;
    if (
      !sourceId
      || !targetId
      || !snapshot
      || snapshot.modelRevision !== request.context.expectedModelRevision
    ) {
      return blockedAction("EditorNavigationSnapshot nu mai corespunde slotului Slider selectat.");
    }
    const source = snapshot.nodes.find((node) => node.sourceNodeId === sourceId);
    const target = snapshot.nodes.find((node) => (
      node.sourceNodeId === targetId && node.parentId === source?.parentId
    ));
    if (!source || !target) {
      return blockedAction("Slide-ul nu mai are o proiecție Editor Navigation stabilă.");
    }
    return await this.move(
      source.id,
      target.id,
      request.position ?? "before",
      null,
      0,
      request.context,
    );
  }
}
