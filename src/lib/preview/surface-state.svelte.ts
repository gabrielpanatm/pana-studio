/** Owns the mounted Canvas iframe identity and its resume barrier. */
export class PreviewSurfaceState {
  frame = $state<HTMLIFrameElement | undefined>(undefined);
  canvasElement: HTMLIFrameElement | null = null;
  generation = 0;
  loadedGeneration = 0;
  resumeRequired = false;
  resumeScheduled = false;
  resumePromise: Promise<void> | null = null;

  reset() {
    this.frame = undefined;
    this.canvasElement = null;
    this.generation += 1;
    this.loadedGeneration = 0;
    this.resumeRequired = false;
    this.resumeScheduled = false;
    this.resumePromise = null;
  }
}
