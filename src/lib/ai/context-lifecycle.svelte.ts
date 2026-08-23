import type { AiContextState } from "$lib/ai/context-state.svelte";
import { ReactiveEffectsLifecycle } from "$lib/lifecycle/reactive-effects.svelte";

/** Tracks the projection input and delegates debounce/cleanup to its owner. */
export class AiContextLifecycle {
  private readonly effects: ReactiveEffectsLifecycle;

  constructor(context: AiContextState) {
    this.effects = new ReactiveEffectsLifecycle([
      () => context.schedule(),
    ]);
  }

  start() {
    return this.effects.start();
  }

  stop() {
    return this.effects.stop();
  }
}
