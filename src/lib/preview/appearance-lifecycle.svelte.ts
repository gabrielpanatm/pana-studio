import { applyApplicationAppearanceToPreviewDocument } from "$lib/preview/bridge";
import { contrastingTextColor } from "$lib/state/app-helpers";
import { ReactiveEffectsLifecycle } from "$lib/lifecycle/reactive-effects.svelte";
import type { ApplicationPreferencesState } from "$lib/application/preferences.svelte";
import type { PreviewSurfaceState } from "$lib/preview/surface-state.svelte";
import type { PreviewWorkspaceState } from "$lib/preview/workspace-state.svelte";

export type PreviewAppearanceLifecycleDependencies = {
  preview: PreviewWorkspaceState;
  surface: Pick<PreviewSurfaceState, "frame">;
};

/** Owns propagation of the application appearance into preview surfaces. */
export class PreviewAppearanceLifecycle {
  private readonly effects: ReactiveEffectsLifecycle;

  constructor(
    { preview, surface }: PreviewAppearanceLifecycleDependencies,
    appearance: Pick<ApplicationPreferencesState, "accent">,
  ) {
    this.effects = new ReactiveEffectsLifecycle([
      () => {
        const accent = appearance.accent;
        surface.frame;
        preview.src;
        preview.reloadSerial;
        preview.documentMarkup;
        const textOnAccent = contrastingTextColor(accent);
        const previewDocument = preview.getDocument();
        if (previewDocument) {
          applyApplicationAppearanceToPreviewDocument(previewDocument, accent, textOnAccent);
        }
        preview.postMessage({
          type: "set-application-appearance",
          accent,
          textOnAccent,
        });
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
