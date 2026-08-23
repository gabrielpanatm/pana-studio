import type { HtmlEditingService } from "$lib/editor/html-editing-service";
import type { TeraEditingService } from "$lib/editor/tera-editing-service";
import type { EditorNavigationService } from "$lib/editor/navigation-service";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import {
  handlePreviewInsertDrop,
  type PreviewInsertControllerHost,
} from "$lib/state/preview-insert-controller";
import {
  handlePreviewTeraInsertDrop,
  type PreviewTeraInsertControllerHost,
} from "$lib/state/preview-tera-insert-controller";

export type PreviewInsertServiceDependencies = Readonly<{
  html: HtmlEditingService;
  tera: TeraEditingService;
  navigation: EditorNavigationService;
  status: GlobalStatusState;
}>;

/** Routes validated Preview drop intents to their domain mutation service. */
export class PreviewInsertService {
  private readonly controller: PreviewInsertControllerHost & PreviewTeraInsertControllerHost;

  constructor(dependencies: PreviewInsertServiceDependencies) {
    this.controller = {
      insertPaletteElementAtTarget: (request) => dependencies.html.insertPalette(request),
      insertTeraPaletteItemAtTarget: (request) => dependencies.tera.insert(request),
      previewDropTargetStatus: (target) => dependencies.navigation.dropTargetStatus(target),
      setGlobalStatus: (text, kind) => dependencies.status.set(text, kind),
    };
  }

  handleHtml(payload: unknown) {
    return handlePreviewInsertDrop(this.controller, payload);
  }

  handleTera(payload: unknown) {
    return handlePreviewTeraInsertDrop(this.controller, payload);
  }
}
