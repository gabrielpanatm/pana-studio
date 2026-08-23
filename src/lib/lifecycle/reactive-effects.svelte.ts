export type ReactiveEffect = () => void | (() => void);
export type RegisterReactiveEffects = (
  effects: readonly ReactiveEffect[],
) => () => void;

function registerSvelteEffects(effects: readonly ReactiveEffect[]) {
  return $effect.root(() => {
    for (const effect of effects) $effect(effect);
  });
}

/** Idempotent owner for a domain's Svelte effects and their cleanups. */
export class ReactiveEffectsLifecycle {
  private stopEffects: (() => void) | null = null;
  private readonly effects: readonly ReactiveEffect[];
  private readonly registerEffects: RegisterReactiveEffects;

  constructor(
    effects: readonly ReactiveEffect[],
    registerEffects: RegisterReactiveEffects = registerSvelteEffects,
  ) {
    this.effects = effects;
    this.registerEffects = registerEffects;
  }

  get active() {
    return this.stopEffects !== null;
  }

  start() {
    if (this.stopEffects) return false;
    this.stopEffects = this.registerEffects(this.effects);
    return true;
  }

  stop() {
    const stopEffects = this.stopEffects;
    if (!stopEffects) return false;
    this.stopEffects = null;
    stopEffects();
    return true;
  }
}
