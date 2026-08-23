import {
  synchronizeCanvasInteractionBinding,
  type CanvasInteractionControllerHost,
} from "$lib/state/canvas-interaction-controller";
import { ReactiveEffectsLifecycle } from "$lib/lifecycle/reactive-effects.svelte";

/** Owns rebinding of the Canvas agent to the current Rust navigation identity. */
export class CanvasInteractionLifecycle {
  private readonly effects: ReactiveEffectsLifecycle;

  constructor(app: CanvasInteractionControllerHost) {
    this.effects = new ReactiveEffectsLifecycle([
      () => {
        app.session.previewFrame;
        app.session.activeCanvasUrl;
        app.session.previewSrc;
        app.session.browserPreviewRoute;
        app.session.applicationSurface;
        app.session.workbenchSnapshot?.activeActivity;
        app.session.centerView;
        app.session.activeScannedPath;
        app.selection.editorSelection.navigationSnapshot?.identity.transactionId;
        app.selection.editorSelection.navigationSnapshot?.identity.previewRevision;
        app.selection.editorSelection.navigationSnapshot?.route;
        app.selection.editorSelection.navigationSnapshot?.focusedView?.activeDocumentPath;
        app.session.activeCanvasIdentity?.projectRoot;
        app.session.activeCanvasIdentity?.runtimeSessionId;
        app.session.activeCanvasIdentity?.workspaceRevision;
        app.session.activeCanvasIdentity?.transactionId;
        app.session.activeCanvasIdentity?.previewRevision;
        synchronizeCanvasInteractionBinding(app);
      },
    ]);
  }

  start() {
    return this.effects.start();
  }

  stop() {
    return this.effects.stop();
  }
}
