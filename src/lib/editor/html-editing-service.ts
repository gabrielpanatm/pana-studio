import type { EditorInteractionRuntime } from "$lib/editor/interaction-runtime.svelte";
import { htmlTargetFromCoordinatedSelection } from "$lib/editor-runtime/commands";
import type { HtmlAuthoringState } from "$lib/editor/html-authoring-state.svelte";
import type { EditorReadModelState } from "$lib/editor/read-model.svelte";
import type { SelectionWorkspaceState } from "$lib/editor/selection-workspace.svelte";
import type { SourceWorkspaceState } from "$lib/editor/source-workspace.svelte";
import type { ProjectDocumentWorkspaceState } from "$lib/project/document-workspace.svelte";
import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import {
  applyAttributesToCapturedHtmlTarget,
  applyAttributesToHtml,
  applyClassesToHtml,
} from "$lib/editor/html-actions/attributes";
import {
  applyImageSourceToHtml,
  applyNativeBlockOptionToHtml,
  applyNativeIconToHtml,
  applyZolaImageProcessingToHtml,
  type ApplyNativeBlockOptionRequest,
  type ApplyNativeIconRequest,
} from "$lib/editor/html-actions/media";
import { applyTextContentToCapturedHtmlTarget } from "$lib/editor/html-actions/text";
import {
  deleteSelectedHtmlElement,
  duplicateSelectedHtmlElement,
  moveSelectedHtmlElements,
  mutateNativeBlockSlotStructure,
} from "$lib/editor/html-actions/structure";
import {
  generateClassForSelectedHtml,
  generateDataAnimForSelectedHtml,
} from "$lib/editor/html-actions/identity";
import { insertPaletteElementAtTarget } from "$lib/editor/html-actions/insertion";
import type { HtmlActionsHost } from "$lib/editor/html-actions/host";
import {
  applyTagChange,
  changeElementTag,
  type HtmlEditControllerHost,
} from "$lib/state/html-edit-controller";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import type { NativeBlockSlotMutationRequest } from "$lib/blocks/contracts";
import type { ProjectZolaImageIntent } from "$lib/preview/contracts";
import type { ProjectFile } from "$lib/project/lifecycle-contract";
import type { SourceEditLocation } from "$lib/source-graph/contracts";
import type { PreviewInsertDropRequest } from "$lib/state/preview-insert-controller";
import type { ProjectMovePosition } from "$lib/preview/contracts";
import { zolaRelativePath } from "$lib/project/files";
import { parseSourceEditLocation } from "$lib/source-graph/location";

export type HtmlEditingServiceDependencies = Readonly<{
  project: ProjectSessionState;
  documents: ProjectDocumentWorkspaceState;
  readModel: EditorReadModelState;
  html: HtmlAuthoringState;
  source: SourceWorkspaceState;
  selection: SelectionWorkspaceState;
  editor: EditorInteractionRuntime;
  status: GlobalStatusState;
  structural: HtmlActionsHost["structural"];
  editStructural: Readonly<{
    run: HtmlEditControllerHost["runStructural"];
    projectCommitted: HtmlEditControllerHost["projectCommitted"];
  }>;
  commands: Readonly<{
    loadProjectFile: (file: ProjectFile) => Promise<void>;
    reconcilePageAssets: (tpl: SourceEditLocation) => Promise<unknown>;
  }>;
}>;

/** Owns the standard HTML Inspector command surface and its controller hosts. */
export class HtmlEditingService {
  private readonly actions: HtmlActionsHost;
  private readonly edit: HtmlEditControllerHost;
  private readonly editorRuntime: EditorInteractionRuntime["commands"];

  constructor(dependencies: HtmlEditingServiceDependencies) {
    this.editorRuntime = dependencies.editor.commands;
    this.actions = {
      context: () => ({
        coordinatedSelection: dependencies.selection.coordinatedElement,
        canEditStructure: dependencies.readModel.canEditHtml,
        activeScannedPath: dependencies.documents.activeScannedPath,
        project: dependencies.project.project,
      }),
      html: dependencies.html,
      draft: dependencies.editor.htmlDraft,
      source: dependencies.source,
      editorSelection: dependencies.selection.session,
      structural: dependencies.structural,
      commands: {
        setPending: (area, pending) => dependencies.html.setHtmlPending(area, pending),
        setStatus: (text, kind) => dependencies.status.set(text, kind),
        loadProjectFile: dependencies.commands.loadProjectFile,
        reconcilePageAssets: dependencies.commands.reconcilePageAssets,
      },
    };
    this.edit = {
      context: () => ({
        coordinatedSelection: dependencies.selection.coordinatedElement,
        activeScannedPath: dependencies.documents.activeScannedPath,
      }),
      html: dependencies.html,
      source: dependencies.source,
      runStructural: dependencies.editStructural.run,
      projectCommitted: dependencies.editStructural.projectCommitted,
      setHtmlPending: (area, pending) => dependencies.html.setHtmlPending(area, pending),
      setGlobalStatus: (text, kind) => dependencies.status.set(text, kind),
    };
  }

  deleteTarget(target: Parameters<typeof deleteSelectedHtmlElement>[1]) {
    return deleteSelectedHtmlElement(this.actions, target);
  }

  duplicateTarget(target: Parameters<typeof duplicateSelectedHtmlElement>[1]) {
    return duplicateSelectedHtmlElement(this.actions, target);
  }

  deleteSelected() {
    const selection = this.actions.context().coordinatedSelection;
    const target = selection ? htmlTargetFromCoordinatedSelection(selection) : null;
    return this.editor().dispatch({
      type: "delete-html",
      surface: "runtime",
      target: target ?? { kind: "html", tag: "" },
    });
  }

  applyTextToTarget(
    target: Parameters<typeof applyTextContentToCapturedHtmlTarget>[1],
    text: string,
    options: Parameters<typeof applyTextContentToCapturedHtmlTarget>[3],
  ) {
    return applyTextContentToCapturedHtmlTarget(this.actions, target, text, options);
  }

  applyAttributesToTarget(
    target: Parameters<typeof applyAttributesToCapturedHtmlTarget>[1],
    attributes: Parameters<typeof applyAttributesToCapturedHtmlTarget>[2],
  ) {
    return applyAttributesToCapturedHtmlTarget(this.actions, target, attributes);
  }

  applyAttributes(attributes: Parameters<typeof applyAttributesToHtml>[1]) {
    return applyAttributesToHtml(this.actions, attributes);
  }

  applyClasses() { return applyClassesToHtml(this.actions); }
  generateClass() { return generateClassForSelectedHtml(this.actions); }
  generateDataAnim() { return generateDataAnimForSelectedHtml(this.actions); }
  async openSource(source: string) {
    const relativePath = parseSourceEditLocation(source)?.file ?? source;
    const project = this.actions.context().project;
    if (!project) return;
    const file = project.files.find(
      (item) => item.relativePath === relativePath
        || zolaRelativePath(item.relativePath) === relativePath,
    );
    if (file) await this.actions.commands.loadProjectFile(file);
  }
  applyImage(src?: string) { return applyImageSourceToHtml(this.actions, src); }
  applyZolaImage(intent: ProjectZolaImageIntent) {
    return applyZolaImageProcessingToHtml(this.actions, intent);
  }
  applyNativeBlockOption(request: ApplyNativeBlockOptionRequest) {
    return applyNativeBlockOptionToHtml(this.actions, request);
  }
  applyNativeIcon(request: ApplyNativeIconRequest) {
    return applyNativeIconToHtml(this.actions, request);
  }
  mutateNativeBlockSlot(request: NativeBlockSlotMutationRequest) {
    return mutateNativeBlockSlotStructure(this.actions, request);
  }
  moveMultipleSelection(
    targetSourceId: string,
    targetTag: string | null,
    position: ProjectMovePosition,
  ) {
    return moveSelectedHtmlElements(this.actions, targetSourceId, targetTag, position);
  }
  changeTag(tag: string) { return changeElementTag(this.edit, tag); }
  applyTag() { return applyTagChange(this.edit); }
  insertPalette(request: PreviewInsertDropRequest) {
    return insertPaletteElementAtTarget(this.actions, request);
  }

  private editor() {
    return this.editorRuntime;
  }
}
