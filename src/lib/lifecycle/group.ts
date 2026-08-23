export type OwnedLifecycle = {
  start: () => boolean;
  stop: () => boolean;
};

/** Starts domain resources once and releases them in reverse ownership order. */
export class LifecycleGroup {
  private activeLifecycleCount = 0;
  private readonly lifecycles: readonly OwnedLifecycle[];

  constructor(lifecycles: readonly OwnedLifecycle[]) {
    this.lifecycles = lifecycles;
  }

  get active() {
    return this.activeLifecycleCount > 0;
  }

  start() {
    if (this.active) return false;
    try {
      for (const lifecycle of this.lifecycles) {
        lifecycle.start();
        this.activeLifecycleCount += 1;
      }
      return true;
    } catch (error) {
      this.stop();
      throw error;
    }
  }

  stop() {
    if (!this.active) return false;
    for (let index = this.activeLifecycleCount - 1; index >= 0; index -= 1) {
      this.lifecycles[index]?.stop();
    }
    this.activeLifecycleCount = 0;
    return true;
  }
}
