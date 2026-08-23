import {
  createEditorRuntime,
  type EditorRuntime,
  type EditorRuntimeHost,
} from "$lib/editor-runtime/runtime";
import {
  HtmlDraftSessionController,
  type HtmlDraftSessionControllerHost,
} from "$lib/state/html-draft-session.svelte";

export type EditorInteractionRuntimeDependencies = Readonly<{
  editor: EditorRuntimeHost;
  htmlDraft: () => HtmlDraftSessionControllerHost;
}>;

/**
 * Owns the two application-long editor runtimes and their cleanup boundary.
 * Domain states remain the source of reactive truth; this object only owns
 * command sequencing and speculative HTML edit sessions.
 */
export class EditorInteractionRuntime {
  readonly commands: EditorRuntime;
  readonly htmlDraft: HtmlDraftSessionController;

  constructor(dependencies: EditorInteractionRuntimeDependencies) {
    this.commands = createEditorRuntime(dependencies.editor);
    this.htmlDraft = new HtmlDraftSessionController(dependencies.htmlDraft);
  }

  destroy() {
    this.htmlDraft.destroy();
  }
}
