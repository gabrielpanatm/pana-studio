import { savePaneDimensions } from "$lib/ui/preferences";
import type { WorkspaceLayoutState } from "$lib/ui/workspace-layout.svelte";

export type WorkspaceLayoutDimensions = Readonly<{
  leftPaneWidth: number;
  rightPaneWidth: number;
  terminalPaneHeight: number;
}>;

type RegisterReactiveEffect = (effect: () => void) => () => void;
type ResolveStorage = () => Storage | null;
type PersistDimensions = (
  storage: Storage,
  dimensions: WorkspaceLayoutDimensions,
) => void;

function registerReactiveEffect(effect: () => void) {
  return $effect.root(() => {
    $effect(effect);
  });
}

function browserStorage() {
  return typeof window === "undefined" ? null : window.localStorage;
}

/** Owns the reactive persistence resource for Workspace Layout. */
export class WorkspaceLayoutPersistenceLifecycle {
  private stopEffect: (() => void) | null = null;
  private readonly layout: WorkspaceLayoutState;
  private readonly resolveStorage: ResolveStorage;
  private readonly persistDimensions: PersistDimensions;
  private readonly registerEffect: RegisterReactiveEffect;

  constructor(
    layout: WorkspaceLayoutState,
    resolveStorage: ResolveStorage = browserStorage,
    persistDimensions: PersistDimensions = savePaneDimensions,
    registerEffect: RegisterReactiveEffect = registerReactiveEffect,
  ) {
    this.layout = layout;
    this.resolveStorage = resolveStorage;
    this.persistDimensions = persistDimensions;
    this.registerEffect = registerEffect;
  }

  get active() {
    return this.stopEffect !== null;
  }

  start() {
    if (this.stopEffect) return false;
    this.stopEffect = this.registerEffect(() => {
      const dimensions: WorkspaceLayoutDimensions = {
        leftPaneWidth: this.layout.leftPaneWidth,
        rightPaneWidth: this.layout.rightPaneWidth,
        terminalPaneHeight: this.layout.terminalPaneHeight,
      };
      const storage = this.resolveStorage();
      if (storage) this.persistDimensions(storage, dimensions);
    });
    return true;
  }

  stop() {
    const stopEffect = this.stopEffect;
    if (!stopEffect) return false;
    this.stopEffect = null;
    stopEffect();
    return true;
  }
}
