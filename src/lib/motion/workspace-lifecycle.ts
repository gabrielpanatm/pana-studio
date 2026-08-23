import {
  registerEditFlushHandler,
  type EditFlushHandler,
  type EditFlushPending,
} from "$lib/session/edit-flush-registry";

export const MOTION_WORKSPACE_FLUSH_HANDLER_ID = "motion-v2-project-workspace";

type RegisterFlushHandler = (
  id: string,
  handler: EditFlushHandler,
  pending?: EditFlushPending,
) => () => void;

/** Owns the global edit-flush registration for the Motion workspace. */
export class MotionWorkspaceLifecycle {
  private unregisterFlush: (() => void) | null = null;
  private readonly flush: () => void | Promise<void>;
  private readonly pending: EditFlushPending;
  private readonly registerFlush: RegisterFlushHandler;

  constructor(
    flush: () => void | Promise<void>,
    pending: EditFlushPending = () => true,
    registerFlush: RegisterFlushHandler = registerEditFlushHandler,
  ) {
    this.flush = flush;
    this.pending = pending;
    this.registerFlush = registerFlush;
  }

  get active() {
    return this.unregisterFlush !== null;
  }

  start() {
    if (this.unregisterFlush) return false;
    this.unregisterFlush = this.registerFlush(
      MOTION_WORKSPACE_FLUSH_HANDLER_ID,
      async () => {
        await this.flush();
      },
      this.pending,
    );
    return true;
  }

  stop() {
    const unregister = this.unregisterFlush;
    if (!unregister) return false;
    this.unregisterFlush = null;
    unregister();
    return true;
  }
}
